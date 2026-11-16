//! The settings dialog behind the "Settings..." button of the Windows screen saver dialog (`/c`).
//!
//! A plain Win32 window with standard controls (no resource file: the controls get the current Windows
//! look through an activation context for Common Controls 6). It shows every setting of config.ini
//! except `idle_seconds` (on Windows the wait time is part of the Windows dialog) and writes only the keys
//! it shows, so comments and anything else in the file stay as they are.
use std::cell::RefCell;
use std::ffi::c_void;

use vivarium_core::config::{self, Monitors, Season};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    CreateFontIndirectW, DeleteObject, GetMonitorInfoW, MonitorFromWindow, COLOR_BTNFACE, HBRUSH, HFONT, MONITORINFO,
    MONITOR_DEFAULTTOPRIMARY,
};
use windows_sys::Win32::System::ApplicationInstallationAndServicing::{ActivateActCtx, CreateActCtxW, ACTCTXW};
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, LoadLibraryW};
use windows_sys::Win32::UI::HiDpi::{
    AdjustWindowRectExForDpi, GetDpiForWindow, SetThreadDpiAwarenessContext, SystemParametersInfoForDpi,
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, SetFocus};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use super::windows::wide;

const ID_OK: i32 = 1; // IDOK: also Enter
const ID_CANCEL: i32 = 2; // IDCANCEL: also Esc
const ID_ALL: i32 = 100;
const ID_NONE: i32 = 101;
const ID_OPEN_FILE: i32 = 102;
const ID_MONITORS: i32 = 103;
const ID_THEME0: i32 = 200;

const CBN_SELCHANGE: u32 = 1;

#[derive(Clone)]
struct Ui {
    themes: Vec<(&'static str, HWND)>,
    seasonal: Vec<(&'static str, HWND)>,
    theme_minutes: HWND,
    max_minutes: HWND,
    monitors: HWND,
    other_monitors: HWND,
    mouse: HWND,
    hide_text: HWND,
    system_monitor: HWND,
    adaptive_fps: HWND,
    /// the themes value as it was - kept when the theme choice did not change
    themes_before: String,
    owner: HWND,
}

thread_local! {
    static UI: RefCell<Option<Ui>> = const { RefCell::new(None) };
}

fn ui() -> Option<Ui> {
    UI.with(|u| u.borrow().clone())
}

fn title_case(name: &str) -> String {
    let mut c = name.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

fn number(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e9 { format!("{}", v as i64) } else { format!("{v}") }
}

fn send(h: HWND, msg: u32, w: usize, l: isize) -> isize {
    // SAFETY: plain message to a window of this thread
    unsafe { SendMessageW(h, msg, w, l) }
}

fn checked(h: HWND) -> bool {
    send(h, BM_GETCHECK, 0, 0) == 1
}

fn set_checked(h: HWND, on: bool) {
    send(h, BM_SETCHECK, on as usize, 0);
}

fn selection(h: HWND) -> usize {
    send(h, CB_GETCURSEL, 0, 0).max(0) as usize
}

fn text(h: HWND) -> String {
    let mut buf = [0u16; 128];
    // SAFETY: the buffer is large enough for the given length
    let n = unsafe { GetWindowTextW(h, buf.as_mut_ptr(), buf.len() as i32) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

fn message(owner: HWND, text: &str, icon: MESSAGEBOX_STYLE) {
    let (t, c) = (wide(text), wide("Claude Vivarium"));
    // SAFETY: null-terminated strings that live during the call
    unsafe { MessageBoxW(owner, t.as_ptr(), c.as_ptr(), MB_OK | icon) };
}

/// Common Controls 6 for the controls created after this (otherwise they look like Windows 95). Uses a
/// manifest file because the program has no resources to carry one.
fn modern_controls() {
    let manifest = std::env::temp_dir().join("claude-vivarium-controls.manifest");
    let xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency><dependentAssembly>
    <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0"
                      processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/>
  </dependentAssembly></dependency>
</assembly>
"#;
    if std::fs::write(&manifest, xml).is_err() {
        return;
    }
    let path = super::windows::wide_path(&manifest);
    // SAFETY: zeroed ACTCTXW is valid; the path lives during the call; the context stays active for
    // the rest of this short-lived process
    unsafe {
        let mut ctx: ACTCTXW = std::mem::zeroed();
        ctx.cbSize = std::mem::size_of::<ACTCTXW>() as u32;
        ctx.lpSource = path.as_ptr();
        let h = CreateActCtxW(&ctx);
        if h as isize != -1 {
            let mut cookie = 0usize;
            ActivateActCtx(h, &mut cookie);
            let lib = wide("comctl32.dll");
            LoadLibraryW(lib.as_ptr());
        }
    }
}

struct Builder {
    parent: HWND,
    dpi: i32,
    font: HFONT,
}

impl Builder {
    fn s(&self, v: i32) -> i32 {
        v * self.dpi / 96
    }

    /// A child control at a position in 96-dpi units.
    fn add(&self, class: &str, label: &str, style: u32, id: i32, (x, y, w, h): (i32, i32, i32, i32)) -> HWND {
        let (c, t) = (wide(class), wide(label));
        // SAFETY: valid parent window and null-terminated strings
        unsafe {
            let ex = if class == "EDIT" { WS_EX_CLIENTEDGE } else { 0 };
            let hwnd = CreateWindowExW(
                ex,
                c.as_ptr(),
                t.as_ptr(),
                WS_CHILD | WS_VISIBLE | style,
                self.s(x),
                self.s(y),
                self.s(w),
                self.s(h),
                self.parent,
                id as isize as *mut c_void,
                GetModuleHandleW(std::ptr::null()),
                std::ptr::null(),
            );
            SendMessageW(hwnd, WM_SETFONT, self.font as usize, 1);
            hwnd
        }
    }

    fn label(&self, text: &str, x: i32, y: i32, w: i32) -> HWND {
        self.add("STATIC", text, 0, -1, (x, y + 4, w, 20))
    }

    fn check(&self, text: &str, id: i32, on: bool, pos: (i32, i32, i32)) -> HWND {
        let h = self.add("BUTTON", text, BS_AUTOCHECKBOX as u32 | WS_TABSTOP, id, (pos.0, pos.1, pos.2, 22));
        set_checked(h, on);
        h
    }

    fn combo(&self, items: &[&str], selected: usize, id: i32, x: i32, y: i32, w: i32) -> HWND {
        let h = self.add("COMBOBOX", "", CBS_DROPDOWNLIST as u32 | WS_TABSTOP | WS_VSCROLL, id, (x, y, w, 200));
        for item in items {
            let t = wide(item);
            send(h, CB_ADDSTRING, 0, t.as_ptr() as isize);
        }
        send(h, CB_SETCURSEL, selected, 0);
        h
    }

    fn edit(&self, value: &str, x: i32, y: i32, w: i32) -> HWND {
        self.add("EDIT", value, ES_AUTOHSCROLL as u32 | WS_TABSTOP, -1, (x, y, w, 24))
    }

    fn button(&self, text: &str, id: i32, default: bool, (x, y, w): (i32, i32, i32)) -> HWND {
        let style = if default { BS_DEFPUSHBUTTON } else { BS_PUSHBUTTON } as u32;
        self.add("BUTTON", text, style | WS_TABSTOP, id, (x, y, w, 28))
    }
}

const WIDTH: i32 = 560;
const HEIGHT: i32 = 616;

fn build(hwnd: HWND, cfg: &config::Config, themes_before: String, owner: HWND, font: HFONT, dpi: i32) -> Ui {
    let b = Builder { parent: hwnd, dpi, font };
    b.label("Themes - the screensaver shows them in random order:", 20, 10, 500);
    let (regular, seasonal) = config::theme_choice(&cfg.themes, vivarium_themes::NAMES);
    let mut themes = Vec::new();
    for (i, (name, on)) in regular.iter().enumerate() {
        let (col, row) = (i as i32 % 4, i as i32 / 4);
        let h = b.check(&title_case(name), ID_THEME0 + i as i32, *on, (24 + col * 130, 38 + row * 26, 124));
        themes.push((*name, h));
    }
    let rows = (regular.len() as i32 + 3) / 4;
    let mut y = 38 + rows * 26 + 4;
    b.button("All", ID_ALL, false, (24, y, 80));
    b.button("None", ID_NONE, false, (112, y, 80));
    y += 44;
    let mut season_boxes = Vec::new();
    for (name, mode) in &seasonal {
        let (label, when) = match *name {
            "halloween" => ("Halloween:", "Only from 1 October to 2 November"),
            "workshop" => ("Christmas workshop:", "Only from 1 December to 6 January"),
            other => (other, "Only in its season"),
        };
        b.label(label, 20, y, 200);
        let sel = match mode {
            Season::InSeason => 0,
            Season::Always => 1,
            Season::Off => 2,
        };
        season_boxes.push((*name, b.combo(&[when, "All year", "Never"], sel, -1, 230, y, 300)));
        y += 32;
    }
    y += 8;
    b.label("Minutes per theme:", 20, y, 200);
    let theme_minutes = b.edit(&number(cfg.theme_minutes), 230, y, 60);
    b.label("0 = never change the theme", 302, y, 240);
    y += 32;
    b.label("End after (minutes):", 20, y, 200);
    let max_minutes = b.edit(&number(cfg.max_minutes), 230, y, 60);
    b.label("0 = run until a key is pressed", 302, y, 240);
    y += 40;
    b.label("Monitors:", 20, y, 200);
    let mon = match cfg.monitors {
        Monitors::All => 0,
        Monitors::Primary => 1,
        Monitors::Secondary => 2,
    };
    let monitors = b.combo(
        &["All monitors", "Only the primary monitor (others black)", "Only the secondary monitor (others black)"],
        mon,
        ID_MONITORS,
        230,
        y,
        300,
    );
    y += 32;
    b.label("Further monitors:", 20, y, 200);
    let other_monitors = b.combo(
        &["Another theme with the same sessions", "Black"],
        if cfg.other_monitors_theme { 0 } else { 1 },
        -1,
        230,
        y,
        300,
    );
    y += 32;
    b.label("Moving the mouse:", 20, y, 200);
    let mouse = b.combo(
        &["Shows a hint, keeps running", "Ends the screensaver"],
        if cfg.mouse_wakes { 1 } else { 0 },
        -1,
        230,
        y,
        300,
    );
    y += 40;
    let hide_text = b.check("Hide prompts, answers and tool names (only names and states)", -1, cfg.hide_text, (20, y, 520));
    y += 26;
    let system_monitor = b.check("CPU / memory panel (top right)", -1, cfg.system_monitor, (20, y, 520));
    y += 26;
    let adaptive_fps =
        b.check("Lower the frame rate while the computer is busy", -1, cfg.adaptive_fps, (20, y, 520));
    y += 36;
    b.add(
        "STATIC",
        "The wait time and \"On resume, display logon screen\" are set in the Windows screen saver settings. \
         Changes apply the next time the screensaver starts.",
        0,
        -1,
        (20, y, 520, 40),
    );
    let y = HEIGHT - 44;
    b.button("Open config.ini", ID_OPEN_FILE, false, (20, y, 130));
    b.button("OK", ID_OK, true, (WIDTH - 210, y, 90));
    b.button("Cancel", ID_CANCEL, false, (WIDTH - 110, y, 90));
    // SAFETY: valid window
    unsafe { EnableWindow(other_monitors, (mon == 0) as i32) };
    Ui {
        themes,
        seasonal: season_boxes,
        theme_minutes,
        max_minutes,
        monitors,
        other_monitors,
        mouse,
        hide_text,
        system_monitor,
        adaptive_fps,
        themes_before,
        owner,
    }
}

/// Read the controls and write config.ini. Err = a message for the user (and the control to fix).
fn save(ui: &Ui) -> Result<(), (String, HWND)> {
    let minutes = |h: HWND, what: &str| -> Result<f64, (String, HWND)> {
        let t = text(h).trim().replace(',', ".");
        match t.parse::<f64>() {
            Ok(v) if v >= 0.0 && v.is_finite() => Ok(v),
            _ => Err((format!("{what}: please enter a number of minutes (0 or more), not \"{t}\"."), h)),
        }
    };
    let theme_minutes = minutes(ui.theme_minutes, "Minutes per theme")?;
    let max_minutes = minutes(ui.max_minutes, "End after")?;
    let regular: Vec<(&str, bool)> = ui.themes.iter().map(|(n, h)| (*n, checked(*h))).collect();
    let seasonal: Vec<(&str, Season)> = ui
        .seasonal
        .iter()
        .map(|(n, h)| {
            let mode = match selection(*h) {
                1 => Season::Always,
                2 => Season::Off,
                _ => Season::InSeason,
            };
            (*n, mode)
        })
        .collect();
    if !regular.iter().any(|(_, on)| *on) && !seasonal.iter().any(|(_, m)| *m == Season::Always) {
        let first = ui.themes.first().map(|t| t.1).unwrap_or(std::ptr::null_mut());
        return Err(("Please choose at least one theme.".into(), first));
    }
    let before = config::theme_choice(&ui.themes_before, vivarium_themes::NAMES);
    let unchanged = before.0 == regular && before.1 == seasonal;
    let themes = if unchanged { ui.themes_before.clone() } else { config::themes_value(&regular, &seasonal) };
    let monitors = match selection(ui.monitors) {
        1 => Monitors::Primary,
        2 => Monitors::Secondary,
        _ => Monitors::All,
    };
    let yes_no = |h: HWND| if checked(h) { "true" } else { "false" }.to_string();
    let values = [
        ("themes", themes),
        ("theme_minutes", number(theme_minutes)),
        ("max_minutes", number(max_minutes)),
        ("monitors", monitors.as_str().to_string()),
        ("other_monitors", if selection(ui.other_monitors) == 1 { "black" } else { "theme" }.to_string()),
        ("mouse_move", if selection(ui.mouse) == 1 { "wake" } else { "hint" }.to_string()),
        ("hide_text", yes_no(ui.hide_text)),
        ("system_monitor", yes_no(ui.system_monitor)),
        ("adaptive_fps", yes_no(ui.adaptive_fps)),
    ];
    config::save(&values).map_err(|e| (format!("Could not save {}: {e}", config::path().display()), std::ptr::null_mut()))
}

fn close(hwnd: HWND) {
    // SAFETY: windows of this thread; the owner is enabled again before this window goes, so Windows
    // activates the owner instead of some other program
    unsafe {
        if let Some(ui) = ui() {
            if !ui.owner.is_null() {
                EnableWindow(ui.owner, 1);
            }
        }
        DestroyWindow(hwnd);
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_COMMAND => {
            let id = (wparam & 0xFFFF) as i32;
            let code = ((wparam >> 16) & 0xFFFF) as u32;
            let Some(ui) = ui() else { return 0 };
            match id {
                ID_OK => match save(&ui) {
                    Ok(()) => close(hwnd),
                    Err((text, field)) => {
                        message(hwnd, &text, MB_ICONWARNING);
                        if !field.is_null() {
                            SetFocus(field);
                        }
                    }
                },
                ID_CANCEL => close(hwnd),
                ID_ALL | ID_NONE => ui.themes.iter().for_each(|(_, h)| set_checked(*h, id == ID_ALL)),
                ID_OPEN_FILE => {
                    // the file is edited by hand now: close without saving so nothing overwrites it
                    let _ = super::windows::open_config_file();
                    close(hwnd);
                }
                ID_MONITORS if code == CBN_SELCHANGE => {
                    EnableWindow(ui.other_monitors, (selection(ui.monitors) == 0) as i32);
                }
                _ => {}
            }
            0
        }
        WM_CLOSE => {
            close(hwnd);
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Show the dialog and wait until it is closed. `owner`: the Windows screen saver dialog (from /c:<hwnd>).
pub fn run(owner: Option<isize>) -> Result<(), String> {
    let cfg = config::load();
    let themes_before = cfg.themes.clone();
    let owner: HWND = owner.map(|h| h as *mut c_void).unwrap_or(std::ptr::null_mut());
    modern_controls();
    // SAFETY: Win32 calls with valid arguments; all windows belong to this thread
    unsafe {
        SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let instance = GetModuleHandleW(std::ptr::null());
        let class = wide("ClaudeVivariumSettings");
        let mut wc: WNDCLASSEXW = std::mem::zeroed();
        wc.cbSize = std::mem::size_of::<WNDCLASSEXW>() as u32;
        wc.lpfnWndProc = Some(wndproc);
        wc.hInstance = instance;
        wc.hCursor = LoadCursorW(std::ptr::null_mut(), IDC_ARROW);
        wc.hbrBackground = (COLOR_BTNFACE + 1) as isize as HBRUSH;
        wc.lpszClassName = class.as_ptr();
        RegisterClassExW(&wc);
        let title = wide("Claude Vivarium - Settings");
        let style = WS_CAPTION | WS_SYSMENU;
        let hwnd = CreateWindowExW(
            WS_EX_DLGMODALFRAME | WS_EX_CONTROLPARENT,
            class.as_ptr(),
            title.as_ptr(),
            style,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            100,
            100,
            owner,
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        );
        if hwnd.is_null() {
            return Err("could not create the settings window".into());
        }
        // size and place it for the DPI of the monitor of the owner (or the primary monitor)
        let anchor = if owner.is_null() { hwnd } else { owner };
        let mon = MonitorFromWindow(anchor, MONITOR_DEFAULTTOPRIMARY);
        let mut mi: MONITORINFO = std::mem::zeroed();
        mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        GetMonitorInfoW(mon, &mut mi);
        let work = mi.rcWork;
        SetWindowPos(hwnd, std::ptr::null_mut(), work.left, work.top, 100, 100, SWP_NOZORDER | SWP_NOACTIVATE);
        let dpi = GetDpiForWindow(hwnd).max(96) as i32;
        let mut r = RECT { left: 0, top: 0, right: WIDTH * dpi / 96, bottom: HEIGHT * dpi / 96 };
        AdjustWindowRectExForDpi(&mut r, style, 0, WS_EX_DLGMODALFRAME | WS_EX_CONTROLPARENT, dpi as u32);
        let (w, h) = (r.right - r.left, r.bottom - r.top);
        let (x, y) = (work.left + (work.right - work.left - w) / 2, work.top + (work.bottom - work.top - h) / 2);
        SetWindowPos(hwnd, std::ptr::null_mut(), x, y, w, h, SWP_NOZORDER | SWP_NOACTIVATE);

        let mut ncm: NONCLIENTMETRICSW = std::mem::zeroed();
        ncm.cbSize = std::mem::size_of::<NONCLIENTMETRICSW>() as u32;
        SystemParametersInfoForDpi(SPI_GETNONCLIENTMETRICS, ncm.cbSize, &mut ncm as *mut _ as *mut c_void, 0, dpi as u32);
        let font = CreateFontIndirectW(&ncm.lfMessageFont);

        let ui = build(hwnd, &cfg, themes_before, owner, font, dpi);
        UI.with(|u| *u.borrow_mut() = Some(ui));
        if !owner.is_null() {
            EnableWindow(owner, 0);
        }
        ShowWindow(hwnd, SW_SHOW);
        SetForegroundWindow(hwnd);

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            if IsDialogMessageW(hwnd, &msg) == 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        UI.with(|u| *u.borrow_mut() = None);
        DeleteObject(font as _);
    }
    Ok(())
}
