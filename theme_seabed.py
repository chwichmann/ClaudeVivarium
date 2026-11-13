"""Seabed theme: every session is a crab scuttling sideways over the sand.

working = scuttles quickly, claws snapping; each tool call snaps hard and releases a labelled bubble
idle    = sits, blinks, wanders now and then     waiting = comes to the front and waves both claws
The shell has the state colour.
"""
import math

import cairo

from scene import TAU, Scene, Walker, clamp, lighten, pulse_rings


class Crab(Walker):
    BASE_RADIUS = 46
    SPEED = {"working": 85, "idle": 22, "waiting": 50, "blocked": 0}

    def __init__(self, scene, info, x, y):
        super().__init__(scene, info, x, y)
        self.snap = 0.0
        self.blink_at = self.rng.uniform(2, 6)
        self.bubble_at = self.rng.uniform(1, 4)

    def move(self, dt, t, target):
        super().move(dt, t, target)
        self.vy *= 0.9  # crabs walk sideways
        self.snap = max(0.0, self.snap - dt * 3)
        if t > self.blink_at + 0.25:
            self.blink_at = t + self.rng.uniform(2, 7)
        if self.state == "rest" and not self.dying and t > self.bubble_at:  # a sleepy bubble now and then
            self.bubble_at = t + self.rng.uniform(3, 8)
            self.scene.spawn_floater(self.x + self.radius * 0.3, self.y - self.radius * 0.7, "", (1, 1, 1),
                                     style="bubble", vy=30, life=9)

    def draw(self, cr, t):
        s = self.scene.s
        a = self.alpha * self.flicker
        if a < 0.01:
            return
        R, c, st = self.radius, self.color, self.state
        x, y = self.x, self.y + math.sin(self.step * 2) * R * 0.03 * self.moving
        dark = (c[0] * 0.45, c[1] * 0.45, c[2] * 0.45)
        if st == "rest":
            self._draw_buried(cr, t, self.x, self.y, R, c, a, dark)
            return
        # glow + shadow on the sand
        g = cairo.RadialGradient(x, y + R * 0.75, 0, x, y + R * 0.75, R * 2.0)
        g.add_color_stop_rgba(0, c[0], c[1], c[2], (0.4 if st != "idle" else 0.22) * a)
        g.add_color_stop_rgba(1, c[0], c[1], c[2], 0)
        cr.save()
        cr.translate(x, y + R * 0.75)
        cr.scale(1.0, 0.35)
        cr.translate(-x, -(y + R * 0.75))
        cr.set_source(g)
        cr.arc(x, y + R * 0.75, R * 2.0, 0, TAU)
        cr.fill()
        cr.restore()
        cr.save()
        cr.translate(x, y + R * 0.72)
        cr.scale(R * 1.3, R * 0.2)
        cr.arc(0, 0, 1, 0, TAU)
        cr.restore()
        cr.set_source_rgba(0, 0, 0, 0.3 * a)
        cr.fill()
        if st == "waiting":
            pulse_rings(cr, x, y, R, c, a, t, s)

        cr.set_line_cap(cairo.LINE_CAP_ROUND)
        cr.set_line_join(cairo.LINE_JOIN_ROUND)
        # legs: 4 per side, alternating gait
        cr.set_source_rgba(dark[0], dark[1], dark[2], a)
        cr.set_line_width(R * 0.11)
        for side in (-1, 1):
            for i in range(4):
                gait = math.sin(self.step + i * math.pi / 2 + (0 if side > 0 else math.pi)) * self.moving
                by = y + R * (-0.05 + i * 0.16)
                cr.move_to(x + side * R * 0.8, by)
                cr.line_to(x + side * R * (1.3 + 0.05 * i), by - R * 0.28 - gait * R * 0.12)
                cr.line_to(x + side * R * (1.55 + 0.08 * i), by + R * 0.55 + gait * R * 0.08)
        cr.stroke()
        # claws
        wave = st == "waiting"
        for side in (-1, 1):
            lift = (0.55 + 0.25 * math.sin(t * 4 + side)) if wave else 0.0
            ex, ey = side * R * 1.1, R * (-0.65 - lift * 0.6)
            cx_, cy_ = side * R * 1.3, R * (-1.05 - lift * 0.9)
            cr.move_to(x + side * R * 0.65, y - R * 0.2)
            cr.line_to(x + ex, y + ey)
            cr.line_to(x + cx_, y + cy_)
            cr.set_source_rgba(dark[0], dark[1], dark[2], a)
            cr.set_line_width(R * 0.16)
            cr.stroke()
            if st == "working":
                opn = 0.15 + 0.25 * max(0.0, math.sin(t * 9 + side))
            elif wave:
                opn = 0.35 + 0.2 * math.sin(t * 6 + side)
            else:
                opn = 0.12 + 0.06 * math.sin(t * 0.8 + self.seed + side)
            opn += self.snap * 0.5
            base_ang = -math.pi / 2 + side * 0.25
            self._pincer(cr, x + cx_, y + cy_, R, base_ang, opn, side, c, a)
        # carapace
        cr.save()
        cr.translate(x, y)
        cr.scale(R, R * 0.62)
        cr.arc(0, 0, 1, 0, TAU)
        cr.restore()
        hi = lighten(c, 0.55)
        sg = cairo.RadialGradient(x - R * 0.25, y - R * 0.35, R * 0.1, x, y, R * 1.1)
        sg.add_color_stop_rgba(0, hi[0], hi[1], hi[2], a)
        sg.add_color_stop_rgba(0.6, c[0], c[1], c[2], a)
        sg.add_color_stop_rgba(1, dark[0], dark[1], dark[2], a)
        cr.set_source(sg)
        cr.fill()
        cr.set_source_rgba(dark[0], dark[1], dark[2], 0.5 * a)  # shell pattern
        for sx_, sy_, sr in ((-0.45, 0.05, 0.08), (0.45, 0.05, 0.08), (-0.2, 0.3, 0.06), (0.2, 0.3, 0.06),
                             (0.0, -0.05, 0.1)):
            cr.arc(x + sx_ * R, y + sy_ * R, sr * R, 0, TAU)
            cr.new_sub_path()
        cr.fill()
        # eyes on stalks
        blink = t < self.blink_at + 0.15 and t > self.blink_at
        for side in (-1, 1):
            ex, ey = x + side * R * 0.3, y - R * 0.95 + math.sin(t * 2 + side) * R * 0.03
            cr.move_to(x + side * R * 0.22, y - R * 0.45)
            cr.line_to(ex, ey)
            cr.set_source_rgba(dark[0], dark[1], dark[2], a)
            cr.set_line_width(R * 0.08)
            cr.stroke()
            cr.save()
            cr.translate(ex, ey)
            cr.scale(1.0, 0.15 if blink else 1.0)
            cr.arc(0, 0, R * 0.13, 0, TAU)
            cr.restore()
            cr.set_source_rgba(1, 1, 1, a)
            cr.fill()
            if not blink:
                look = clamp(self.vx / (60 * s), -1, 1) * R * 0.04
                cr.arc(ex + look, ey + R * 0.02, R * 0.065, 0, TAU)
                cr.set_source_rgba(0, 0, 0, a)
                cr.fill()

    def _draw_buried(self, cr, t, x, y, R, c, a, dark):
        """Resting crab: dug into the sand, only the top of the shell and the eyes show."""
        s = self.scene.s
        blink = self.blink_at < t < self.blink_at + 0.25
        look = math.sin(t * 0.4 + self.seed) * R * 0.04  # slowly looks left and right
        for side in (-1, 1):
            ex, ey = x + side * R * 0.3, y - R * 0.55
            cr.move_to(x + side * R * 0.22, y - R * 0.05)
            cr.line_to(ex, ey)
            cr.set_source_rgba(dark[0], dark[1], dark[2], a)
            cr.set_line_width(R * 0.08)
            cr.stroke()
            cr.save()
            cr.translate(ex, ey)
            cr.scale(1.0, 0.15 if blink else 1.0)
            cr.arc(0, 0, R * 0.13, 0, TAU)
            cr.restore()
            cr.set_source_rgba(1, 1, 1, a)
            cr.fill()
            if not blink:
                cr.arc(ex + look, ey + R * 0.02, R * 0.065, 0, TAU)
                cr.set_source_rgba(0, 0, 0, a)
                cr.fill()
        cr.save()  # top of the shell
        cr.translate(x, y + R * 0.2)
        cr.scale(R, R * 0.55)
        cr.arc(0, 0, 1, math.pi, TAU)
        cr.restore()
        cr.close_path()
        sg = cairo.LinearGradient(x, y - R * 0.35, x, y + R * 0.2)
        hi = lighten(c, 0.4)
        sg.add_color_stop_rgba(0, hi[0], hi[1], hi[2], a)
        sg.add_color_stop_rgba(1, dark[0], dark[1], dark[2], a)
        cr.set_source(sg)
        cr.fill()
        # sand mound over the rest of the crab
        cr.save()
        cr.translate(x, y + R * 0.32)
        cr.scale(R * 1.6, R * 0.38)
        cr.arc(0, 0, 1, 0, TAU)
        cr.restore()
        mg = cairo.LinearGradient(x, y, x, y + R * 0.7)
        mg.add_color_stop_rgba(0, 0.42, 0.38, 0.29, a)
        mg.add_color_stop_rgba(1, 0.27, 0.24, 0.18, a)
        cr.set_source(mg)
        cr.fill()
        cr.set_source_rgba(0.6, 0.55, 0.42, 0.5 * a)  # a few grains
        for k in range(9):
            gx = x + math.sin(k * 2.3 + self.seed) * R * 1.2
            gy = y + R * (0.18 + 0.2 * abs(math.cos(k * 1.7)))
            cr.arc(gx, gy, 1.4 * s, 0, TAU)
            cr.fill()

    def _pincer(self, cr, px, py, R, ang, opn, side, c, a):
        hi = lighten(c, 0.35)
        for k, (length, width, da) in enumerate(((R * 0.42, R * 0.17, -opn), (R * 0.32, R * 0.11, opn))):
            cr.save()
            cr.translate(px, py)
            cr.rotate(ang + da * side)
            cr.scale(length, width)
            cr.arc(0.8, 0, 1, 0, TAU)
            cr.restore()
            g = cairo.LinearGradient(px, py - R * 0.4, px, py + R * 0.2)
            g.add_color_stop_rgba(0, hi[0], hi[1], hi[2], a)
            g.add_color_stop_rgba(1, c[0] * 0.7, c[1] * 0.7, c[2] * 0.7, a)
            cr.set_source(g)
            cr.fill()


class Seabed(Scene):
    name = "seabed"
    actor_class = Crab
    label_bg = (0.02, 0.05, 0.07)
    empty_color = (0.8, 0.9, 1.0)
    waiting_row = 0.78

    def __init__(self, **kw):
        super().__init__(**kw)
        self.bg = None
        self.caustics = []
        self.snow = []
        self.anemones = []
        self.next_bubble = 0.0

    def spawn_point(self):
        return self.rng.uniform(self.w * 0.15, self.w * 0.85), self.rng.uniform(self.ground_top + 40, self.h * 0.85)

    def on_tool(self, actor, name, index):
        actor.snap = 1.0
        self.spawn_floater(actor.x + actor.radius * 0.8, actor.y - actor.radius * 1.2 - index * 26 * self.s,
                           name, actor.color, style="bubble", vy=55, life=5.0)

    def setup_world(self):
        w, h, s, rng = self.w, self.h, self.s, self.rng
        self.ground_top = gt = h * 0.55
        self.bg = cairo.ImageSurface(cairo.FORMAT_RGB24, w, h)
        cr = cairo.Context(self.bg)
        g = cairo.LinearGradient(0, 0, 0, gt)
        g.add_color_stop_rgb(0, 0.04, 0.24, 0.34)
        g.add_color_stop_rgb(1, 0.03, 0.13, 0.20)
        cr.set_source(g)
        cr.paint()

        def sand_y(x):
            return gt - h * 0.03 - h * 0.015 * math.sin(x / w * 5 + 1) - h * 0.01 * math.sin(x / w * 13)
        cr.move_to(0, h)
        for xi in range(0, w + 9, 8):
            cr.line_to(xi, sand_y(xi))
        cr.line_to(w, h)
        cr.close_path()
        sg = cairo.LinearGradient(0, gt - h * 0.05, 0, h)
        sg.add_color_stop_rgb(0, 0.26, 0.27, 0.24)
        sg.add_color_stop_rgb(0.4, 0.33, 0.30, 0.23)
        sg.add_color_stop_rgb(1, 0.20, 0.17, 0.12)
        cr.set_source(sg)
        cr.fill()
        cr.set_line_width(1.5 * s)  # sand ripples
        for _ in range(int(90 * w / 1920)):
            rx, ry = rng.uniform(0, w), rng.uniform(gt, h)
            ln = rng.uniform(40, 120) * s
            cr.move_to(rx, ry)
            cr.curve_to(rx + ln * 0.3, ry - 4 * s, rx + ln * 0.7, ry + 4 * s, rx + ln, ry)
            cr.set_source_rgba(1, 0.95, 0.8, rng.uniform(0.04, 0.09))
            cr.stroke()
        for _ in range(int(9 * w / 1920)):  # rocks
            rx, ry = rng.uniform(0, w), rng.uniform(gt - h * 0.02, gt + h * 0.1)
            rr = rng.uniform(30, 80) * s
            cr.save()
            cr.translate(rx, ry)
            cr.scale(rr, rr * rng.uniform(0.45, 0.65))
            cr.arc(0, 0, 1, math.pi, TAU)
            cr.restore()
            cr.close_path()
            rg = cairo.LinearGradient(rx, ry - rr * 0.6, rx, ry)
            rg.add_color_stop_rgb(0, 0.22, 0.26, 0.28)
            rg.add_color_stop_rgb(1, 0.08, 0.10, 0.11)
            cr.set_source(rg)
            cr.fill()
            self.anemones.append({"x": rx + rng.uniform(-rr * 0.4, rr * 0.4), "y": ry - rr * 0.45,
                                  "n": rng.randint(7, 12), "len": rng.uniform(18, 34) * s, "ph": rng.uniform(0, TAU),
                                  "c": rng.choice([(1.0, 0.45, 0.55), (1.0, 0.6, 0.3), (0.7, 0.5, 1.0),
                                                   (0.4, 0.9, 0.7)])})
        for _ in range(int(30 * w / 1920)):  # shells and pebbles
            px, py = rng.uniform(0, w), rng.uniform(gt, h)
            cr.set_source_rgba(0.85, 0.8, 0.7, rng.uniform(0.25, 0.5))
            cr.save()
            cr.translate(px, py)
            cr.scale(rng.uniform(3, 7) * s, rng.uniform(2, 4) * s)
            cr.arc(0, 0, 1, 0, TAU)
            cr.restore()
            cr.fill()
        self.caustics = [{"x": rng.uniform(0, w), "y": rng.uniform(gt - 20 * s, h), "r": rng.uniform(40, 110) * s,
                          "ph": rng.uniform(0, TAU), "sp": rng.uniform(0.2, 0.5)} for _ in range(int(26 * w / 1920))]
        self.snow = [[rng.uniform(0, w), rng.uniform(0, h), rng.uniform(0.2, 1.0), rng.uniform(0, TAU)]
                     for _ in range(int(110 * w / 1920))]

    def update_world(self, dt):
        s, w, h = self.s, self.w, self.h
        for p in self.snow:
            p[0] += math.sin(self.t * 0.2 + p[3]) * 5 * s * dt
            p[1] += p[2] * 6 * s * dt
            if p[1] > h:
                p[1] = -5
                p[0] = self.rng.uniform(0, w)
        if self.t > self.next_bubble:
            self.next_bubble = self.t + self.rng.uniform(1.5, 4)
            if self.anemones:
                an = self.rng.choice(self.anemones)
                self.spawn_floater(an["x"], an["y"] - an["len"], "", (1, 1, 1), style="bubble", vy=40, life=9)

    def draw_world(self, cr, t):
        s = self.s
        cr.set_source_surface(self.bg, 0, 0)
        cr.paint()
        for k in range(5):  # light rays from above
            x0 = self.w * (0.1 + k * 0.2) + math.sin(t * 0.1 + k) * self.w * 0.04
            gr = cairo.LinearGradient(0, 0, 0, self.ground_top)
            gr.add_color_stop_rgba(0, 0.6, 0.9, 1.0, 0.06)
            gr.add_color_stop_rgba(1, 0.6, 0.9, 1.0, 0)
            cr.move_to(x0 - 30 * s, 0)
            cr.line_to(x0 + 30 * s, 0)
            cr.line_to(x0 + 160 * s, self.ground_top)
            cr.line_to(x0 + 20 * s, self.ground_top)
            cr.close_path()
            cr.set_source(gr)
            cr.fill()
        for cst in self.caustics:  # dancing light on the sand
            cx = cst["x"] + math.sin(t * cst["sp"] + cst["ph"]) * 30 * s
            cy = cst["y"] + math.cos(t * cst["sp"] * 0.8 + cst["ph"]) * 10 * s
            r = cst["r"] * (0.8 + 0.2 * math.sin(t * 0.7 + cst["ph"]))
            g = cairo.RadialGradient(cx, cy, r * 0.2, cx, cy, r)
            g.add_color_stop_rgba(0, 0.8, 1.0, 0.95, 0.07)
            g.add_color_stop_rgba(1, 0.8, 1.0, 0.95, 0)
            cr.save()
            cr.translate(cx, cy)
            cr.scale(1.0, 0.35)
            cr.translate(-cx, -cy)
            cr.set_source(g)
            cr.arc(cx, cy, r, 0, TAU)
            cr.fill()
            cr.restore()
        cr.set_line_cap(cairo.LINE_CAP_ROUND)
        for an in self.anemones:
            c = an["c"]
            for i in range(an["n"]):
                f = i / (an["n"] - 1) - 0.5
                sway = math.sin(t * 1.4 + an["ph"] + i * 0.5) * 8 * s
                cr.move_to(an["x"] + f * 16 * s, an["y"])
                cr.curve_to(an["x"] + f * 30 * s, an["y"] - an["len"] * 0.5,
                            an["x"] + f * 40 * s + sway, an["y"] - an["len"] * 0.8,
                            an["x"] + f * 46 * s + sway, an["y"] - an["len"])
                cr.set_source_rgba(c[0], c[1], c[2], 0.75)
                cr.set_line_width(4 * s)
                cr.stroke()
        for x, y, z, ph in self.snow:
            cr.set_source_rgba(0.85, 0.95, 1.0, 0.1 + 0.3 * z)
            cr.arc(x, y, (0.6 + 1.2 * z) * s, 0, TAU)
            cr.fill()
        for a in self.sorted_actors():
            a.draw(cr, t)
