# Claude Vivarium - Rust version (GPU)

A rewrite of Claude Vivarium in Rust. Every theme is drawn on the graphics card (Metal on macOS,
Direct3D 12 on Windows, Vulkan on Linux) and the whole program is one file per platform, with no
Python, Cairo, GTK or PyObjC to install. It lives next to the Python version so both can be compared.

**Status.** All eleven themes, the overlays, the settings file, the session reader, the adaptive frame
rate and the platform integration are ported. macOS was built and run on an M1 (window, full screen,
self test, benchmarks). **Linux and Windows are only compile-checked so far - they have not been run.**
The lock path (key press -> lock -> close) has not been exercised on any platform yet.

## What is different from the Python version

- At most **25 frames per second** (20 and 15 on a busy computer); Python runs at 30.
- Drawn at the real resolution of the screen (Python on macOS drew at most 3.5 megapixels and scaled up).
- Linux shows a window on **every** monitor, like Windows and macOS (Python covered one).
- Text is drawn from font outlines with the same system fonts; it looks slightly different, and the
  bullet "●" comes from another font.
- Same `config.ini`, same CPU/memory history file, same session files. The two versions can be
  installed on one machine, but only one should be the active screensaver (the installers switch).

## Layout

```
core/        everything without a window or GPU: sessions, settings, CPU/memory history, adaptive
             frame rate, the shared scene (labels, banner, status strip, panel) and a drawing surface
             with Cairo's calls (canvas.rs), so a theme reads like its Python original
themes/      one file per theme (themes/src/<name>.rs) and the registry (themes/src/lib.rs)
app/         the program: GPU renderer (gpu.rs), window and lock logic (run.rs), commands (main.rs),
             and platform/{macos,linux,windows}.rs
```

To add a theme: copy a small one (`themes/src/synthwave.rs`), implement `Theme` (the world) and
`ActorT` (one creature per session), and add it to the two lists in `themes/src/lib.rs` and to the
theme list in `core/src/config.rs`.

## Build

Needs Rust (https://rustup.rs). On Windows also the Visual Studio C++ build tools, on Ubuntu
`sudo apt install build-essential`.

```bash
cd rust
cargo build --release        # -> target/release/claude-vivarium-rs
cargo test
```

Every push to the `rust-rewrite` branch also builds all three platforms on GitHub (Actions > "Rust
version"); the programs can be downloaded from the run.

## Try it

```bash
claude-vivarium-rs run --windowed --demo        # preview; N / Right / Space = next theme, Esc = close
claude-vivarium-rs run --windowed               # your real sessions
claude-vivarium-rs selftest                     # fonts, sessions, CPU/memory, graphics card, PNGs of all themes
claude-vivarium-rs sessions                     # the session state as JSON (same fields as sessions.py)
claude-vivarium-rs render --demo --sysmon --themes all --times 6,24 --out preview    # PNGs, no window
claude-vivarium-rs render --empty --themes all --times 6 --out preview               # the resting creatures
claude-vivarium-rs run --demo --no-lock --max-minutes 0.2    # full screen for 12 s, closes without locking
```

`run` without `--windowed` is the real screensaver: any key or click **locks the screen first** and
then closes (right arrow = next theme). Options: `--themes`, `--theme-minutes`, `--hide-text`,
`--max-minutes`, `--mouse-move hint|wake`, `--no-system-monitor`, `--no-lock`.
`VIVARIUM_TEST="8;/tmp/shots"` saves every window as PNG after 8 s and exits.

## Install

### macOS

```bash
cd rust && ./install-mac.sh          # builds, installs to ~/Library/Application Support/ClaudeVivarium/bin,
                                     # starts the idle agent (LaunchAgent), stops the Python agent
./install-mac.sh --uninstall
tail -f ~/Library/Logs/ClaudeVivarium.log
```

### Ubuntu (GNOME on Wayland)

```bash
cd rust && ./install-linux.sh        # builds, installs ~/.local/bin/claude-vivarium-rs and the systemd
                                     # user service claude-vivarium-rs, disables the Python service
./install-linux.sh --uninstall
journalctl --user -u claude-vivarium-rs -f
./deploy-linux.sh path/to/claude-vivarium-rs laptop1 laptop2     # copy a Linux build to other machines
```

Needs a Vulkan driver (`mesa-vulkan-drivers`, normally installed with the Ubuntu desktop).

### Windows

```powershell
cd rust
.\build-windows.ps1 -Install         # builds dist\ClaudeVivariumRS.scr and installs it for the current user
dist\ClaudeVivariumRS.exe --uninstall
```

Installs to `%LOCALAPPDATA%\Programs\ClaudeVivariumRS`, sets it as the screen saver (timeout =
`idle_seconds`, "display logon screen" on), adds the recorder to Startup and Ctrl+Alt+V ("start now").
Log: `%LOCALAPPDATA%\ClaudeVivarium\vivarium-rs.log`. A downloaded build is not signed, so Windows
SmartScreen (and Gatekeeper on macOS) will ask before running it.

## Comparing the performance with the Python version

Both versions have the same benchmark: a real window, one theme, made-up sessions and monitor data, a
fixed time. It prints frames per second, the time to produce a frame, late frames and the CPU time of
the process.

```bash
rust/target/release/claude-vivarium-rs bench --theme synthwave --seconds 30 --fps 30
.venv/bin/python vivarium_mac.py --bench 30 --themes synthwave          # macOS front end of the Python version
claude-vivarium-rs bench --theme synthwave --seconds 30 --fullscreen    # native resolution
```

Measured on a MacBook with M1, 1680x1050 pixels drawn, 15 s per run (process CPU as a share of one core):

| Theme | Python, 30 fps | Rust, 30 fps | Rust, 25 fps |
|---|---|---|---|
| meerkats | 29 %, 8.7 ms per frame | 16 %, 2.6 ms | 13 %, 2.5 ms |
| synthwave | 38 %, 11.7 ms | 17 %, 2.7 ms | 14 %, 2.8 ms |
| aquarium | 35 %, 10.7 ms | 16 %, 2.8 ms | - |
| synthwave, all 8 cores busy | 33 %, 10.0 ms | 7 %, 1.0 ms | 6 %, 1.1 ms |
| synthwave, full screen 3360x2100 | - | - | 16 %, 3.1 ms |

Building the list of shapes for a frame takes 0.1 to 0.4 ms in Rust (Python: 4 to 9 ms of drawing);
the rest of the Rust figure is the renderer handing the frame to the graphics card. Neither version
dropped frames in these runs. The figures do not include the work of the graphics card itself or of
the system compositor, and power use was not measured.
