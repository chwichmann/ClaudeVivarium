# Claude Vivarium

A screensaver that shows the live state of your local [Claude Code](https://claude.com/claude-code) sessions -
as creatures in a vivarium. Runs on **Ubuntu/GNOME (Wayland)** and **Windows**.

Every session is one creature. The state colour is the same in every theme:

| State | Colour | Behaviour |
|---|---|---|
| working | teal | moves briskly; every tool call floats up with its name |
| idle | blue | slow, grazes / rests |
| needs you (waiting for permission/input) | amber | comes to the front, pulsing rings, banner at the top |
| blocked | red | flickers |

Six themes rotate in random order (default every 10 minutes, fade through black):

| Theme | Sessions are | Tool calls |
|---|---|---|
| aquarium | glowing jellyfish in the deep sea | rising bubbles |
| space | planets orbiting a Claude-orange star | meteors |
| synthwave | neon wireframe cubes over an 80s grid | glitchy neon text |
| pasture | sheep - one session one sheep, many a flock | the sheep hops |
| seabed | crabs scuttling sideways | claw snap + bubble |
| forest | deer with a rim light in a moonlit clearing | fireflies |

Also on screen: a status strip with all sessions, time in state and a clock; a small panel with CPU %
and memory GB and a 30-minute graph. With no session running, a resting creature appears (a sleeping
sheep, a crab buried in the sand, a deer lying down, ...). Everything drifts slowly (no burn-in).

## How it works

Claude Code writes `~/.claude/sessions/<pid>.json` (name, folder, status) for every running session and
a transcript `~/.claude/projects/*/<sessionId>.jsonl`. Claude Vivarium only **reads** these files.

```
sessions.py        reads session state + recent tool calls        (all platforms)
sysmon.py          CPU / memory history                            (all platforms)
config.py          settings file                                   (all platforms)
scene.py           shared drawing: labels, banner, strip, panel    (all platforms, Cairo)
theme_*.py         one file per theme                              (all platforms)
themes.py          theme registry + random rotation                (all platforms)
vivarium_gtk.py    Linux: full-screen GTK4 window, locks on wake
vivariumd.py       Linux: idle watcher (systemd user service) + CPU/memory recorder
gnome.py           Linux: GNOME D-Bus helpers (lock, idle, inhibit)
vivarium_win.py    Windows: the .scr screensaver (/s /p /c), recorder, installer
render_preview.py  render themes to PNG without a display (testing)
```

### Linux (Ubuntu, GNOME on Wayland)

GNOME has no screensaver plug-in API and does not allow app windows on the lock screen, so:

- `vivariumd.py` asks Mutter to signal after `idle_seconds` and starts `vivarium_gtk.py` (before GNOME
  blanks). It also records CPU/memory every 5 s so the graph has history right away.
- `vivarium_gtk.py` inhibits GNOME's idle blanking while visible. On a key, click, focus loss or a kill
  signal it **locks the screen first**, waits until GNOME confirms, then closes. If it never gets focus,
  it locks immediately. If it ever exits without the screen being locked, the daemon locks (fail closed).
- After a wake you are at the normal lock screen; the screensaver returns after unlocking + idle time.

### Windows

A real Windows screensaver (`ClaudeVivarium.scr`, built with PyInstaller). Windows starts it on idle and,
with "On resume, display logon screen", locks on wake itself. Main monitor: everything; other monitors:
another theme as decoration (rendered in a second thread). `--record` runs at logon and keeps the
CPU/memory history.

## Settings

| Platform | File |
|---|---|
| Linux | `~/.config/claude-vivarium/config.ini` |
| Windows | `%APPDATA%\ClaudeVivarium\config.ini` (the "Settings..." button in the Windows dialog opens it) |

```ini
idle_seconds = 240     # Linux: start after this much idle time (below GNOME's blank delay);
                       # Windows: used as the screen saver timeout when installing
max_minutes = 0        # 0 = run forever (display stays on); otherwise end after N minutes
mouse_move = hint      # hint = only show "press a key or click to unlock"; wake = end
hide_text = false      # true = only names and states, no prompt/answer/tool text
themes = all           # all | one theme | comma list, e.g. pasture, seabed, forest
theme_minutes = 10     # minutes per theme, 0 = never switch
system_monitor = true  # CPU/memory panel top right
other_monitors = theme # theme | black  (Windows, further monitors)
```

## Install

### Ubuntu (from a Windows or Linux machine with SSH access)

```bash
./deploy.sh laptop1 laptop2 laptop3   # code + config.ini, install, enable, restart
./deploy-config.sh laptop3            # only push config.ini
```

Or on the machine itself: `./install.sh && systemctl --user enable --now claude-vivarium.service`.
Needs `python3-gi`, `gir1.2-gtk-4.0`, `python3-gi-cairo` (the installer adds the last one).

```bash
claude-vivarium --windowed --demo                 # preview; N / Right / Space = next theme
journalctl --user -u claude-vivarium -f           # log
systemctl --user disable --now claude-vivarium    # turn off
```

### Windows (no admin rights needed)

```powershell
.\build-windows.ps1 -Install     # build with PyInstaller and install for the current user
```

This copies the program to `%LOCALAPPDATA%\Programs\ClaudeVivarium`, sets it as your screen saver
(timeout = `idle_seconds`, "display logon screen" on), and adds a Startup shortcut for the recorder.
Undo with `ClaudeVivarium.exe --uninstall`. Log: `%LOCALAPPDATA%\ClaudeVivarium\vivarium.log`.

```powershell
.venv\Scripts\python vivarium_win.py --windowed --demo    # preview from source
```

## Testing without watching

```bash
python3 render_preview.py --demo --sysmon --themes all --out /tmp/preview   # PNGs + frame times
VIVARIUM_TEST="8;C:\temp\shots" ClaudeVivarium.exe /s                       # Windows: save each monitor after 8 s
```

## License

MIT
