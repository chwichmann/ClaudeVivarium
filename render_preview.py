#!/usr/bin/env python3
"""Render themes offscreen to PNG files (no display needed) and measure frame time.

  python3 render_preview.py --demo --themes all --times 6,24 --out /tmp/preview
"""
import argparse
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.realpath(__file__)))

import cairo  # noqa: E402

import sessions  # noqa: E402
import themes  # noqa: E402


def fake_system():
    """30 minutes of made-up CPU/memory history for previews."""
    import math
    n = 360
    cpu = [max(2.0, min(100.0, 18 + 14 * math.sin(i / 23) + 30 * math.exp(-((i - 250) / 18) ** 2)
                        + 6 * math.sin(i * 1.7))) for i in range(n)]
    gib = 1024 ** 3
    mem = [(6.2 + 1.5 * math.sin(i / 70) + 0.002 * i) * gib for i in range(n)]
    return {"interval": 5, "time": time.time(), "cpu": cpu, "mem": mem, "mem_total": 15.4 * gib, "cores": 8}


def render(name, opts, w, h):
    scene = themes.THEMES[name](seed=1)
    if opts.sysmon:
        scene.set_system(fake_system())
    surf = cairo.ImageSurface(cairo.FORMAT_ARGB32, w, h)
    cr = cairo.Context(surf)
    scene.draw(cr, w, h)
    dt = 1.0 / opts.fps
    sim, frame = 0.0, 0
    draw_times = []
    for target in sorted(float(x) for x in opts.times.split(",")):
        while sim < target:
            if frame % opts.fps == 0:
                scene.set_sessions(sessions.demo_sessions(sim) if opts.demo else sessions.read_sessions())
            scene.update(dt)
            sim += dt
            frame += 1
            if frame % 10 == 0:  # draw every 10th frame to measure the cost
                t0 = time.perf_counter()
                scene.draw(cr, w, h)
                draw_times.append(time.perf_counter() - t0)
        scene.draw(cr, w, h)
        path = os.path.join(opts.out, f"{name}_{target:04.1f}s.png")
        surf.write_to_png(path)
        print(path)
    if draw_times:
        avg = sum(draw_times) / len(draw_times) * 1000
        print(f"{name}: draw avg {avg:.1f} ms, max {max(draw_times) * 1000:.1f} ms (~{1000 / avg:.0f} fps possible)")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--demo", action="store_true")
    ap.add_argument("--themes", default="all")
    ap.add_argument("--times", default="6,24")
    ap.add_argument("--size", default="1920x1200")
    ap.add_argument("--fps", type=int, default=30)
    ap.add_argument("--out", default="preview")
    ap.add_argument("--sysmon", action="store_true", help="show the system monitor with sample data")
    opts = ap.parse_args()
    w, h = map(int, opts.size.split("x"))
    os.makedirs(opts.out, exist_ok=True)
    for name in themes.parse_list(opts.themes):
        render(name, opts, w, h)


if __name__ == "__main__":
    main()
