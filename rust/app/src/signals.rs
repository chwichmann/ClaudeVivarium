//! Kill signals (service stop, logout, kill): they only set a flag; the main loop then locks first.
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

/// A flag that becomes true when the process is asked to stop (SIGTERM, SIGINT, SIGHUP).
pub fn stop_flag() -> Arc<AtomicBool> {
    let flag = Arc::new(AtomicBool::new(false));
    #[cfg(unix)]
    for sig in [signal_hook::consts::SIGTERM, signal_hook::consts::SIGINT, signal_hook::consts::SIGHUP] {
        let _ = signal_hook::flag::register(sig, flag.clone());
    }
    flag
}
