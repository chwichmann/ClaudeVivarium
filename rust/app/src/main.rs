//! Claude Vivarium - a screensaver that shows the live state of your local Claude Code sessions.
//!
//!   run [--windowed] [--demo] [--themes x] ...   the screensaver (full screen, locks on input)
//!   daemon [--idle N] [--now]                    idle watcher: starts the screensaver, records CPU/memory
//!   selftest [--out DIR]                         checks + PNGs of every theme
//!   sessions [--demo] [--watch]                  print the session state as JSON
//!   render ...                                   render themes to PNG files without a window
//!   bench --theme x --seconds 30                 measure CPU use and frame times in a real window
//! Windows also understands the screen saver switches /s, /p <hwnd>, /c and --install, --uninstall,
//! --record, --now.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod args;
mod gpu;
mod platform;
mod render;
mod run;
mod signals;

use std::path::PathBuf;

use args::Args;
use run::{Mode, Options};
use vivarium_core::config::{self, Config};

const USAGE: &str = "\
Claude Vivarium (Rust version)

  claude-vivarium-rs run                      full screen, locks the screen on any key or click
  claude-vivarium-rs run --windowed --demo    preview in a window with made-up sessions
                                              (N / Right / Space = next theme, Esc = close)
      --themes all|name|a,b|all,-x   --theme-minutes N   --hide-text   --max-minutes N
      --mouse-move hint|wake         --no-system-monitor --no-lock (close without locking)
  claude-vivarium-rs daemon [--idle SECONDS] [--now]   idle watcher (Linux, macOS)
  claude-vivarium-rs selftest [--out DIR]     check fonts, sessions, graphics; write PNGs of all themes
  claude-vivarium-rs sessions [--demo] [--watch]
  claude-vivarium-rs render --demo --sysmon --themes all --times 6,24 --size 1920x1200 --out preview
  claude-vivarium-rs bench --theme meerkats --seconds 30 [--fps 25] [--size 1680x1050] [--fullscreen]
";

fn options(args: &Args, cfg: &Config, mode: Mode) -> Result<Options, String> {
    let mouse_wakes = match args.value("--mouse-move") {
        Some("wake") => true,
        Some("hint") => false,
        Some(v) => return Err(format!("--mouse-move: hint or wake, not '{v}'")),
        None => cfg.mouse_wakes,
    };
    let test = std::env::var("VIVARIUM_TEST").ok(); // "<seconds>;<folder>": save each window as PNG, then exit
    let shot = test.as_deref().and_then(|t| {
        let (secs, dir) = t.split_once(';')?;
        Some((secs.trim().parse().ok()?, PathBuf::from(dir)))
    });
    Ok(Options {
        mode,
        demo: args.flag("--demo"),
        no_lock: args.flag("--no-lock"),
        themes: args.value("--themes").map(str::to_string).unwrap_or_else(|| cfg.themes.clone()),
        theme_minutes: args.num("--theme-minutes", cfg.theme_minutes)?,
        max_minutes: args.num("--max-minutes", cfg.max_minutes)?,
        mouse_wakes,
        hide_text: args.flag("--hide-text") || cfg.hide_text,
        system_monitor: cfg.system_monitor && !args.flag("--no-system-monitor"),
        other_monitors_theme: cfg.other_monitors_theme,
        adaptive_fps: cfg.adaptive_fps,
        fps: args.num("--fps", vivarium_core::governor::FPS as f64)?.clamp(1.0, 240.0) as u32,
        window_size: args.size("--size", (1280, 800))?,
        window_physical: false,
        bench: None,
        shot,
        seed: None,
    })
}

fn sessions_cmd(args: &Args) -> Result<(), String> {
    let mut reader = vivarium_core::sessions::Reader::new();
    let start = std::time::Instant::now();
    loop {
        let data = if args.flag("--demo") {
            vivarium_core::sessions::demo_sessions(start.elapsed().as_secs_f64())
        } else {
            reader.read()
        };
        println!("{}", vivarium_core::sessions::to_json(&data));
        if !args.flag("--watch") {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
        print!("\x1b[2J\x1b[H");
    }
}

fn selftest(args: &Args) -> Result<(), String> {
    let out = match args.value("--out") {
        Some(o) => PathBuf::from(o),
        None => vivarium_core::sessions::home_dir().join("Desktop").join("vivarium-selftest"),
    };
    println!("Claude Vivarium {} (Rust), {} {}", env!("CARGO_PKG_VERSION"), std::env::consts::OS, std::env::consts::ARCH);
    let (sans, mono) = vivarium_core::text::resolved_fonts();
    println!("fonts: {sans}; {mono}");
    println!("settings: {}", config::path().display());
    let ss = vivarium_core::sessions::Reader::new().read();
    println!("sessions: {}", ss.len());
    for s in &ss {
        println!("  {}: {} -> {}, {}, tool calls {}", s.name, s.status, s.state.as_str(), s.cwd, s.tool_events.len());
    }
    let mut smp = vivarium_core::sysmon::Sampler::new();
    smp.sample();
    std::thread::sleep(std::time::Duration::from_secs(1));
    smp.sample();
    let d = smp.data();
    let gib = 1024f64.powi(3);
    match (d.cpu.last(), d.mem.last()) {
        (Some(c), Some(m)) => println!("cpu {c:.1} %, memory {:.1} / {:.1} GB", m / gib, d.mem_total / gib),
        _ => println!("cpu/memory: could NOT be read"),
    }
    for line in platform::os::selftest_lines() {
        println!("{line}");
    }
    let mut gpu = gpu::Gpu::new();
    match gpu.render_pixels(&vello::Scene::new(), 64, 64) {
        Ok(_) => println!("graphics: {}", gpu.adapter_name(0)),
        Err(e) => println!("graphics: NOT usable - {e}"),
    }
    let list = vec![
        "render".to_string(),
        "--demo".into(),
        "--sysmon".into(),
        "--themes".into(),
        "all".into(),
        "--times".into(),
        "8".into(),
        "--size".into(),
        "1600x1000".into(),
        "--out".into(),
        out.to_string_lossy().to_string(),
    ];
    render::render(&Args::new(list))?;
    println!("PNGs written to {}", out.display());
    Ok(())
}

fn bench(args: &Args, cfg: &Config) -> Result<(), String> {
    let full = args.flag("--fullscreen");
    let mut o = options(args, cfg, if full { Mode::Full } else { Mode::Windowed })?;
    o.demo = true;
    o.no_lock = true;
    o.themes = args.value("--theme").or(args.value("--themes")).unwrap_or("meerkats").to_string();
    o.theme_minutes = 0.0;
    o.max_minutes = 0.0;
    o.adaptive_fps = args.flag("--adaptive");
    o.system_monitor = true;
    o.window_size = args.size("--size", (1680, 1050))?;
    o.window_physical = true;
    o.bench = Some(args.num("--seconds", 30.0)?);
    o.seed = Some(1);
    o.shot = None;
    run::run(o)
}

fn dispatch(list: Vec<String>) -> Result<(), String> {
    let first = list.first().map(|s| s.to_lowercase()).unwrap_or_default();
    let args = Args::new(list.clone());
    let cfg = config::load();
    // Windows calls a screen saver with /s, /p <hwnd> or /c[:hwnd] (also seen with a dash)
    let switch = first.strip_prefix('/').or_else(|| first.strip_prefix('-').filter(|r| !r.starts_with('-')));
    if let Some(sw) = switch {
        return match sw.chars().next() {
            Some('s') => run::run(options(&args, &cfg, Mode::Full)?),
            Some('p') => {
                let hwnd = sw.split_once(':').map(|(_, h)| h.to_string()).or_else(|| list.get(1).cloned()).unwrap_or_default();
                let hwnd: isize = hwnd.trim().parse().map_err(|_| format!("/p: '{hwnd}' is not a window handle"))?;
                let mut o = options(&args, &cfg, Mode::Preview(hwnd))?;
                o.fps = 15;
                o.system_monitor = false;
                run::run(o)
            }
            _ => platform::os::settings(),
        };
    }
    match first.trim_start_matches("--") {
        "run" => {
            let mode = if args.flag("--windowed") { Mode::Windowed } else { Mode::Full };
            run::run(options(&args, &cfg, mode)?)
        }
        "windowed" => run::run(options(&args, &cfg, Mode::Windowed)?),
        "daemon" => platform::os::daemon(&args),
        "selftest" => selftest(&args),
        "sessions" => sessions_cmd(&args),
        "render" => render::render(&args),
        "bench" => bench(&args, &cfg),
        "record" | "now" | "install" | "uninstall" => platform::os::extra(first.trim_start_matches("--"), &cfg),
        "" if cfg!(windows) => platform::os::settings(), // double click on the .scr: settings
        "help" | "h" | "" => {
            print!("{USAGE}");
            Ok(())
        }
        other => Err(format!("unknown command '{other}'\n\n{USAGE}")),
    }
}

fn main() {
    #[cfg(windows)]
    platform::windows::attach_console();
    if let Err(e) = dispatch(std::env::args().skip(1).collect()) {
        platform::log(&format!("error: {e}"));
        #[cfg(windows)]
        platform::windows::say(&e);
        #[cfg(not(windows))]
        eprintln!("claude-vivarium: {e}");
        std::process::exit(1);
    }
}
