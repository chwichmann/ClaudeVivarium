"""Guppies theme: a planted freshwater tank.

A working session becomes a male guppy - slim, colourful, with a big flowing tail fin. Idle sessions are
females - larger, plainer, small tail. A fish changes smoothly between the two when its state changes.
working = darts around, each tool call is a bubble with the tool name
idle    = cruises slowly       waiting = swims to the front glass, grows, pulses
"""
import math

import cairo

from scene import TAU, Actor, Scene, clamp, lerp, lerp3, lighten, pulse_rings

SPEED = {"working": 110, "idle": 32, "waiting": 60, "blocked": 8, "rest": 10}
TURN = {"working": 1.6, "idle": 0.6, "waiting": 0.5, "blocked": 0.2, "rest": 0.3}
FEMALE = (0.66, 0.68, 0.62)
TAIL_COLORS = [(1.0, 0.45, 0.20), (0.95, 0.25, 0.55), (0.30, 0.55, 1.0), (1.0, 0.80, 0.25), (0.55, 0.35, 1.0)]


class Guppy(Actor):
    BASE_RADIUS = 48

    def __init__(self, scene, info, x, y):
        super().__init__(scene, info, x, y)
        self.face = 1.0
        self.tail_phase = self.rng.uniform(0, TAU)
        self.male = 1.0 if self.state == "working" else 0.0
        self.tail_color = TAIL_COLORS[int(self.seed) % len(TAIL_COLORS)]
        self.spots = [(self.rng.uniform(-0.5, 0.4), self.rng.uniform(-0.15, 0.15), self.rng.uniform(0.06, 0.12),
                       self.rng.choice([(0.05, 0.05, 0.08), (1.0, 0.55, 0.2), (0.3, 0.6, 1.0)])) for _ in range(5)]

    def move(self, dt, t, target):
        sc, st, s = self.scene, self.state, self.scene.s
        self.male = lerp(self.male, 1.0 if st == "working" else 0.0, min(1.0, dt * 0.8))
        speed = SPEED.get(st, 30) * s
        r = self.radius
        self.wander(dt, t, target, speed, TURN.get(st, 0.5),
                    (90 * s + r, sc.surface + 50 * s + r, sc.w - 90 * s - r, sc.gravel_top - r * 1.2), spacing=2.4)
        self.vy *= 0.97
        if abs(self.vx) > 8 * s:
            self.face = lerp(self.face, 1.0 if self.vx > 0 else -1.0, min(1.0, dt * 5))
        sp = math.hypot(self.vx, self.vy)
        self.tail_phase += dt * (4 + sp / (6 * s))

    def draw(self, cr, t):
        s = self.scene.s
        a = self.alpha * self.flicker
        if a < 0.01:
            return
        R, c, st, m = self.radius, self.color, self.state, self.male
        if st == "waiting":
            pulse_rings(cr, self.x, self.y, R, c, a, t, s)
        g = cairo.RadialGradient(self.x, self.y, 0, self.x, self.y, R * 2.2)
        g.add_color_stop_rgba(0, c[0], c[1], c[2], (0.30 if st != "idle" else 0.16) * a)
        g.add_color_stop_rgba(1, c[0], c[1], c[2], 0)
        cr.set_source(g)
        cr.arc(self.x, self.y, R * 2.2, 0, TAU)
        cr.fill()

        cr.save()
        cr.translate(self.x, self.y)
        cr.rotate(clamp(self.vy / (300 * s), -0.4, 0.4) * (1 if self.face > 0 else -1))
        cr.scale(math.copysign(max(0.35, abs(self.face)), self.face), 1.0)  # never a thin sliver while turning
        body_h = R * lerp(0.48, 0.34, m)           # females are rounder
        body_l = R * lerp(1.0, 0.9, m)
        wag = math.sin(self.tail_phase)
        # tail fin: small fan (female) -> big flowing veil (male)
        tl = R * lerp(0.45, 1.25, m)
        spread = R * lerp(0.22, 0.75, m)
        bx = -body_l * 0.82
        tail_col = lerp3(lerp3(FEMALE, c, 0.4), lerp3(self.tail_color, c, 0.35), m)
        cr.move_to(bx + R * 0.05, -R * 0.06)
        cr.curve_to(bx - tl * 0.4, -spread * 0.5 + wag * R * 0.08, bx - tl, -spread + wag * R * 0.2,
                    bx - tl * 1.05, wag * R * 0.25)
        cr.curve_to(bx - tl, spread + wag * R * 0.2, bx - tl * 0.4, spread * 0.5 + wag * R * 0.08,
                    bx + R * 0.05, R * 0.06)
        cr.close_path()
        tg = cairo.LinearGradient(bx, 0, bx - tl, 0)
        tg.add_color_stop_rgba(0, tail_col[0], tail_col[1], tail_col[2], 0.95 * a)
        tg.add_color_stop_rgba(1, tail_col[0], tail_col[1], tail_col[2], 0.55 * a)
        cr.set_source(tg)
        cr.fill()
        if m > 0.3:  # fin rays and the dark spot pattern of a fancy male tail
            cr.set_line_width(1.0 * s)
            cr.set_source_rgba(1, 1, 1, 0.25 * m * a)
            for k in range(-3, 4):
                cr.move_to(bx, 0)
                cr.line_to(bx - tl * 0.95, k / 3 * spread * 0.9 + wag * R * 0.2)
            cr.stroke()
            cr.set_source_rgba(0.05, 0.05, 0.1, 0.35 * m * a)
            for k in range(4):
                cr.arc(bx - tl * (0.45 + 0.12 * k), math.sin(k * 2.1 + self.seed) * spread * 0.45 + wag * R * 0.1,
                       R * 0.07, 0, TAU)
                cr.fill()
        # dorsal fin (long on males)
        dl = R * lerp(0.25, 0.6, m)
        cr.move_to(-body_l * 0.1, -body_h * 0.85)
        cr.curve_to(-body_l * 0.3, -body_h * 1.6 - m * R * 0.2, -body_l * 0.3 - dl, -body_h * 1.4,
                    -body_l * 0.55 - dl * 0.4, -body_h * 0.55)
        cr.close_path()
        cr.set_source_rgba(tail_col[0], tail_col[1], tail_col[2], 0.7 * a)
        cr.fill()
        # body
        cr.move_to(body_l, 0)
        cr.curve_to(body_l * 0.9, -body_h * 1.05, -body_l * 0.2, -body_h * 1.15, bx, -R * 0.08)
        cr.line_to(bx, R * 0.08)
        cr.curve_to(-body_l * 0.2, body_h * (1.25 - 0.25 * m), body_l * 0.85, body_h * 1.1, body_l, 0)
        cr.close_path()
        body_col = lerp3(lerp3(FEMALE, c, 0.25), lighten(c, 0.15), m)
        bg = cairo.LinearGradient(0, -body_h, 0, body_h)
        hi = lighten(body_col, 0.45)
        bg.add_color_stop_rgba(0, body_col[0] * 0.75, body_col[1] * 0.75, body_col[2] * 0.75, a)
        bg.add_color_stop_rgba(0.55, hi[0], hi[1], hi[2], a)
        bg.add_color_stop_rgba(1, 0.92, 0.92, 0.88, a)
        cr.set_source(bg)
        cr.fill()
        if m > 0.2:  # colourful male spots
            for sx, sy, sr, sc_ in self.spots:
                cr.set_source_rgba(sc_[0], sc_[1], sc_[2], 0.75 * m * a)
                cr.arc(sx * body_l, sy * body_h * 2, sr * R, 0, TAU)
                cr.fill()
        else:  # female "gravid spot"
            cr.set_source_rgba(0.15, 0.12, 0.1, 0.35 * (1 - m) * a)
            cr.arc(-body_l * 0.15, body_h * 0.45, R * 0.12, 0, TAU)
            cr.fill()
        # pectoral fin, eye
        cr.set_source_rgba(1, 1, 1, 0.25 * a)
        cr.save()
        cr.translate(body_l * 0.35, body_h * 0.2)
        cr.rotate(0.6 + math.sin(t * 6 + self.seed) * 0.3)
        cr.scale(R * 0.18, R * 0.07)
        cr.arc(0, 0, 1, 0, TAU)
        cr.restore()
        cr.fill()
        ex = body_l * 0.68
        cr.set_source_rgba(1, 1, 1, a)
        cr.arc(ex, -body_h * 0.18, R * 0.11, 0, TAU)
        cr.fill()
        cr.set_source_rgba(0, 0, 0, a)
        cr.arc(ex + R * 0.02, -body_h * 0.18, R * 0.06, 0, TAU)
        cr.fill()
        cr.restore()


class Guppies(Scene):
    name = "guppies"
    actor_class = Guppy
    label_bg = (0.02, 0.06, 0.05)
    empty_color = (0.8, 1.0, 0.9)
    waiting_row = 0.55

    def __init__(self, **kw):
        super().__init__(**kw)
        self.bg = None
        self.plants = []
        self.motes = []
        self.next_bubble = 0.0
        self.surface = 0.0
        self.gravel_top = 0.0

    def on_tool(self, actor, name, index):
        dx = actor.radius * (1 if actor.face > 0 else -1)
        self.spawn_floater(actor.x + dx, actor.y - actor.radius * 0.4 - index * 26 * self.s, name, actor.color,
                           style="bubble", vy=60, life=6.0)

    def setup_world(self):
        w, h, s, rng = self.w, self.h, self.s, self.rng
        self.surface = h * 0.06
        self.gravel_top = h - self.strip_h - 90 * s
        self.bg = cairo.ImageSurface(cairo.FORMAT_RGB24, w, h)
        cr = cairo.Context(self.bg)
        g = cairo.LinearGradient(0, 0, 0, h)
        g.add_color_stop_rgb(0, 0.14, 0.36, 0.36)
        g.add_color_stop_rgb(0.6, 0.06, 0.20, 0.20)
        g.add_color_stop_rgb(1, 0.03, 0.09, 0.08)
        cr.set_source(g)
        cr.paint()
        cr.set_source_rgba(0.75, 0.95, 0.95, 0.25)  # water surface line
        cr.rectangle(0, self.surface - 2 * s, w, 3 * s)
        cr.fill()
        # driftwood
        cr.set_source_rgb(0.12, 0.08, 0.05)
        cr.set_line_cap(cairo.LINE_CAP_ROUND)
        for _ in range(2):
            x0 = rng.uniform(0.1, 0.8) * w
            cr.set_line_width(rng.uniform(18, 30) * s)
            cr.move_to(x0, self.gravel_top + 20 * s)
            cr.curve_to(x0 + 80 * s, self.gravel_top - 120 * s, x0 + 220 * s, self.gravel_top - 160 * s,
                        x0 + 320 * s, self.gravel_top - 260 * s)
            cr.stroke()
        # gravel
        gt = self.gravel_top
        cr.rectangle(0, gt, w, h - gt)
        gg = cairo.LinearGradient(0, gt, 0, h)
        gg.add_color_stop_rgb(0, 0.30, 0.27, 0.22)
        gg.add_color_stop_rgb(1, 0.12, 0.10, 0.08)
        cr.set_source(gg)
        cr.fill()
        for _ in range(int(1400 * w / 1920)):
            px, py = rng.uniform(0, w), rng.uniform(gt, h)
            col = rng.choice([(0.55, 0.50, 0.42), (0.35, 0.30, 0.25), (0.65, 0.60, 0.52), (0.45, 0.35, 0.28)])
            cr.set_source_rgb(*col)
            cr.save()
            cr.translate(px, py)
            cr.scale(rng.uniform(3, 7) * s, rng.uniform(2, 4) * s)
            cr.arc(0, 0, 1, 0, TAU)
            cr.restore()
            cr.fill()
        # plants: vallisneria ribbons (back layer darker, front layer brighter)
        self.plants = []
        for layer in (0, 1):
            for _ in range(int((40 if layer == 0 else 18) * w / 1920)):
                x = rng.uniform(0, w)
                if layer == 1 and 0.25 * w < x < 0.75 * w:
                    continue  # keep the middle open in front
                self.plants.append({"x": x, "h": rng.uniform(0.35, 0.75 if layer == 0 else 0.55) * (gt - self.surface),
                                    "w": rng.uniform(4, 8) * s, "ph": rng.uniform(0, TAU), "layer": layer,
                                    "c": (rng.uniform(0.15, 0.3), rng.uniform(0.45, 0.65) * (0.65 if layer == 0 else 1),
                                          rng.uniform(0.15, 0.25))})
        self.motes = [[rng.uniform(0, w), rng.uniform(self.surface, gt), rng.uniform(0.2, 1), rng.uniform(0, TAU)]
                      for _ in range(int(90 * w / 1920))]

    def update_world(self, dt):
        s = self.s
        for p in self.motes:
            p[0] += math.sin(self.t * 0.2 + p[3]) * 4 * s * dt
            p[1] += p[2] * 3 * s * dt
            if p[1] > self.gravel_top:
                p[1] = self.surface
        if self.t > self.next_bubble:  # air stone on the left
            self.next_bubble = self.t + self.rng.uniform(0.18, 0.4)
            self.spawn_floater(self.w * 0.06 + self.rng.uniform(-6, 6) * s, self.gravel_top, "", (1, 1, 1),
                               style="bubble", vy=self.rng.uniform(90, 140), life=12)

    def _draw_plants(self, cr, t, layer):
        s = self.s
        cr.set_line_cap(cairo.LINE_CAP_ROUND)
        for p in self.plants:
            if p["layer"] != layer:
                continue
            base = self.gravel_top + 6 * s
            cr.move_to(p["x"], base)
            for k in range(1, 9):
                f = k / 8
                xx = p["x"] + math.sin(t * 0.6 + p["ph"] + f * 2.5) * f * 26 * s
                cr.line_to(xx, base - f * p["h"])
            c = p["c"]
            cr.set_source_rgba(c[0], c[1], c[2], 0.85)
            cr.set_line_width(p["w"])
            cr.stroke()

    def draw_world(self, cr, t):
        s = self.s
        cr.set_source_surface(self.bg, 0, 0)
        cr.paint()
        for k in range(6):  # light shimmer from the surface
            x0 = self.w * (k + 0.5) / 6 + math.sin(t * 0.3 + k) * 60 * s
            gr = cairo.LinearGradient(0, self.surface, 0, self.gravel_top)
            gr.add_color_stop_rgba(0, 0.8, 1.0, 0.9, 0.07)
            gr.add_color_stop_rgba(1, 0.8, 1.0, 0.9, 0)
            cr.move_to(x0 - 40 * s, self.surface)
            cr.line_to(x0 + 40 * s, self.surface)
            cr.line_to(x0 + 110 * s, self.gravel_top)
            cr.line_to(x0 - 10 * s, self.gravel_top)
            cr.close_path()
            cr.set_source(gr)
            cr.fill()
        self._draw_plants(cr, t, 0)
        for x, y, z, ph in self.motes:
            cr.set_source_rgba(0.85, 1.0, 0.9, 0.08 + 0.2 * z)
            cr.arc(x, y, (0.6 + z) * s, 0, TAU)
            cr.fill()
        for a in self.sorted_actors():
            a.draw(cr, t)
        self._draw_plants(cr, t, 1)
