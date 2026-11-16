//! Windows: a real Windows screen saver (.scr).
//!
//! Windows calls a screen saver with:
//!   /s            show full screen on all monitors (main monitor: labels, banner, strip, panel;
//!                 other monitors: another theme as decoration)
//!   /p <hwnd>     draw the small preview into the Screen Saver Settings dialog
//!   /c[:hwnd]     the "Settings..." button -> opens config.ini
//! Locking on wake is done by Windows itself ("On resume, display logon screen").
use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};

use vivarium_core::{config, sysmon};
use windows_sys::Win32::Foundation::{FILETIME, GENERIC_READ, GENERIC_WRITE, HWND, RECT};
use windows_sys::Win32::Media::{timeBeginPeriod, timeEndPeriod};
use windows_sys::Win32::Storage::FileSystem::{CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING};
use windows_sys::Win32::System::Console::{
    AttachConsole, GetStdHandle, SetStdHandle, ATTACH_PARENT_PROCESS, STD_ERROR_HANDLE, STD_OUTPUT_HANDLE,
};
use windows_sys::Win32::System::Power::{SetThreadExecutionState, ES_CONTINUOUS, ES_DISPLAY_REQUIRED};
use windows_sys::Win32::System::Threading::{CreateMutexW, GetCurrentProcess, GetProcessTimes};
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetClientRect, GetDesktopWindow, IsWindow, MessageBoxW, SendMessageW, SystemParametersInfoW, MB_ICONINFORMATION, MSG,
    SW_SHOWNORMAL, WM_SYSCOMMAND,
};

use super::Platform;
use crate::args::Args;

const SC_SCREENSAVE: u32 = 0xF140;
const SC_MONITORPOWER: u32 = 0xF170;
const APP_NAME: &str = "ClaudeVivariumRS";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const DETACHED: u32 = 0x0000_0008 | 0x0000_0200; // detached, own process group
static CONSOLE: AtomicBool = AtomicBool::new(false);

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn wide_path(p: &Path) -> Vec<u16> {
    p.as_os_str().encode_wide().chain(std::iter::once(0)).collect()
}

fn local_appdata() -> PathBuf {
    std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(vivarium_core::sessions::home_dir)
}

fn appdata() -> PathBuf {
    std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(vivarium_core::sessions::home_dir)
}

fn install_dir() -> PathBuf {
    local_appdata().join("Programs").join(APP_NAME)
}

fn log_path() -> PathBuf {
    local_appdata().join("ClaudeVivarium").join("vivarium-rs.log")
}

pub fn log(msg: &str) {
    use std::io::Write;
    let p = log_path();
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&p) {
        let _ = writeln!(f, "{} {msg}", vivarium_core::util::strftime("%Y-%m-%d %H:%M:%S"));
    }
}

/// The program is built without a console (a screen saver must not open one). When it is started
/// from a terminal, connect to that terminal so `render`, `bench`, `selftest` can print.
pub fn attach_console() {
    // SAFETY: plain Win32 calls; the handles stay open for the life of the process
    unsafe {
        let valid = |h: *mut c_void| !h.is_null() && h as isize != -1;
        if valid(GetStdHandle(STD_OUTPUT_HANDLE)) {
            CONSOLE.store(true, Ordering::Relaxed); // output is redirected to a file or pipe: keep it
            return;
        }
        if AttachConsole(ATTACH_PARENT_PROCESS) == 0 {
            return;
        }
        let name = wide("CONOUT$");
        let h = CreateFileW(
            name.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        );
        if !h.is_null() && h as isize != -1 {
            SetStdHandle(STD_OUTPUT_HANDLE, h);
            SetStdHandle(STD_ERROR_HANDLE, h);
            CONSOLE.store(true, Ordering::Relaxed);
        }
    }
}

/// Show a message: console if there is one, otherwise a message box.
pub fn say(msg: &str) {
    log(&msg.replace('\n', " | "));
    if CONSOLE.load(Ordering::Relaxed) {
        println!("{msg}");
    } else {
        let (text, title) = (wide(msg), wide("Claude Vivarium"));
        // SAFETY: both strings are null-terminated and live during the call
        unsafe { MessageBoxW(std::ptr::null_mut(), text.as_ptr(), title.as_ptr(), MB_ICONINFORMATION) };
    }
}

pub fn client_size(hwnd: isize) -> (u32, u32) {
    let mut rc = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    // SAFETY: valid out pointer; a wrong handle only makes the call fail
    unsafe { GetClientRect(hwnd as HWND, &mut rc) };
    ((rc.right - rc.left).max(1) as u32, (rc.bottom - rc.top).max(1) as u32)
}

pub fn is_window(hwnd: isize) -> bool {
    // SAFETY: any value may be passed
    unsafe { IsWindow(hwnd as HWND) != 0 }
}

/// Message hook while the screen saver shows: do not start another screen saver (or switch the
/// monitor off) on top of us. Returns true for messages that should be dropped.
pub fn swallow_screensave_message(msg: *const c_void) -> bool {
    if msg.is_null() {
        return false;
    }
    // SAFETY: winit passes a pointer to the MSG it is about to dispatch
    let m = unsafe { &*(msg as *const MSG) };
    m.message == WM_SYSCOMMAND && matches!((m.wParam & 0xFFF0) as u32, SC_SCREENSAVE | SC_MONITORPOWER)
}

pub fn process_cpu_seconds() -> f64 {
    let zero = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
    let (mut c, mut e, mut k, mut u) = (zero, zero, zero, zero);
    // SAFETY: four valid out pointers
    if unsafe { GetProcessTimes(GetCurrentProcess(), &mut c, &mut e, &mut k, &mut u) } == 0 {
        return 0.0;
    }
    let ft = |f: &FILETIME| (((f.dwHighDateTime as u64) << 32) | f.dwLowDateTime as u64) as f64 / 1e7;
    ft(&k) + ft(&u)
}

pub struct Os {
    awake: bool,
}

impl Os {
    pub fn new() -> Self {
        Os { awake: false }
    }
}

impl Platform for Os {
    fn locks(&self) -> bool {
        false // Windows shows the logon screen itself when the screen saver ends
    }

    fn screen_locked(&mut self) -> bool {
        false
    }

    fn request_lock(&mut self) -> bool {
        false
    }

    fn begin_fullscreen(&mut self) {
        // keep the display on while the screen saver is shown (like the GNOME version); max_minutes ends it
        // SAFETY: plain Win32 calls
        unsafe {
            SetThreadExecutionState(ES_CONTINUOUS | ES_DISPLAY_REQUIRED);
            timeBeginPeriod(1);
        }
        self.awake = true;
    }

    fn end(&mut self) {
        if self.awake {
            // SAFETY: plain Win32 calls
            unsafe {
                SetThreadExecutionState(ES_CONTINUOUS); // back to normal power management
                timeEndPeriod(1);
            }
            self.awake = false;
        }
    }
}

pub fn daemon(_args: &Args) -> Result<(), String> {
    Err("Windows starts the screen saver itself - use --install; the recorder is --record".into())
}

pub fn selftest_lines() -> Vec<String> {
    vec![format!("log: {}", log_path().display()), format!("install folder: {}", install_dir().display())]
}

/// The "Settings..." button: open config.ini.
pub fn settings() -> Result<(), String> {
    let _ = config::load(); // creates the file with defaults if missing
    let (verb, file) = (wide("open"), wide_path(&config::path()));
    // SAFETY: null-terminated strings that live during the call
    let r = unsafe {
        ShellExecuteW(std::ptr::null_mut(), verb.as_ptr(), file.as_ptr(), std::ptr::null(), std::ptr::null(), SW_SHOWNORMAL)
    };
    if r as isize > 32 { Ok(()) } else { Err(format!("could not open {}", config::path().display())) }
}

/// Background CPU/memory recorder (one instance per user, shared with the Python version).
fn record() -> Result<(), String> {
    let name = wide("Local\\ClaudeVivariumRecorder");
    // SAFETY: null-terminated name; the mutex is held until the process ends
    unsafe {
        CreateMutexW(std::ptr::null(), 0, name.as_ptr());
    }
    if std::io::Error::last_os_error().raw_os_error() == Some(183) {
        return Ok(()); // ERROR_ALREADY_EXISTS
    }
    let mut s = sysmon::Sampler::resumed();
    s.sample();
    loop {
        std::thread::sleep(std::time::Duration::from_secs(sysmon::INTERVAL));
        s.sample();
        s.save();
    }
}

/// Ask Windows to start the screen saver now. Because Windows starts it (not us), its
/// "On resume, display logon screen" setting applies: any key afterwards leads to the login.
fn start_now() -> Result<(), String> {
    std::thread::sleep(std::time::Duration::from_millis(600)); // let the hotkey be released first
    // SAFETY: plain Win32 call
    unsafe { SendMessageW(GetDesktopWindow(), WM_SYSCOMMAND, SC_SCREENSAVE as usize, 0) };
    Ok(())
}

fn quiet(program: &str, args: &[&str]) -> bool {
    Command::new(program).args(args).creation_flags(CREATE_NO_WINDOW).output().map(|o| o.status.success()).unwrap_or(false)
}

fn shortcut(target: &Path, args: &str, path: &Path, hotkey: &str, description: &str) -> Result<(), String> {
    let ps = "$s=(New-Object -ComObject WScript.Shell).CreateShortcut($env:P);\
              $s.TargetPath=$env:T;$s.Arguments=$env:A;$s.WorkingDirectory=(Split-Path $env:T);\
              if($env:K){$s.Hotkey=$env:K};$s.Description=$env:D;$s.Save()";
    let ok = Command::new("powershell")
        .args(["-NoProfile", "-Command", ps])
        .env("P", path)
        .env("T", target)
        .env("A", args)
        .env("K", hotkey)
        .env("D", description)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if ok { Ok(()) } else { Err(format!("could not create the shortcut {}", path.display())) }
}

fn start_menu() -> PathBuf {
    appdata().join("Microsoft\\Windows\\Start Menu\\Programs")
}

fn start_now_lnk() -> PathBuf {
    start_menu().join("Claude Vivarium - start now.lnk")
}

fn recorder_lnk() -> PathBuf {
    start_menu().join("Startup").join("Claude Vivarium recorder.lnk")
}

const DESKTOP_KEY: &str = "HKCU\\Control Panel\\Desktop";

fn set_screen_saver_params(active: bool, timeout: Option<u32>) {
    // SPI_SETSCREENSAVEACTIVE, SPI_SETSCREENSAVETIMEOUT, SPI_SETSCREENSAVESECURE with
    // SPIF_UPDATEINIFILE | SPIF_SENDCHANGE
    // SAFETY: these actions take no pointer
    unsafe {
        SystemParametersInfoW(0x0011, active as u32, std::ptr::null_mut(), 3);
        if let Some(t) = timeout {
            SystemParametersInfoW(0x000F, t, std::ptr::null_mut(), 3);
            SystemParametersInfoW(0x0077, 1, std::ptr::null_mut(), 3);
        }
    }
}

/// Install for the current user (no admin rights needed).
fn install(cfg: &config::Config) -> Result<(), String> {
    let src = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = install_dir();
    let exe = dir.join(format!("{APP_NAME}.exe"));
    let scr = dir.join(format!("{APP_NAME}.scr"));
    quiet("taskkill", &["/F", "/IM", &format!("{APP_NAME}.scr")]); // stop the old recorder
    std::thread::sleep(std::time::Duration::from_millis(500));
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    if src != exe {
        std::fs::copy(&src, &exe).map_err(|e| format!("{}: {e}", exe.display()))?;
    }
    std::fs::copy(&exe, &scr).map_err(|e| format!("{}: {e}", scr.display()))?;
    let timeout = cfg.idle_seconds.max(60) as u32;
    let scr_s = scr.to_string_lossy().to_string();
    for (name, value) in [
        ("SCRNSAVE.EXE", scr_s.as_str()),
        ("ScreenSaveActive", "1"),
        ("ScreenSaveTimeOut", &timeout.to_string()),
        ("ScreenSaverIsSecure", "1"),
    ] {
        if !quiet("reg", &["add", DESKTOP_KEY, "/v", name, "/t", "REG_SZ", "/d", value, "/f"]) {
            return Err(format!("could not write the registry value {name}"));
        }
    }
    set_screen_saver_params(true, Some(timeout));
    shortcut(&scr, "--record", &recorder_lnk(), "", "Claude Vivarium CPU/memory recorder")?;
    shortcut(&exe, "--now", &start_now_lnk(), "CTRL+ALT+V", "Start the Claude Vivarium screensaver now (locks on resume)")?;
    let _ = Command::new(&scr).arg("--record").creation_flags(DETACHED).spawn();
    say(&format!(
        "Installed to {}\nScreensaver: {}\nStarts after {timeout} s idle, locks on resume.\n\
         Start it right away (instead of Win+L): Ctrl+Alt+V or Start menu 'Claude Vivarium - start now'.\n\
         Settings: {}",
        dir.display(),
        scr.display(),
        config::path().display()
    ));
    Ok(())
}

fn uninstall() -> Result<(), String> {
    quiet("taskkill", &["/F", "/IM", &format!("{APP_NAME}.scr")]);
    let current = Command::new("reg")
        .args(["query", DESKTOP_KEY, "/v", "SCRNSAVE.EXE"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_lowercase())
        .unwrap_or_default();
    if current.contains(&APP_NAME.to_lowercase()) {
        quiet("reg", &["delete", DESKTOP_KEY, "/v", "SCRNSAVE.EXE", "/f"]);
        set_screen_saver_params(false, None);
    }
    for p in [recorder_lnk(), start_now_lnk()] {
        let _ = std::fs::remove_file(p);
    }
    say(&format!("Uninstalled. The files in {} can be deleted.", install_dir().display()));
    Ok(())
}

pub fn extra(cmd: &str, cfg: &config::Config) -> Result<(), String> {
    match cmd {
        "record" => record(),
        "now" => start_now(),
        "install" => install(cfg),
        "uninstall" => uninstall(),
        _ => Err(format!("unknown command '{cmd}'")),
    }
}
