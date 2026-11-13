"""Espresso theme: winged espresso pots (moka pots) glide diagonally across a starry night sky,
coffee beans tumble by. An original homage to the flying-objects screensavers of the 90s.

working = flaps hard and flies fast; every tool call puffs a steam cloud with the tool name
idle    = glides calmly           waiting = leaves the formation, hovers in the middle, pulses
rest    = one pot with folded wings drifts by asleep
"""
import math

import cairo

from scene import TAU, Actor, Scene, Text, Weight, clamp, lerp, lerp3, lighten, pulse_rings

SPEED = {"working": 125, "idle": 48, "waiting": 60, "blocked": 15, "rest": 22}
FLAP = {"working": 11.0, "idle": 4.0, "waiting": 6.0, "blocked": 0.5, "rest": 0.0}
DIR = (-1.0, 0.55)  # everybody flies to the lower left, like a flock
METAL = (0.78, 0.79, 0.82)


class MokaPot(Actor):
    BASE_RADIUS = 62

    def __init__(self, scene, info, x, y):
        super().__init__(scene, info, x, y)
        self.flap = self.rng.uniform(0, TAU)
        self.lane = self.rng.uniform(-1, 1)   # small personal offset so they do not fly in lockstep
        self.puff_acc = 0.0

    def move(self, dt, t, target):
        sc, st, s = self.scene, self.state, self.scene.s
        self.flap += dt * FLAP.get(st, 4.0)
        if target is not None:  # hover in the middle
            tx, ty = target
            tx += math.sin(t * 0.7 + self.seed) * 30 * s
            ty += math.sin(t * 1.3 + self.seed) * 15 * s
            k = min(1.0, dt * 1.2)
            self.vx = lerp(self.vx, (tx - self.x) * 1.5, k)
            self.vy = lerp(self.vy, (ty - self.y) * 1.5, k)
        else:
            sp = SPEED.get(st, 45) * s * (1 + 0.08 * self.lane)
            k = min(1.0, dt * 0.8)
            self.vx = lerp(self.vx, DIR[0] * sp, k)
            self.vy = lerp(self.vy, DIR[1] * sp + math.sin(t * 0.4 + self.seed) * 12 * s, k)
            for o in sc.actors.values():  # keep apart, sideways to the flight direction
                if o is not self and not o.dying:
                    dx, dy = self.x - o.x, self.y - o.y
                    d = math.hypot(dx, dy)
                    md = (self.radius + o.radius) * 2.4
                    if 0.01 < d < md:
                        self.vx += dx / d * (md - d) * 1.5 * dt * 10
                        self.vy += dy / d * (md - d) * 1.5 * dt * 10
        bob = math.sin(self.flap) * (6 * s if FLAP.get(st, 0) else 0)
        self.x += self.vx * dt
        self.y += (self.vy + bob) * dt
        # wrap around the edges (only while flying with the flock)
        m = self.radius * 3
        bottom = sc.h - sc.strip_h
        if target is None:
            if self.x < -m or self.y > bottom + m:
                if self.rng.random() < 0.55:      # come back on the right edge ...
                    self.x, self.y = sc.w + m * 0.9, self.rng.uniform(-m, bottom * 0.7)
                else:                             # ... or on the top edge
                    self.x, self.y = self.rng.uniform(sc.w * 0.3, sc.w + m), -m * 0.9
        if st == "working" and not self.dying:
            self.puff_acc += dt
            if self.puff_acc > 0.25:
                self.puff_acc = 0.0
                sc.puff(self.x - self.radius * 0.95, self.y - self.radius * 0.25, small=True)

    def draw(self, cr, t):
        s = self.scene.s
        a = self.alpha * self.flicker
        if a < 0.01:
            return
        R, c, st = self.radius, self.color, self.state
        if st == "waiting":
            pulse_rings(cr, self.x, self.y, R, c, a, t, s)
        g = cairo.RadialGradient(self.x, self.y, 0, self.x, self.y, R * 2.3)
        g.add_color_stop_rgba(0, c[0], c[1], c[2], (0.32 if st not in ("idle", "rest") else 0.16) * a)
        g.add_color_stop_rgba(1, c[0], c[1], c[2], 0)
        cr.set_source(g)
        cr.arc(self.x, self.y, R * 2.3, 0, TAU)
        cr.fill()
        cr.save()
        cr.translate(self.x, self.y)
        cr.rotate(clamp(self.vy / (900 * s), -0.15, 0.15))
        folded = st == "rest"
        flap = math.sin(self.flap)
        self._wing(cr, R, flap, a, back=True, folded=folded)
        self._pot(cr, R, c, a, s)
        self._wing(cr, R, flap, a, back=False, folded=folded)
        cr.restore()
        if folded:  # z z z
            T = self.scene.text
            for k in range(3):
                f = (t * 0.3 + k / 3) % 1.0
                lay = T.layout(cr, "z", (14 + 12 * f) * s, Weight.BOLD)
                Text.show(cr, lay, self.x + R * (0.6 + 0.5 * f), self.y - R * (1.0 + 1.2 * f),
                          (1, 1, 1), 0.75 * math.sin(math.pi * f) * a)

    def _pot(self, cr, R, c, a, s):
        """Side view: spout left, handle right; two eight-sided chambers with a waist."""
        metal = lerp3(METAL, c, 0.38)
        hi, lo = lighten(metal, 0.5), (metal[0] * 0.45, metal[1] * 0.45, metal[2] * 0.5)

        def chamber(y_top, y_bot, w_top, w_bot):
            cr.move_to(-w_top, y_top)
            cr.line_to(w_top, y_top)
            cr.line_to(w_bot, y_bot)
            cr.line_to(-w_bot, y_bot)
            cr.close_path()
            g = cairo.LinearGradient(-w_bot, 0, w_bot, 0)
            g.add_color_stop_rgba(0, lo[0], lo[1], lo[2], a)
            g.add_color_stop_rgba(0.35, hi[0], hi[1], hi[2], a)
            g.add_color_stop_rgba(0.6, metal[0], metal[1], metal[2], a)
            g.add_color_stop_rgba(1, lo[0], lo[1], lo[2], a)
            cr.set_source(g)
            cr.fill()
        # handle (black, right side)
        cr.set_source_rgba(0.08, 0.08, 0.09, a)
        cr.set_line_width(R * 0.13)
        cr.set_line_cap(cairo.LINE_CAP_ROUND)
        cr.move_to(R * 0.38, -R * 0.62)
        cr.curve_to(R * 0.95, -R * 0.62, R * 0.95, -R * 0.05, R * 0.42, -R * 0.02)
        cr.stroke()
        # spout (left)
        cr.move_to(-R * 0.36, -R * 0.62)
        cr.line_to(-R * 0.78, -R * 0.78)
        cr.line_to(-R * 0.72, -R * 0.55)
        cr.line_to(-R * 0.4, -R * 0.42)
        cr.close_path()
        cr.set_source_rgba(lo[0], lo[1], lo[2], a)
        cr.fill()
        chamber(R * 0.08, R * 0.78, R * 0.38, R * 0.52)        # lower (water) chamber
        chamber(-R * 0.72, R * 0.02, R * 0.42, R * 0.36)       # upper (coffee) chamber
        cr.set_source_rgba(lo[0] * 0.7, lo[1] * 0.7, lo[2] * 0.7, a)  # waist
        cr.rectangle(-R * 0.4, R * 0.0, R * 0.8, R * 0.1)
        cr.fill()
        cr.set_source_rgba(lo[0], lo[1], lo[2], a)              # lid + knob
        cr.save()
        cr.translate(0, -R * 0.74)
        cr.scale(R * 0.44, R * 0.08)
        cr.arc(0, 0, 1, math.pi, TAU)
        cr.restore()
        cr.fill()
        cr.set_source_rgba(0.08, 0.08, 0.09, a)
        cr.arc(0, -R * 0.86, R * 0.08, 0, TAU)
        cr.fill()
        cr.set_source_rgba(1, 1, 1, 0.35 * a)                   # metal shine
        cr.set_line_width(R * 0.05)
        cr.move_to(-R * 0.18, -R * 0.62)
        cr.line_to(-R * 0.15, -R * 0.08)
        cr.move_to(-R * 0.2, R * 0.2)
        cr.line_to(-R * 0.24, R * 0.66)
        cr.stroke()

    def _wing(self, cr, R, flap, a, back, folded):
        """A feathered wing hinged at the top of the pot; the back wing is darker and mirrored in phase."""
        cr.save()
        cr.translate(R * 0.05, -R * 0.45)
        if folded:
            ang = 0.25 if back else 0.35
        else:
            ang = -0.15 + flap * 0.95 * (0.85 if back else 1.0)
        cr.rotate(-ang - (0.2 if back else 0.0))
        shade = 0.7 if back else 1.0
        for k in range(4):  # four feathers fanning out
            fa = -0.25 + k * 0.17
            ln = R * (1.25 - k * 0.16) * (0.75 if folded else 1.0)
            cr.save()
            cr.rotate(fa)
            cr.move_to(0, 0)
            cr.curve_to(ln * 0.3, -R * 0.28, ln * 0.8, -R * 0.3, ln, -R * 0.1)
            cr.curve_to(ln * 0.75, R * 0.05, ln * 0.3, R * 0.06, 0, 0)
            cr.restore()
            cr.set_source_rgba(0.95 * shade, 0.95 * shade, 0.98 * shade, a)
            cr.fill_preserve()
            cr.set_source_rgba(0.55 * shade, 0.58 * shade, 0.65 * shade, 0.8 * a)
            cr.set_line_width(max(1.0, R * 0.025))
            cr.stroke()
        cr.restore()


class Espresso(Scene):
    name = "espresso"
    actor_class = MokaPot
    label_bg = (0.02, 0.02, 0.06)
    empty_color = (0.95, 0.85, 0.7)
    waiting_row = 0.45

    def __init__(self, **kw):
        super().__init__(**kw)
        self.bg = None
        self.beans = []
        self.stars = []
        self.puffs = []

    def spawn_point(self):
        return self.rng.uniform(self.w * 0.15, self.w * 0.95), self.rng.uniform(self.h * 0.05, self.h * 0.6)

    def puff(self, x, y, small=False):
        s = self.s
        self.puffs.append({"x": x, "y": y, "r": (6 if small else 12) * s, "age": 0.0,
                           "life": 1.2 if small else 2.5, "vx": -20 * s, "vy": -25 * s})

    def on_tool(self, actor, name, index):
        x, y = actor.x - actor.radius * 1.0, actor.y - actor.radius * (0.9 + 0.35 * index)
        self.puff(x, y)
        self.spawn_floater(x - 10 * self.s, y - 14 * self.s, name, actor.color, style="text", vy=30, life=3.5)

    def setup_world(self):
        w, h, s, rng = self.w, self.h, self.s, self.rng
        self.bg = cairo.ImageSurface(cairo.FORMAT_RGB24, w, h)
        cr = cairo.Context(self.bg)
        g = cairo.LinearGradient(0, 0, w * 0.3, h)
        g.add_color_stop_rgb(0, 0.04, 0.03, 0.10)
        g.add_color_stop_rgb(1, 0.005, 0.005, 0.02)
        cr.set_source(g)
        cr.paint()
        mx, my, mr = w * 0.82, h * 0.18, 46 * s   # crescent moon
        mg = cairo.RadialGradient(mx, my, mr, mx, my, mr * 5)
        mg.add_color_stop_rgba(0, 0.9, 0.9, 1.0, 0.18)
        mg.add_color_stop_rgba(1, 0.9, 0.9, 1.0, 0)
        cr.set_source(mg)
        cr.paint()
        cr.save()  # crescent: the moon disc minus an offset disc, so the glow shows through the cut
        cr.arc(mx, my, mr, 0, TAU)
        cr.clip()
        cr.rectangle(mx - mr, my - mr, 2 * mr, 2 * mr)
        cr.new_sub_path()
        cr.arc(mx + mr * 0.45, my - mr * 0.15, mr * 0.92, 0, TAU)
        cr.set_fill_rule(cairo.FILL_RULE_EVEN_ODD)
        cr.set_source_rgb(0.95, 0.93, 0.85)
        cr.fill()
        cr.restore()
        self.stars = [[rng.uniform(0, w), rng.uniform(0, h), rng.uniform(0.2, 1.0), rng.uniform(0, TAU)]
                      for _ in range(int(220 * w * h / (1920 * 1200)))]
        self.beans = []
        for _ in range(int(26 * w / 1920)):
            z = rng.uniform(0.3, 1.0)  # depth: far beans are small, dark and slow
            self.beans.append({"x": rng.uniform(0, w), "y": rng.uniform(0, h), "z": z,
                               "rot": rng.uniform(0, TAU), "spin": rng.uniform(-1.5, 1.5)})
        self.beans.sort(key=lambda b: b["z"])

    def update_world(self, dt):
        s, w, h = self.s, self.w, self.h
        for b in self.beans:
            sp = 70 * s * b["z"]
            b["x"] += DIR[0] * sp * dt
            b["y"] += DIR[1] * sp * dt
            b["rot"] += b["spin"] * dt
            if b["x"] < -40 * s or b["y"] > h + 40 * s:
                if self.rng.random() < 0.55:
                    b["x"], b["y"] = w + 40 * s, self.rng.uniform(-40 * s, h * 0.7)
                else:
                    b["x"], b["y"] = self.rng.uniform(w * 0.3, w + 40 * s), -40 * s
        for p in self.puffs:
            p["age"] += dt
            p["x"] += p["vx"] * dt
            p["y"] += p["vy"] * dt
        self.puffs = [p for p in self.puffs if p["age"] < p["life"]]

    def _bean(self, cr, b):
        s = self.s
        z = b["z"]
        L = (10 + 16 * z) * s
        cr.save()
        cr.translate(b["x"], b["y"])
        cr.rotate(b["rot"])
        cr.save()
        cr.scale(L, L * 0.68)
        cr.arc(0, 0, 1, 0, TAU)
        cr.restore()
        k = 0.35 + 0.65 * z
        g = cairo.RadialGradient(-L * 0.3, -L * 0.25, L * 0.1, 0, 0, L)
        g.add_color_stop_rgb(0, 0.55 * k, 0.33 * k, 0.18 * k)
        g.add_color_stop_rgb(1, 0.25 * k, 0.13 * k, 0.06 * k)
        cr.set_source(g)
        cr.fill()
        cr.set_source_rgb(0.12 * k, 0.06 * k, 0.03 * k)  # the crease
        cr.set_line_width(max(1.0, L * 0.12))
        cr.move_to(-L * 0.75, 0)
        cr.curve_to(-L * 0.3, -L * 0.2, L * 0.3, L * 0.2, L * 0.75, 0)
        cr.stroke()
        cr.restore()

    def draw_world(self, cr, t):
        s = self.s
        cr.set_source_surface(self.bg, 0, 0)
        cr.paint()
        for x, y, z, ph in self.stars:
            tw = 0.5 + 0.5 * math.sin(t * (0.8 + z * 2) + ph * 5)
            cr.set_source_rgba(0.9, 0.9, 1.0, (0.15 + 0.6 * z) * (0.4 + 0.6 * tw))
            cr.arc(x, y, (0.5 + 1.1 * z) * s, 0, TAU)
            cr.fill()
        far = [b for b in self.beans if b["z"] < 0.7]
        near = [b for b in self.beans if b["z"] >= 0.7]
        for b in far:
            self._bean(cr, b)
        for p in self.puffs:
            f = p["age"] / p["life"]
            r = p["r"] * (1 + 1.5 * f)
            cr.set_source_rgba(0.95, 0.95, 1.0, 0.45 * (1 - f))
            for dx, dy, k in ((0, 0, 1.0), (r * 0.7, -r * 0.2, 0.75), (-r * 0.6, -r * 0.3, 0.7)):
                cr.arc(p["x"] + dx, p["y"] + dy, r * k, 0, TAU)
                cr.new_sub_path()
            cr.fill()
        for a in sorted(self.actors.values(), key=lambda a: (a.state == "waiting", a.y)):
            a.draw(cr, t)
        for b in near:
            self._bean(cr, b)
