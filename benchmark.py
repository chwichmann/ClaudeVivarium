#!/usr/bin/env python3
"""Time update() and draw() of every theme (offscreen, no display needed), or profile one theme.

  python3 benchmark.py                    table: update / draw avg / p95 / max in ms per theme
  python3 benchmark.py --profile aquarium cProfile of one theme (where does the time go?)

Run it on a quiet machine, then again while something keeps the CPU busy, to see how much slower and
more uneven frames get. At 30 fps one frame has 33 ms.
"""
import argparse
import cProfile
import os
import pstats
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.realpath(__file__)))

import cairo  # noqa: E402

import render_preview  # noqa: E402
import sessions  # noqa: E402
import themes  # noqa: E402

W, H, FRAMES = 1680, 1050, 150


def run(name, profile=False):
    scene = themes.THEMES[name](seed=1)
    scene.set_system(render_preview.fake_system())
    cr = cairo.Context(cairo.ImageSurface(cairo.FORMAT_RGB24, W, H))
    scene.draw(cr, W, H)
    ups, draws = [], []

    def loop():
        for i in range(FRAMES):
            if i % 30 == 0:
                scene.set_sessions(sessions.demo_sessions(i / 30))
            t0 = time.perf_counter()
            scene.update(1 / 30)
            t1 = time.perf_counter()
            scene.draw(cr, W, H)
            t2 = time.perf_counter()
            ups.append(t1 - t0)
            draws.append(t2 - t1)

    if profile:
        pr = cProfile.Profile()
        pr.runcall(loop)
        pstats.Stats(pr).sort_stats("tottime").print_stats(14)
    else:
        loop()
    ds = sorted(draws)
    return sum(ups) / FRAMES * 1000, sum(draws) / FRAMES * 1000, ds[int(FRAMES * 0.95)] * 1000, ds[-1] * 1000


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--profile", metavar="THEME", help="cProfile this theme instead of the table")
    args = ap.parse_args()
    if args.profile:
        run(args.profile, True)
        return
    print(f"{'theme':12s} {'update':>8s} {'draw avg':>9s} {'draw p95':>9s} {'draw max':>9s}   (ms, {W}x{H}, 33 ms per frame at 30 fps)")
    for name in themes.parse_list("all"):
        u, d, p95, mx = run(name)
        print(f"{name:12s} {u:8.2f} {d:9.2f} {p95:9.2f} {mx:9.2f}")


if __name__ == "__main__":
    main()
