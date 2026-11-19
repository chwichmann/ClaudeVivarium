//! The screensaver itself: windows on every screen, the frame loop, and - most important - what happens
//! on input. On Linux and macOS any key, click or focus loss LOCKS THE SCREEN FIRST and only then closes;
//! if the lock is not confirmed, or anything goes wrong, the program exits with an error so the idle
//! watcher locks (fails closed). On Windows the system locks when the screensaver ends.
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use vello::util::RenderSurface;
use vivarium_core::canvas::Canvas;
use vivarium_core::governor::{Governor, LEVELS};
use vivarium_core::scene::SceneOpts;
use vivarium_core::sessions::{self, Reader};
use vivarium_core::show::{parse_rotation, PickerKey, Show};
use vivarium_core::sysmon::{self, Monitor};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

use crate::cpu::CpuScreen;
use crate::gpu::{choose, write_png, Gpu, Kind, VelloSink};
use crate::platform::{self, log, Platform};

const GRACE: f64 = 1.0; // s after start during which input is ignored (avoids instant wake)
const MOTION_THRESHOLD: f64 = 25.0; // px of mouse movement that counts as "wake up"
const HINT: &str = "Press a key or click to unlock   ·   → next theme   ·   ↓ all themes";

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    /// the real screensaver: every screen, locks on input
    Full,
    /// preview in a normal window, never locks
    Windowed,
    /// Windows: drawn into the small preview of the Screen Saver Settings dialog (parent window handle)
    Preview(isize),
}

#[derive(Clone, Debug)]
pub struct Options {
    pub mode: Mode,
    pub demo: bool,
    pub no_lock: bool,
    pub themes: String,
    pub theme_minutes: f64,
    pub max_minutes: f64,
    pub mouse_wakes: bool,
    pub hide_text: bool,
    pub system_monitor: bool,
    pub other_monitors_theme: bool,
    pub adaptive_fps: bool,
    pub fps: u32,
    pub window_size: (u32, u32),
    /// true = `window_size` is in real pixels (benchmarks), false = in points
    pub window_physical: bool,
    /// measure for this many seconds, print the figures, exit
    pub bench: Option<f64>,
    /// save every window as PNG after this many seconds into this folder, then exit (testing)
    pub shot: Option<(f64, PathBuf)>,
    pub seed: Option<u64>,
    /// open the theme picker right at the start (testing, screenshots)
    pub open_picker: bool,
    /// "gpu", "cpu" or None for automatic
    pub renderer: Option<String>,
}

enum Screen {
    Gpu(Box<RenderSurface<'static>>),
    Cpu(Box<CpuScreen>),
}

struct View {
    window: Arc<Window>,
    screen: Screen,
    show: Option<Show>,
    focused: bool,
    /// draw the scene as if the window had this size and scale it down (tiny preview window)
    virtual_size: Option<(f64, f64)>,
}

struct Exit {
    deadline: Instant,
    by_user: bool,
    locked_at: Option<Instant>,
}

struct Bench {
    work: Vec<f64>,
    gaps: Vec<f64>,
    cpu0: f64,
    t0: Instant,
}

struct App {
    o: Options,
    os: Box<dyn Platform>,
    kind: Kind,
    gpu: Gpu,
    sink: VelloSink,
    views: Vec<View>,
    reader: Reader,
    monitor: Option<Monitor>,
    /// benchmarks: the same made-up history every time, so runs are comparable
    fake_system: Option<Rc<sysmon::SysData>>,
    gov: Governor,
    started: Instant,
    /// when the made-up sessions started (so switching to demo data begins at the start of its cycle)
    demo_since: Instant,
    last_frame: Instant,
    /// when the next frame is due (advances in even steps, so timer slop does not add up)
    due: Instant,
    next_refresh: Instant,
    exiting: Option<Exit>,
    was_active: bool,
    focus_lost: Option<Instant>,
    mouse_origin: Option<(WindowId, f64, f64)>,
    stop: Arc<AtomicBool>,
    bench: Option<Bench>,
    frames: u64,
}

fn factory(name: &str, opts: SceneOpts) -> Option<Box<dyn vivarium_core::scene::Scene>> {
    vivarium_themes::make(name, opts)
}

impl App {
    fn full(&self) -> bool {
        self.o.mode == Mode::Full
    }

    fn locking(&self) -> bool {
        self.full() && !self.o.no_lock && self.os.locks()
    }

    fn in_grace(&self) -> bool {
        self.started.elapsed().as_secs_f64() < GRACE
    }

    fn secs(&self) -> f64 {
        self.started.elapsed().as_secs_f64()
    }

    fn make_show(&self, decor: bool, avoid: Option<&str>) -> Show {
        let names = parse_rotation(&self.o.themes, vivarium_themes::NAMES, &vivarium_themes::resting());
        let opts = SceneOpts { show_text: !self.o.hide_text, decor, seed: self.o.seed };
        Show::new(factory, names, vivarium_themes::NAMES, self.o.theme_minutes, opts, avoid)
    }

    fn add_view(&mut self, window: Window, show: Option<Show>, virtual_size: Option<(f64, f64)>) -> Result<(), String> {
        let window = Arc::new(window);
        let size = window.inner_size();
        let screen = match self.kind {
            Kind::Gpu => Screen::Gpu(Box::new(self.gpu.create_surface(window.clone(), size.width, size.height)?)),
            Kind::Cpu => Screen::Cpu(Box::new(CpuScreen::new(window.clone())?)),
        };
        self.os.window_created(&window, self.full());
        self.views.push(View { window, screen, show, focused: false, virtual_size });
        Ok(())
    }

    fn create_windows(&mut self, el: &ActiveEventLoop) -> Result<(), String> {
        match self.o.mode {
            Mode::Full => {
                let mut monitors: Vec<_> = el.available_monitors().collect();
                if let Some(p) = el.primary_monitor() {
                    monitors.sort_by_key(|m| *m != p); // primary first
                }
                if monitors.is_empty() {
                    return Err("no monitor found".into());
                }
                for (i, m) in monitors.into_iter().enumerate() {
                    let attrs = Window::default_attributes().with_title("Claude Vivarium").with_decorations(false);
                    // macOS: a borderless window over the whole screen, raised to screen-saver level by the
                    // platform code (native full screen would animate into its own Space)
                    #[cfg(target_os = "macos")]
                    let attrs = attrs.with_position(m.position()).with_inner_size(m.size());
                    #[cfg(not(target_os = "macos"))]
                    let attrs = attrs.with_fullscreen(Some(winit::window::Fullscreen::Borderless(Some(m))));
                    #[cfg(windows)]
                    let attrs = {
                        use winit::platform::windows::WindowAttributesExtWindows;
                        attrs.with_window_level(winit::window::WindowLevel::AlwaysOnTop).with_skip_taskbar(true)
                    };
                    let window = el.create_window(attrs).map_err(|e| format!("window: {e}"))?;
                    window.set_cursor_visible(false);
                    let show = if i == 0 {
                        Some(self.make_show(false, None))
                    } else if self.o.other_monitors_theme {
                        let main = self.views[0].show.as_ref().map(|s| s.name());
                        Some(self.make_show(true, main))
                    } else {
                        None
                    };
                    self.add_view(window, show, None)?;
                    if i == 0 {
                        self.views[0].window.focus_window();
                    }
                }
                self.os.begin_fullscreen();
            }
            Mode::Windowed => {
                let (w, h) = self.o.window_size;
                let attrs = Window::default_attributes().with_title("Claude Vivarium (preview - N: next theme, D: demo, Esc: close)");
                let attrs = if self.o.window_physical {
                    attrs.with_inner_size(PhysicalSize::new(w, h))
                } else {
                    attrs.with_inner_size(LogicalSize::new(w, h))
                };
                let window = el.create_window(attrs).map_err(|e| format!("window: {e}"))?;
                let show = self.make_show(false, None);
                self.add_view(window, Some(show), None)?;
            }
            Mode::Preview(parent) => {
                #[cfg(windows)]
                {
                    let (w, h) = platform::windows::client_size(parent);
                    let handle = raw_window_handle::Win32WindowHandle::new(
                        std::num::NonZeroIsize::new(parent).ok_or("preview: no parent window")?,
                    );
                    let attrs = Window::default_attributes()
                        .with_title("preview")
                        .with_decorations(false)
                        .with_position(PhysicalPosition::new(0, 0))
                        .with_inner_size(PhysicalSize::new(w.max(1), h.max(1)));
                    // SAFETY: the handle is the window Windows passed us with /p; we check that it stays alive
                    let attrs = unsafe { attrs.with_parent_window(Some(raw_window_handle::RawWindowHandle::Win32(handle))) };
                    let window = el.create_window(attrs).map_err(|e| format!("window: {e}"))?;
                    let show = self.make_show(false, None);
                    self.add_view(window, Some(show), Some((800.0, 500.0)))?;
                }
                #[cfg(not(windows))]
                {
                    let _ = (parent, PhysicalPosition::new(0, 0), PhysicalSize::new(1u32, 1u32));
                    return Err("the preview mode exists on Windows only".into());
                }
            }
        }
        let theme = self.views[0].show.as_ref().map(|s| s.name()).unwrap_or("black");
        log(&format!(
            "start: mode={:?}, screens={}, theme={theme}, {} data, lock={}, max_minutes={}, mouse_move={}",
            self.o.mode,
            self.views.len(),
            if self.o.demo { "demo" } else { "live" },
            if self.locking() { "on" } else if self.full() && !self.os.locks() { "by the system" } else { "off" },
            if self.o.max_minutes > 0.0 { self.o.max_minutes.to_string() } else { "never".into() },
            if self.o.mouse_wakes { "wake" } else { "hint" },
        ));
        Ok(())
    }

    // ------------------------------------------------------------------ data
    fn refresh(&mut self) {
        let data = if self.o.demo { sessions::demo_sessions(self.demo_since.elapsed().as_secs_f64()) } else { self.reader.read() };
        let system = match &self.fake_system {
            Some(f) => Some(f.clone()),
            None => self.monitor.as_mut().and_then(|m| m.get()),
        };
        for (i, v) in self.views.iter_mut().enumerate() {
            if let Some(show) = v.show.as_mut() {
                show.set_sessions(&data);
                if i == 0 {
                    show.set_system(system.clone());
                }
            }
        }
    }

    // ------------------------------------------------------------------ wake = lock, then close
    fn wake(&mut self, reason: &str) {
        if self.exiting.is_some() {
            return;
        }
        log(&format!("wake: {reason}"));
        if !self.locking() {
            self.finish(0);
        }
        let by_user = ["key", "mouse", "scroll", "focus"].iter().any(|p| reason.starts_with(p));
        if !self.os.screen_locked() {
            self.os.request_lock();
        }
        self.exiting = Some(Exit { deadline: Instant::now() + Duration::from_secs(5), by_user, locked_at: None });
    }

    fn finish(&mut self, code: i32) -> ! {
        self.os.end();
        if let Some(b) = self.bench.take() {
            print_bench(&b, &self.o, &self.views, self.gov.fps());
        }
        log(&format!("exit (code {code})"));
        std::process::exit(code) // leave immediately; nothing needs cleaning up
    }

    /// Waiting until the system confirms the lock.
    fn wait_locked(&mut self) {
        let Some(ex) = self.exiting.as_mut() else { return };
        if let Some(at) = ex.locked_at {
            if at.elapsed().as_secs_f64() >= self.os.settle_seconds() {
                let by_user = ex.by_user;
                self.os.locked(by_user);
                self.finish(0);
            }
            return;
        }
        if self.os.screen_locked() {
            log("screen is locked - closing");
            if let Some(ex) = self.exiting.as_mut() {
                ex.locked_at = Some(Instant::now());
            }
        } else if Instant::now() > ex.deadline {
            log("lock NOT confirmed within 5 s - exiting with error so the idle watcher locks");
            self.finish(3);
        }
    }

    // ------------------------------------------------------------------ main loop
    fn tick(&mut self, el: &ActiveEventLoop) {
        let now = Instant::now();
        if self.stop.load(Ordering::Relaxed) {
            self.stop.store(false, Ordering::Relaxed);
            if self.full() {
                self.wake("signal"); // killed: lock first, then go - never leave the desktop open
            } else {
                self.finish(0);
            }
        }
        if self.exiting.is_some() {
            self.wait_locked();
            el.set_control_flow(ControlFlow::WaitUntil(now + Duration::from_millis(100)));
            return;
        }
        if self.views.is_empty() {
            return;
        }
        if self.full() {
            let active = self.os.app_active().unwrap_or_else(|| self.views.iter().any(|v| v.focused));
            if active {
                if !self.was_active {
                    log(&format!("got the focus after {:.1} s", self.secs()));
                }
                self.was_active = true;
                self.focus_lost = None;
            } else if self.was_active {
                // something (a shortcut, another program) took the focus away -> lock right away
                let since = *self.focus_lost.get_or_insert(now);
                if since.elapsed().as_secs_f64() >= self.os.focus_patience() && !self.in_grace() {
                    self.wake("focus lost");
                    return;
                }
            } else if self.locking() && self.secs() > 3.0 {
                log("never got the keyboard focus - failing closed");
                self.wake("no focus");
                return;
            }
            if self.o.max_minutes > 0.0 && self.secs() > self.o.max_minutes * 60.0 {
                self.wake("max time reached");
                return;
            }
        }
        if let Mode::Preview(_parent) = self.o.mode {
            #[cfg(windows)]
            if !platform::windows::is_window(_parent) {
                self.finish(0);
            }
        }
        if now >= self.next_refresh {
            self.next_refresh = now + Duration::from_secs(1);
            if self.locking() && self.os.screen_locked() {
                // the system locked by other means (suspend, shortcut, ...): we are done
                log("screen got locked by the system - closing");
                self.finish(0);
            }
            self.refresh();
        }
        if let Some(limit) = self.o.bench {
            if self.secs() >= limit {
                self.finish(0);
            }
        }
        if let Some((after, dir)) = self.o.shot.clone() {
            if self.secs() >= after {
                self.save_shots(&dir);
                self.finish(0);
            }
        }
        let waited = (now - self.last_frame).as_secs_f64();
        if now >= self.due && self.gov.due(waited) {
            self.last_frame = now;
            let period = Duration::from_secs_f64(1.0 / self.gov.fps() as f64);
            self.due += period;
            if self.due < now {
                self.due = now + period / 2; // fell behind: do not try to catch up
            }
            if let Err(e) = self.frame(waited) {
                // an error must never leave the screen open
                log(&format!("frame error: {e}"));
                if self.full() {
                    self.wake("error");
                    if self.exiting.is_none() {
                        self.finish(1);
                    }
                    return;
                }
                self.finish(1);
            }
            let work = now.elapsed().as_secs_f64();
            // a frame that came late also counts as time lost (busy computer), so the frame rate adapts
            let late = (waited - 1.0 / self.gov.fps() as f64).max(0.0);
            self.gov.record(work + late.min(0.2), self.secs());
            if let Some(b) = self.bench.as_mut() {
                if self.frames > 5 {
                    b.work.push(work);
                    b.gaps.push(waited);
                } else {
                    b.cpu0 = platform::process_cpu_seconds();
                    b.t0 = Instant::now();
                }
            }
            self.frames += 1;
        }
        el.set_control_flow(ControlFlow::WaitUntil(self.due.max(now + Duration::from_millis(1))));
    }

    /// Build the frame of one window in its renderer. Returns the size of the window in pixels.
    fn build(sink: &mut VelloSink, v: &mut View) -> (u32, u32) {
        let size = v.window.inner_size();
        let (w, h) = (size.width.max(1), size.height.max(1));
        // the CPU renderer may draw a big window at half size
        let (dw, dh, sink): (u32, u32, &mut dyn vivarium_core::canvas::Sink) = match &mut v.screen {
            Screen::Gpu(_) => {
                sink.begin();
                (w, h, sink)
            }
            Screen::Cpu(c) => {
                let (dw, dh) = c.begin(w, h);
                (dw, dh, &mut c.sink)
            }
        };
        if let Some(show) = v.show.as_mut() {
            let mut cr = Canvas::new(sink);
            match v.virtual_size {
                Some((vw, vh)) => {
                    cr.scale(dw as f64 / vw, dh as f64 / vh);
                    show.draw(&mut cr, vw, vh);
                }
                None => show.draw(&mut cr, dw as f64, dh as f64),
            }
            cr.finish();
        }
        (w, h)
    }

    fn frame(&mut self, dt: f64) -> Result<(), String> {
        let main_theme = self.views[0].show.as_ref().map(|s| s.name());
        for i in 0..self.views.len() {
            let v = &mut self.views[i];
            if let Some(show) = v.show.as_mut() {
                show.update(dt, if i == 0 { None } else { main_theme });
            }
            let (w, h) = Self::build(&mut self.sink, v);
            match &mut v.screen {
                Screen::Gpu(surface) => {
                    self.gpu.resize(surface, w, h);
                    self.gpu.present(surface, &self.sink.scene)?;
                }
                Screen::Cpu(c) => c.present(w, h)?,
            }
        }
        Ok(())
    }

    fn save_shots(&mut self, dir: &std::path::Path) {
        let _ = std::fs::create_dir_all(dir);
        let fps = self.frames as f64 / self.secs().max(0.001);
        let mut names = Vec::new();
        let mode = format!("{:?}", self.o.mode).to_lowercase();
        let mode = mode.split('(').next().unwrap_or("window").to_string();
        for i in 0..self.views.len() {
            let (w, h) = Self::build(&mut self.sink, &mut self.views[i]);
            names.push(self.views[i].show.as_ref().map(|s| s.name()).unwrap_or("black"));
            let shot = match &self.views[i].screen {
                Screen::Gpu(_) => self.gpu.render_pixels(&self.sink.scene, w, h).map(|px| (px, w, h)),
                Screen::Cpu(c) => {
                    let (sw, sh) = c.sink.size();
                    Ok((c.sink.opaque_pixels(), sw, sh))
                }
            };
            match shot {
                Ok((px, pw, ph)) => {
                    if let Err(e) = write_png(&dir.join(format!("{mode}_{i}.png")), &px, pw, ph) {
                        log(&format!("test: {e}"));
                    }
                }
                Err(e) => log(&format!("test: {e}")),
            }
        }
        let sizes: Vec<_> = self.views.iter().map(|v| (v.window.inner_size().width, v.window.inner_size().height)).collect();
        log(&format!("test: {fps:.1} fps, windows {sizes:?}, themes {names:?}"));
    }

    // ------------------------------------------------------------------ input
    /// Keys of the theme picker. Returns true if the key was used (so it must not wake or close).
    fn picker_key(&mut self, key: &Key) -> bool {
        let Some(show) = self.views.first_mut().and_then(|v| v.show.as_mut()) else { return false };
        let t = matches!(key, Key::Character(c) if c.eq_ignore_ascii_case("t"));
        let named = match key {
            Key::Named(n) => Some(*n),
            _ => None,
        };
        if !show.picker_open() {
            // Down, Up, Tab or T: show all themes to choose from
            if t || matches!(named, Some(NamedKey::ArrowDown | NamedKey::ArrowUp | NamedKey::Tab)) {
                show.open_picker();
                return true;
            }
            return false;
        }
        match named {
            Some(NamedKey::ArrowLeft) => show.picker_move(PickerKey::Left),
            Some(NamedKey::ArrowRight) => show.picker_move(PickerKey::Right),
            Some(NamedKey::ArrowUp) => show.picker_move(PickerKey::Up),
            Some(NamedKey::ArrowDown) => show.picker_move(PickerKey::Down),
            Some(NamedKey::Enter | NamedKey::Space) => show.picker_choose(),
            Some(NamedKey::Escape) => show.close_picker(),
            _ if t => show.close_picker(),
            _ => {} // other keys do nothing while choosing (the picker closes by itself after a while)
        }
        true
    }

    /// D: switch between the real sessions and made-up ones, while running.
    fn toggle_demo(&mut self) {
        self.o.demo = !self.o.demo;
        self.demo_since = Instant::now();
        log(&format!("data: {}", if self.o.demo { "demo" } else { "live" }));
        self.refresh();
        let text = if self.o.demo { "Demo sessions   ·   D: back to your real sessions" } else { "Your real sessions" };
        if let Some(s) = self.views.first_mut().and_then(|v| v.show.as_mut()) {
            s.show_hint(text, 3.0);
        }
    }

    fn on_key(&mut self, key: &Key) {
        let next = matches!(key, Key::Named(NamedKey::ArrowRight));
        let d = matches!(key, Key::Character(c) if c.eq_ignore_ascii_case("d"));
        match self.o.mode {
            Mode::Preview(_) => {}
            Mode::Windowed => {
                if self.picker_key(key) {
                    return;
                }
                if d {
                    self.toggle_demo();
                    return;
                }
                let n = matches!(key, Key::Character(c) if c.eq_ignore_ascii_case("n"));
                if matches!(key, Key::Named(NamedKey::Escape)) {
                    self.finish(0);
                } else if next || n || matches!(key, Key::Named(NamedKey::Space)) {
                    for v in &mut self.views {
                        if let Some(s) = v.show.as_mut() {
                            s.next_theme(); // preview: flip through the themes
                        }
                    }
                }
            }
            Mode::Full => {
                if self.in_grace() {
                    return;
                }
                if self.picker_key(key) {
                    return;
                }
                if next {
                    // right arrow = next theme, D = demo data, any other key wakes (and locks)
                    if let Some(s) = self.views.first_mut().and_then(|v| v.show.as_mut()) {
                        s.next_theme();
                    }
                } else if d {
                    self.toggle_demo();
                } else {
                    self.wake("key");
                }
            }
        }
    }

    fn on_motion(&mut self, id: WindowId, x: f64, y: f64, scale: f64) {
        if !self.full() {
            return;
        }
        match self.mouse_origin {
            Some((oid, ox, oy)) if oid == id && !self.in_grace() => {
                if (x - ox).abs() + (y - oy).abs() > MOTION_THRESHOLD * scale {
                    if self.o.mouse_wakes {
                        self.wake("mouse moved");
                    } else {
                        self.mouse_origin = Some((id, x, y));
                        if let Some(s) = self.views.first_mut().and_then(|v| v.show.as_mut()) {
                            s.show_hint(HINT, 4.0);
                        }
                    }
                }
            }
            _ => self.mouse_origin = Some((id, x, y)),
        }
    }
}

fn print_bench(b: &Bench, o: &Options, views: &[View], fps_now: u32) {
    let renderer = match views.first().map(|v| &v.screen) {
        Some(Screen::Cpu(_)) => "cpu",
        _ => "gpu",
    };
    let n = b.work.len();
    if n == 0 {
        println!("bench: no frames");
        return;
    }
    let wall = b.t0.elapsed().as_secs_f64();
    let cpu = platform::process_cpu_seconds() - b.cpu0;
    let mut work = b.work.clone();
    work.sort_by(f64::total_cmp);
    let target = 1.0 / o.fps as f64;
    let late = b.gaps.iter().filter(|g| **g > target * 1.5).count();
    let size = views.first().map(|v| v.window.inner_size()).unwrap_or_default();
    let theme = views.first().and_then(|v| v.show.as_ref()).map(|s| s.name()).unwrap_or("black");
    println!("bench rust ({renderer}): theme={theme} size={}x{} target={} fps (now {fps_now}) seconds={wall:.1}", size.width, size.height, o.fps);
    println!(
        "  frames={n}  fps={:.1}  frame avg={:.2} ms  p95={:.2} ms  max={:.2} ms  late={late} ({:.1} %)",
        n as f64 / wall,
        work.iter().sum::<f64>() / n as f64 * 1000.0,
        work[(n as f64 * 0.95) as usize % n] * 1000.0,
        work[n - 1] * 1000.0,
        late as f64 / n as f64 * 100.0,
    );
    println!("  process CPU: {:.1} % of one core ({cpu:.2} s CPU in {wall:.1} s)", cpu / wall * 100.0);
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if !self.views.is_empty() {
            return;
        }
        if let Err(e) = self.create_windows(el) {
            log(&format!("cannot start: {e}"));
            eprintln!("claude-vivarium: {e}");
            self.os.end();
            std::process::exit(2); // the idle watcher locks if the screen is not locked
        }
        if self.o.open_picker {
            self.refresh();
            let t0 = Instant::now();
            if let Some(show) = self.views.first_mut().and_then(|v| v.show.as_mut()) {
                show.open_picker();
            }
            log(&format!("theme picker prepared in {:.0} ms", t0.elapsed().as_secs_f64() * 1000.0));
        }
        self.started = Instant::now();
        self.demo_since = self.started;
        self.last_frame = Instant::now();
        self.due = self.last_frame;
        self.refresh();
        self.next_refresh = Instant::now() + Duration::from_secs(1);
    }

    fn new_events(&mut self, el: &ActiveEventLoop, cause: StartCause) {
        if matches!(cause, StartCause::ResumeTimeReached { .. } | StartCause::Poll) {
            self.tick(el);
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        if self.views.is_empty() {
            return;
        }
        // make sure the timer is armed even when no frame was due yet
        if matches!(el.control_flow(), ControlFlow::Wait) {
            self.tick(el);
        }
    }

    fn window_event(&mut self, _el: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                if self.full() {
                    self.wake("close requested"); // we close ourselves once locked
                } else {
                    self.finish(0);
                }
            }
            WindowEvent::Destroyed => {
                if !self.full() {
                    self.finish(0);
                }
            }
            WindowEvent::Focused(f) => {
                if let Some(v) = self.views.iter_mut().find(|v| v.window.id() == id) {
                    v.focused = f;
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed && !event.repeat {
                    self.on_key(&event.logical_key);
                }
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, .. } => {
                if self.full() && !self.in_grace() {
                    self.wake("mouse button");
                }
            }
            WindowEvent::MouseWheel { .. } => {
                if self.full() && !self.in_grace() {
                    self.wake("scroll");
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let scale = self.views.iter().find(|v| v.window.id() == id).map(|v| v.window.scale_factor()).unwrap_or(1.0);
                self.on_motion(id, position.x, position.y, scale);
            }
            _ => {}
        }
    }
}

pub fn run(o: Options) -> Result<(), String> {
    let mut builder = EventLoop::builder();
    #[cfg(target_os = "macos")]
    if o.mode == Mode::Full {
        use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
        builder.with_activation_policy(ActivationPolicy::Accessory); // no Dock icon
    }
    #[cfg(windows)]
    if o.mode == Mode::Full {
        use winit::platform::windows::EventLoopBuilderExtWindows;
        builder.with_msg_hook(platform::windows::swallow_screensave_message);
    }
    let event_loop = builder.build().map_err(|e| format!("event loop: {e}"))?;
    event_loop.set_control_flow(ControlFlow::Wait);
    vivarium_core::text::warm_up();
    let now = Instant::now();
    let fps = o.fps.max(1);
    let levels: Vec<u32> = LEVELS.iter().copied().filter(|l| *l < fps).collect();
    let mut all = vec![fps];
    all.extend(levels);
    let benching = o.bench.is_some();
    let monitor = if o.system_monitor && !benching { Some(Monitor::new()) } else { None };
    let fake_system = if o.system_monitor && benching { Some(Rc::new(sysmon::fake())) } else { None };
    let bench = o.bench.map(|_| Bench { work: Vec::new(), gaps: Vec::new(), cpu0: 0.0, t0: now });
    let (kind, why) = choose(o.renderer.as_deref());
    log(&format!("drawing with: {why}"));
    let mut app = App {
        kind,
        gov: Governor::new(fps, &all, o.adaptive_fps, 0.0),
        os: Box::new(platform::os::Os::new()),
        gpu: Gpu::new(),
        sink: VelloSink::new(),
        views: Vec::new(),
        reader: Reader::new(),
        monitor,
        fake_system,
        started: now,
        demo_since: now,
        last_frame: now,
        due: now,
        next_refresh: now,
        exiting: None,
        was_active: false,
        focus_lost: None,
        mouse_origin: None,
        stop: crate::signals::stop_flag(),
        bench,
        frames: 0,
        o,
    };
    let r = event_loop.run_app(&mut app).map_err(|e| format!("event loop: {e}"));
    app.os.end();
    r
}
