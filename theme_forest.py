"""Forest theme: every session is a deer in a moonlit forest clearing.

Deer are dark silhouettes with a rim light in the state colour.
working = walks briskly, fireflies follow it, every tool call sends up a labelled firefly
idle    = wanders, stops and grazes      waiting = comes to the front, head up, ears up
"""
import math

import cairo

from scene import TAU, Scene, Walker, lerp, pulse_rings

BODY = (0.06, 0.05, 0.05)


class Deer(Walker):
    BASE_RADIUS = 54
    SPEED = {"working": 65, "idle": 18, "waiting": 40, "blocked": 0}

    def __init__(self, scene, info, x, y):
        super().__init__(scene, info, x, y)
        self.antlers = self.rng.random() < 0.55
        self.fly_acc = 0.0

    def move(self, dt, t, target):
        super().move(dt, t, target)
        if self.state == "working" and not self.dying:
            self.fly_acc += dt
            if self.fly_acc > 0.6:
                self.fly_acc = 0.0
                sc = self.scene
                sc.fireflies.append({"x": self.x + self.rng.uniform(-self.radius, self.radius),
                                     "y": self.y - self.rng.uniform(0, self.radius * 1.5),
                                     "vx": self.rng.uniform(-15, 15) * sc.s, "vy": -self.rng.uniform(5, 20) * sc.s,
                                     "age": 0.0, "life": self.rng.uniform(3, 6), "ph": self.rng.uniform(0, TAU)})

    def _shape(self, cr, t, R, extra, rgba):
        """The whole deer in local coordinates; extra > 0 draws a wider copy used as the rim light."""
        cr.set_source_rgba(*rgba)
        cr.set_line_cap(cairo.LINE_CAP_ROUND)
        cr.set_line_join(cairo.LINE_JOIN_ROUND)
        # legs
        lying = self.state == "rest"
        cr.set_line_width(R * 0.09 + extra)
        if lying:  # legs folded under the body
            for x0, x1 in ((0.35, 0.8), (-0.55, -0.1)):
                cr.move_to(x0 * R, R * 0.2)
                cr.line_to(x1 * R, R * 0.34)
        else:
            for i, lx in enumerate((-0.6, -0.42, 0.42, 0.6)):
                sw = math.sin(self.step + (0 if i in (0, 3) else math.pi)) * 0.4 * self.moving
                knee = (lx * R + math.sin(sw) * R * 0.25, R * 0.65)
                cr.move_to(lx * R, R * 0.1)
                cr.line_to(*knee)
                cr.line_to(knee[0] + math.sin(sw * 1.4) * R * 0.2, R * 1.15)
        cr.stroke()
        # body: barrel with rounded chest and rump
        cr.save()
        cr.scale(R * 0.85, R * 0.34)
        cr.arc(0, 0, 1, 0, TAU)
        cr.restore()
        cr.new_sub_path()
        cr.arc(R * 0.52, R * 0.0, R * 0.32, 0, TAU)
        cr.new_sub_path()
        cr.arc(-R * 0.58, -R * 0.04, R * 0.30, 0, TAU)
        if extra:
            cr.set_line_width(extra * 2)
            cr.stroke_preserve()
        cr.fill()
        # neck + head
        alert = 1.0 if self.state == "waiting" else 0.0
        hx = R * lerp(0.98, 1.15, self.graze)
        hy = R * lerp(-0.72 - 0.1 * alert, 0.85, self.graze)
        if lying:  # resting, head up, slowly looking around
            hx, hy = R * 0.9, R * (-0.55 + 0.03 * math.sin(t * 0.5 + self.seed))
        cr.set_line_width(R * 0.26 + extra)
        cr.move_to(R * 0.58, -R * 0.05)
        cr.line_to(hx - R * 0.06, hy + R * 0.06)
        cr.stroke()
        ang = lerp(0.35, 1.3, self.graze)
        cr.save()
        cr.translate(hx, hy)
        cr.rotate(ang)
        cr.save()
        cr.scale(R * 0.30 + extra, R * 0.15 + extra)
        cr.arc(0.4, 0, 1, 0, TAU)
        cr.restore()
        cr.fill()
        twitch = math.sin(t * 3 + self.seed) * 0.15 + alert * 0.3
        for side in (-1, 1):  # ears
            cr.save()
            cr.translate(-R * 0.05, -R * 0.12)
            cr.rotate(-1.9 + side * 0.35 - twitch * side)
            cr.scale(R * 0.22 + extra, R * 0.08 + extra)
            cr.arc(0.8, 0, 1, 0, TAU)
            cr.restore()
            cr.fill()
        if self.antlers:
            cr.set_line_width(R * 0.05 + extra)
            for side in (-1, 1):
                bx, by = -R * 0.02 + side * R * 0.05, -R * 0.15
                cr.move_to(bx, by)
                tipx, tipy = bx - R * 0.35 + side * R * 0.1, by - R * 0.6
                cr.curve_to(bx - R * 0.1, by - R * 0.3, tipx + R * 0.1, tipy + R * 0.2, tipx, tipy)
                cr.move_to(bx - R * 0.12, by - R * 0.3)
                cr.line_to(bx + R * 0.08, by - R * 0.48)
                cr.move_to(bx - R * 0.25, by - R * 0.45)
                cr.line_to(bx - R * 0.08, by - R * 0.65)
            cr.stroke()
        cr.restore()

    def draw(self, cr, t):
        s = self.scene.s
        a = self.alpha * self.flicker
        if a < 0.01:
            return
        R, c, st = self.radius, self.color, self.state
        g = cairo.RadialGradient(self.x, self.y, 0, self.x, self.y, R * 2.4)
        g.add_color_stop_rgba(0, c[0], c[1], c[2], (0.3 if st != "idle" else 0.16) * a)
        g.add_color_stop_rgba(1, c[0], c[1], c[2], 0)
        cr.set_source(g)
        cr.arc(self.x, self.y, R * 2.4, 0, TAU)
        cr.fill()
        if st == "waiting":
            pulse_rings(cr, self.x, self.y - R * 0.2, R, c, a, t, s)
        cr.save()
        cr.translate(self.x, self.y + R * 1.15)
        cr.scale(R * 1.1, R * 0.12)
        cr.arc(0, 0, 1, 0, TAU)
        cr.restore()
        cr.set_source_rgba(0, 0, 0, 0.35 * a)
        cr.fill()
        cr.save()
        cr.translate(self.x, self.y + (R * 0.62 if st == "rest" else 0.0))  # lying deer sits lower
        cr.scale(self.face if abs(self.face) > 0.15 else 0.15, 1.0)
        self._shape(cr, t, R, 3.0 * s, (c[0], c[1], c[2], 0.85 * a))
        self._shape(cr, t, R, 0.0, (BODY[0], BODY[1], BODY[2], a))
        cr.set_source_rgba(0.95, 0.92, 0.88, 0.85 * a)  # white tail
        cr.save()
        cr.translate(-R * 0.95, -R * 0.12)
        cr.rotate(-0.6 + math.sin(t * 2.5 + self.seed) * 0.15)
        cr.scale(R * 0.12, R * 0.07)
        cr.arc(0, 0, 1, 0, TAU)
        cr.restore()
        cr.fill()
        cr.restore()


class Forest(Scene):
    name = "forest"
    actor_class = Deer
    label_bg = (0.02, 0.03, 0.03)
    empty_color = (0.85, 0.95, 0.8)
    waiting_row = 0.76

    def __init__(self, **kw):
        super().__init__(**kw)
        self.bg = None
        self.fireflies = []
        self.ferns = []

    def spawn_point(self):
        return self.rng.uniform(self.w * 0.15, self.w * 0.85), self.rng.uniform(self.ground_top + 40, self.h * 0.85)

    def on_tool(self, actor, name, index):
        self.spawn_floater(actor.x, actor.y - actor.radius * 1.6 - index * 26 * self.s, name, actor.color,
                           style="firefly", vy=30, life=4.5)

    @staticmethod
    def _pine(cr, x, base, height, width):
        cr.move_to(x, base - height)
        tiers = 5
        for k in range(tiers):  # jagged right side
            f = (k + 1) / tiers
            cr.line_to(x + width * f * 0.5, base - height * (1 - f) - height * 0.02)
            cr.line_to(x + width * f * 0.28, base - height * (1 - f) + height * 0.03 * (k < tiers - 1))
        cr.line_to(x + width * 0.06, base)
        cr.line_to(x - width * 0.06, base)
        for k in reversed(range(tiers)):
            f = (k + 1) / tiers
            cr.line_to(x - width * f * 0.28, base - height * (1 - f) + height * 0.03 * (k < tiers - 1))
            cr.line_to(x - width * f * 0.5, base - height * (1 - f) - height * 0.02)
        cr.close_path()

    def setup_world(self):
        w, h, s, rng = self.w, self.h, self.s, self.rng
        self.ground_top = gt = h * 0.60
        self.bg = cairo.ImageSurface(cairo.FORMAT_RGB24, w, h)
        cr = cairo.Context(self.bg)
        g = cairo.LinearGradient(0, 0, 0, gt)
        g.add_color_stop_rgb(0, 0.02, 0.03, 0.08)
        g.add_color_stop_rgb(1, 0.10, 0.14, 0.22)
        cr.set_source(g)
        cr.paint()
        for _ in range(int(260 * w / 1920)):
            cr.set_source_rgba(0.9, 0.92, 1.0, rng.uniform(0.1, 0.6))
            cr.arc(rng.uniform(0, w), rng.uniform(0, gt * 0.7), rng.uniform(0.4, 1.2) * s, 0, TAU)
            cr.fill()
        mx, my, mr = w * 0.22, h * 0.15, 42 * s  # moon
        mg = cairo.RadialGradient(mx, my, mr, mx, my, mr * 6)
        mg.add_color_stop_rgba(0, 0.8, 0.85, 1.0, 0.25)
        mg.add_color_stop_rgba(1, 0.8, 0.85, 1.0, 0)
        cr.set_source(mg)
        cr.paint()
        cr.set_source_rgb(0.93, 0.93, 0.86)
        cr.arc(mx, my, mr, 0, TAU)
        cr.fill()
        cr.set_source_rgba(0.75, 0.75, 0.7, 0.5)
        for ox, oy, rr in ((-0.3, -0.2, 0.18), (0.25, 0.15, 0.12), (0.05, 0.4, 0.09)):
            cr.arc(mx + ox * mr, my + oy * mr, rr * mr, 0, TAU)
            cr.new_sub_path()
        cr.fill()
        # ground of the clearing
        fg = cairo.LinearGradient(0, gt - h * 0.05, 0, h)
        fg.add_color_stop_rgb(0, 0.09, 0.12, 0.10)
        fg.add_color_stop_rgb(1, 0.03, 0.05, 0.03)
        cr.set_source(fg)
        cr.rectangle(0, gt - h * 0.05, w, h)
        cr.fill()
        # tree layers, far to near, with fog between them
        layers = [(0.13, 0.20, (0.10, 0.15, 0.20), 34), (0.09, 0.28, (0.06, 0.10, 0.12), 26),
                  (0.06, 0.40, (0.03, 0.05, 0.06), 16)]
        for li, (gap, hh, col, n) in enumerate(layers):
            base = gt - h * 0.035 + li * h * 0.012
            for _ in range(int(n * w / 1920)):
                x = rng.uniform(-30, w + 30)
                if li == 2 and w * 0.22 < x < w * 0.78:
                    continue  # keep the clearing open
                ht = h * hh * rng.uniform(0.7, 1.25)
                self._pine(cr, x, base, ht, ht * rng.uniform(0.38, 0.5))
            cr.set_source_rgb(*col)
            cr.fill()
            fog = cairo.LinearGradient(0, base - h * 0.12, 0, base + h * 0.02)
            fog.add_color_stop_rgba(0, 0.6, 0.7, 0.8, 0)
            fog.add_color_stop_rgba(1, 0.6, 0.7, 0.8, 0.10 - li * 0.02)
            cr.set_source(fog)
            cr.rectangle(0, base - h * 0.12, w, h * 0.14)
            cr.fill()
        for _ in range(int(60 * w / 1920)):  # moss and stones in the clearing
            px, py = rng.uniform(0, w), rng.uniform(gt, h)
            cr.set_source_rgba(0.15, 0.22, 0.14, rng.uniform(0.3, 0.6))
            cr.save()
            cr.translate(px, py)
            cr.scale(rng.uniform(8, 30) * s, rng.uniform(2, 6) * s)
            cr.arc(0, 0, 1, 0, TAU)
            cr.restore()
            cr.fill()
        self.fireflies = [{"x": rng.uniform(0, w), "y": rng.uniform(gt - h * 0.2, h * 0.9),
                           "vx": rng.uniform(-10, 10) * s, "vy": rng.uniform(-6, 6) * s, "age": 0.0,
                           "life": 1e9, "ph": rng.uniform(0, TAU)} for _ in range(int(28 * w / 1920))]
        self.ferns = [(rng.uniform(0, w), h - rng.uniform(0, 30) * s, rng.uniform(40, 90) * s, rng.uniform(0, TAU))
                      for _ in range(int(26 * w / 1920))]

    def update_world(self, dt):
        s, w, h, t = self.s, self.w, self.h, self.t
        for ff in self.fireflies:
            ff["age"] += dt
            ff["vx"] += math.sin(t * 0.7 + ff["ph"]) * 6 * s * dt
            ff["vy"] += math.cos(t * 0.9 + ff["ph"] * 2) * 6 * s * dt
            ff["vx"] *= 0.99
            ff["vy"] *= 0.99
            ff["x"] = (ff["x"] + ff["vx"] * dt) % w
            ff["y"] = min(h * 0.95, max(h * 0.3, ff["y"] + ff["vy"] * dt))
        self.fireflies = [ff for ff in self.fireflies if ff["age"] < ff["life"]]

    def draw_world(self, cr, t):
        s = self.s
        cr.set_source_surface(self.bg, 0, 0)
        cr.paint()
        for a in self.sorted_actors():
            a.draw(cr, t)
        for ff in self.fireflies:
            fade = min(1.0, ff["age"]) * min(1.0, (ff["life"] - ff["age"]))
            b = max(0.0, math.sin(t * 1.7 + ff["ph"] * 3)) * fade
            if b < 0.05:
                continue
            g = cairo.RadialGradient(ff["x"], ff["y"], 0, ff["x"], ff["y"], 10 * s)
            g.add_color_stop_rgba(0, 1.0, 0.95, 0.55, 0.9 * b)
            g.add_color_stop_rgba(1, 1.0, 0.9, 0.4, 0)
            cr.set_source(g)
            cr.arc(ff["x"], ff["y"], 10 * s, 0, TAU)
            cr.fill()
        cr.set_line_cap(cairo.LINE_CAP_ROUND)
        cr.set_source_rgba(0.02, 0.03, 0.02, 1.0)
        for x, y, ln, ph in self.ferns:  # foreground ferns
            for k in range(-3, 4):
                ang = -math.pi / 2 + k * 0.28 + math.sin(t * 0.8 + ph) * 0.06
                cr.move_to(x, y)
                cr.curve_to(x + math.cos(ang) * ln * 0.4, y + math.sin(ang) * ln * 0.5,
                            x + math.cos(ang) * ln * 0.8, y + math.sin(ang) * ln * 0.8,
                            x + math.cos(ang + 0.4 * (1 if k >= 0 else -1)) * ln, y + math.sin(ang) * ln * 0.75)
            cr.set_line_width(5 * s)
            cr.stroke()
