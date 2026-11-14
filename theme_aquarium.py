"""Aquarium theme: every session is a glowing jellyfish in the deep sea.

working = swims fast, sparkle trail, tool calls rise as labelled bubbles
idle    = drifts slowly          waiting = swims to the centre, grows, sends out rings
"""
import math

import cairo

from scene import TAU, Actor, Scene, Text, clamp, lighten, pulse_rings, Weight
from scene import glow as glow_sprite

SPEED = {"working": 80, "idle": 20, "waiting": 45, "blocked": 10, "rest": 9}
TURN = {"working": 1.3, "idle": 0.45, "waiting": 0.6, "blocked": 0.2, "rest": 0.25}
PULSE = {"working": 5.2, "idle": 1.5, "waiting": 3.4, "blocked": 9.0, "rest": 0.8}


class Jelly(Actor):
    BASE_RADIUS = 50

    def __init__(self, scene, info, x, y):
        super().__init__(scene, info, x, y)
        self.spark_acc = 0.0

    def move(self, dt, t, target):
        sc, st, s = self.scene, self.state, self.scene.s
        self.phase += dt * PULSE.get(st, 1.5)
        speed = SPEED.get(st, 20) * s
        r = self.radius
        self.wander(dt, t, target, speed, TURN.get(st, 0.5),
                    (110 * s, 140 * s, sc.w - 110 * s, sc.h - sc.strip_h - r * 2.6))
        self.y += -(max(0.0, math.sin(self.phase)) - 0.318) * speed * 0.4 * dt  # push on each contraction
        if st == "working" and not self.dying:
            self.spark_acc += dt
            while self.spark_acc > 0.05:
                self.spark_acc -= 0.05
                sc.sparkles.append({
                    "x": self.x + self.rng.uniform(-r * 0.7, r * 0.7),
                    "y": self.y + r * self.rng.uniform(0.3, 1.9),
                    "vx": -self.vx * 0.15 + self.rng.uniform(-8, 8) * s,
                    "vy": self.rng.uniform(-12, 4) * s,
                    "age": 0.0, "life": self.rng.uniform(1.0, 2.2), "c": self.color,
                })

    def draw(self, cr, t):
        s = self.scene.s
        a = self.alpha * self.flicker
        if a < 0.01:
            return
        x, y, R, c = self.x, self.y, self.radius, self.color
        st = self.state
        p = math.sin(self.phase)
        bw, bh = R * (1 + 0.10 * p), R * (0.80 - 0.10 * p)

        glow = {"working": 0.34, "waiting": 0.45, "blocked": 0.4}.get(st, 0.22)
        glow_sprite(cr, x, y - bh * 0.3, R * 2.8, c, glow * a)
        if st == "waiting":
            pulse_rings(cr, x, y - bh * 0.3, R, c, a, t, s)

        cr.save()
        cr.translate(x, y)
        cr.rotate(clamp(self.vx / (400 * s), -0.35, 0.35))
        drag = clamp(self.vx / (60 * s), -2.5, 2.5)
        cr.set_line_cap(cairo.LINE_CAP_ROUND)
        n = 9
        length = R * (1.9 - 0.25 * p)
        for i in range(n):  # tentacles
            fx = (i / (n - 1) - 0.5) * bw * 1.5
            cr.move_to(fx, R * 0.08)
            for k in range(1, 13):
                f = k / 12
                xx = fx * (1 - 0.25 * f) + math.sin(t * 2.2 + i * 0.9 - f * 4.0 + self.seed) * R * 0.14 * f \
                    - drag * R * 0.18 * f * f
                cr.line_to(xx, R * 0.08 + length * f)
            cr.set_source_rgba(c[0], c[1], c[2], 0.40 * a)
            cr.set_line_width(1.6 * s)
            cr.stroke()
        lc = lighten(c, 0.3)
        for i in (-1, 1):  # oral arms
            cr.move_to(i * R * 0.15, 0)
            for k in range(1, 11):
                f = k / 10
                xx = i * R * 0.15 + math.sin(t * 1.5 + i + f * 3.0) * R * 0.22 * f - drag * R * 0.12 * f * f
                cr.line_to(xx, length * 0.75 * f)
            cr.set_source_rgba(lc[0], lc[1], lc[2], 0.30 * a)
            cr.set_line_width(5 * s)
            cr.stroke()

        cr.new_path()  # bell
        cr.save()
        cr.scale(bw, bh)
        cr.arc(0, 0, 1, math.pi, TAU)
        cr.restore()
        for j in range(8):
            x0 = bw - 2 * bw * j / 8
            x1 = bw - 2 * bw * (j + 1) / 8
            cr.curve_to(x0 - bw * 0.05, R * 0.16, x1 + bw * 0.05, R * 0.16, x1, 0)
        cr.close_path()
        hi = lighten(c, 0.6)
        lg = cairo.RadialGradient(0, -bh * 0.55, 0, 0, -bh * 0.3, bw * 1.15)
        lg.add_color_stop_rgba(0, hi[0], hi[1], hi[2], 0.85 * a)
        lg.add_color_stop_rgba(0.55, c[0], c[1], c[2], 0.50 * a)
        lg.add_color_stop_rgba(1, c[0], c[1], c[2], 0.20 * a)
        cr.set_source(lg)
        cr.fill_preserve()
        ec = lighten(c, 0.4)
        cr.set_source_rgba(ec[0], ec[1], ec[2], 0.8 * a)
        cr.set_line_width(1.5 * s)
        cr.stroke()

        core = 0.45 + (0.35 * max(0, math.sin(t * 9 + self.seed)) if st == "working" else 0.1 * p)
        cr.set_source_rgba(1, 1, 1, core * a)
        cr.save()
        cr.translate(0, -bh * 0.38)
        cr.scale(R * 0.30, R * 0.20)
        cr.arc(0, 0, 1, 0, TAU)
        cr.restore()
        cr.fill()
        cr.set_source_rgba(1, 1, 1, 0.22 * a)
        cr.set_line_width(3 * s)
        cr.arc(0, 0, bh * 0.75, math.pi * 1.15, math.pi * 1.45)
        cr.stroke()
        cr.restore()


class Aquarium(Scene):
    name = "aquarium"
    actor_class = Jelly

    def __init__(self, **kw):
        super().__init__(**kw)
        self.bubbles = []
        self.sparkles = []
        self.plankton = []
        self.rays = []
        self.weeds = []
        self.bg = None
        self.bg_rays = None
        self.rays_t = -1.0
        self.next_ambient = 0.0

    def on_tool(self, actor, name, index):
        r = actor.radius
        self.spawn_bubble(actor.x + self.rng.uniform(-r * 0.5, r * 0.5), actor.y - r * 0.2 + index * 26 * self.s,
                          label=name, color=actor.color)

    def spawn_bubble(self, x, y, label=None, color=(0.8, 0.9, 1.0)):
        s = self.s
        self.bubbles.append({
            "x0": x, "x": x, "y": y, "ph": self.rng.uniform(0, TAU), "age": 0.0,
            "r": s * (self.rng.uniform(9, 13) if label else self.rng.uniform(2, 5)),
            "vy": s * (self.rng.uniform(45, 65) if label else self.rng.uniform(25, 50)),
            "life": 9.0 if label else 30.0, "label": label, "c": color,
        })

    def _bed_y(self, x, far=False):
        w, h, s = self.w, self.h, self.s
        if far:
            return h - s * (150 + 35 * math.sin(x / w * 4.0 + 3.0) + 15 * math.sin(x / w * 11 + 1.0))
        return h - s * (78 + 24 * math.sin(x / w * 6.0 + 0.7) + 12 * math.sin(x / w * 17 + 2.1))

    def setup_world(self):
        w, h, s, rng = self.w, self.h, self.s, self.rng
        self.bg = cairo.ImageSurface(cairo.FORMAT_RGB24, w, h)
        self.bg_rays = cairo.ImageSurface(cairo.FORMAT_RGB24, w, h)
        self.rays_t = -1.0
        cr = cairo.Context(self.bg)
        g = cairo.LinearGradient(0, 0, 0, h)
        g.add_color_stop_rgb(0, 0.03, 0.18, 0.31)
        g.add_color_stop_rgb(0.5, 0.01, 0.08, 0.17)
        g.add_color_stop_rgb(1, 0.00, 0.02, 0.05)
        cr.set_source(g)
        cr.paint()
        v = cairo.RadialGradient(w / 2, h * 0.4, min(w, h) * 0.3, w / 2, h * 0.4, max(w, h) * 0.8)
        v.add_color_stop_rgba(0, 0, 0, 0, 0)
        v.add_color_stop_rgba(1, 0, 0, 0, 0.6)
        cr.set_source(v)
        cr.paint()
        for far, col in ((True, (0.02, 0.07, 0.12, 0.85)), (False, (0.005, 0.025, 0.045, 1.0))):
            cr.move_to(0, h)
            for xi in range(0, w + 9, 8):
                cr.line_to(xi, self._bed_y(xi, far))
            cr.line_to(w, h)
            cr.close_path()
            cr.set_source_rgba(*col)
            cr.fill()
        cr.set_line_width(1.5 * s)
        cr.set_source_rgba(0.3, 0.8, 0.9, 0.07)
        for xi in range(0, w + 9, 8):
            cr.line_to(xi, self._bed_y(xi))
        cr.stroke()

        self.rays = [{"x": rng.uniform(-0.1, 1.0), "w": rng.uniform(0.04, 0.12), "ph": rng.uniform(0, TAU),
                      "sp": rng.uniform(0.05, 0.14), "a": rng.uniform(0.03, 0.06)} for _ in range(7)]
        n = int(160 * w * h / (1920 * 1200)) + 20
        self.plankton = [[rng.uniform(0, w), rng.uniform(0, h), rng.uniform(0.15, 1.0), rng.uniform(0, TAU)]
                         for _ in range(n)]
        self.weeds = [{"x": rng.uniform(0, w), "h": s * rng.uniform(50, 190), "ph": rng.uniform(0, TAU),
                       "lw": s * rng.uniform(3, 7),
                       "c": (rng.uniform(0.02, 0.08), rng.uniform(0.25, 0.45), rng.uniform(0.3, 0.45))}
                      for _ in range(int(w / (70 * s)))]

    def update_world(self, dt):
        t, s, rng = self.t, self.s, self.rng
        if t >= self.next_ambient:
            self.next_ambient = t + rng.uniform(0.4, 1.6)
            x = rng.uniform(0, self.w)
            self.spawn_bubble(x, self._bed_y(x))
        for b in self.bubbles:
            b["age"] += dt
            b["y"] -= b["vy"] * dt
            b["x"] = b["x0"] + math.sin(b["age"] * 2.2 + b["ph"]) * s * 10
        self.bubbles = [b for b in self.bubbles if b["y"] > -40 and b["age"] < b["life"]]
        for p in self.sparkles:
            p["age"] += dt
            p["x"] += p["vx"] * dt
            p["y"] += p["vy"] * dt
        self.sparkles = [p for p in self.sparkles if p["age"] < p["life"]]
        for p in self.plankton:
            p[0] += math.sin(t * 0.1 + p[3]) * p[2] * 6 * s * dt
            p[1] -= p[2] * 4 * s * dt
            if p[1] < -5:
                p[1] = self.h + 5
                p[0] = rng.uniform(0, self.w)

    def draw_world(self, cr, t):
        s = self.s
        # The light rays sway very slowly, so background + rays are baked into one surface that is only
        # refreshed 10x per second (the rays cost ~4 ms per frame when drawn every frame).
        if not (0 <= t - self.rays_t < 0.1):
            rc = cairo.Context(self.bg_rays)
            rc.set_source_surface(self.bg, 0, 0)
            rc.paint()
            self._draw_rays(rc, t)
            self.rays_t = t
        cr.set_source_surface(self.bg_rays, 0, 0)
        cr.paint()
        self._draw_plankton(cr, t, far=True)
        self._draw_weeds(cr, t)
        self._draw_bubbles(cr, labels=False)
        for p in self.sparkles:
            f = p["age"] / p["life"]
            c = p["c"]
            cr.set_source_rgba(c[0], c[1], c[2], (1 - f) * 0.8)
            cr.arc(p["x"], p["y"], s * 2.2 * (1 - f) + 0.4, 0, TAU)
            cr.fill()
        for a in self.sorted_actors():
            a.draw(cr, t)
        self._draw_bubbles(cr, labels=True)
        self._draw_plankton(cr, t, far=False)

    def _draw_rays(self, cr, t):
        w, h = self.w, self.h
        for r in self.rays:
            sway = math.sin(t * r["sp"] + r["ph"])
            x0 = r["x"] * w + sway * w * 0.04
            x1 = x0 + w * 0.18
            tw, bw = r["w"] * w * 0.35, r["w"] * w
            a = r["a"] * (0.6 + 0.4 * math.sin(t * 0.27 + r["ph"] * 3))
            g = cairo.LinearGradient(0, 0, 0, h * 0.85)
            g.add_color_stop_rgba(0, 0.6, 0.9, 1.0, a)
            g.add_color_stop_rgba(1, 0.6, 0.9, 1.0, 0)
            cr.move_to(x0 - tw / 2, 0)
            cr.line_to(x0 + tw / 2, 0)
            cr.line_to(x1 + bw / 2, h * 0.85)
            cr.line_to(x1 - bw / 2, h * 0.85)
            cr.close_path()
            cr.set_source(g)
            cr.fill()

    def _draw_plankton(self, cr, t, far):
        s = self.s
        for x, y, z, ph in self.plankton:
            if (z < 0.6) != far:
                continue
            tw = 0.5 + 0.5 * math.sin(t * (0.8 + z) + ph * 5)
            cr.set_source_rgba(0.7, 0.9, 1.0, (0.08 + 0.3 * z) * (0.4 + 0.6 * tw))
            cr.arc(x, y, s * (0.6 + 1.6 * z), 0, TAU)
            cr.fill()

    def _draw_weeds(self, cr, t):
        s = self.s
        cr.set_line_cap(cairo.LINE_CAP_ROUND)
        cr.set_line_join(cairo.LINE_JOIN_ROUND)
        for wd in self.weeds:
            base = self._bed_y(wd["x"]) + 3 * s
            cr.move_to(wd["x"], base)
            for k in range(1, 10):
                f = k / 9
                xx = wd["x"] + math.sin(t * 0.7 + wd["ph"] + f * 2.2) * f * s * 22 \
                    + f * f * math.sin(t * 0.3 + wd["ph"]) * s * 12
                cr.line_to(xx, base - f * wd["h"])
            c = wd["c"]
            cr.set_source_rgba(c[0], c[1], c[2], 0.55)
            cr.set_line_width(wd["lw"])
            cr.stroke()

    def _draw_bubbles(self, cr, labels):
        s = self.s
        for b in self.bubbles:
            if bool(b["label"]) != labels:
                continue
            fade = min(1.0, b["age"] * 3) * min(1.0, (b["life"] - b["age"]) / 1.5)
            x, y, r = b["x"], b["y"], b["r"]
            cr.set_source_rgba(0.7, 0.9, 1.0, 0.07 * fade)
            cr.arc(x, y, r, 0, TAU)
            cr.fill_preserve()
            cr.set_source_rgba(0.75, 0.92, 1.0, 0.45 * fade)
            cr.set_line_width(max(1.0, 1.2 * s))
            cr.stroke()
            cr.set_source_rgba(1, 1, 1, 0.55 * fade)
            cr.arc(x - r * 0.35, y - r * 0.35, r * 0.25, 0, TAU)
            cr.fill()
            if b["label"]:
                lay = self.text.layout(cr, b["label"], 15 * s, Weight.SEMIBOLD)
                th = lay.get_pixel_size()[1]
                Text.show(cr, lay, x + r + 6 * s, y - th / 2, lighten(b["c"], 0.3), 0.9 * fade, shadow=0.6)
