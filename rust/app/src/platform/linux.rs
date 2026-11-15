//! Linux (GNOME on Wayland): GNOME has no screensaver plug-in API and does not allow app windows on the
//! lock screen. So an idle watcher (systemd user service) asks Mutter to signal after `idle_seconds` and
//! starts the screensaver; the screensaver keeps GNOME from blanking while it is visible and, on any
//! input, LOCKS THE SCREEN FIRST, waits until GNOME confirms, then closes.
use std::process::{Child, Command};
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use vivarium_core::{config, sysmon};
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::OwnedObjectPath;

use super::Platform;
use crate::args::Args;

const SS: (&str, &str, &str) = ("org.gnome.ScreenSaver", "/org/gnome/ScreenSaver", "org.gnome.ScreenSaver");
const IDLE: (&str, &str, &str) =
    ("org.gnome.Mutter.IdleMonitor", "/org/gnome/Mutter/IdleMonitor/Core", "org.gnome.Mutter.IdleMonitor");
const SM: (&str, &str, &str) = ("org.gnome.SessionManager", "/org/gnome/SessionManager", "org.gnome.SessionManager");
const INHIBIT_IDLE: u32 = 8;

pub fn log(msg: &str) {
    eprintln!("[claude-vivarium] {msg}");
}

fn session_bus() -> Option<Connection> {
    match Connection::session() {
        Ok(c) => Some(c),
        Err(e) => {
            log(&format!("no session bus: {e}"));
            None
        }
    }
}

fn screen_locked(bus: &Connection) -> bool {
    let r = bus.call_method(Some(SS.0), SS.1, Some(SS.2), "GetActive", &()).and_then(|m| m.body().deserialize::<bool>());
    match r {
        Ok(v) => v,
        Err(e) => {
            log(&format!("GetActive failed: {e}"));
            false
        }
    }
}

fn graphical_session_id() -> Option<String> {
    if let Ok(sid) = std::env::var("XDG_SESSION_ID") {
        if !sid.is_empty() {
            return Some(sid);
        }
    }
    // SAFETY: getuid has no failure modes
    let uid = unsafe { libc::getuid() };
    let out = Command::new("loginctl").args(["show-user", &uid.to_string(), "-p", "Display", "--value"]).output().ok()?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

/// Ask GNOME to lock. Returns true if the request was accepted.
fn request_lock(bus: Option<&Connection>) -> bool {
    if let Some(bus) = bus {
        match bus.call_method(Some(SS.0), SS.1, Some(SS.2), "Lock", &()) {
            Ok(_) => return true,
            Err(e) => log(&format!("ScreenSaver.Lock failed: {e}; falling back to loginctl")),
        }
    }
    let mut cmd = Command::new("loginctl");
    cmd.arg("lock-session");
    if let Some(sid) = graphical_session_id() {
        cmd.arg(sid);
    }
    match cmd.status() {
        Ok(s) => s.success(),
        Err(e) => {
            log(&format!("loginctl lock-session failed: {e}"));
            false
        }
    }
}

/// Wake the lock screen after we locked it ourselves.
///
/// GNOME fades to black 0.3 s after any programmatic lock and only wakes up on input that arrives
/// after that - so the key that woke the screensaver is not enough and a second key would be needed.
/// A 1-pixel virtual pointer move (Mutter's remote desktop API, open to programs in the session)
/// counts as new input, so the lock screen stays visible. On a locked screen input can only show
/// the lock screen, never unlock it.
fn nudge_pointer(bus: &Connection) -> bool {
    let rd = "org.gnome.Mutter.RemoteDesktop";
    let iface = "org.gnome.Mutter.RemoteDesktop.Session";
    let path: OwnedObjectPath = match bus
        .call_method(Some(rd), "/org/gnome/Mutter/RemoteDesktop", Some(rd), "CreateSession", &())
        .and_then(|m| m.body().deserialize())
    {
        Ok(p) => p,
        Err(e) => {
            log(&format!("could not wake the lock screen: {e}"));
            return false;
        }
    };
    let run = || -> zbus::Result<()> {
        bus.call_method(Some(rd), &path, Some(iface), "Start", &())?;
        for dx in [1.0f64, -1.0] {
            bus.call_method(Some(rd), &path, Some(iface), "NotifyPointerMotionRelative", &(dx, 0.0f64))?;
        }
        Ok(())
    };
    let ok = match run() {
        Ok(()) => true,
        Err(e) => {
            log(&format!("could not wake the lock screen: {e}"));
            false
        }
    };
    let _ = bus.call_method(Some(rd), &path, Some(iface), "Stop", &());
    ok
}

/// True if some app (video player, presentation, ...) asked GNOME not to go idle.
fn idle_inhibited(bus: &Connection) -> bool {
    bus.call_method(Some(SM.0), SM.1, Some(SM.2), "IsInhibited", &(INHIBIT_IDLE,))
        .and_then(|m| m.body().deserialize::<bool>())
        .unwrap_or(false)
}

/// GNOME's own blank/lock delay in seconds (0 = never or unknown).
fn gnome_idle_delay() -> i64 {
    let Ok(out) = Command::new("gsettings").args(["get", "org.gnome.desktop.session", "idle-delay"]).output() else {
        return 0;
    };
    String::from_utf8_lossy(&out.stdout).split_whitespace().last().and_then(|v| v.parse().ok()).unwrap_or(0)
}

pub struct Os {
    bus: Option<Connection>,
}

impl Os {
    pub fn new() -> Self {
        Os { bus: session_bus() }
    }
}

impl Platform for Os {
    fn locks(&self) -> bool {
        true
    }

    fn screen_locked(&mut self) -> bool {
        self.bus.as_ref().is_some_and(screen_locked)
    }

    fn request_lock(&mut self) -> bool {
        request_lock(self.bus.as_ref())
    }

    fn settle_seconds(&self) -> f64 {
        0.5 // until GNOME's 0.3 s "fade to black" has started
    }

    /// So one key is enough: without this, GNOME blanks right after our lock and needs a second key.
    /// Only when a person woke us - after max time, a kill signal etc. nobody is there and black is right.
    fn locked(&mut self, by_user: bool) {
        if by_user {
            if let Some(bus) = &self.bus {
                nudge_pointer(bus);
            }
        }
    }

    fn begin_fullscreen(&mut self) {
        // keep GNOME from blanking/locking underneath us while we are visible; the inhibitor lives as
        // long as our bus connection, so it ends with this process
        if let Some(bus) = &self.bus {
            let r = bus.call_method(
                Some(SM.0),
                SM.1,
                Some(SM.2),
                "Inhibit",
                &("io.github.ClaudeVivarium", 0u32, "Showing Claude Vivarium", INHIBIT_IDLE),
            );
            if let Err(e) = r {
                log(&format!("could not inhibit idle blanking: {e}"));
            }
        }
    }
}

enum Event {
    Fired(u32),
}

struct Daemon {
    bus: Connection,
    idle_override: i64,
    passthrough: Vec<String>,
    no_lock: bool,
    child: Option<Child>,
    watch_id: Option<u32>,
    idle: i64,
    cfg_mtime: Option<f64>,
}

impl Daemon {
    /// (Re)register the idle watch when the configured idle time changes.
    fn reload(&mut self) {
        let mt = config::mtime();
        if Some(mt) == self.cfg_mtime && self.watch_id.is_some() {
            return;
        }
        self.cfg_mtime = Some(mt);
        let wanted = if self.idle_override > 0 { self.idle_override } else { config::load().idle_seconds };
        let mut idle = wanted;
        let gnome_delay = gnome_idle_delay();
        if gnome_delay > 0 && idle > gnome_delay - 20 {
            idle = (gnome_delay - 30).max(20);
            log(&format!("idle {wanted}s is too close to GNOME's blank delay ({gnome_delay}s); using {idle}s"));
        }
        if idle == self.idle && self.watch_id.is_some() {
            return;
        }
        if let Some(id) = self.watch_id.take() {
            if let Err(e) = self.bus.call_method(Some(IDLE.0), IDLE.1, Some(IDLE.2), "RemoveWatch", &(id,)) {
                log(&format!("RemoveWatch failed: {e}"));
            }
        }
        let r = self
            .bus
            .call_method(Some(IDLE.0), IDLE.1, Some(IDLE.2), "AddIdleWatch", &((idle.max(1) as u64) * 1000,))
            .and_then(|m| m.body().deserialize::<u32>());
        match r {
            Ok(id) => {
                self.watch_id = Some(id);
                self.idle = idle;
                let blanks = if gnome_delay > 0 { format!("after {gnome_delay}s") } else { "never".into() };
                log(&format!("watching for {idle}s of idle time (GNOME blanks {blanks})"));
            }
            Err(e) => log(&format!("AddIdleWatch failed: {e}")),
        }
    }

    fn start_screensaver(&mut self) {
        if self.child.is_some() {
            return;
        }
        if screen_locked(&self.bus) {
            return; // already on the lock screen
        }
        if idle_inhibited(&self.bus) {
            log("idle is inhibited by another app (video, presentation?) - not starting");
            return;
        }
        let Ok(exe) = std::env::current_exe() else { return };
        log("idle - starting screensaver");
        match Command::new(exe).arg("run").args(&self.passthrough).spawn() {
            Ok(c) => self.child = Some(c),
            Err(e) => log(&format!("could not start the screensaver: {e}")),
        }
    }

    fn check_child(&mut self) {
        let Some(c) = self.child.as_mut() else { return };
        let Ok(Some(status)) = c.try_wait() else { return };
        self.child = None;
        let locked = screen_locked(&self.bus);
        log(&format!("screensaver exited (code {}, locked={locked})", status.code().unwrap_or(-1)));
        if !locked && !self.no_lock {
            log("screen not locked after screensaver - locking now");
            request_lock(Some(&self.bus));
        }
    }
}

/// Idle watcher: starts the screensaver when the desktop has been idle for a while.
///
/// Runs as a systemd user service inside the GNOME session. Asks Mutter to signal when the user has
/// been idle for the configured seconds (must be shorter than GNOME's own blank delay), then launches
/// the screensaver. Whenever the screensaver exits and the screen is not locked, this daemon locks it
/// (fail closed). It also records CPU/memory for the monitor panel.
/// Arguments after "--" are passed to the screensaver (e.g. -- --hide-text --max-minutes 60).
pub fn daemon(args: &Args) -> Result<(), String> {
    let passthrough = args.after_dashes();
    let bus = Connection::session().map_err(|e| format!("no session bus: {e}"))?;
    let stop = crate::signals::stop_flag();
    let (tx, rx) = mpsc::channel();
    {
        let bus = bus.clone();
        std::thread::spawn(move || {
            let Ok(proxy) = Proxy::new(&bus, IDLE.0, IDLE.1, IDLE.2) else { return };
            let Ok(signals) = proxy.receive_signal("WatchFired") else { return };
            for msg in signals {
                if let Ok(id) = msg.body().deserialize::<u32>() {
                    if tx.send(Event::Fired(id)).is_err() {
                        return;
                    }
                }
            }
        });
    }
    let mut d = Daemon {
        bus,
        idle_override: args.num("--idle", 0.0)? as i64,
        no_lock: passthrough.iter().any(|a| a == "--no-lock"),
        passthrough,
        child: None,
        watch_id: None,
        idle: 0,
        cfg_mtime: None,
    };
    d.reload();
    // CPU/memory history for the system monitor panel (always recorded)
    let mut sampler = sysmon::Sampler::resumed(); // keeps the history across a quick service restart
    sampler.sample();
    let mut next_sample = Instant::now() + Duration::from_secs(sysmon::INTERVAL);
    let mut next_reload = Instant::now() + Duration::from_secs(30);
    if args.flag("--now") {
        d.start_screensaver();
    }
    while !stop.load(Ordering::Relaxed) {
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(Event::Fired(id)) => {
                if Some(id) == d.watch_id {
                    d.start_screensaver();
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err("lost the connection to GNOME (idle monitor)".into()); // systemd restarts us
            }
        }
        d.check_child();
        let now = Instant::now();
        if now >= next_sample {
            sampler.sample();
            sampler.save();
            next_sample = now + Duration::from_secs(sysmon::INTERVAL);
        }
        if now >= next_reload {
            d.reload();
            next_reload = now + Duration::from_secs(30);
        }
    }
    // Service stopped/restarted while the screensaver is up: lock first, never leave it open.
    if let Some(c) = d.child.as_mut() {
        if matches!(c.try_wait(), Ok(None)) {
            log("stopping while the screensaver is shown - locking first");
            if !d.no_lock && !screen_locked(&d.bus) {
                request_lock(Some(&d.bus));
            }
            // SAFETY: SIGTERM to our own child (the screensaver also locks on SIGTERM itself)
            unsafe { libc::kill(c.id() as i32, libc::SIGTERM) };
        }
    }
    Ok(())
}

pub fn selftest_lines() -> Vec<String> {
    let mut out = Vec::new();
    match session_bus() {
        Some(bus) => {
            out.push(format!("screen locked: {}, idle inhibited by another app: {}", screen_locked(&bus), idle_inhibited(&bus)));
            let idle = bus
                .call_method(Some(IDLE.0), IDLE.1, Some(IDLE.2), "GetIdletime", &())
                .and_then(|m| m.body().deserialize::<u64>());
            match idle {
                Ok(ms) => out.push(format!("idle monitor: available, idle for {} s", ms / 1000)),
                Err(e) => out.push(format!("idle monitor: NOT available ({e}) - the idle watcher needs GNOME")),
            }
        }
        None => out.push("session bus: NOT available".into()),
    }
    out.push(format!("GNOME blank delay: {} s (0 = never)", gnome_idle_delay()));
    out.push(format!(
        "session type: {}",
        std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "unknown".into())
    ));
    out
}

pub fn settings() -> Result<(), String> {
    Command::new("xdg-open").arg(config::path()).status().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn extra(cmd: &str, _cfg: &config::Config) -> Result<(), String> {
    Err(format!("'{cmd}' exists on Windows only; on Linux use rust/install-linux.sh"))
}
