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
#         archipelago, gorillas, penguins, capricorns, dolphins, cockroaches,
#         halloween (part of \"all\" only from 1 October to 2 November),
#         workshop (Christmas: part of \"all\" only from 1 December to 6 January)
#   all             = rotate through every theme
#   one name        = always this theme
#   comma list      = rotate through these, e.g.  themes = pasture, seabed, forest
#   all, -name      = everything except these, e.g.  themes = all, -synthwave, -space
themes = all

# Minutes until the next theme (0 = never switch)
theme_minutes = 10

# Small panel top right: CPU % and memory GB with a graph of the last 30 minutes
system_monitor = true

# Which monitors show the vivarium:
#   all       = every monitor (see other_monitors for the further ones)
#   primary   = only the primary monitor, the others stay black
#   secondary = only the first monitor that is not the primary one, the primary stays black
# With a single monitor that one is always used.
monitors = all

# Further monitors: theme = another theme with the same creatures (no labels), black = dark
other_monitors = theme

# true = on a busy computer lower the frame rate (25 -> 20 -> 15 fps) so the animation stays even and
# leaves CPU to your work; it goes back up when the computer calms down. false = always full speed.
adaptive_fps = true
";

/// Which monitors show the vivarium (the others stay black).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Monitors {
    All,
    Primary,
    Secondary,
}

impl Monitors {
    pub fn as_str(self) -> &'static str {
        match self {
            Monitors::All => "all",
            Monitors::Primary => "primary",
            Monitors::Secondary => "secondary",
        }
    }
}

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
    pub monitors: Monitors,
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
            monitors: Monitors::All,
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
    if let Some(v) = sec.get("monitors") {
        cfg.monitors = match v.trim().to_lowercase().as_str() {
            "primary" => Monitors::Primary,
            "secondary" => Monitors::Secondary,
            _ => Monitors::All,
        };
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

/// Set some keys of the [screensaver] section and keep everything else - comments, other keys, other
/// sections, the line endings - exactly as it is. A key that is not there yet is added at the end of
/// the section (the section is created if missing).
pub fn set_values(text: &str, values: &[(&str, String)]) -> String {
    let nl = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let mut done = vec![false; values.len()];
    let (mut inside, mut section_end) = (false, None::<usize>);
    for (i, line) in lines.iter_mut().enumerate() {
        let t = line.trim();
        if let Some(name) = t.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            if inside && section_end.is_none() {
                section_end = Some(i);
            }
            inside = name.trim() == "screensaver";
            continue;
        }
        if !inside || t.is_empty() || t.starts_with('#') || t.starts_with(';') {
            continue;
        }
        let Some(eq) = t.find(['=', ':']) else { continue };
        let key = t[..eq].trim().to_lowercase();
        for (k, (want, value)) in values.iter().enumerate() {
            if key == want.to_lowercase() && !done[k] {
                *line = format!("{want} = {value}");
                done[k] = true;
            }
        }
    }
    let missing: Vec<String> =
        values.iter().zip(&done).filter(|(_, d)| !**d).map(|((k, v), _)| format!("{k} = {v}")).collect();
    if !missing.is_empty() {
        let has_section = lines.iter().any(|l| l.trim() == "[screensaver]");
        if !has_section {
            if lines.last().is_some_and(|l| !l.trim().is_empty()) {
                lines.push(String::new());
            }
            lines.push("[screensaver]".into());
            lines.extend(missing);
        } else {
            let at = section_end.unwrap_or(lines.len());
            // insert before the blank lines that separate the section from the next one
            let mut at = at;
            while at > 0 && lines[at - 1].trim().is_empty() && section_end.is_some() {
                at -= 1;
            }
            for (n, l) in missing.into_iter().enumerate() {
                lines.insert(at + n, l);
            }
        }
    }
    let mut out = lines.join(nl);
    if text.ends_with('\n') || text.is_empty() {
        out.push_str(nl);
    }
    out
}

/// Write some settings into the settings file (see `set_values`).
pub fn save(values: &[(&str, String)]) -> std::io::Result<()> {
    let p = path();
    if let Some(dir) = p.parent() {
        fs::create_dir_all(dir)?;
    }
    let text = fs::read_to_string(&p).unwrap_or_else(|_| DEFAULT_TEXT.to_string());
    fs::write(&p, set_values(&text, values))
}

/// The seasonal themes: with "all" they only take part in their season.
pub const SEASONAL: &[&str] = &["halloween", "workshop"];

/// How a seasonal theme is chosen in the settings dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Season {
    Off,
    InSeason,
    Always,
}

/// The regular themes (on/off) and the seasonal ones (off / in season / always).
pub type ThemeChoice = (Vec<(&'static str, bool)>, Vec<(&'static str, Season)>);

/// The `themes` setting as the settings dialog shows it: the regular themes on or off, the seasonal
/// ones off / in their season / always.
pub fn theme_choice(value: &str, all: &[&'static str]) -> ThemeChoice {
    let included = crate::show::parse_list(value, all);
    let named: Vec<String> = value.replace(';', ",").split(',').map(|w| w.trim().to_lowercase()).collect();
    let regular = all.iter().filter(|n| !SEASONAL.contains(n)).map(|n| (*n, included.contains(n))).collect();
    let seasonal = all
        .iter()
        .filter(|n| SEASONAL.contains(n))
        .map(|n| {
            let mode = if !included.contains(n) {
                Season::Off
            } else if named.iter().any(|w| w == n) {
                Season::Always
            } else {
                Season::InSeason
            };
            (*n, mode)
        })
        .collect();
    (regular, seasonal)
}

/// The `themes` setting for a choice from the settings dialog. Always written as "all" plus
/// exceptions, because only that form lets a seasonal theme take part in its season only.
pub fn themes_value(regular: &[(&str, bool)], seasonal: &[(&str, Season)]) -> String {
    let mut words = vec!["all".to_string()];
    words.extend(regular.iter().filter(|(_, on)| !on).map(|(n, _)| format!("-{n}")));
    for (n, mode) in seasonal {
        match mode {
            Season::Off => words.push(format!("-{n}")),
            Season::Always => words.push(n.to_string()),
            Season::InSeason => {}
        }
    }
    words.join(", ")
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
        assert_eq!(c.monitors, Monitors::All);
        assert_eq!(parse("[screensaver]\nmonitors = Secondary\n").monitors, Monitors::Secondary);
        assert_eq!(parse("[screensaver]\nmonitors = primary\n").monitors, Monitors::Primary);
    }

    #[test]
    fn set_values_keeps_comments_and_adds_missing_keys() {
        let text = "# my comment\n[screensaver]\n# about idle\nidle_seconds = 240\nThemes: all\n\n[other]\nthemes = x\n";
        let out = set_values(text, &[("themes", "all, -space".into()), ("monitors", "primary".into())]);
        assert_eq!(
            out,
            "# my comment\n[screensaver]\n# about idle\nidle_seconds = 240\nthemes = all, -space\nmonitors = primary\n\n[other]\nthemes = x\n"
        );
        assert_eq!(parse(&out).monitors, Monitors::Primary);
        // Windows line endings stay, a missing section is created
        let out = set_values("a = 1\r\n", &[("hide_text", "true".into())]);
        assert_eq!(out, "a = 1\r\n\r\n[screensaver]\r\nhide_text = true\r\n");
        // the default file: every value round-trips, comments stay
        let out = set_values(DEFAULT_TEXT, &[("theme_minutes", "5".into())]);
        assert_eq!(parse(&out).theme_minutes, 5.0);
        assert_eq!(out.lines().count(), DEFAULT_TEXT.lines().count());
    }

    #[test]
    fn theme_choice_round_trip() {
        const ALL: &[&str] = &["aquarium", "space", "halloween", "workshop"];
        let (reg, sea) = theme_choice("all", ALL);
        assert_eq!(reg, vec![("aquarium", true), ("space", true)]);
        assert_eq!(sea, vec![("halloween", Season::InSeason), ("workshop", Season::InSeason)]);
        assert_eq!(themes_value(&reg, &sea), "all");
        let (reg, sea) = theme_choice("all, -space, workshop, -halloween", ALL);
        assert_eq!(reg, vec![("aquarium", true), ("space", false)]);
        assert_eq!(sea, vec![("halloween", Season::Off), ("workshop", Season::Always)]);
        assert_eq!(themes_value(&reg, &sea), "all, -space, -halloween, workshop");
        // a plain list leaves out what it does not name
        let (reg, sea) = theme_choice("aquarium", ALL);
        assert_eq!(reg, vec![("aquarium", true), ("space", false)]);
        assert_eq!(sea, vec![("halloween", Season::Off), ("workshop", Season::Off)]);
    }
}
