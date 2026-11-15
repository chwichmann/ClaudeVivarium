//! What differs between the operating systems: locking, idle time, keeping the display awake, logging.
use winit::window::Window;

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(windows)]
pub mod windows;

#[cfg(target_os = "linux")]
pub use linux as os;
#[cfg(target_os = "macos")]
pub use macos as os;
#[cfg(windows)]
pub use windows as os;

/// The screensaver's view of the operating system.
pub trait Platform {
    /// true = this program locks the screen before it closes (Linux, macOS).
    /// false = the system locks when the screensaver ends (Windows: "On resume, display logon screen").
    fn locks(&self) -> bool;
    fn screen_locked(&mut self) -> bool;
    /// Ask the system to lock now. Returns true if the request was accepted.
    fn request_lock(&mut self) -> bool;
    /// Seconds to wait after the lock is confirmed before closing.
    fn settle_seconds(&self) -> f64 {
        0.0
    }
    /// The lock is confirmed and we are about to close. `by_user`: a person woke us (key, mouse).
    fn locked(&mut self, _by_user: bool) {}
    /// A window was created (`fullscreen`: one of the screensaver windows, not the preview).
    fn window_created(&mut self, _window: &Window, _fullscreen: bool) {}
    /// All full-screen windows exist: take the focus, keep the display awake.
    fn begin_fullscreen(&mut self) {}
    /// Undo `begin_fullscreen` (called before exit).
    fn end(&mut self) {}
    /// Whether this program is the active application, if the system can tell (else window focus is used).
    fn app_active(&mut self) -> Option<bool> {
        None
    }
    /// Seconds without the focus that count as "focus lost".
    fn focus_patience(&self) -> f64 {
        0.3
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
pub mod os {
    pub struct Os;
    impl Os {
        pub fn new() -> Self {
            Os
        }
    }
    impl super::Platform for Os {
        fn locks(&self) -> bool {
            false
        }
        fn screen_locked(&mut self) -> bool {
            false
        }
        fn request_lock(&mut self) -> bool {
            false
        }
    }
    pub fn log(msg: &str) {
        eprintln!("[claude-vivarium] {msg}");
    }
    pub fn daemon(_args: &crate::args::Args) -> Result<(), String> {
        Err("the idle watcher is not available on this system".into())
    }
    pub fn selftest_lines() -> Vec<String> {
        Vec::new()
    }
    pub fn settings() -> Result<(), String> {
        Err("not available on this system".into())
    }
    pub fn extra(_cmd: &str, _cfg: &vivarium_core::config::Config) -> Result<(), String> {
        Err("not available on this system".into())
    }
}

pub fn log(msg: &str) {
    os::log(msg);
}

/// CPU time (user + system) this process has used so far, in seconds.
pub fn process_cpu_seconds() -> f64 {
    #[cfg(unix)]
    {
        // SAFETY: getrusage fills the struct we pass
        unsafe {
            let mut ru: libc::rusage = std::mem::zeroed();
            if libc::getrusage(libc::RUSAGE_SELF, &mut ru) != 0 {
                return 0.0;
            }
            let tv = |t: libc::timeval| t.tv_sec as f64 + t.tv_usec as f64 / 1e6;
            tv(ru.ru_utime) + tv(ru.ru_stime)
        }
    }
    #[cfg(windows)]
    {
        windows::process_cpu_seconds()
    }
    #[cfg(not(any(unix, windows)))]
    {
        0.0
    }
}
