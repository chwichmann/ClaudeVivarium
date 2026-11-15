//! Render themes without a window to PNG files and measure the time per frame
//! (the counterpart of render_preview.py and benchmark.py).
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Instant;

use vivarium_core::canvas::Canvas;
use vivarium_core::scene::SceneOpts;
use vivarium_core::sessions::{self, Reader};
use vivarium_core::show::parse_list;
use vivarium_core::sysmon;

use crate::args::Args;
use crate::gpu::{write_png, Gpu, VelloSink};

/// render --demo --sysmon --themes all --times 6,24 --size 1920x1200 --out preview
/// (--empty: no sessions, shows the resting creature; --decor: the extra-monitor look)
pub fn render(args: &Args) -> Result<(), String> {
    let (w, h) = args.size("--size", (1920, 1200))?;
    let fps = args.num("--fps", 30.0)?.max(1.0) as usize;
    let out = PathBuf::from(args.value("--out").unwrap_or("preview"));
    let mut times: Vec<f64> = args.value("--times").unwrap_or("6,24").split(',').filter_map(|v| v.trim().parse().ok()).collect();
    times.sort_by(f64::total_cmp);
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let demo = args.flag("--demo");
    let empty = args.flag("--empty"); // no sessions: shows the resting creature
    let mut gpu = Gpu::new();
    let mut sink = VelloSink::new();
    let mut reader = Reader::new();
    for name in parse_list(args.value("--themes").unwrap_or("all"), vivarium_themes::NAMES) {
        let mut scene = vivarium_themes::make(name, SceneOpts { show_text: !args.flag("--hide-text"), decor: args.flag("--decor"), seed: Some(1) })
            .ok_or("unknown theme")?;
        if args.flag("--sysmon") {
            scene.set_system(Some(Rc::new(sysmon::fake())));
        }
        let frame_scene = |scene: &mut Box<dyn vivarium_core::scene::Scene>, sink: &mut VelloSink| {
            sink.begin();
            let mut cr = Canvas::new(sink);
            scene.draw(&mut cr, w as f64, h as f64);
            cr.finish();
        };
        frame_scene(&mut scene, &mut sink);
        let dt = 1.0 / fps as f64;
        let (mut sim, mut frame) = (0.0, 0usize);
        let mut build: Vec<f64> = Vec::new();
        for &target in &times {
            while sim < target {
                if frame % fps == 0 {
                    scene.set_sessions(&if empty {
                        Vec::new()
                    } else if demo {
                        sessions::demo_sessions(sim)
                    } else {
                        reader.read()
                    });
                }
                scene.update(dt);
                sim += dt;
                frame += 1;
                if frame % 10 == 0 {
                    let t0 = Instant::now();
                    frame_scene(&mut scene, &mut sink);
                    build.push(t0.elapsed().as_secs_f64());
                }
            }
            frame_scene(&mut scene, &mut sink);
            let t0 = Instant::now();
            let pixels = gpu.render_pixels(&sink.scene, w, h)?;
            let gpu_ms = t0.elapsed().as_secs_f64() * 1000.0;
            let path = out.join(format!("{name}_{target:04.1}s.png"));
            write_png(&path, &pixels, w, h)?;
            println!("{}   (render + read back {gpu_ms:.1} ms)", path.display());
        }
        if !build.is_empty() {
            let avg = build.iter().sum::<f64>() / build.len() as f64 * 1000.0;
            let max = build.iter().cloned().fold(0.0, f64::max) * 1000.0;
            println!("{name}: building a frame on the CPU avg {avg:.2} ms, max {max:.2} ms");
        }
    }
    Ok(())
}
