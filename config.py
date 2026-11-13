"""Claude Vivarium settings (created with defaults on first run).

Linux:   ~/.config/claude-vivarium/config.ini
Windows: %APPDATA%\\ClaudeVivarium\\config.ini
macOS:   ~/Library/Application Support/ClaudeVivarium/config.ini
"""
import configparser
import os
import sys

if sys.platform == "win32":
    PATH = os.path.join(os.environ.get("APPDATA") or os.path.expanduser("~"), "ClaudeVivarium", "config.ini")
elif sys.platform == "darwin":
    PATH = os.path.expanduser("~/Library/Application Support/ClaudeVivarium/config.ini")
else:
    PATH = os.path.join(os.environ.get("XDG_CONFIG_HOME", os.path.expanduser("~/.config")),
                        "claude-vivarium", "config.ini")

DEFAULT_TEXT = """\
# Claude Vivarium settings. Changes apply the next time the screensaver starts.
[screensaver]
# Seconds without input before the screensaver starts (Linux only - on Windows the
# delay is set in the Windows screen saver settings). Must be below GNOME's own
# blank delay (Settings > Privacy > Screen Lock > Blank Screen Delay).
idle_seconds = 240

# After this many minutes the screen is locked and GNOME may blank the display.
# 0 = keep the screensaver running forever (display stays on).
max_minutes = 0

# What moving the mouse does:
#   hint = show "press a key or click to unlock" and keep running
#   wake = lock the screen, like a key press
mouse_move = hint

# true = show only session names and states, no prompt/answer/tool text
hide_text = false

# Themes: aquarium, space, synthwave, pasture, seabed, forest
#   all             = rotate through every theme
#   one name        = always this theme
#   comma list      = rotate through these, e.g.  themes = pasture, seabed, forest
themes = all

# Minutes until the next theme (0 = never switch)
theme_minutes = 10

# Small panel top right: CPU % and memory GB with a graph of the last 30 minutes
system_monitor = true

# Further monitors: theme = another theme with the same creatures (no labels), black = dark
other_monitors = theme
"""

DEFAULTS = {"idle_seconds": 240, "max_minutes": 0.0, "mouse_move": "hint", "hide_text": False,
            "themes": "all", "theme_minutes": 10.0, "system_monitor": True, "other_monitors": "theme"}


def load():
    if not os.path.exists(PATH):
        try:
            os.makedirs(os.path.dirname(PATH), exist_ok=True)
            with open(PATH, "w", encoding="utf-8") as f:
                f.write(DEFAULT_TEXT)
        except OSError:
            pass
    cp = configparser.ConfigParser()
    try:
        cp.read(PATH, encoding="utf-8")
    except configparser.Error:
        pass
    sec = cp["screensaver"] if cp.has_section("screensaver") else {}
    cfg = dict(DEFAULTS)
    try:
        cfg["idle_seconds"] = int(sec.get("idle_seconds", cfg["idle_seconds"]))
    except ValueError:
        pass
    try:
        cfg["max_minutes"] = float(sec.get("max_minutes", cfg["max_minutes"]))
    except ValueError:
        pass
    mm = str(sec.get("mouse_move", cfg["mouse_move"])).strip().lower()
    cfg["mouse_move"] = mm if mm in ("hint", "wake") else "hint"
    cfg["hide_text"] = str(sec.get("hide_text", "false")).strip().lower() in ("1", "true", "yes", "on")
    cfg["system_monitor"] = str(sec.get("system_monitor", "true")).strip().lower() in ("1", "true", "yes", "on")
    cfg["themes"] = str(sec.get("themes", cfg["themes"])).strip() or "all"
    om = str(sec.get("other_monitors", "theme")).strip().lower()
    cfg["other_monitors"] = om if om in ("theme", "black") else "theme"
    try:
        cfg["theme_minutes"] = float(sec.get("theme_minutes", cfg["theme_minutes"]))
    except ValueError:
        pass
    return cfg


def mtime():
    try:
        return os.path.getmtime(PATH)
    except OSError:
        return 0
