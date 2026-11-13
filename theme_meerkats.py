"""Meerkats theme: a savanna at sunset with burrows.

working = scurries on all fours and digs; every tool call throws up sand with the tool name
idle    = sits half upright and basks, now and then wanders
waiting = stands upright as the sentinel at the front, turns its head, pulses
rest    = only the head peeks out of a burrow, blinking
The fur stays natural; the state colour shows as a rim light, a glow on the sand and a tint.
"""
import math

import cairo

from scene import TAU, Scene, Walker, clamp, lerp, lerp3, lighten, pulse_rings

FUR = (0.80, 0.64, 0.44)
DARK = (0.22, 0.15, 0.10)


class Meerkat(Walker):
    BASE_RADIUS = 60
    SPEED = {"working": 95, "idle": 25, "waiting": 50, "blocked": 0}

    def __init__(self, scene, info, x, y):
        super().__init__(scene, info, x, y)
        self.upright = 0.0
        self.dig = 0.0
        self.head_turn = 0.0
        self.spray_acc = 0.0
        self.blink_at = self.rng.uniform(1, 4)

    def move(self, dt, t, target):
        st = self.state
        if st == "working":  # scurry, stop and dig, scurry...
            self.dig = clamp(math.sin(t * 0.5 + self.seed) * 3 - 1.5, 0.0, 1.0)
        else:
            self.dig = 0.0
        super().move(dt, t, target)
        if self.dig > 0.5:
            self.x -= self.vx * dt * 0.9  # (almost) stands still while digging
            self.y -= self.vy * dt * 0.9
        want = {"waiting": 1.0, "idle": 0.62, "working": 0.0, "blocked": 0.4, "rest": 0.0}.get(st, 0.0)
        if st == "idle" and self.moving > 0.3:
            want = 0.0
        self.upright = lerp(self.upright, want, min(1.0, dt * 2.5))
        self.head_turn = math.sin(t * (1.6 if st == "waiting" else 0.5) + self.seed) * (1.0 if st == "waiting" else 0.4)
        if self.dig > 0.5 and not self.dying:  # sand spray behind
            self.spray_acc += dt
            sc = self.scene
            while self.spray_acc > 0.03:
                self.spray_acc -= 0.03
                side = -1 if self.face >= 0 else 1
                sc.sand.append({"x": self.x + side * self.radius * 0.5, "y": self.y + self.radius * 0.6,
                                "vx": side * self.rng.uniform(60, 160) * sc.s, "vy": -self.rng.uniform(60, 180) * sc.s,
                                "age": 0.0, "life": self.rng.uniform(0.6, 1.1)})
        if t > self.blink_at + 0.2:
            self.blink_at = t + self.rng.uniform(2, 6)

    def draw(self, cr, t):
        sc, s = self.scene, self.scene.s
        a = self.alpha * self.flicker
        if a < 0.01:
            return
        R, c, st = self.radius, self.color, self.state
        if st == "rest":
            self._draw_peeking(cr, t, R, c, a)
            return
        gy = self.y + R * 0.95
        g = cairo.RadialGradient(self.x, gy, 0, self.x, gy, R * 1.8)
        g.add_color_stop_rgba(0, c[0], c[1], c[2], (0.45 if st != "idle" else 0.25) * a)
        g.add_color_stop_rgba(1, c[0], c[1], c[2], 0)
        cr.save()
        cr.translate(self.x, gy)
        cr.scale(1.0, 0.3)
        cr.translate(-self.x, -gy)
        cr.set_source(g)
        cr.arc(self.x, gy, R * 1.8, 0, TAU)
        cr.fill()
        cr.restore()
        cr.save()
        cr.translate(self.x, gy)
        cr.scale(R * 0.8, R * 0.14)
        cr.arc(0, 0, 1, 0, TAU)
        cr.restore()
        cr.set_source_rgba(0, 0, 0, 0.3 * a)
        cr.fill()
        if st == "waiting":
            pulse_rings(cr, self.x, self.y - R * 0.3, R, c, a, t, s)
        cr.save()
        cr.translate(self.x, self.y)
        cr.scale(self.face if abs(self.face) > 0.15 else 0.15, 1.0)
        fur = lerp3(FUR, c, 0.18)
        self._shape(cr, t, R, 3.0 * s, (c[0], c[1], c[2], 0.85 * a), None, a)   # rim light
        self._shape(cr, t, R, 0.0, (fur[0], fur[1], fur[2], a), fur, a)
        cr.restore()

    def _shape(self, cr, t, R, extra, rgba, fur, a):
        """The meerkat in local coordinates (feet at y = +0.95 R). upright: 0 = on all fours, 1 = standing."""
        u = self.upright
        cr.set_source_rgba(*rgba)
        cr.set_line_cap(cairo.LINE_CAP_ROUND)
        cr.set_line_join(cairo.LINE_JOIN_ROUND)
        # body axis from hips to shoulders: horizontal when walking, vertical when standing
        hip = (lerp(-R * 0.45, -R * 0.05, u), lerp(R * 0.45, R * 0.62, u))
        ang = lerp(-0.12, -math.pi / 2 + 0.08, u)
        blen = R * lerp(0.95, 1.05, u)
        sh = (hip[0] + math.cos(ang) * blen, hip[1] + math.sin(ang) * blen)
        # tail
        cr.set_line_width(R * 0.11 + extra)
        cr.move_to(*hip)
        cr.curve_to(hip[0] - R * 0.4, hip[1] + R * 0.1, hip[0] - R * lerp(0.9, 0.55, u), hip[1] + R * lerp(0.2, 0.35, u),
                    hip[0] - R * lerp(1.15, 0.75, u), hip[1] + R * lerp(0.05, 0.33, u))
        cr.stroke()
        # legs
        cr.set_line_width(R * 0.11 + extra)
        step = self.step
        if u < 0.5:
            for i, base in enumerate((hip, sh)):
                for k in (0, 1):
                    sw = math.sin(step + (math.pi if (i + k) % 2 else 0)) * 0.5 * self.moving
                    if self.dig > 0.5 and i == 1:
                        sw = math.sin(t * 22 + k * math.pi) * 0.6  # fast digging front paws
                    cr.move_to(base[0], base[1])
                    cr.line_to(base[0] + math.sin(sw) * R * 0.4, R * 0.95)
            cr.stroke()
        else:  # standing: bent hind legs, front paws hanging in front of the chest
            cr.move_to(hip[0] + R * 0.05, hip[1])
            cr.line_to(hip[0] + R * 0.3, R * 0.8)
            cr.line_to(hip[0] + R * 0.12, R * 0.95)
            cr.stroke()
            mid = (lerp(hip[0], sh[0], 0.75) + R * 0.12, lerp(hip[1], sh[1], 0.75))
            cr.set_line_width(R * 0.08 + extra)
            cr.move_to(*mid)
            cr.line_to(mid[0] + R * 0.18, mid[1] + R * 0.22)
            cr.line_to(mid[0] + R * 0.12, mid[1] + R * 0.3)
            cr.stroke()
        # body
        cr.set_line_width(R * 0.5 + extra * 2)
        cr.move_to(*hip)
        cr.line_to(*sh)
        cr.stroke()
        # head
        hx = sh[0] + math.cos(ang) * R * 0.2 + R * lerp(0.15, 0.05, u)
        hy = sh[1] + math.sin(ang) * R * 0.2 - R * lerp(0.05, 0.08, u)
        cr.save()
        cr.translate(hx, hy)
        cr.rotate(lerp(0.15, 0.0, u) + self.head_turn * 0.15)
        cr.save()
        cr.scale(R * 0.27 + extra, R * 0.21 + extra)
        cr.arc(0, 0, 1, 0, TAU)
        cr.restore()
        cr.fill()
        cr.move_to(R * 0.1, -R * 0.14 - extra)          # pointed snout
        cr.line_to(R * 0.48 + extra, R * 0.02)
        cr.line_to(R * 0.1, R * 0.16 + extra)
        cr.close_path()
        cr.fill()
        if fur is not None:  # details only on the real pass
            cr.set_source_rgba(DARK[0], DARK[1], DARK[2], a)
            cr.arc(R * 0.46, R * 0.02, R * 0.05, 0, TAU)        # nose
            cr.fill()
            cr.save()                                           # dark eye patch
            cr.translate(R * 0.1, -R * 0.03)
            cr.scale(R * 0.11, R * 0.08)
            cr.arc(0, 0, 1, 0, TAU)
            cr.restore()
            cr.fill()
            cr.arc(-R * 0.14, -R * 0.15, R * 0.07, 0, TAU)      # ear
            cr.fill()
            blink = self.blink_at < t < self.blink_at + 0.2
            if not blink:
                cr.set_source_rgba(1, 1, 1, 0.85 * a)
                cr.arc(R * 0.13, -R * 0.04, R * 0.03, 0, TAU)
                cr.fill()
            cr.restore()
            # darker back stripes and tail tip
            cr.set_source_rgba(DARK[0], DARK[1], DARK[2], 0.25 * a)
            for k in range(3):
                f = 0.25 + k * 0.22
                px, py = lerp(hip[0], sh[0], f), lerp(hip[1], sh[1], f)
                cr.save()
                cr.translate(px, py)
                cr.rotate(ang + math.pi / 2)
                cr.scale(R * 0.04, R * 0.2)
                cr.arc(0, -1.0, 1, 0, TAU)
                cr.restore()
                cr.fill()
            tip = (hip[0] - R * lerp(1.15, 0.75, u), hip[1] + R * lerp(0.05, 0.33, u))
            cr.set_source_rgba(DARK[0], DARK[1], DARK[2], a)
            cr.arc(tip[0], tip[1], R * 0.07, 0, TAU)
            cr.fill()
            belly = lighten(fur, 0.35)                          # light chest
            cr.set_source_rgba(belly[0], belly[1], belly[2], 0.55 * a)
            cr.set_line_width(R * 0.18)
            cr.move_to(lerp(hip[0], sh[0], 0.2) + R * 0.12 * u, lerp(hip[1], sh[1], 0.2) + R * 0.1 * (1 - u))
            cr.line_to(lerp(hip[0], sh[0], 0.85) + R * 0.12 * u, lerp(hip[1], sh[1], 0.85) + R * 0.1 * (1 - u))
            cr.stroke()
        else:
            cr.restore()

    def _draw_peeking(self, cr, t, R, c, a):
        """Resting: only the head looks out of a burrow, blinking."""
        x, y = self.x, self.y + R * 0.6
        cr.set_source_rgba(0.62, 0.46, 0.28, a)  # mound
        cr.save()
        cr.translate(x, y + R * 0.15)
        cr.scale(R * 1.6, R * 0.45)
        cr.arc(0, 0, 1, math.pi, TAU)
        cr.restore()
        cr.fill()
        cr.set_source_rgba(0.08, 0.05, 0.03, a)  # hole
        cr.save()
        cr.translate(x, y)
        cr.scale(R * 0.6, R * 0.22)
        cr.arc(0, 0, 1, 0, TAU)
        cr.restore()
        cr.fill()
        bob = math.sin(t * 0.6 + self.seed) * R * 0.05
        hy = y - R * 0.18 + bob
        cr.save()
        cr.rectangle(x - R, hy - R, 2 * R, (y + R * 0.05) - (hy - R))
        cr.clip()
        cr.set_source_rgba(FUR[0], FUR[1], FUR[2], a)
        cr.save()
        cr.translate(x, hy)
        cr.scale(R * 0.32, R * 0.36)
        cr.arc(0, 0, 1, 0, TAU)
        cr.restore()
        cr.fill()
        cr.set_source_rgba(DARK[0], DARK[1], DARK[2], a)
        blink = self.blink_at < t < self.blink_at + 0.25
        for side in (-1, 1):
            cr.save()
            cr.translate(x + side * R * 0.13, hy - R * 0.06)
            cr.scale(R * 0.09, R * (0.02 if blink else 0.08))
            cr.arc(0, 0, 1, 0, TAU)
            cr.restore()
            cr.fill()
            cr.arc(x + side * R * 0.28, hy - R * 0.26, R * 0.07, 0, TAU)
            cr.fill()
        cr.arc(x, hy + R * 0.12, R * 0.05, 0, TAU)
        cr.fill()
        cr.restore()


class Meerkats(Scene):
    name = "meerkats"
    actor_class = Meerkat
    label_bg = (0.10, 0.06, 0.03)
    empty_color = (1.0, 0.92, 0.75)
    waiting_row = 0.74

    def __init__(self, **kw):
        super().__init__(**kw)
        self.bg = None
        self.sand = []
        self.tufts = []
        self.birds = []

    def spawn_point(self):
        return self.rng.uniform(self.w * 0.15, self.w * 0.85), self.rng.uniform(self.ground_top + 40, self.h * 0.85)

    def on_tool(self, actor, name, index):
        actor.dig = 1.0
        self.spawn_floater(actor.x, actor.y - actor.radius * 1.1 - index * 26 * self.s, name, actor.color,
                           style="text", vy=40, life=3.5)

    def setup_world(self):
        w, h, s, rng = self.w, self.h, self.s, self.rng
        self.ground_top = gt = h * 0.6
        hz = h * 0.52
        self.bg = cairo.ImageSurface(cairo.FORMAT_RGB24, w, h)
        cr = cairo.Context(self.bg)
        g = cairo.LinearGradient(0, 0, 0, hz)
        g.add_color_stop_rgb(0, 0.22, 0.24, 0.42)
        g.add_color_stop_rgb(0.55, 0.75, 0.48, 0.42)
        g.add_color_stop_rgb(1, 1.0, 0.70, 0.38)
        cr.set_source(g)
        cr.rectangle(0, 0, w, h)
        cr.fill()
        sx, sy, sr = w * 0.3, hz - h * 0.02, h * 0.09  # big low sun
        sg = cairo.RadialGradient(sx, sy, sr, sx, sy, sr * 4)
        sg.add_color_stop_rgba(0, 1.0, 0.8, 0.5, 0.5)
        sg.add_color_stop_rgba(1, 1.0, 0.8, 0.5, 0)
        cr.set_source(sg)
        cr.paint()
        cr.set_source_rgb(1.0, 0.88, 0.62)
        cr.arc(sx, sy, sr, 0, TAU)
        cr.fill()
        # distant hills and acacia trees
        cr.move_to(0, hz + 4 * s)
        for xi in range(0, w + 9, 8):
            cr.line_to(xi, hz - (18 + 14 * math.sin(xi / w * 7 + 1) + 8 * math.sin(xi / w * 19)) * s)
        cr.line_to(w, hz + 4 * s)
        cr.close_path()
        cr.set_source_rgb(0.45, 0.30, 0.30)
        cr.fill()
        for _ in range(int(5 * w / 1920)):
            tx = rng.uniform(0.05, 0.95) * w
            th = rng.uniform(60, 130) * s
            cr.set_source_rgb(0.18, 0.11, 0.10)
            cr.set_line_width(5 * s)
            cr.move_to(tx, hz)
            cr.line_to(tx + th * 0.1, hz - th)
            cr.stroke()
            cr.save()  # flat umbrella crown
            cr.translate(tx + th * 0.1, hz - th)
            cr.scale(th * 0.9, th * 0.18)
            cr.arc(0, 0, 1, 0, TAU)
            cr.restore()
            cr.fill()
        # sand
        cr.move_to(0, hz)
        for xi in range(0, w + 9, 8):
            cr.line_to(xi, hz + (6 + 5 * math.sin(xi / w * 5 + 2)) * s)
        cr.line_to(w, h)
        cr.line_to(0, h)
        cr.close_path()
        ag = cairo.LinearGradient(0, hz, 0, h)
        ag.add_color_stop_rgb(0, 0.78, 0.55, 0.34)
        ag.add_color_stop_rgb(1, 0.45, 0.30, 0.17)
        cr.set_source(ag)
        cr.fill()
        for _ in range(int(500 * w / 1920)):  # pebbles
            cr.set_source_rgba(0.35, 0.24, 0.14, rng.uniform(0.2, 0.5))
            cr.arc(rng.uniform(0, w), rng.uniform(gt - 20 * s, h), rng.uniform(1, 3) * s, 0, TAU)
            cr.fill()
        for _ in range(int(4 * w / 1920)):  # burrows
            bx, by = rng.uniform(0.1, 0.9) * w, rng.uniform(gt + 20 * s, h - self.strip_h - 40 * s)
            cr.set_source_rgba(0.62, 0.44, 0.27, 1)
            cr.save()
            cr.translate(bx, by + 6 * s)
            cr.scale(70 * s, 22 * s)
            cr.arc(0, 0, 1, math.pi, TAU)
            cr.restore()
            cr.fill()
            cr.set_source_rgba(0.1, 0.06, 0.03, 1)
            cr.save()
            cr.translate(bx, by)
            cr.scale(26 * s, 9 * s)
            cr.arc(0, 0, 1, 0, TAU)
            cr.restore()
            cr.fill()
        self.tufts = sorted(([rng.uniform(0, w), rng.uniform(gt, h), rng.uniform(0, TAU)]
                             for _ in range(int(70 * w / 1920))), key=lambda tf: tf[1])
        self.birds = [{"cx": rng.uniform(0.2, 0.8) * w, "cy": rng.uniform(0.12, 0.3) * h,
                       "r": rng.uniform(40, 90) * s, "ph": rng.uniform(0, TAU), "sp": rng.uniform(0.2, 0.4)}
                      for _ in range(2)]

    def update_world(self, dt):
        g = 400 * self.s
        for p in self.sand:
            p["age"] += dt
            p["vy"] += g * dt
            p["x"] += p["vx"] * dt
            p["y"] += p["vy"] * dt
        self.sand = [p for p in self.sand if p["age"] < p["life"]]

    def _tuft(self, cr, x, y, ph, t):
        s = self.s
        k = 0.6 + 0.6 * (y - self.ground_top) / max(1.0, self.h - self.ground_top)
        cr.set_line_width(2 * s * k)
        for i in (-2, -1, 0, 1, 2):
            sway = math.sin(t * 1.1 + ph) * 4 * s * k
            cr.move_to(x + i * 3 * s * k, y)
            cr.line_to(x + i * 8 * s * k + sway, y - (18 + 6 * abs(i)) * s * k)
        cr.set_source_rgba(0.55, 0.45, 0.20, 0.9)
        cr.stroke()

    def draw_world(self, cr, t):
        s = self.s
        cr.set_source_surface(self.bg, 0, 0)
        cr.paint()
        cr.set_source_rgba(0.12, 0.08, 0.08, 0.8)  # vultures circling far away
        cr.set_line_width(2 * s)
        for b in self.birds:
            ang = t * b["sp"] + b["ph"]
            bx, by = b["cx"] + math.cos(ang) * b["r"] * 2, b["cy"] + math.sin(ang) * b["r"] * 0.5
            flap = math.sin(t * 2 + b["ph"]) * 3 * s
            cr.move_to(bx - 12 * s, by - flap)
            cr.curve_to(bx - 6 * s, by - 6 * s, bx - 2 * s, by - 3 * s, bx, by)
            cr.curve_to(bx + 2 * s, by - 3 * s, bx + 6 * s, by - 6 * s, bx + 12 * s, by - flap)
            cr.stroke()
        ti = 0
        for a in self.sorted_actors():
            while ti < len(self.tufts) and self.tufts[ti][1] < a.y + a.radius:
                self._tuft(cr, *self.tufts[ti], t)
                ti += 1
            a.draw(cr, t)
        while ti < len(self.tufts):
            self._tuft(cr, *self.tufts[ti], t)
            ti += 1
        for p in self.sand:
            f = p["age"] / p["life"]
            cr.set_source_rgba(0.85, 0.65, 0.40, (1 - f) * 0.9)
            cr.arc(p["x"], p["y"], 2.2 * s, 0, TAU)
            cr.fill()
