//! Claude Vivarium settings (created with defaults on first run). Same file as the Python version.
//!
//! Linux:   ~/.config/claude-vivarium/config.ini
//! Windows: %APPDATA%\ClaudeVivarium\config.ini
//! macOS:   ~/Library/Application Support/ClaudeVivarium/config.ini
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::sessions::home_dir;

pub const DEFAULT_TEXT: &str = "\
# Claude Vivarium settings. Changes apply the next time the screensaver starts.
[screensaver]
# Seconds without input before the screensaver starts (Linux, macOS - on Windows the
# delay is set in the Windows screen saver settings). On Linux it must be below GNOME's own
# blank delay (Settings > Privacy > Screen Lock > Blank Screen Delay).
idle_seconds = 240

# After this many minutes the screen is locked and the display may blank.
# 0 = keep the screensaver running forever (display stays on).
max_minutes = 0

# What moving the mouse does:
#   hint = show \"press a key or click to unlock\" and keep running
#   wake = lock the screen, like a key press
mouse_move = hint

# true = show only session names and states, no prompt/answer/tool text
hide_text = false

# Themes: aquarium, space, synthwave, pasture, seabed, forest, guppies, terrarium, meerkats, espresso,
#         archipelago, workshop (Christmas: part of \"all\" only from 1 December to 6 January)
#   all             = rotate through every theme
#   one name        = always this theme
#   comma list      = rotate through these, e.g.  themes = pasture, seabed, forest
#   all, -name      = everything except these, e.g.  themes = all, -synthwave, -space
themes = all

# Minutes until the next theme (0 = never switch)
theme_minutes = 10

# Small panel top right: CPU % and memory GB with a graph of the last 30 minutes
system_monitor = true

# Further monitors: theme = another theme with the same creatures (no labels), black = dark
other_monitors = theme

# true = on a busy computer lower the frame rate (25 -> 20 -> 15 fps) so the animation stays even and
# leaves CPU to your work; it goes back up when the computer calms down. false = always full speed.
adaptive_fps = true
";

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub idle_seconds: i64,
    pub max_minutes: f64,
    /// true = moving the mouse wakes (locks); false = it only shows a hint
    pub mouse_wakes: bool,
    pub hide_text: bool,
    pub themes: String,
    pub theme_minutes: f64,
    pub system_monitor: bool,
    /// true = further monitors show another theme; false = black
    pub other_monitors_theme: bool,
    pub adaptive_fps: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            idle_seconds: 240,
            max_minutes: 0.0,
            mouse_wakes: false,
            hide_text: false,
            themes: "all".into(),
            theme_minutes: 10.0,
            system_monitor: true,
            other_monitors_theme: true,
            adaptive_fps: true,
        }
    }
}

pub fn path() -> PathBuf {
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(home_dir).join("ClaudeVivarium").join("config.ini")
    }
    #[cfg(target_os = "macos")]
    {
        home_dir().join("Library/Application Support/ClaudeVivarium/config.ini")
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home_dir().join(".config"))
            .join("claude-vivarium")
            .join("config.ini")
    }
}

/// The keys of the [screensaver] section (INI rules of Python's configparser: `=` or `:`,
/// lines starting with # or ; are comments, keys are case-insensitive).
fn parse_section(text: &str, wanted: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let mut inside = false;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            inside = name.trim() == wanted;
            continue;
        }
        if !inside {
            continue;
        }
        if let Some(i) = line.find(['=', ':']) {
            out.insert(line[..i].trim().to_lowercase(), line[i + 1..].trim().to_string());
        }
    }
    out
}

fn truthy(v: &str) -> bool {
    matches!(v.trim().to_lowercase().as_str(), "1" | "true" | "yes" | "on")
}

pub fn parse(text: &str) -> Config {
    let sec = parse_section(text, "screensaver");
    let mut cfg = Config::default();
    if let Some(v) = sec.get("idle_seconds").and_then(|v| v.parse().ok()) {
        cfg.idle_seconds = v;
    }
    if let Some(v) = sec.get("max_minutes").and_then(|v| v.parse::<f64>().ok()) {
        cfg.max_minutes = v;
    }
    if let Some(v) = sec.get("mouse_move") {
        cfg.mouse_wakes = v.trim().eq_ignore_ascii_case("wake");
    }
    if let Some(v) = sec.get("hide_text") {
        cfg.hide_text = truthy(v);
    }
    if let Some(v) = sec.get("system_monitor") {
        cfg.system_monitor = truthy(v);
    }
    if let Some(v) = sec.get("adaptive_fps") {
        cfg.adaptive_fps = truthy(v);
    }
    if let Some(v) = sec.get("themes") {
        if !v.trim().is_empty() {
            cfg.themes = v.trim().to_string();
        }
    }
    if let Some(v) = sec.get("other_monitors") {
        cfg.other_monitors_theme = !v.trim().eq_ignore_ascii_case("black");
    }
    if let Some(v) = sec.get("theme_minutes").and_then(|v| v.parse::<f64>().ok()) {
        cfg.theme_minutes = v;
    }
    cfg
}

pub fn load() -> Config {
    let p = path();
    if !p.exists() {
        if let Some(dir) = p.parent() {
            let _ = fs::create_dir_all(dir);
        }
        let _ = fs::write(&p, DEFAULT_TEXT);
    }
    parse(&fs::read_to_string(&p).unwrap_or_default())
}

/// Modification time of the settings file (0 if missing), to notice changes.
pub fn mtime() -> f64 {
    fs::metadata(path())
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_values() {
        assert_eq!(parse(DEFAULT_TEXT), Config::default());
        let c = parse("[other]\nthemes = x\n[screensaver]\nIdle_Seconds = 90\nmouse_move = WAKE\nthemes = all, -space\n\
                       other_monitors: black\nhide_text = yes\nmax_minutes = abc\ntheme_minutes = 2.5\n");
        assert_eq!(c.idle_seconds, 90);
        assert!(c.mouse_wakes && c.hide_text && !c.other_monitors_theme);
        assert_eq!(c.themes, "all, -space");
        assert_eq!(c.max_minutes, 0.0);
        assert_eq!(c.theme_minutes, 2.5);
    }
}
