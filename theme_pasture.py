"""Pasture theme: every session is a sheep on an evening meadow - one session one sheep, many a flock.

working = trots around, hops on every tool call (the tool name floats up)
idle    = wanders a bit, stops and grazes      waiting = comes to the front and bleats "Baa!"
State colour shows as the sheep's bell, a tint in the wool and a glow on the grass.
"""
import math

import cairo

from scene import TAU, Scene, Text, Walker, lerp, lerp3, lighten, pulse_rings, Weight

WOOL = (0.96, 0.95, 0.91)
SKIN = (0.18, 0.16, 0.15)
WOOL_BLOBS = [(-0.55, -0.05), (-0.3, -0.32), (0.05, -0.38), (0.38, -0.3), (0.6, -0.05),
              (0.42, 0.22), (0.05, 0.28), (-0.35, 0.22), (0.0, -0.02)]


class Sheep(Walker):
    BASE_RADIUS = 54
    SPEED = {"working": 70, "idle": 20, "waiting": 45, "blocked": 0}

    def draw(self, cr, t):
        s = self.scene.s
        a = self.alpha * self.flicker
        if a < 0.01:
            return
        R, c, st = self.radius, self.color, self.state
        hop = math.sin(math.pi * self.hop) * R * 0.35 if self.hop > 0 else 0.0
        # glow on the grass + shadow
        g = cairo.RadialGradient(self.x, self.y + R * 0.95, 0, self.x, self.y + R * 0.95, R * 1.6)
        g.add_color_stop_rgba(0, c[0], c[1], c[2], (0.45 if st != "idle" else 0.25) * a)
        g.add_color_stop_rgba(1, c[0], c[1], c[2], 0)
        cr.set_source(g)
        cr.save()
        cr.translate(self.x, self.y + R * 0.95)
        cr.scale(1.0, 0.3)
        cr.arc(0, 0, R * 1.6, 0, TAU)
        cr.restore()
        cr.fill()
        cr.save()
        cr.translate(self.x, self.y + R * 0.95)
        cr.scale(R * 0.95, R * 0.16)
        cr.arc(0, 0, 1, 0, TAU)
        cr.restore()
        cr.set_source_rgba(0, 0, 0, 0.25 * a)
        cr.fill()
        if st == "waiting":
            pulse_rings(cr, self.x, self.y, R, c, a, t, s)

        lying = st == "rest"  # asleep in the grass while no session is running
        cr.save()
        cr.translate(self.x, self.y - hop + (R * 0.4 if lying else 0.0))
        cr.scale(self.face if abs(self.face) > 0.15 else 0.15, 1.0)
        # legs (tucked away when lying)
        cr.set_line_cap(cairo.LINE_CAP_ROUND)
        cr.set_line_width(R * 0.13)
        cr.set_source_rgba(SKIN[0], SKIN[1], SKIN[2], a)
        if not lying:
            for i, lx in enumerate((-0.5, -0.25, 0.3, 0.55)):
                sw = math.sin(self.step + (0 if i % 2 else math.pi)) * 0.35 * self.moving
                cr.move_to(lx * R, R * 0.35)
                cr.line_to(lx * R + math.sin(sw) * R * 0.55, R * 0.35 + math.cos(sw) * R * 0.6)
            cr.stroke()
        # wool: darker outline pass, then the fluffy fill
        wool = lerp3(WOOL, c, 0.22)
        for grow, col, al in ((0.05, lerp3(wool, (0.4, 0.4, 0.4), 0.45), 1.0), (0.0, wool, 1.0)):
            for bx, by in WOOL_BLOBS:
                breathe = 1 + 0.03 * math.sin(t * 1.5 + bx * 3)
                cr.arc(bx * R, by * R, R * (0.36 + grow) * breathe, 0, TAU)
                cr.new_sub_path()
            cr.set_source_rgba(col[0], col[1], col[2], al * a)
            cr.fill()
        sh = cairo.RadialGradient(-R * 0.2, -R * 0.4, R * 0.1, 0, 0, R * 1.1)
        sh.add_color_stop_rgba(0, 1, 1, 1, 0.25 * a)
        sh.add_color_stop_rgba(1, 0, 0, 0, 0.18 * a)
        for bx, by in WOOL_BLOBS:
            cr.arc(bx * R, by * R, R * 0.36, 0, TAU)
            cr.new_sub_path()
        cr.set_source(sh)
        cr.fill()
        # head: up when walking/alert, down in the grass when grazing
        chew = math.sin(t * 9) * 0.03 * self.graze
        hx = R * (0.82 + 0.15 * self.graze)
        hy = R * (-0.25 + 0.95 * self.graze + chew)
        ang = lerp(-0.25, 1.1, self.graze)
        if lying:  # chin resting on the grass, breathing slowly
            hx, hy, ang = R * 0.9, R * (0.22 + 0.02 * math.sin(t * 1.2)), 0.25
        cr.save()
        cr.translate(hx, hy)
        cr.rotate(ang)
        cr.set_source_rgba(SKIN[0], SKIN[1], SKIN[2], a)
        cr.save()
        cr.scale(R * 0.30, R * 0.19)
        cr.arc(0.35, 0, 1, 0, TAU)
        cr.restore()
        cr.fill()
        ear = math.sin(t * 2 + self.seed) * 0.15
        for side in (-1, 1):
            cr.save()
            cr.translate(-R * 0.05, side * R * 0.12)
            cr.rotate(side * (0.9 + ear))
            cr.scale(R * 0.18, R * 0.07)
            cr.arc(-0.6, 0, 1, 0, TAU)
            cr.restore()
            cr.fill()
        cr.set_source_rgba(WOOL[0], WOOL[1], WOOL[2], a)  # wool tuft on the head
        cr.arc(-R * 0.05, -R * 0.14, R * 0.15, 0, TAU)
        cr.fill()
        if lying:  # closed eye
            cr.set_source_rgba(0.85, 0.85, 0.85, 0.8 * a)
            cr.set_line_width(R * 0.025)
            cr.arc(R * 0.22, -R * 0.08, R * 0.05, 0.2, math.pi - 0.2)
            cr.stroke()
        else:
            cr.set_source_rgba(1, 1, 1, 0.9 * a)
            cr.arc(R * 0.22, -R * 0.06, R * 0.05, 0, TAU)
            cr.fill()
        cr.restore()
        # bell in the state colour
        bc = lighten(c, 0.2)
        cr.set_source_rgba(bc[0], bc[1], bc[2], a)
        cr.arc(R * 0.62, R * 0.12, R * 0.10, 0, TAU)
        cr.fill()
        cr.restore()

        if lying:  # z z z
            T = self.scene.text
            side = 1 if self.face >= 0 else -1
            for k in range(3):
                f = (t * 0.3 + k / 3) % 1.0
                lay = T.layout(cr, "z", (14 + 14 * f) * s, Weight.BOLD)
                zx = self.x + side * R * (0.9 + 0.6 * f) + math.sin(f * 6) * 4 * s
                zy = self.y + R * 0.1 - f * R * 1.6
                Text.show(cr, lay, zx, zy, (1, 1, 1), 0.8 * math.sin(math.pi * f) * a, shadow=0.4)

        if st == "waiting":  # speech bubble
            T = self.scene.text
            lay = T.layout(cr, "Baa!", 20 * s, Weight.BOLD)
            tw, th = lay.get_pixel_size()
            bx, by = self.x - tw / 2 - R * 0.2, self.y - R * 1.55 - th + math.sin(t * 4) * 3 * s
            pad = 8 * s
            cr.set_source_rgba(1, 1, 1, 0.92 * a)
            cr.save()
            cr.translate(bx + tw / 2, by + th / 2)
            cr.scale((tw / 2 + pad) * 1.15, th / 2 + pad)
            cr.arc(0, 0, 1, 0, TAU)
            cr.restore()
            cr.fill()
            cr.move_to(bx + tw / 2 - 6 * s, by + th + pad * 0.6)
            cr.line_to(bx + tw / 2 + 4 * s, by + th + pad * 0.6)
            cr.line_to(bx + tw / 2 + 8 * s, by + th + pad * 2.2)
            cr.close_path()
            cr.fill()
            Text.show(cr, lay, bx, by, (0.55, 0.32, 0.0), a)


class Pasture(Scene):
    name = "pasture"
    actor_class = Sheep
    label_bg = (0.04, 0.07, 0.04)
    empty_color = (1.0, 0.95, 0.8)
    waiting_row = 0.80

    def __init__(self, **kw):
        super().__init__(**kw)
        self.bg = None
        self.clouds = []
        self.tufts = []
        self.birds = None
        self.next_birds = 12.0

    def spawn_point(self):
        return self.rng.uniform(self.w * 0.15, self.w * 0.85), self.rng.uniform(self.ground_top + 40, self.h * 0.85)

    def on_tool(self, actor, name, index):
        actor.hop = 1.0
        self.spawn_floater(actor.x, actor.y - actor.radius * 1.1 - index * 26 * self.s, name, actor.color,
                           style="text", vy=35, life=3.5)

    def _hill(self, x, base, amp, f1, p1, f2, p2):
        w = self.w
        return base - amp * (0.6 * math.sin(x / w * f1 + p1) + 0.4 * math.sin(x / w * f2 + p2))

    def setup_world(self):
        w, h, s, rng = self.w, self.h, self.s, self.rng
        self.ground_top = h * 0.58
        hz = h * 0.47
        self.bg = cairo.ImageSurface(cairo.FORMAT_RGB24, w, h)
        cr = cairo.Context(self.bg)
        g = cairo.LinearGradient(0, 0, 0, hz)
        g.add_color_stop_rgb(0, 0.16, 0.22, 0.42)
        g.add_color_stop_rgb(0.55, 0.52, 0.45, 0.58)
        g.add_color_stop_rgb(1, 0.95, 0.66, 0.45)
        cr.set_source(g)
        cr.rectangle(0, 0, w, h)
        cr.fill()
        sx, sy = w * 0.76, hz - h * 0.04
        sg = cairo.RadialGradient(sx, sy, 0, sx, sy, h * 0.35)
        sg.add_color_stop_rgba(0, 1.0, 0.92, 0.7, 0.9)
        sg.add_color_stop_rgba(0.12, 1.0, 0.85, 0.55, 0.6)
        sg.add_color_stop_rgba(1, 1.0, 0.7, 0.4, 0)
        cr.set_source(sg)
        cr.paint()
        layers = [(hz + h * 0.02, h * 0.05, (0.36, 0.42, 0.40), 3.0, 1.0, 9.0, 0.3),
                  (hz + h * 0.06, h * 0.045, (0.26, 0.40, 0.26), 4.0, 2.0, 11.0, 1.2),
                  (self.ground_top - h * 0.01, h * 0.03, (0.20, 0.38, 0.16), 5.0, 0.4, 13.0, 2.0)]
        for li, (base, amp, col, f1, p1, f2, p2) in enumerate(layers):
            cr.move_to(0, h)
            for xi in range(0, w + 9, 8):
                cr.line_to(xi, self._hill(xi, base, amp, f1, p1, f2, p2))
            cr.line_to(w, h)
            cr.close_path()
            if li == 2:
                mg = cairo.LinearGradient(0, base - amp, 0, h)
                mg.add_color_stop_rgb(0, 0.30, 0.50, 0.20)
                mg.add_color_stop_rgb(1, 0.10, 0.25, 0.08)
                cr.set_source(mg)
            else:
                cr.set_source_rgb(*col)
            cr.fill()
            if li < 2:  # trees on the far hills
                for _ in range(rng.randint(4, 8)):
                    tx = rng.uniform(0, w)
                    ty = self._hill(tx, base, amp, f1, p1, f2, p2)
                    tr = rng.uniform(10, 22) * s * (0.7 if li == 0 else 1.0)
                    dark = lerp3(col, (0.05, 0.12, 0.08), 0.5)
                    cr.set_source_rgb(*dark)
                    cr.rectangle(tx - tr * 0.12, ty - tr * 1.2, tr * 0.24, tr * 1.2)
                    cr.fill()
                    cr.arc(tx, ty - tr * 1.5, tr, 0, TAU)
                    cr.fill()
        # fence along the back of the meadow
        fy = self.ground_top - h * 0.005
        cr.set_source_rgba(0.30, 0.22, 0.15, 0.9)
        for xi in range(int(-20 * s), w + 40, int(110 * s)):
            cr.rectangle(xi, fy - 46 * s, 7 * s, 50 * s)
        cr.fill()
        cr.set_line_width(4 * s)
        for ry in (fy - 38 * s, fy - 20 * s):
            cr.move_to(0, ry)
            cr.line_to(w, ry)
        cr.stroke()
        for _ in range(int(260 * w / 1920)):  # flowers
            fx, fy2 = rng.uniform(0, w), rng.uniform(self.ground_top + 10 * s, h)
            col = rng.choice([(1, 1, 0.95), (1, 0.9, 0.3), (0.95, 0.6, 0.8), (0.75, 0.75, 1.0)])
            cr.set_source_rgba(col[0], col[1], col[2], 0.8)
            cr.arc(fx, fy2, rng.uniform(1.5, 3.2) * s * (0.6 + 0.6 * (fy2 - self.ground_top) / (h - self.ground_top)), 0, TAU)
            cr.fill()
        self.clouds = [{"x": rng.uniform(0, w), "y": rng.uniform(h * 0.05, h * 0.3), "sc": rng.uniform(0.6, 1.3),
                        "sp": rng.uniform(4, 10), "blobs": [(rng.uniform(-1.6, 1.6), rng.uniform(-0.4, 0.3),
                                                            rng.uniform(0.5, 1.0)) for _ in range(6)]}
                       for _ in range(6)]
        self.tufts = sorted(([rng.uniform(0, w), rng.uniform(self.ground_top + 5 * s, h), rng.uniform(0, TAU)]
                             for _ in range(int(150 * w / 1920))), key=lambda tf: tf[1])

    def update_world(self, dt):
        s, w = self.s, self.w
        for c in self.clouds:
            c["x"] += c["sp"] * s * dt
            if c["x"] - 200 * s * c["sc"] > w:
                c["x"] = -200 * s * c["sc"]
        if self.t > self.next_birds:
            self.next_birds = self.t + self.rng.uniform(20, 45)
            self.birds = {"x": -50.0, "y": self.rng.uniform(self.h * 0.08, self.h * 0.3), "age": 0.0}
        if self.birds:
            self.birds["x"] += 90 * s * dt
            self.birds["age"] += dt
            if self.birds["x"] > w + 100:
                self.birds = None

    def _draw_tuft(self, cr, x, y, ph, t):
        s = self.s
        k = 0.6 + 0.6 * (y - self.ground_top) / max(1.0, self.h - self.ground_top)
        cr.set_line_width(2 * s * k)
        for i in (-1, 0, 1):
            sway = math.sin(t * 1.3 + ph + x * 0.01) * 4 * s * k
            cr.move_to(x + i * 4 * s * k, y)
            cr.curve_to(x + i * 5 * s * k, y - 8 * s * k, x + i * 7 * s * k + sway, y - 14 * s * k,
                        x + i * 9 * s * k + sway, y - 20 * s * k)
        cr.set_source_rgba(0.16, 0.36, 0.12, 0.9)
        cr.stroke()

    def draw_world(self, cr, t):
        s = self.s
        cr.set_source_surface(self.bg, 0, 0)
        cr.paint()
        for c in self.clouds:
            for bx, by, br in c["blobs"]:
                cr.arc(c["x"] + bx * 60 * s * c["sc"], c["y"] + by * 60 * s * c["sc"], br * 50 * s * c["sc"], 0, TAU)
                cr.new_sub_path()
            cr.set_source_rgba(0.95, 0.80, 0.80, 0.85)
            cr.fill()
        if self.birds:
            b = self.birds
            cr.set_line_width(2 * s)
            cr.set_source_rgba(0.15, 0.12, 0.2, 0.7)
            for i, (ox, oy) in enumerate(((0, 0), (-30, 14), (-34, -12), (-62, 6), (-66, -22))):
                x, y = b["x"] + ox * s, b["y"] + oy * s
                flap = math.sin(b["age"] * 8 + i) * 5 * s
                cr.move_to(x - 9 * s, y - flap)
                cr.line_to(x, y)
                cr.line_to(x + 9 * s, y - flap)
            cr.stroke()
        actors = self.sorted_actors()
        ti = 0
        for a in actors:  # tufts and sheep sorted by depth
            while ti < len(self.tufts) and self.tufts[ti][1] < a.y + a.radius:
                self._draw_tuft(cr, *self.tufts[ti], t)
                ti += 1
            a.draw(cr, t)
        while ti < len(self.tufts):
            self._draw_tuft(cr, *self.tufts[ti], t)
            ti += 1
