"""Terrarium theme: every session is a chameleon on a branch - and it takes on the state colour.

working = creeps along its branch with the typical rocking gait; every tool call it shoots its tongue
          at a fly, the tool name floats up from the catch
idle    = sits still, the eye swivels around      waiting = goes to the middle of its branch, grows, pulses
rest    = sleeps, pale, eyes closed
"""
import math

import cairo

from scene import TAU, Actor, Scene, clamp, lerp, lighten, pulse_rings

SPEED = {"working": 38, "idle": 6, "waiting": 30, "blocked": 0, "rest": 0}


class Branch:
    """A branch as a smooth curve; y_at(x) gives the top surface, slope_at(x) the angle."""

    def __init__(self, pts, thick):
        self.pts = pts          # sampled (x, y) along the branch, sorted by x
        self.thick = thick
        self.x0, self.x1 = pts[0][0], pts[-1][0]

    def y_at(self, x):
        pts = self.pts
        x = clamp(x, self.x0, self.x1)
        for (ax, ay), (bx, by) in zip(pts, pts[1:]):
            if ax <= x <= bx:
                return lerp(ay, by, (x - ax) / max(1e-6, bx - ax))
        return pts[-1][1]

    def slope_at(self, x):
        d = 6.0
        return math.atan2(self.y_at(x + d) - self.y_at(x - d), 2 * d)


class Chameleon(Actor):
    BASE_RADIUS = 40

    def __init__(self, scene, info, x, y):
        super().__init__(scene, info, x, y)
        self.branch = None
        self.face = 1.0
        self.dir = 1.0
        self.step = 0.0
        self.tongue = 0.0       # 0..1 animation of a tongue strike
        self.tongue_target = (0.0, 0.0)
        self.eye_angle = 0.0

    def move(self, dt, t, target):
        sc, st, s = self.scene, self.state, self.scene.s
        if self.branch is None:
            self.branch = sc.branch_for(self)
            self.x = clamp(self.x, self.branch.x0 + 80 * s, self.branch.x1 - 80 * s)
        b = self.branch
        lo, hi = b.x0 + 70 * s, b.x1 - 70 * s
        speed = SPEED.get(st, 6) * s
        if st == "working":  # chameleon gait: rocking forward and back, mostly forward
            speed *= 0.6 + 0.8 * max(0.0, math.sin(t * 2.2 + self.seed))
        elif st == "idle":
            speed *= max(0.0, math.sin(t * 0.15 + self.seed))
        if target is not None:
            mid = (b.x0 + b.x1) / 2
            self.dir = 1.0 if mid > self.x else -1.0
            speed = min(speed, abs(mid - self.x) * 1.5)
        # keep a distance to other chameleons on the same branch
        for o in sc.actors.values():
            if o is not self and o.branch is b and not o.dying and abs(o.x - self.x) < (self.radius + o.radius) * 2.2:
                self.dir = 1.0 if self.x > o.x else -1.0
        if self.x < lo:
            self.dir = 1.0
        elif self.x > hi:
            self.dir = -1.0
        self.vx = self.dir * speed
        self.x += self.vx * dt
        self.y = b.y_at(self.x) - self.radius * 0.5
        self.step += dt * abs(self.vx) / max(1.0, self.radius * 0.25)
        if abs(self.vx) > 1:
            self.face = lerp(self.face, self.dir, min(1.0, dt * 3))
        self.tongue = max(0.0, self.tongue - dt * 2.5)
        self.eye_angle = math.sin(t * 0.9 + self.seed) * 1.2 + math.sin(t * 2.3 + self.seed * 2) * 0.4

    def companion_spots(self, t):
        if self.branch is None:
            return
        n = len(self.info.get("subagents") or [])
        back = -1.0 if self.face >= 0 else 1.0
        for i in range(n):
            x = self.x + back * self.radius * (1.4 + i * 0.85)
            yield (x, self.branch.y_at(x) - self.radius * self.COMPANION_SCALE * 0.5, 0.0)

    def strike(self, tx, ty):
        self.tongue = 1.0
        self.tongue_target = (tx, ty)

    def draw(self, cr, t):
        sc, s = self.scene, self.scene.s
        a = self.alpha * self.flicker
        if a < 0.01 or self.branch is None:
            return
        R, c, st = self.radius, self.color, self.state
        if st == "waiting":
            pulse_rings(cr, self.x, self.y, R, c, a, t, s)
        g = cairo.RadialGradient(self.x, self.y, 0, self.x, self.y, R * 2.2)
        g.add_color_stop_rgba(0, c[0], c[1], c[2], (0.30 if st not in ("idle", "rest") else 0.15) * a)
        g.add_color_stop_rgba(1, c[0], c[1], c[2], 0)
        cr.set_source(g)
        cr.arc(self.x, self.y, R * 2.2, 0, TAU)
        cr.fill()
        sleeping = st == "rest"
        dark = (c[0] * 0.45, c[1] * 0.5, c[2] * 0.4)
        cr.save()
        cr.translate(self.x, self.y)
        cr.rotate(self.branch.slope_at(self.x))
        fs = self.face if abs(self.face) > 0.15 else 0.15
        cr.scale(fs, 1.0)
        # tongue (behind the head)
        if self.tongue > 0:
            f = math.sin(math.pi * self.tongue)  # out and back
            mx, my = R * 0.98, -R * 0.22
            tx, ty = self.tongue_target
            # target in local coordinates (undo translation/rotation/flip roughly)
            ang = self.branch.slope_at(self.x)
            dx, dy = tx - self.x, ty - self.y
            lx = (dx * math.cos(-ang) - dy * math.sin(-ang)) / fs
            ly = dx * math.sin(-ang) + dy * math.cos(-ang)
            ex, ey = lerp(mx, lx, f), lerp(my, ly, f)
            cr.set_line_cap(cairo.LINE_CAP_ROUND)
            cr.set_source_rgba(0.95, 0.45, 0.55, a)
            cr.set_line_width(R * 0.07)
            cr.move_to(mx, my)
            cr.line_to(ex, ey)
            cr.stroke()
            cr.arc(ex, ey, R * 0.1, 0, TAU)
            cr.fill()
        # curled tail (spiral, thinner towards the tip)
        cr.set_line_cap(cairo.LINE_CAP_ROUND)
        curl = 1.0 if sleeping or st == "idle" else 0.75 + 0.15 * math.sin(t * 0.7 + self.seed)
        px, py = -R * 0.85, R * 0.05
        ang = math.pi * 0.95
        width = R * 0.22
        for k in range(14):
            step = R * 0.16 * (1 - k / 18)
            nx, ny = px + math.cos(ang) * step, py + math.sin(ang) * step
            cr.move_to(px, py)
            cr.line_to(nx, ny)
            cr.set_line_width(width * (1 - k / 16))
            cr.set_source_rgba(dark[0], dark[1], dark[2], a)
            cr.stroke()
            px, py = nx, ny
            ang -= 0.45 * curl
        # legs gripping the branch
        cr.set_line_width(R * 0.11)
        cr.set_source_rgba(dark[0], dark[1], dark[2], a)
        for i, lx in enumerate((-0.55, -0.35, 0.35, 0.55)):
            sw = math.sin(self.step + (0 if i in (0, 3) else math.pi)) * 0.18 * (abs(self.vx) > 1)
            cr.move_to(lx * R, R * 0.05)
            cr.line_to(lx * R + sw * R, R * 0.38)
            cr.line_to(lx * R + sw * R + R * 0.08, R * 0.5)
        cr.stroke()
        # body: arched back, casque on the head
        cr.move_to(-R * 0.9, R * 0.05)
        cr.curve_to(-R * 0.75, -R * 0.75, R * 0.35, -R * 0.85, R * 0.62, -R * 0.55)
        cr.line_to(R * 0.7, -R * 0.78)                       # casque
        cr.curve_to(R * 0.85, -R * 0.62, R * 1.02, -R * 0.4, R * 1.05, -R * 0.22)
        cr.curve_to(R * 1.0, R * 0.0, R * 0.7, R * 0.22, R * 0.35, R * 0.25)
        cr.curve_to(-R * 0.1, R * 0.32, -R * 0.6, R * 0.3, -R * 0.9, R * 0.05)
        cr.close_path()
        hi = lighten(c, 0.5)
        bg = cairo.LinearGradient(0, -R * 0.85, 0, R * 0.3)
        bg.add_color_stop_rgba(0, hi[0], hi[1], hi[2], a)
        bg.add_color_stop_rgba(0.6, c[0], c[1], c[2], a)
        bg.add_color_stop_rgba(1, dark[0], dark[1], dark[2], a)
        cr.set_source(bg)
        cr.fill_preserve()
        cr.set_source_rgba(dark[0], dark[1], dark[2], 0.8 * a)
        cr.set_line_width(1.5 * s)
        cr.stroke()
        # skin pattern: dark bands and a light side stripe
        cr.set_source_rgba(dark[0], dark[1], dark[2], 0.35 * a)
        for k in range(4):
            bx = -R * 0.6 + k * R * 0.32
            cr.move_to(bx, -R * 0.72 + abs(k - 1.5) * R * 0.07)
            cr.line_to(bx + R * 0.12, -R * 0.72 + abs(k - 1.5) * R * 0.07)
            cr.line_to(bx + R * 0.06, R * 0.15)
            cr.close_path()
            cr.fill()
        cr.set_source_rgba(1, 1, 1, 0.35 * a)
        cr.set_line_width(R * 0.05)
        cr.move_to(-R * 0.6, -R * 0.05)
        cr.curve_to(-R * 0.2, R * 0.05, R * 0.3, R * 0.05, R * 0.7, -R * 0.05)
        cr.stroke()
        # turret eye
        ex, ey, er = R * 0.72, -R * 0.38, R * 0.17
        cr.set_source_rgba(c[0] * 0.8, c[1] * 0.8, c[2] * 0.8, a)
        cr.arc(ex, ey, er, 0, TAU)
        cr.fill()
        cr.set_source_rgba(dark[0], dark[1], dark[2], 0.6 * a)
        cr.set_line_width(1.2 * s)
        for k in (0.55, 0.8):
            cr.arc(ex, ey, er * k, 0, TAU)
            cr.stroke()
        if sleeping:
            cr.set_source_rgba(0, 0, 0, 0.8 * a)
            cr.set_line_width(2 * s)
            cr.move_to(ex - er * 0.6, ey)
            cr.line_to(ex + er * 0.6, ey)
            cr.stroke()
        else:
            look = self.eye_angle if self.tongue <= 0 else -0.4
            cr.set_source_rgba(0, 0, 0, a)
            cr.arc(ex + math.cos(look) * er * 0.5, ey + math.sin(look) * er * 0.5, er * 0.28, 0, TAU)
            cr.fill()
        # mouth line
        cr.set_source_rgba(dark[0] * 0.6, dark[1] * 0.6, dark[2] * 0.6, a)
        cr.set_line_width(1.5 * s)
        cr.move_to(R * 1.03, -R * 0.2)
        cr.curve_to(R * 0.85, -R * 0.12, R * 0.7, -R * 0.12, R * 0.55, -R * 0.18)
        cr.stroke()
        cr.restore()


class Terrarium(Scene):
    name = "terrarium"
    actor_class = Chameleon
    label_bg = (0.03, 0.06, 0.03)
    empty_color = (0.85, 1.0, 0.8)

    def __init__(self, **kw):
        super().__init__(**kw)
        self.bg = None
        self.branches = []
        self.flies = []
        self.vines = []

    # each chameleon gets the branch with the fewest chameleons on it
    def branch_for(self, actor):
        counts = [sum(1 for o in self.actors.values() if o is not actor and o.branch is b and not o.dying)
                  for b in self.branches]
        best = min(range(len(self.branches)), key=lambda i: (counts[i], -i if actor.state == "waiting" else i))
        return self.branches[best]

    def waiting_targets(self, waiting):
        return {id(a): (a.x, a.y) for a in waiting}  # chameleons stay on their branch (see move)

    def on_tool(self, actor, name, index):
        if actor.branch is None:
            return
        side = 1 if actor.face >= 0 else -1
        tx = actor.x + side * actor.radius * (2.2 + 0.3 * index)
        ty = actor.y - actor.radius * (0.9 + 0.4 * self.rng.random())
        actor.strike(tx, ty)
        self.spawn_floater(tx, ty - 10 * self.s, name, actor.color, style="text", vy=35, life=3.5)

    def setup_world(self):
        w, h, s, rng = self.w, self.h, self.s, self.rng
        bottom = h - self.strip_h
        self.bg = cairo.ImageSurface(cairo.FORMAT_RGB24, w, h)
        cr = cairo.Context(self.bg)
        g = cairo.LinearGradient(0, 0, 0, h)
        g.add_color_stop_rgb(0, 0.10, 0.17, 0.10)
        g.add_color_stop_rgb(1, 0.03, 0.06, 0.03)
        cr.set_source(g)
        cr.paint()
        lg = cairo.RadialGradient(w * 0.15, -h * 0.1, 0, w * 0.15, -h * 0.1, h * 0.9)  # heat lamp
        lg.add_color_stop_rgba(0, 1.0, 0.85, 0.5, 0.35)
        lg.add_color_stop_rgba(1, 1.0, 0.85, 0.5, 0)
        cr.set_source(lg)
        cr.paint()
        for _ in range(int(26 * w / 1920)):  # big leaves in the background
            lx, ly = rng.uniform(0, w), rng.uniform(h * 0.05, bottom)
            ln = rng.uniform(90, 200) * s
            ang = rng.uniform(-2.6, -0.5)
            cr.save()
            cr.translate(lx, ly)
            cr.rotate(ang)
            cr.move_to(0, 0)
            cr.curve_to(ln * 0.3, -ln * 0.28, ln * 0.75, -ln * 0.2, ln, 0)
            cr.curve_to(ln * 0.75, ln * 0.2, ln * 0.3, ln * 0.28, 0, 0)
            cr.restore()
            shade = rng.uniform(0.6, 1.0)
            cr.set_source_rgba(0.08 * shade, 0.22 * shade, 0.09 * shade, 0.9)
            cr.fill()
        cr.rectangle(0, bottom - 40 * s, w, 40 * s + self.strip_h)  # substrate with moss
        sg = cairo.LinearGradient(0, bottom - 40 * s, 0, h)
        sg.add_color_stop_rgb(0, 0.16, 0.11, 0.06)
        sg.add_color_stop_rgb(1, 0.07, 0.05, 0.03)
        cr.set_source(sg)
        cr.fill()
        for _ in range(int(60 * w / 1920)):
            cr.set_source_rgba(0.2, 0.4, 0.15, rng.uniform(0.4, 0.8))
            cr.arc(rng.uniform(0, w), bottom - 40 * s + rng.uniform(-6, 10) * s, rng.uniform(8, 22) * s, math.pi, TAU)
            cr.fill()
        # branches (3), drawn into the background
        self.branches = []
        levels = [0.33, 0.55, 0.77]
        for i, lv in enumerate(levels):
            y0 = h * lv
            x0, x1 = (-40, w * 0.78) if i % 2 == 0 else (w * 0.22, w + 40)
            amp = rng.uniform(20, 45) * s
            pts = []
            for k in range(61):
                x = lerp(x0, x1, k / 60)
                pts.append((x, y0 + math.sin(k / 60 * math.pi * 1.3 + i) * amp + (k / 60 - 0.5) * 40 * s *
                            (1 if i % 2 else -1)))
            thick = rng.uniform(22, 30) * s
            self.branches.append(Branch(pts, thick))
            cr.set_line_cap(cairo.LINE_CAP_ROUND)
            cr.set_line_join(cairo.LINE_JOIN_ROUND)
            for width, col in ((thick, (0.22, 0.14, 0.08)), (thick * 0.55, (0.32, 0.22, 0.13)),
                               (thick * 0.15, (0.45, 0.33, 0.2))):
                cr.move_to(*pts[0])
                for p in pts[1:]:
                    cr.line_to(p[0], p[1] + (thick - width) * 0.25)
                cr.set_source_rgb(*col)
                cr.set_line_width(width)
                cr.stroke()
        for _ in range(int(240 * w / 1920)):  # condensation drops on the glass
            cr.set_source_rgba(0.8, 1.0, 0.9, rng.uniform(0.05, 0.18))
            cr.arc(rng.uniform(0, w), rng.uniform(0, h), rng.uniform(1, 3.5) * s, 0, TAU)
            cr.fill()
        # the branch's top surface is what the chameleons walk on
        for b in self.branches:
            b.pts = [(x, y - b.thick * 0.45) for x, y in b.pts]
        self.vines = [{"x": rng.uniform(0, w), "len": rng.uniform(0.15, 0.4) * h, "ph": rng.uniform(0, TAU)}
                      for _ in range(int(9 * w / 1920))]
        self.flies = [{"x": rng.uniform(0, w), "y": rng.uniform(h * 0.15, h * 0.8), "ph": rng.uniform(0, TAU),
                       "vx": 0.0, "vy": 0.0} for _ in range(5)]
        for a in self.actors.values():
            a.branch = None  # re-assign on the new layout

    def update_world(self, dt):
        s, t = self.s, self.t
        for f in self.flies:
            f["vx"] += math.sin(t * 3.1 + f["ph"]) * 300 * s * dt
            f["vy"] += math.cos(t * 2.7 + f["ph"] * 2) * 300 * s * dt
            f["vx"] *= 0.95
            f["vy"] *= 0.95
            f["x"] = (f["x"] + f["vx"] * dt) % self.w
            f["y"] = clamp(f["y"] + f["vy"] * dt, self.h * 0.1, self.h * 0.85)

    def draw_world(self, cr, t):
        s = self.s
        cr.set_source_surface(self.bg, 0, 0)
        cr.paint()
        for f in self.flies:
            cr.set_source_rgba(0.05, 0.05, 0.05, 0.8)
            cr.arc(f["x"], f["y"], 2.5 * s, 0, TAU)
            cr.fill()
            wing = abs(math.sin(t * 40 + f["ph"]))
            cr.set_source_rgba(0.9, 0.95, 1.0, 0.35)
            cr.save()
            cr.translate(f["x"], f["y"] - 2 * s)
            cr.scale(4 * s, (1 + 2 * wing) * s)
            cr.arc(0, 0, 1, 0, TAU)
            cr.restore()
            cr.fill()
        for a in sorted(self.actors.values(), key=lambda a: a.y):
            a.draw(cr, t)
        cr.set_line_cap(cairo.LINE_CAP_ROUND)  # hanging vines in front
        for v in self.vines:
            cr.move_to(v["x"], 0)
            for k in range(1, 11):
                f = k / 10
                cr.line_to(v["x"] + math.sin(t * 0.5 + v["ph"] + f * 2) * f * 18 * s, f * v["len"])
            cr.set_source_rgba(0.12, 0.30, 0.10, 0.9)
            cr.set_line_width(4 * s)
            cr.stroke()
            for k in range(2, 10, 2):
                f = k / 10
                lx = v["x"] + math.sin(t * 0.5 + v["ph"] + f * 2) * f * 18 * s
                cr.set_source_rgba(0.16, 0.42, 0.14, 0.9)
                cr.save()
                cr.translate(lx, f * v["len"])
                cr.rotate(0.8 if k % 4 else -0.8)
                cr.scale(14 * s, 6 * s)
                cr.arc(0.8, 0, 1, 0, TAU)
                cr.restore()
                cr.fill()
