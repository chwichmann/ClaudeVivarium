# Claude Vivarium

A screensaver that shows the live state of your local [Claude Code](https://claude.com/claude-code)
sessions - as creatures in a vivarium. Written in Rust: every theme is drawn on the graphics card
(Metal on macOS, Vulkan or Direct3D 12 on Windows, Vulkan on Linux), and the whole program is one file
per platform with nothing else to install.

Every session is one creature. The state colour is the same in every theme:

| State | Colour | Behaviour |
|---|---|---|
| working | teal | moves briskly; every tool call shows up with its name |
| idle | blue | slow, grazes / rests |
| needs you (waiting for permission/input) | amber | comes to the front, pulsing rings, banner at the top |
| background (turn finished, a background task such as a test suite still runs) | violet | moves like working; the label shows the task |
| blocked | red | flickers |

Running **sub-agents** appear as small companions (baby jellyfish, lambs, moons, calves, ...), the label
lists them and the status strip shows "+2 agents". With no session running, a resting creature appears.
A status strip lists all sessions with their state and a clock; a small panel shows CPU % and memory
with a 30-minute graph. Everything drifts slowly (no burn-in).

**Themes** (random order, a new one every 10 minutes): aquarium, archipelago, capricorns, cockroaches,
dolphins, espresso, forest, gorillas, guppies, meerkats, pasture, penguins, seabed, space, synthwave,
terrarium - plus the seasonal **halloween** (1 Oct - 2 Nov) and **workshop** (1 Dec - 6 Jan), which can
also be named at any time.

## Status

| Platform | Tested |
|---|---|
| macOS (M1) | window, full screen, self test, benchmarks, both renderers, idle agent, hotkey, lock path (key press -> locked within a second -> closed) |
| Windows 11 x86 (Intel Arc 140V) | GPU renderer (picks Vulkan; Direct3D 12 also works), self test, benchmark, full screen on two monitors, installation as the screen saver with recorder and Ctrl+Alt+V. The logon screen on resume and the preview pane are not confirmed yet |
| Windows 11 ARM (virtual machine) | CPU renderer: window, full screen `/s`, self test, benchmark |
| Ubuntu 24.04 / 26.04 x86 (Intel Iris Xe, GNOME on Wayland) | GPU renderer via Vulkan, self test, benchmark, idle start after `idle_seconds`, lock path (key press -> locked within a second -> closed) |

Known limitations:

- **GNOME needs a second key press after waking.** GNOME fades to black right after a programmatic lock
  and only wakes up on new input; it blocks the virtual input (remote desktop API) that could do this
  while the screen is locked. The attempt is made and fails harmlessly ("Session creation inhibited").
- On a locked Linux screen the Rust version keeps drawing (about 8 % of one core) although nothing is shown.
- Forcing the GPU renderer onto Windows' software adapter ("Microsoft Basic Render Driver") crashes.

**Two renderers.** With a real graphics card the program draws on the GPU. Without one - or when the
system only offers a software stand-in for a GPU, as in virtual machines and some remote desktop
sessions - it draws on the CPU instead (slower; above 2.6 megapixels at half size). `selftest` shows the
adapters and the choice; `--renderer gpu|cpu` forces one.

## How it works

Claude Code writes `~/.claude/sessions/<pid>.json` (name, folder, status) for every running session, a
transcript `~/.claude/projects/*/<sessionId>.jsonl` and, for sub-agents, `.../subagents/agent-<id>.meta.json`.
Claude Vivarium only **reads** these files.

- **Linux (GNOME on Wayland):** GNOME has no screensaver plug-in API and does not show app windows on
  the lock screen. The idle watcher (`claude-vivarium-rs daemon`, a systemd user service) starts the
  screensaver after `idle_seconds`, before GNOME blanks, and records CPU/memory every 5 s. On a key,
  click or focus loss the screensaver **locks the screen first**, waits until GNOME confirms, then
  closes; if it ever ends unlocked, the watcher locks (fail closed).
- **Windows:** a real screen saver (`ClaudeVivariumRS.scr`). Windows starts it on idle and, with
  "On resume, display logon screen", locks on wake itself. Windows does not run screen savers on a
  locked PC, so **Ctrl+Alt+V** ("start now") starts it right away instead of Win+L.
- **macOS:** a LaunchAgent starts it on idle; it locks first on wake like on Linux.
  **Ctrl+Option+V** starts it right away.

While it runs, **right arrow** switches to the next theme and **down arrow** (also up arrow, Tab or T)
opens the **theme picker**; every other key or click ends it (and locks).

## Build

Needs Rust (https://rustup.rs). On Windows also the Visual Studio C++ build tools, on Ubuntu
`sudo apt install build-essential`.

```bash
cargo build --release        # -> target/release/claude-vivarium-rs
cargo test
```

Every push to `main` that touches the code or the scripts builds Linux, macOS and Windows (x86 and ARM)
on GitHub (Actions > "Build"); the programs can be downloaded from the run. The builds are attested:
`gh attestation verify <file> --repo chwichmann/ClaudeVivarium`.

## Try it

```bash
claude-vivarium-rs run --windowed --demo        # preview; N / Right / Space = next theme, Esc = close
claude-vivarium-rs run --windowed               # your real sessions
claude-vivarium-rs selftest                     # fonts, sessions, CPU/memory, graphics card, PNGs of all themes
claude-vivarium-rs sessions                     # the session state as JSON
claude-vivarium-rs render --demo --sysmon --themes all --times 6,24 --out preview    # PNGs, no window
claude-vivarium-rs render --empty --themes all --times 6 --out preview               # the resting creatures
claude-vivarium-rs run --demo --no-lock --max-minutes 0.2    # full screen for 12 s, closes without locking
```

`run` without `--windowed` is the real screensaver. Options: `--themes`, `--theme-minutes`,
`--hide-text`, `--max-minutes`, `--mouse-move hint|wake`, `--no-system-monitor`, `--no-lock`.
`VIVARIUM_TEST="8;/tmp/shots"` saves every window as PNG after 8 s and exits.

`test-linux.sh` (Linux, macOS) and `test-windows.cmd` run these checks in one go next to the program and
collect a log and screenshots in a `vivarium-test` folder; they install nothing and do not lock.

On Windows the program is a GUI program: PowerShell does not wait for it. To see the output of `bench`,
start it with redirected output (e.g. `Start-Process -Wait -RedirectStandardOutput out.txt`).

## Install

### macOS

```bash
./install-mac.sh                     # builds (or uses a downloaded claude-vivarium-rs next to it), installs to
                                     # ~/Library/Application Support/ClaudeVivarium/bin, starts the idle agent
./install-mac.sh --uninstall
tail -f ~/Library/Logs/ClaudeVivarium.log
```

### Ubuntu (GNOME on Wayland)

```bash
./install-linux.sh                   # builds (or uses a downloaded claude-vivarium-rs next to it), installs
                                     # ~/.local/bin/claude-vivarium-rs and the systemd user service
./install-linux.sh --uninstall
journalctl --user -u claude-vivarium-rs -f
./deploy-linux.sh path/to/claude-vivarium-rs laptop1 laptop2     # copy a Linux build to other machines over SSH
```

Needs a Vulkan driver (`mesa-vulkan-drivers`, normally installed with the Ubuntu desktop).

### Windows

```powershell
.\build-windows.ps1 -Install         # builds dist\ClaudeVivariumRS.scr and installs it for the current user
claude-vivarium-rs.exe --install     # or: install a downloaded build
dist\ClaudeVivariumRS.exe --uninstall
```

Installs to `%LOCALAPPDATA%\Programs\ClaudeVivariumRS`, sets it as the screen saver (timeout =
`idle_seconds`, "display logon screen" on), adds the CPU/memory recorder to Startup and Ctrl+Alt+V
("start now"). Log: `%LOCALAPPDATA%\ClaudeVivarium\vivarium-rs.log`. A downloaded build is not signed,
so Windows SmartScreen (and Gatekeeper on macOS) will ask before running it.

## Settings

| Platform | File |
|---|---|
| Linux | `~/.config/claude-vivarium/config.ini` |
| Windows | `%APPDATA%\ClaudeVivarium\config.ini` (the "Settings..." button of the Windows dialog opens it) |
| macOS | `~/Library/Application Support/ClaudeVivarium/config.ini` |

```ini
idle_seconds = 240     # start after this much idle time (Linux, macOS; below GNOME's blank delay);
                       # Windows: used as the screen saver timeout when installing
max_minutes = 0        # 0 = run forever (display stays on); otherwise end after N minutes
mouse_move = hint      # hint = only show "press a key or click to unlock"; wake = end
hide_text = false      # true = only names and states, no prompt/answer/tool text
themes = all           # all | one theme | comma list, e.g. pasture, seabed, forest |
                       # all except some: all, -synthwave, -space
theme_minutes = 10     # minutes per theme, 0 = never switch
system_monitor = true  # CPU/memory panel top right
other_monitors = theme # theme | black  (further monitors)
adaptive_fps = true    # lower the frame rate (25 -> 20/15 fps) while the computer is busy
```

`deploy-config.sh laptop1 laptop2` copies `config.ini` from this folder to Ubuntu machines.

## Layout

```
core/        everything without a window or GPU: sessions, settings, CPU/memory history, adaptive
             frame rate, the shared scene (labels, banner, status strip, panel) and a drawing surface
             with Cairo's calls (canvas.rs)
themes/      one file per theme (themes/src/<name>.rs) and the registry (themes/src/lib.rs)
app/         the program: GPU renderer (gpu.rs), CPU renderer (cpu.rs), window and lock logic (run.rs),
             commands (main.rs) and platform/{macos,linux,windows}.rs
```

To add a theme: copy a small one (`themes/src/synthwave.rs`), implement `Theme` (the world) and
`ActorT` (one creature per session), and add it to the two lists in `themes/src/lib.rs` and to the
theme list in `core/src/config.rs`.

## Performance

`bench` opens a real window with one theme, made-up sessions and monitor data for a fixed time and
prints frames per second, the time to produce a frame, late frames and the CPU time of the process:

```bash
claude-vivarium-rs bench --theme synthwave --seconds 30 --fps 30
claude-vivarium-rs bench --theme synthwave --seconds 30 --fullscreen    # native resolution
```

Compared with the former Python version (Cairo on the CPU, same benchmark), 1680x1050, 30 fps, process
CPU as a share of one core:

| Computer | Theme | Python | Rust (GPU) |
|---|---|---|---|
| MacBook M1 | meerkats / synthwave / aquarium | 29 / 38 / 35 % | 16 / 17 / 16 % |
| Windows, Core Ultra 7 268V, Arc 140V | meerkats / synthwave / aquarium | 25 / 51 / 34 % | 5.5 / 5.0 / 4.4 % |
| Ubuntu, i7 TigerLake, Iris Xe, all cores busy with a test suite | meerkats / synthwave / aquarium | 81 / 83 / 85 %, 20-28 fps, up to 45 % late frames | 10 / 6 / 12 %, 29.9 fps, no late frames |

Building the list of shapes for a frame takes 0.1 to 0.5 ms in Rust (Python: 5 to 20 ms of drawing);
the rest is handing the frame to the graphics card. The figures do not include the work of the graphics
card itself or of the compositor. The CPU renderer needs about as much as the Python version (synthwave
on the Windows PC: 63 % of one core, 21 ms per frame).

## History

Claude Vivarium started as a Python program (Cairo, GTK on Linux, PyObjC on macOS, a PyInstaller `.scr`
on Windows). The last state of that version is the tag **`python-final`** (`git checkout python-final`).
