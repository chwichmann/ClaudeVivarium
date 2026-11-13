"""Synthwave theme: 80s neon sunset, scrolling grid floor, sessions are rotating wireframe cubes.

working = spins fast, tool calls appear as glitchy neon text
idle    = slow spin, drifts          waiting = comes to the front, grows, pulses
"""
import math

import cairo

from scene import MONO, TAU, Actor, Scene, Text, clamp, lighten, pulse_rings, Weight

SPIN = {"working": 1.8, "idle": 0.35, "waiting": 0.9, "blocked": 0.05, "rest": 0.15}
SPEED = {"working": 70, "idle": 18, "waiting": 40, "blocked": 8, "rest": 8}
TURN = {"working": 1.1, "idle": 0.4, "waiting": 0.5, "blocked": 0.2, "rest": 0.25}
NEON_PINK = (1.0, 0.25, 0.75)
NEON_CYAN = (0.25, 0.95, 1.0)

CUBE_V = [(x, y, z) for x in (-1, 1) for y in (-1, 1) for z in (-1, 1)]
CUBE_E = [(i, j) for i in range(8) for j in range(i + 1, 8)
          if sum(a != b for a, b in zip(CUBE_V[i], CUBE_V[j])) == 1]


class Cube(Actor):
    BASE_RADIUS = 44

    def __init__(self, scene, info, x, y):
        super().__init__(scene, info, x, y)
        self.ax = self.rng.uniform(0, TAU)
        self.ay = self.rng.uniform(0, TAU)

    def move(self, dt, t, target):
        sc, st, s = self.scene, self.state, self.scene.s
        spin = SPIN.get(st, 0.3)
        self.ax += spin * 0.6 * dt
        self.ay += spin * dt
        r = self.radius
        self.wander(dt, t, target, SPEED.get(st, 20) * s, TURN.get(st, 0.4),
                    (110 * s, 150 * s, sc.w - 110 * s, sc.h - sc.strip_h - r * 2.0), spacing=3.0)

    def _project(self, R):
        ca, sa, cb, sb = math.cos(self.ax), math.sin(self.ax), math.cos(self.ay), math.sin(self.ay)
        pts = []
        for x, y, z in CUBE_V:
            y, z = y * ca - z * sa, y * sa + z * ca      # rotate around X
            x, z = x * cb + z * sb, -x * sb + z * cb     # rotate around Y
            p = 3.2 / (3.2 + z)
            pts.append((self.x + x * R * 0.75 * p, self.y + y * R * 0.75 * p))
        return pts

    def draw(self, cr, t):
        sc, s = self.scene, self.scene.s
        a = self.alpha * self.flicker
        if a < 0.01:
            return
        R, c = self.radius, self.color
        # floor shadow/reflection when hovering over the grid
        if self.y < sc.h - sc.strip_h:
            fy = max(self.y + R * 1.6, sc.horizon + 10 * s)
            cr.save()
            cr.translate(self.x, fy)
            cr.scale(R * 1.1, R * 0.18)
            cr.arc(0, 0, 1, 0, TAU)
            cr.restore()
            cr.set_source_rgba(c[0], c[1], c[2], 0.12 * a)
            cr.fill()
        g = cairo.RadialGradient(self.x, self.y, 0, self.x, self.y, R * 2.4)
        g.add_color_stop_rgba(0, c[0], c[1], c[2], (0.35 if self.state != "idle" else 0.18) * a)
        g.add_color_stop_rgba(1, c[0], c[1], c[2], 0)
        cr.set_source(g)
        cr.arc(self.x, self.y, R * 2.4, 0, TAU)
        cr.fill()
        if self.state == "waiting":
            pulse_rings(cr, self.x, self.y, R, c, a, t, s)
        pts = self._project(R)
        cr.set_line_cap(cairo.LINE_CAP_ROUND)
        cr.set_line_join(cairo.LINE_JOIN_ROUND)
        for width, alpha, col in ((8 * s, 0.18, c), (2.4 * s, 0.95, lighten(c, 0.35))):
            for i, j in CUBE_E:
                cr.move_to(*pts[i])
                cr.line_to(*pts[j])
            cr.set_source_rgba(col[0], col[1], col[2], alpha * a)
            cr.set_line_width(width)
            cr.stroke()
        for px, py in pts:  # glowing vertices
            cr.set_source_rgba(1, 1, 1, 0.8 * a)
            cr.arc(px, py, 2.4 * s, 0, TAU)
            cr.fill()


class Synthwave(Scene):
    name = "synthwave"
    actor_class = Cube
    label_bg = (0.06, 0.0, 0.10)
    empty_color = NEON_PINK

    def __init__(self, **kw):
        super().__init__(**kw)
        self.words = []
        self.bg = None
        self.scan = None
        self.stars = []
        self.mountains = []
        self.horizon = 0.0

    def on_tool(self, actor, name, index):
        r = actor.radius
        self.words.append({"x": actor.x + self.rng.uniform(-r, r * 0.3), "y": actor.y - r * 1.2 - index * 30 * self.s,
                           "age": 0.0, "life": 3.5, "label": name.upper(), "c": actor.color})

    def setup_world(self):
        w, h, s, rng = self.w, self.h, self.s, self.rng
        self.horizon = hz = h * 0.60
        self.bg = cairo.ImageSurface(cairo.FORMAT_RGB24, w, h)
        cr = cairo.Context(self.bg)
        g = cairo.LinearGradient(0, 0, 0, hz)
        g.add_color_stop_rgb(0, 0.03, 0.0, 0.08)
        g.add_color_stop_rgb(0.6, 0.16, 0.02, 0.24)
        g.add_color_stop_rgb(1, 0.45, 0.06, 0.35)
        cr.set_source(g)
        cr.rectangle(0, 0, w, hz)
        cr.fill()
        fg = cairo.LinearGradient(0, hz, 0, h)
        fg.add_color_stop_rgb(0, 0.10, 0.0, 0.14)
        fg.add_color_stop_rgb(1, 0.02, 0.0, 0.04)
        cr.set_source(fg)
        cr.rectangle(0, hz, w, h - hz)
        cr.fill()
        for _ in range(int(220 * w / 1920)):
            cr.set_source_rgba(1, 0.9, 1, rng.uniform(0.1, 0.6))
            cr.arc(rng.uniform(0, w), rng.uniform(0, hz * 0.75), rng.uniform(0.4, 1.2) * s, 0, TAU)
            cr.fill()
        self.stars = [[rng.uniform(0, w), rng.uniform(0, hz * 0.6), rng.uniform(0, TAU)] for _ in range(40)]
        # mountains (two layers)
        self.mountains = []
        for layer, (amp, col) in enumerate(((0.16, (0.10, 0.02, 0.18)), (0.09, (0.05, 0.0, 0.10)))):
            pts = []
            x = -20.0
            while x < w + 40:
                if abs(x - w / 2) < w * 0.16 and layer == 0:
                    yy = hz - rng.uniform(0.0, 0.02) * h  # leave the sun visible
                else:
                    yy = hz - rng.uniform(0.15, 1.0) * amp * h
                pts.append((x, yy))
                x += rng.uniform(40, 120) * s
            self.mountains.append((pts, col))
        # sun, mountains and horizon glow never change -> drawn once into the background
        self._draw_sun(cr)
        for pts, col in self.mountains:
            cr.move_to(-20, hz + 2)
            for p in pts:
                cr.line_to(*p)
            cr.line_to(w + 40, hz + 2)
            cr.close_path()
            cr.set_source_rgb(*col)
            cr.fill_preserve()
            cr.set_source_rgba(NEON_CYAN[0], NEON_CYAN[1], NEON_CYAN[2], 0.25)
            cr.set_line_width(1.2 * s)
            cr.stroke()
        hg = cairo.LinearGradient(0, hz - 6 * s, 0, hz + 6 * s)
        hg.add_color_stop_rgba(0, 1, 0.5, 0.9, 0)
        hg.add_color_stop_rgba(0.5, 1, 0.6, 0.95, 0.8)
        hg.add_color_stop_rgba(1, 1, 0.5, 0.9, 0)
        cr.set_source(hg)
        cr.rectangle(0, hz - 6 * s, w, 12 * s)
        cr.fill()
        # scanlines
        self.scan = cairo.ImageSurface(cairo.FORMAT_ARGB32, w, h)
        sc = cairo.Context(self.scan)
        sc.set_source_rgba(0, 0, 0, 0.10)
        for yy in range(0, h, 3):
            sc.rectangle(0, yy, w, 1)
        sc.fill()

    def update_world(self, dt):
        for wd in self.words:
            wd["age"] += dt
            wd["y"] -= 35 * self.s * dt
        self.words = [wd for wd in self.words if wd["age"] < wd["life"]]

    def draw_world(self, cr, t):
        w, h, s, hz = self.w, self.h, self.s, self.horizon
        cr.set_source_surface(self.bg, 0, 0)
        cr.paint()
        for x, y, ph in self.stars:
            cr.set_source_rgba(1, 0.9, 1, 0.3 + 0.6 * (0.5 + 0.5 * math.sin(t * 1.3 + ph * 5)))
            cr.arc(x, y, 1.4 * s, 0, TAU)
            cr.fill()
        self._draw_grid(cr, t)
        for a in self.sorted_actors():
            a.draw(cr, t)
        for wd in self.words:
            f = min(1.0, wd["age"] * 4) * min(1.0, (wd["life"] - wd["age"]) / 1.0)
            lay = self.text.layout(cr, wd["label"], 20 * s, Weight.HEAVY, family=MONO)
            jx = (self.rng.uniform(-3, 3) * s) if self.rng.random() < 0.08 else 0  # glitch
            Text.show(cr, lay, wd["x"] - 2 * s + jx, wd["y"], NEON_CYAN, 0.55 * f)
            Text.show(cr, lay, wd["x"] + 2 * s + jx, wd["y"], NEON_PINK, 0.55 * f)
            Text.show(cr, lay, wd["x"], wd["y"], lighten(wd["c"], 0.6), 0.95 * f)
        cr.set_source_surface(self.scan, 0, 0)
        cr.paint()

    def _draw_sun(self, cr):
        w, hz = self.w, self.horizon
        R = self.h * 0.23
        cx, cy = w / 2, hz - R * 0.35
        cr.save()
        cr.rectangle(0, 0, w, hz)  # nothing of the sun below the horizon
        cr.clip()
        g = cairo.RadialGradient(cx, cy, R * 0.8, cx, cy, R * 2.0)
        g.add_color_stop_rgba(0, 1.0, 0.3, 0.6, 0.35)
        g.add_color_stop_rgba(1, 1.0, 0.3, 0.6, 0)
        cr.set_source(g)
        cr.arc(cx, cy, R * 2.0, 0, TAU)
        cr.fill()
        cr.arc(cx, cy, R, 0, TAU)
        cr.clip()
        cr.new_path()
        cr.rectangle(cx - R, cy - R, 2 * R, 2 * R)
        top = cy - R * 0.15
        for k in range(7):  # the classic cut stripes, thicker towards the horizon
            f = k / 7
            yy = top + f * (hz - top)
            th = (hz - top) * (0.02 + 0.09 * f)
            cr.rectangle(cx - R, yy, 2 * R, th)
        cr.set_fill_rule(cairo.FILL_RULE_EVEN_ODD)
        sg = cairo.LinearGradient(0, cy - R, 0, cy + R)
        sg.add_color_stop_rgb(0, 1.0, 0.88, 0.35)
        sg.add_color_stop_rgb(0.55, 1.0, 0.45, 0.4)
        sg.add_color_stop_rgb(1, 1.0, 0.15, 0.6)
        cr.set_source(sg)
        cr.fill()
        cr.restore()
        cr.set_fill_rule(cairo.FILL_RULE_WINDING)

    def _draw_grid(self, cr, t):
        w, h, s, hz = self.w, self.h, self.s, self.horizon
        vx = w / 2
        # lines grouped by brightness so each group is a single stroke
        groups = {}
        for i in range(-24, 25):  # lines towards the vanishing point
            groups.setdefault(1.0, []).append(((vx + i * w * 0.004, hz), (vx + i * w * 0.075, h)))
        off = (t * 0.9) % 1.0  # floor moves towards the viewer
        for k in range(22):
            z = k + 1 - off
            yy = hz + (h - hz) * clamp(1.0 / (z * 0.45 + 1.0), 0, 1)
            f = round(clamp(1.2 - z * 0.06, 0.15, 1.0) * 4) / 4
            groups.setdefault(f, []).append(((0, yy), (w, yy)))
        for width, alpha in ((5 * s, 0.12), (1.4 * s, 0.75)):
            cr.set_line_width(width)
            for f, segs in groups.items():
                for (x0, y0), (x1, y1) in segs:
                    cr.move_to(x0, y0)
                    cr.line_to(x1, y1)
                cr.set_source_rgba(NEON_PINK[0], NEON_PINK[1], NEON_PINK[2], alpha * f)
                cr.stroke()
