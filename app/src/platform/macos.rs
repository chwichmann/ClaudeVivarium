//! macOS: windows at screen-saver level on every screen, lock via the menu-bar "Lock Screen" call,
//! idle agent (LaunchAgent). A native .saver bundle runs sandboxed inside Apple's screensaver host
//! and could not read ~/.claude, so the agent starts this program instead.
use std::ffi::{c_char, c_void, CStr};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use objc2::runtime::{AnyClass, AnyObject, Bool};
use objc2::msg_send;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use vivarium_core::{config, sysmon};
use winit::window::Window;

use super::Platform;
use crate::args::Args;

const SCREENSAVER_LEVEL: isize = 1000; // NSScreenSaverWindowLevel
const BEHAVIOR: usize = (1 << 0) | (1 << 4) | (1 << 8); // all spaces, stationary, full-screen auxiliary

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    static kCFBooleanTrue: *const c_void;
    fn CFStringCreateWithCString(alloc: *const c_void, s: *const c_char, encoding: u32) -> *const c_void;
    fn CFDictionaryGetValue(dict: *const c_void, key: *const c_void) -> *const c_void;
    fn CFGetTypeID(v: *const c_void) -> usize;
    fn CFNumberGetTypeID() -> usize;
    fn CFNumberGetValue(n: *const c_void, kind: isize, out: *mut c_void) -> u8;
    fn CFRelease(v: *const c_void);
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGSessionCopyCurrentDictionary() -> *const c_void;
    fn CGEventSourceSecondsSinceLastEventType(state: i32, event_type: u32) -> f64;
}

extern "C" {
    fn dlopen(path: *const c_char, mode: i32) -> *mut c_void;
    fn dlsym(handle: *mut c_void, name: *const c_char) -> *mut c_void;
}

// Global hotkey for "start the screensaver now" (Carbon hotkeys need no accessibility permission).
#[repr(C)]
#[derive(Clone, Copy)]
struct EventHotKeyId {
    signature: u32,
    id: u32,
}

#[repr(C)]
struct EventTypeSpec {
    class: u32,
    kind: u32,
}

#[link(name = "Carbon", kind = "framework")]
extern "C" {
    fn GetApplicationEventTarget() -> *mut c_void;
    fn RegisterEventHotKey(
        key_code: u32,
        modifiers: u32,
        id: EventHotKeyId,
        target: *mut c_void,
        options: u32,
        out: *mut *mut c_void,
    ) -> i32;
    fn InstallEventHandler(
        target: *mut c_void,
        handler: extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> i32,
        count: usize,
        list: *const EventTypeSpec,
        user_data: *mut c_void,
        out: *mut *mut c_void,
    ) -> i32;
}

#[link(name = "AppKit", kind = "framework")]
extern "C" {
    static NSDefaultRunLoopMode: *mut AnyObject;
}

/// Wait up to `seconds`, handing events (among them a hotkey press) to their handlers meanwhile.
fn pump_events(seconds: f64) {
    let app = nsapp();
    let Some(date_cls) = AnyClass::get(c"NSDate") else {
        std::thread::sleep(Duration::from_secs_f64(seconds));
        return;
    };
    if app.is_null() {
        std::thread::sleep(Duration::from_secs_f64(seconds));
        return;
    }
    // SAFETY: main thread, valid application object; the event is used before the next call
    unsafe {
        let until: *mut AnyObject = msg_send![date_cls, dateWithTimeIntervalSinceNow: seconds];
        loop {
            let ev: *mut AnyObject = msg_send![app, nextEventMatchingMask: u64::MAX, untilDate: until, inMode: NSDefaultRunLoopMode, dequeue: Bool::YES];
            if ev.is_null() {
                break; // time is up
            }
            let _: () = msg_send![app, sendEvent: ev];
            if HOTKEY_PRESSED.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
        }
    }
}

static HOTKEY_PRESSED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

extern "C" fn on_hotkey(_call: *mut c_void, _event: *mut c_void, _user: *mut c_void) -> i32 {
    HOTKEY_PRESSED.store(true, std::sync::atomic::Ordering::Relaxed);
    0
}

/// Register Ctrl+Option+V ("start now", like Ctrl+Alt+V on Windows). Returns false if the keys are taken.
fn register_hotkey() -> bool {
    const KEY_V: u32 = 9;
    const CONTROL: u32 = 0x1000;
    const OPTION: u32 = 0x0800;
    let spec = EventTypeSpec { class: u32::from_be_bytes(*b"keyb"), kind: 5 }; // hot key pressed
    // SAFETY: plain Carbon calls with valid pointers; the handler is a static function
    unsafe {
        // hotkeys are only delivered to a process that is known to the window server as an application
        let app = nsapp();
        if !app.is_null() {
            let _: Bool = msg_send![app, setActivationPolicy: 1isize]; // accessory: no Dock icon
            let _: () = msg_send![app, finishLaunching];
        }
        let target = GetApplicationEventTarget();
        if InstallEventHandler(target, on_hotkey, 1, &spec, std::ptr::null_mut(), std::ptr::null_mut()) != 0 {
            return false;
        }
        let mut hotkey: *mut c_void = std::ptr::null_mut();
        let id = EventHotKeyId { signature: u32::from_be_bytes(*b"CViv"), id: 1 };
        RegisterEventHotKey(KEY_V, CONTROL | OPTION, id, target, 0, &mut hotkey) == 0
    }
}

pub fn log(msg: &str) {
    eprintln!("{} [vivarium] {msg}", vivarium_core::util::strftime("%Y-%m-%d %H:%M:%S"));
}

pub fn screen_locked() -> bool {
    // SAFETY: CoreFoundation calls on objects we own or borrow for the duration of the call
    unsafe {
        let d = CGSessionCopyCurrentDictionary();
        if d.is_null() {
            return false;
        }
        let key = CFStringCreateWithCString(std::ptr::null(), c"CGSSessionScreenIsLocked".as_ptr(), 0x0800_0100);
        let v = CFDictionaryGetValue(d, key);
        let mut locked = false;
        if !v.is_null() {
            if v == kCFBooleanTrue {
                locked = true;
            } else if CFGetTypeID(v) == CFNumberGetTypeID() {
                let mut n: i64 = 0;
                CFNumberGetValue(v, 4, &mut n as *mut i64 as *mut c_void); // kCFNumberSInt64Type
                locked = n != 0;
            }
        }
        CFRelease(key);
        CFRelease(d);
        locked
    }
}

const LOGIN_FRAMEWORK: &CStr = c"/System/Library/PrivateFrameworks/login.framework/Versions/Current/login";

fn lock_call() -> Option<unsafe extern "C" fn() -> i32> {
    // SAFETY: looking up a symbol; the function takes no arguments
    unsafe {
        let h = dlopen(LOGIN_FRAMEWORK.as_ptr(), 1);
        if h.is_null() {
            return None;
        }
        let f = dlsym(h, c"SACLockScreenImmediate".as_ptr());
        if f.is_null() { None } else { Some(std::mem::transmute::<*mut c_void, unsafe extern "C" fn() -> i32>(f)) }
    }
}

/// Lock now. Uses the same call as the menu-bar "Lock Screen"; falls back to sleeping the display,
/// which locks when "Require password immediately" is set (System Settings > Lock Screen).
pub fn lock_screen() -> bool {
    if let Some(f) = lock_call() {
        // SAFETY: see lock_call
        unsafe { f() };
        return true;
    }
    log("SACLockScreenImmediate not available; using pmset displaysleepnow");
    Command::new("pmset").arg("displaysleepnow").status().map(|s| s.success()).unwrap_or(false)
}

pub fn idle_seconds() -> f64 {
    // SAFETY: plain query; 1 = HID system state, !0 = any input event
    unsafe { CGEventSourceSecondsSinceLastEventType(1, !0u32) }
}

fn nsapp() -> *mut AnyObject {
    let Some(cls) = AnyClass::get(c"NSApplication") else { return std::ptr::null_mut() };
    // SAFETY: +[NSApplication sharedApplication] returns the application object
    unsafe { msg_send![cls, sharedApplication] }
}

pub struct Os {
    caffeinate: Option<Child>,
}

impl Os {
    pub fn new() -> Self {
        Os { caffeinate: None }
    }
}

impl Platform for Os {
    fn locks(&self) -> bool {
        true
    }

    fn screen_locked(&mut self) -> bool {
        screen_locked()
    }

    fn request_lock(&mut self) -> bool {
        lock_screen()
    }

    fn window_created(&mut self, window: &Window, fullscreen: bool) {
        if !fullscreen {
            return;
        }
        let Ok(handle) = window.window_handle() else { return };
        let RawWindowHandle::AppKit(h) = handle.as_raw() else { return };
        let view: *mut AnyObject = h.ns_view.as_ptr().cast();
        // SAFETY: the view belongs to the window we just created, on the main thread
        unsafe {
            let win: *mut AnyObject = msg_send![view, window];
            if win.is_null() {
                return;
            }
            let _: () = msg_send![win, setLevel: SCREENSAVER_LEVEL];
            let _: () = msg_send![win, setCollectionBehavior: BEHAVIOR];
            let _: () = msg_send![win, setOpaque: Bool::YES];
            let _: () = msg_send![win, setAcceptsMouseMovedEvents: Bool::YES];
            let _: () = msg_send![win, makeKeyAndOrderFront: std::ptr::null::<AnyObject>()];
        }
    }

    fn begin_fullscreen(&mut self) {
        let app = nsapp();
        if !app.is_null() {
            // SAFETY: main thread, valid application object
            unsafe {
                let _: () = msg_send![app, activateIgnoringOtherApps: Bool::YES];
            }
        }
        // keep the display awake while we are visible (like the GNOME inhibitor)
        self.caffeinate =
            Command::new("caffeinate").args(["-d", "-w", &std::process::id().to_string()]).spawn().ok();
    }

    fn end(&mut self) {
        if let Some(mut c) = self.caffeinate.take() {
            let _ = c.kill();
        }
    }

    fn app_active(&mut self) -> Option<bool> {
        let app = nsapp();
        if app.is_null() {
            return None;
        }
        // SAFETY: main thread, valid application object
        let active: Bool = unsafe { msg_send![app, isActive] };
        Some(active.as_bool())
    }
}

/// True if another app (video player, presentation) prevents display sleep.
fn display_kept_awake_by_others() -> bool {
    let Ok(out) = Command::new("pmset").args(["-g", "assertions"]).output() else { return false };
    String::from_utf8_lossy(&out.stdout).lines().any(|line| {
        let s = line.trim();
        s.starts_with("PreventUserIdleDisplaySleep") && s.split_whitespace().last() != Some("0")
    })
}

/// Idle agent (runs as a LaunchAgent, see install-mac.sh).
///
/// Every 2 s it checks how long the Mac has been idle. After idle_seconds (and if the screen is not
/// locked and no app keeps the display awake, e.g. a video) it starts the screensaver. When the
/// screensaver exits and the screen is not locked, it locks (fail closed). It also records CPU/memory
/// every 5 s so the graph has a 30-minute history right away.
pub fn daemon(args: &Args) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let stop = crate::signals::stop_flag();
    let mut child: Option<Child> = None;
    let mut started_this_idle = false;
    let mut sampler = sysmon::Sampler::resumed();
    sampler.sample();
    let mut next_sample = Instant::now() + Duration::from_secs(sysmon::INTERVAL);
    let mut cfg = config::load();
    let idle_override = args.num("--idle", 0.0)?;
    log(&format!("agent started, idle_seconds={}", cfg.idle_seconds));
    let mut start_now = args.flag("--now");
    let hotkey = register_hotkey();
    log(if hotkey { "hotkey Ctrl+Option+V starts the screensaver now" } else { "the hotkey Ctrl+Option+V could not be registered" });

    while !stop.load(std::sync::atomic::Ordering::Relaxed) {
        if Instant::now() >= next_sample {
            sampler.sample();
            sampler.save();
            next_sample = Instant::now() + Duration::from_secs(sysmon::INTERVAL);
            cfg = config::load(); // pick up changed settings
        }
        let idle = idle_seconds();
        // the hotkey starts it right away, whatever the idle time or other apps say
        let by_hotkey = HOTKEY_PRESSED.swap(false, std::sync::atomic::Ordering::Relaxed);
        if by_hotkey && child.is_none() {
            log("hotkey pressed");
            std::thread::sleep(Duration::from_millis(600)); // let the keys be released first
            start_now = true;
        }
        if let Some(c) = child.as_mut() {
            if let Ok(Some(status)) = c.try_wait() {
                child = None;
                let locked = screen_locked();
                log(&format!("screensaver exited (code {}, locked={locked})", status.code().unwrap_or(-1)));
                if !locked {
                    log("screen not locked after screensaver - locking now");
                    lock_screen();
                }
            }
        } else {
            let wanted = if idle_override > 0.0 { idle_override } else { cfg.idle_seconds as f64 };
            if idle < 5.0 && !start_now {
                started_this_idle = false; // user is back: arm for the next idle period
            } else if (idle >= wanted && !started_this_idle) || start_now {
                started_this_idle = true;
                start_now = false;
                if screen_locked() {
                } else if !by_hotkey && display_kept_awake_by_others() {
                    log("another app keeps the display awake (video?) - not starting");
                } else {
                    log("idle - starting screensaver");
                    match Command::new(&exe).arg("run").spawn() {
                        Ok(c) => child = Some(c),
                        Err(e) => log(&format!("could not start the screensaver: {e}")),
                    }
                }
            }
        }
        if hotkey {
            // waits up to 2 s and delivers a hotkey press in the meantime
            pump_events(2.0);
        } else {
            std::thread::sleep(Duration::from_secs(2));
        }
    }

    if let Some(mut c) = child {
        if matches!(c.try_wait(), Ok(None)) {
            // agent stopped while shown: lock first
            log("stopping while the screensaver is shown - locking first");
            if !screen_locked() {
                lock_screen();
            }
            // SAFETY: sending SIGTERM to our own child (the screensaver also locks on SIGTERM itself)
            unsafe { libc::kill(c.id() as i32, libc::SIGTERM) };
        }
    }
    log("agent stopped");
    Ok(())
}

pub fn selftest_lines() -> Vec<String> {
    vec![
        format!("idle for {:.0} s, screen locked: {}", idle_seconds(), screen_locked()),
        if lock_call().is_some() {
            "lock call: SACLockScreenImmediate available".to_string()
        } else {
            "lock call: NOT available - will use 'pmset displaysleepnow' (set 'Require password' to Immediately)"
                .to_string()
        },
    ]
}

pub fn settings(_owner: Option<isize>) -> Result<(), String> {
    let p = config::path();
    Command::new("open").arg("-t").arg(&p).status().map_err(|e| e.to_string())?;
    Ok(())
}

/// Check that the hotkey arrives: waits up to 10 s for Ctrl+Option+V and says what happened.
pub fn hotkey_test() -> Result<(), String> {
    if !register_hotkey() {
        return Err("the hotkey could not be registered".into());
    }
    println!("press Ctrl+Option+V within 10 seconds ...");
    for _ in 0..20 {
        pump_events(0.5);
        if HOTKEY_PRESSED.load(std::sync::atomic::Ordering::Relaxed) {
            println!("hotkey received");
            return Ok(());
        }
    }
    Err("no hotkey arrived".into())
}

pub fn extra(cmd: &str, _cfg: &config::Config) -> Result<(), String> {
    if cmd == "hotkey-test" {
        return hotkey_test();
    }
    Err(format!("'{cmd}' exists on Windows only; on macOS use install-mac.sh"))
}
