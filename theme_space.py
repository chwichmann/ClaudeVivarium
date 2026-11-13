"""Space theme: sessions are planets orbiting a Claude-orange star.

working = orbits fast with a comet tail, tool calls shoot away as labelled meteors
idle    = crawls along its orbit       waiting = leaves its orbit, comes to the front, pulses
"""
import math

import cairo

from scene import CLAUDE_ORANGE, TAU, Actor, Scene, Text, lerp, lighten, pulse_rings, Weight

OMEGA = {"working": 0.30, "idle": 0.035, "waiting": 0.02, "blocked": 0.0, "rest": 0.05}
FLAT = 0.40  # orbits are ellipses: we look at the system from slightly above


class Planet(Actor):
    BASE_RADIUS = 30

    def __init__(self, scene, info, x, y):
        super().__init__(scene, info, x, y)
        self.theta = self.rng.uniform(0, TAU)
        self.orbit_r = 0.0
        self.orbit_target = 0.0
        self.trail = []
        self.has_ring = self.rng.random() < 0.5
        self.ring_tilt = self.rng.uniform(-0.45, 0.45)
        self.depth = 1.0

    def move(self, dt, t, target):
        sc = self.scene
        if self.orbit_r == 0.0:
            self.orbit_r = self.orbit_target
        self.orbit_r = lerp(self.orbit_r, self.orbit_target, min(1.0, dt * 0.8))
        rx = max(1.0, self.orbit_r)
        self.theta += OMEGA.get(self.state, 0.03) * math.sqrt(sc.orbit_base / rx) * dt
        ox = sc.cx + math.cos(self.theta) * rx
        oy = sc.cy + math.sin(self.theta) * rx * FLAT
        gx, gy = target if target is not None else (ox, oy)
        k = min(1.0, dt * 1.0)
        nx, ny = lerp(self.x, gx, k), lerp(self.y, gy, k)
        if dt > 0:
            self.vx, self.vy = (nx - self.x) / dt, (ny - self.y) / dt
        self.x, self.y = nx, ny
        self.depth = 1.0 if target is not None else math.sin(self.theta)
        if self.state == "working" and not self.dying:
            self.trail.append((self.x, self.y))
            if len(self.trail) > 45:
                self.trail.pop(0)
        elif self.trail:
            self.trail.pop(0)

    def draw(self, cr, t):
        sc, s = self.scene, self.scene.s
        a = self.alpha * self.flicker
        if a < 0.01:
            return
        x, y, R, c = self.x, self.y, self.radius, self.color
        # comet tail
        n = len(self.trail)
        cr.set_line_cap(cairo.LINE_CAP_ROUND)
        for i in range(1, n):
            f = i / n
            cr.set_source_rgba(c[0], c[1], c[2], 0.45 * f * a)
            cr.set_line_width(R * 0.9 * f)
            cr.move_to(*self.trail[i - 1])
            cr.line_to(*self.trail[i])
            cr.stroke()
        # glow
        g = cairo.RadialGradient(x, y, R * 0.5, x, y, R * 2.6)
        g.add_color_stop_rgba(0, c[0], c[1], c[2], (0.4 if self.state != "idle" else 0.22) * a)
        g.add_color_stop_rgba(1, c[0], c[1], c[2], 0)
        cr.set_source(g)
        cr.arc(x, y, R * 2.6, 0, TAU)
        cr.fill()
        if self.state == "waiting":
            pulse_rings(cr, x, y, R, c, a, t, s)
        rc = lighten(c, 0.35)
        if self.has_ring:
            self._ring(cr, x, y, R, rc, a, back=True)
        # sphere lit from the star
        lx, ly = sc.cx - x, sc.cy - y
        d = math.hypot(lx, ly) or 1.0
        lx, ly = lx / d, ly / d
        hi = lighten(c, 0.65)
        sg = cairo.RadialGradient(x + lx * R * 0.45, y + ly * R * 0.45, R * 0.1, x, y, R * 1.05)
        sg.add_color_stop_rgba(0, hi[0], hi[1], hi[2], a)
        sg.add_color_stop_rgba(0.45, c[0], c[1], c[2], a)
        sg.add_color_stop_rgba(1, c[0] * 0.18, c[1] * 0.18, c[2] * 0.22, a)
        cr.set_source(sg)
        cr.arc(x, y, R, 0, TAU)
        cr.fill()
        # surface bands
        cr.save()
        cr.arc(x, y, R, 0, TAU)
        cr.clip()
        for k in range(3):
            yy = y + (k - 1) * R * 0.45 + math.sin(t * 0.5 + k + self.seed) * R * 0.05
            cr.set_source_rgba(1, 1, 1, 0.06 * a)
            cr.rectangle(x - R, yy - R * 0.07, 2 * R, R * 0.14)
            cr.fill()
        cr.restore()
        if self.has_ring:
            self._ring(cr, x, y, R, rc, a, back=False)

    def _ring(self, cr, x, y, R, c, a, back):
        cr.save()
        cr.translate(x, y)
        cr.rotate(self.ring_tilt)
        cr.scale(R * 1.9, R * 0.45)
        if back:
            cr.arc(0, 0, 1, math.pi, TAU)
        else:
            cr.arc(0, 0, 1, 0, math.pi)
        cr.restore()
        cr.set_source_rgba(c[0], c[1], c[2], 0.55 * a)
        cr.set_line_width(3.0 * self.scene.s)
        cr.stroke()


class Space(Scene):
    name = "space"
    actor_class = Planet
    label_bg = (0.02, 0.01, 0.06)
    empty_color = (0.95, 0.75, 0.6)

    def __init__(self, **kw):
        super().__init__(**kw)
        self.stars = []
        self.meteors = []
        self.shooting = None
        self.next_shooting = 8.0
        self.bg = None
        self.cx = self.cy = 0.0
        self.orbit_base = 300.0

    def spawn_point(self):
        return self.cx or self.w / 2, self.cy or self.h / 2  # new planets are born from the star

    def waiting_targets(self, waiting):
        n = len(waiting)
        return {id(a): (self.cx + (i - (n - 1) / 2) * self.w * 0.24, self.cy + self.h * 0.24)
                for i, a in enumerate(waiting)}

    def on_tool(self, actor, name, index):
        dx, dy = actor.x - self.cx, actor.y - self.cy
        ang = math.atan2(dy, dx) + self.rng.uniform(-0.5, 0.5)
        sp = self.s * self.rng.uniform(170, 240)
        self.meteors.append({"x": actor.x, "y": actor.y, "vx": math.cos(ang) * sp, "vy": math.sin(ang) * sp,
                             "age": -index * 0.25, "life": 3.2, "label": name, "c": actor.color})

    def setup_world(self):
        w, h, s, rng = self.w, self.h, self.s, self.rng
        self.cx, self.cy = w / 2, h * 0.45
        self.orbit_base = min(w, h * 1.6) * 0.16
        self.bg = cairo.ImageSurface(cairo.FORMAT_RGB24, w, h)
        cr = cairo.Context(self.bg)
        g = cairo.LinearGradient(0, 0, 0, h)
        g.add_color_stop_rgb(0, 0.01, 0.01, 0.04)
        g.add_color_stop_rgb(1, 0.03, 0.01, 0.06)
        cr.set_source(g)
        cr.paint()
        for _ in range(6):  # nebulae
            nx, ny = rng.uniform(0, w), rng.uniform(0, h)
            r = rng.uniform(0.2, 0.45) * w
            col = rng.choice([(0.45, 0.15, 0.6), (0.1, 0.4, 0.55), (0.6, 0.2, 0.35), (0.2, 0.2, 0.6)])
            ng = cairo.RadialGradient(nx, ny, 0, nx, ny, r)
            ng.add_color_stop_rgba(0, col[0], col[1], col[2], rng.uniform(0.07, 0.14))
            ng.add_color_stop_rgba(1, col[0], col[1], col[2], 0)
            cr.set_source(ng)
            cr.paint()
        for _ in range(int(500 * w * h / (1920 * 1200))):  # faint static stars
            cr.set_source_rgba(0.8, 0.85, 1.0, rng.uniform(0.05, 0.35))
            cr.arc(rng.uniform(0, w), rng.uniform(0, h), rng.uniform(0.3, 0.9) * s, 0, TAU)
            cr.fill()
        n = int(220 * w * h / (1920 * 1200)) + 20
        self.stars = [[rng.uniform(0, w), rng.uniform(0, h), rng.uniform(0.2, 1.0), rng.uniform(0, TAU)]
                      for _ in range(n)]

    def update_world(self, dt):
        s, w, h, rng = self.s, self.w, self.h, self.rng
        # assign orbits: oldest session innermost
        live = sorted((a for a in self.actors.values() if not a.dying), key=lambda a: a.info.get("started", 0))
        n = len(live)
        gap = min(w * 0.085, (w * 0.47 - self.orbit_base) / max(1, n - 1)) if n > 1 else 0
        for i, a in enumerate(live):
            a.orbit_target = self.orbit_base + i * gap
        for st in self.stars:
            st[0] -= st[2] * 3 * s * dt
            if st[0] < -2:
                st[0] = w + 2
                st[1] = rng.uniform(0, h)
        for m in self.meteors:
            m["age"] += dt
            if m["age"] > 0:
                m["x"] += m["vx"] * dt
                m["y"] += m["vy"] * dt
        self.meteors = [m for m in self.meteors if m["age"] < m["life"]]
        if self.t > self.next_shooting:
            self.next_shooting = self.t + rng.uniform(7, 16)
            y0 = rng.uniform(0, h * 0.35)
            self.shooting = {"x": rng.uniform(w * 0.2, w), "y": y0, "age": 0.0,
                             "vx": -s * rng.uniform(700, 1000), "vy": s * rng.uniform(150, 300)}
        if self.shooting:
            self.shooting["age"] += dt
            self.shooting["x"] += self.shooting["vx"] * dt
            self.shooting["y"] += self.shooting["vy"] * dt
            if self.shooting["age"] > 1.2:
                self.shooting = None

    def draw_world(self, cr, t):
        s = self.s
        cr.set_source_surface(self.bg, 0, 0)
        cr.paint()
        for x, y, z, ph in self.stars:
            tw = 0.5 + 0.5 * math.sin(t * (1.0 + z * 2) + ph * 7)
            cr.set_source_rgba(0.85, 0.9, 1.0, (0.2 + 0.6 * z) * (0.35 + 0.65 * tw))
            cr.arc(x, y, s * (0.5 + 1.3 * z), 0, TAU)
            cr.fill()
        if self.shooting:
            sh = self.shooting
            f = 1 - sh["age"] / 1.2
            g = cairo.LinearGradient(sh["x"], sh["y"], sh["x"] - sh["vx"] * 0.15, sh["y"] - sh["vy"] * 0.15)
            g.add_color_stop_rgba(0, 1, 1, 1, 0.8 * f)
            g.add_color_stop_rgba(1, 1, 1, 1, 0)
            cr.set_source(g)
            cr.set_line_width(2 * s)
            cr.move_to(sh["x"], sh["y"])
            cr.line_to(sh["x"] - sh["vx"] * 0.15, sh["y"] - sh["vy"] * 0.15)
            cr.stroke()
        # orbits
        for a in self.actors.values():
            if a.orbit_r <= 0:
                continue
            c = a.color if a.state == "working" else (0.6, 0.65, 0.85)
            cr.save()
            cr.translate(self.cx, self.cy)
            cr.scale(a.orbit_r, a.orbit_r * 0.40)
            cr.arc(0, 0, 1, 0, TAU)
            cr.restore()
            cr.set_source_rgba(c[0], c[1], c[2], (0.16 if a.state == "working" else 0.08) * a.alpha)
            cr.set_line_width(1.2 * s)
            cr.stroke()
        actors = sorted(self.actors.values(), key=lambda a: (a.state == "waiting", a.y))
        for a in actors:
            if a.depth < 0:
                a.draw(cr, t)
        self._draw_star(cr, t)
        for a in actors:
            if a.depth >= 0:
                a.draw(cr, t)
        for m in self.meteors:
            if m["age"] <= 0:
                continue
            f = min(1.0, (m["life"] - m["age"]) / 1.0) * min(1.0, m["age"] * 4)
            c = lighten(m["c"], 0.3)
            tx, ty = m["x"] - m["vx"] * 0.3, m["y"] - m["vy"] * 0.3
            g = cairo.LinearGradient(m["x"], m["y"], tx, ty)
            g.add_color_stop_rgba(0, c[0], c[1], c[2], 0.9 * f)
            g.add_color_stop_rgba(1, c[0], c[1], c[2], 0)
            cr.set_source(g)
            cr.set_line_width(3 * s)
            cr.set_line_cap(cairo.LINE_CAP_ROUND)
            cr.move_to(m["x"], m["y"])
            cr.line_to(tx, ty)
            cr.stroke()
            cr.set_source_rgba(1, 1, 1, f)
            cr.arc(m["x"], m["y"], 3 * s, 0, TAU)
            cr.fill()
            lay = self.text.layout(cr, m["label"], 15 * s, Weight.SEMIBOLD)
            Text.show(cr, lay, m["x"] + 8 * s, m["y"] - 22 * s, c, 0.95 * f, shadow=0.6)

    def _draw_star(self, cr, t):
        s, x, y = self.s, self.cx, self.cy
        busy = any(a.state == "working" for a in self.actors.values())
        R = 46 * s * (1 + 0.04 * math.sin(t * (2.5 if busy else 0.8)))
        c = CLAUDE_ORANGE
        g = cairo.RadialGradient(x, y, R * 0.5, x, y, R * 5)
        g.add_color_stop_rgba(0, c[0], c[1], c[2], 0.45 if busy else 0.3)
        g.add_color_stop_rgba(1, c[0], c[1], c[2], 0)
        cr.set_source(g)
        cr.arc(x, y, R * 5, 0, TAU)
        cr.fill()
        # rotating asterisk rays
        cr.save()
        cr.translate(x, y)
        cr.rotate(t * (0.12 if busy else 0.04))
        for k in range(8):
            ln = R * (2.4 + 0.5 * math.sin(t * 0.9 + k * 1.7))
            cr.save()
            cr.rotate(k * TAU / 8)
            rg = cairo.LinearGradient(0, 0, ln, 0)
            rg.add_color_stop_rgba(0, 1, 0.8, 0.6, 0.6)
            rg.add_color_stop_rgba(1, c[0], c[1], c[2], 0)
            cr.set_source(rg)
            cr.move_to(0, -R * 0.22)
            cr.line_to(ln, 0)
            cr.line_to(0, R * 0.22)
            cr.close_path()
            cr.fill()
            cr.restore()
        cr.restore()
        cg = cairo.RadialGradient(x - R * 0.25, y - R * 0.25, R * 0.1, x, y, R)
        cg.add_color_stop_rgba(0, 1, 0.95, 0.85, 1)
        cg.add_color_stop_rgba(0.6, 1, 0.68, 0.45, 1)
        cg.add_color_stop_rgba(1, c[0], c[1], c[2], 1)
        cr.set_source(cg)
        cr.arc(x, y, R, 0, TAU)
        cr.fill()
