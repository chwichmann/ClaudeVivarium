"""Archipelago theme: every session is a small island with a castaway, in a wide ocean.

The castaway wears a striped shirt in the state colour.
working = hammers away at a raft; every tool call is a message in a bottle drifting off with the tool name
idle    = sits under the palm and fishes      waiting = stands on the beach waving a flag by a signal fire
rest    = sleeps in a hammock
The sky follows the real time of day: sun arc, sunset, moon and stars at night. Now and then a sailing
ship passes on the horizon and seagulls fly by. (Set VIVARIUM_HOUR=22 to preview the night.)
"""
import math
import os
import time

import cairo

from scene import TAU, Actor, Scene, Text, Weight, clamp, lerp, lerp3, lighten, pulse_rings

SKIN = (0.93, 0.74, 0.58)
HAT = (0.86, 0.72, 0.42)
SHORTS = (0.25, 0.32, 0.50)

# hour: sky top, sky horizon, sea near horizon, sea in front, daylight 0..1
KEYS = [
    (0.0, (0.02, 0.03, 0.08), (0.06, 0.08, 0.16), (0.04, 0.07, 0.13), (0.01, 0.03, 0.06), 0.25),
    (5.0, (0.02, 0.03, 0.08), (0.06, 0.08, 0.16), (0.04, 0.07, 0.13), (0.01, 0.03, 0.06), 0.25),
    (6.5, (0.25, 0.30, 0.55), (0.98, 0.66, 0.46), (0.32, 0.36, 0.50), (0.08, 0.18, 0.28), 0.7),
    (9.0, (0.22, 0.52, 0.88), (0.72, 0.86, 0.98), (0.12, 0.48, 0.62), (0.02, 0.22, 0.36), 1.0),
    (16.0, (0.22, 0.52, 0.88), (0.72, 0.86, 0.98), (0.12, 0.48, 0.62), (0.02, 0.22, 0.36), 1.0),
    (18.5, (0.30, 0.30, 0.56), (1.00, 0.60, 0.36), (0.38, 0.32, 0.42), (0.08, 0.12, 0.22), 0.75),
    (20.5, (0.05, 0.06, 0.17), (0.22, 0.16, 0.30), (0.06, 0.08, 0.16), (0.02, 0.04, 0.08), 0.35),
    (24.0, (0.02, 0.03, 0.08), (0.06, 0.08, 0.16), (0.04, 0.07, 0.13), (0.01, 0.03, 0.06), 0.25),
]


def hour_now():
    forced = os.environ.get("VIVARIUM_HOUR")
    if forced:
        return float(forced) % 24
    lt = time.localtime()
    return lt.tm_hour + lt.tm_min / 60 + lt.tm_sec / 3600


def palette(hour):
    for a, b in zip(KEYS, KEYS[1:]):
        if a[0] <= hour <= b[0]:
            f = (hour - a[0]) / max(1e-6, b[0] - a[0])
            return tuple(lerp3(x, y, f) for x, y in zip(a[1:5], b[1:5])) + (lerp(a[5], b[5], f),)
    return KEYS[0][1:]


def shade(c, light):
    """Scene colours get darker (and a bit bluer) at night."""
    k = 0.35 + 0.65 * light
    return (c[0] * k, c[1] * k, c[2] * (k + (1 - k) * 0.25))


class Castaway(Actor):
    BASE_RADIUS = 140  # half the width of the island (labels go beside the island)

    def __init__(self, scene, info, x, y):
        super().__init__(scene, info, x, y)
        self.slot = (x, y)
        self.hammer_hit = 0.0

    def move(self, dt, t, target):
        k = min(1.0, dt * 0.8)  # islands glide to a new place when sessions come or go
        self.x = lerp(self.x, self.slot[0], k)
        self.y = lerp(self.y, self.slot[1], k)
        self.radius *= self.scene.depth_scale(self.y)

    # ------------------------------------------------------------------ drawing
    def draw(self, cr, t):
        sc, s = self.scene, self.scene.s
        a = self.alpha * self.flicker
        if a < 0.01:
            return
        R, c, st, L = self.radius, self.color, self.state, sc.light
        x, y = self.x, self.y
        # island: shadow in the water, foam ring, sand
        cr.save()
        cr.translate(x, y + R * 0.08)
        cr.scale(R * 1.15, R * 0.3)
        cr.arc(0, 0, 1, 0, TAU)
        cr.restore()
        cr.set_source_rgba(0, 0, 0, 0.18 * a)
        cr.fill()
        foam = 1.0 + 0.04 * math.sin(t * 1.3 + self.seed)
        cr.save()
        cr.translate(x, y)
        cr.scale(R * 1.08 * foam, R * 0.26 * foam)
        cr.arc(0, 0, 1, 0, TAU)
        cr.restore()
        cr.set_source_rgba(0.9, 0.97, 1.0, 0.45 * a * (0.5 + 0.5 * L))
        cr.set_line_width(2.5 * s)
        cr.stroke()
        sand = shade((0.93, 0.82, 0.58), L)
        cr.save()
        cr.translate(x, y)
        cr.scale(R, R * 0.24)
        cr.arc(0, 0, 1, math.pi, TAU)       # sand dome above the water line
        cr.restore()
        cr.save()
        cr.translate(x, y)
        cr.scale(R, R * 0.07)
        cr.arc(0, 0, 1, 0, math.pi)
        cr.restore()
        g = cairo.LinearGradient(x, y - R * 0.24, x, y + R * 0.07)
        hs = lighten(sand, 0.25)
        g.add_color_stop_rgba(0, hs[0], hs[1], hs[2], a)
        g.add_color_stop_rgba(1, sand[0] * 0.8, sand[1] * 0.8, sand[2] * 0.8, a)
        cr.set_source(g)
        cr.fill()
        if st == "waiting":
            pulse_rings(cr, x, y - R * 0.35, R * 0.45, c, a, t, s)
        u = R * 0.3  # size unit of the castaway
        palm_x, palm_y = x + R * 0.38, y - R * 0.14
        if st == "rest":
            self._hammock(cr, t, x, y, R, u, c, a, L, palm_x, palm_y)
        self._palm(cr, t, palm_x, palm_y, R, a, L)
        if st == "working":
            self._working(cr, t, x, y, R, u, c, a, L)
        elif st == "waiting":
            self._waiting(cr, t, x, y, R, u, c, a, L)
        elif st != "rest":
            self._fishing(cr, t, x, y, R, u, c, a, L, rod=st != "blocked")

    def _palm(self, cr, t, px, py, R, a, L):
        s = self.scene.s
        sway = math.sin(t * 0.8 + self.seed) * R * 0.04
        tx, ty = px - R * 0.25 + sway, py - R * 1.25
        cr.set_line_cap(cairo.LINE_CAP_ROUND)
        trunk = shade((0.50, 0.36, 0.22), L)
        cr.set_source_rgba(trunk[0], trunk[1], trunk[2], a)
        cr.set_line_width(R * 0.09)
        cr.move_to(px, py)
        cr.curve_to(px + R * 0.05, py - R * 0.5, px - R * 0.05, py - R * 0.9, tx, ty)
        cr.stroke()
        cr.set_source_rgba(trunk[0] * 0.7, trunk[1] * 0.7, trunk[2] * 0.7, a)  # rings on the trunk
        cr.set_line_width(1.2 * s)
        for k in range(1, 7):
            f = k / 7
            bx = lerp(px, tx, f) + math.sin(f * math.pi) * R * 0.03
            by = lerp(py, ty, f)
            cr.move_to(bx - R * 0.045, by)
            cr.line_to(bx + R * 0.045, by - R * 0.02)
        cr.stroke()
        leaf = shade((0.20, 0.55, 0.22), L)
        for k in range(7):  # fronds
            ang = -math.pi / 2 + (k - 3) * 0.55 + math.sin(t * 1.1 + k + self.seed) * 0.06
            ln = R * (0.75 - abs(k - 3) * 0.05)
            ex, ey = tx + math.cos(ang) * ln, ty + math.sin(ang) * ln * 0.6 + ln * 0.35
            mx, my = tx + math.cos(ang) * ln * 0.5, ty + math.sin(ang) * ln * 0.5 - R * 0.08
            cr.move_to(tx, ty)
            cr.curve_to(mx, my - R * 0.06, ex, ey - R * 0.08, ex, ey)
            cr.curve_to(ex - math.cos(ang) * R * 0.1, ey + R * 0.02, mx, my + R * 0.06, tx, ty)
            cr.set_source_rgba(leaf[0] * (0.85 + 0.03 * k), leaf[1], leaf[2], a)
            cr.fill()
        cr.set_source_rgba(0.35 * L + 0.1, 0.22 * L + 0.05, 0.1, a)  # coconuts
        for dx in (-0.06, 0.05):
            cr.arc(tx + R * dx, ty + R * 0.06, R * 0.045, 0, TAU)
            cr.fill()

    def _figure(self, cr, fx, fy, u, c, a, L, sitting=False, lying=False, arm_l=0.2, arm_r=-0.2, face=1.0,
                item=None, item_angle=0.0, t=0.0):
        """The castaway (own design: wide straw hat, striped shirt in the state colour).
        Arm angles: 0 = hanging down, pi = straight up; positive = forward (in facing direction)."""
        s = self.scene.s
        skin, shorts = shade(SKIN, L), shade(SHORTS, L)
        cr.save()
        cr.translate(fx, fy)
        if lying:
            cr.rotate(-math.pi / 2 * face)
        cr.scale(face, 1.0)
        cr.set_line_cap(cairo.LINE_CAP_ROUND)
        if sitting:
            hip, sh, head = (0, -0.38 * u), (0, -1.22 * u), (0, -1.66 * u)
            cr.set_source_rgba(shorts[0], shorts[1], shorts[2], a)
            cr.set_line_width(0.3 * u)
            cr.move_to(*hip)
            cr.line_to(0.62 * u, -0.38 * u)
            cr.stroke()
            cr.set_source_rgba(skin[0], skin[1], skin[2], a)
            cr.set_line_width(0.2 * u)
            cr.move_to(0.62 * u, -0.38 * u)
            cr.line_to(0.7 * u, 0)
            cr.stroke()
        else:
            hip, sh, head = (0, -0.95 * u), (0, -1.8 * u), (0, -2.24 * u)
            for dx in (-0.12, 0.12):
                cr.set_source_rgba(skin[0], skin[1], skin[2], a)
                cr.set_line_width(0.18 * u)
                cr.move_to(dx * u, hip[1] + 0.35 * u)
                cr.line_to(dx * u * 1.3, 0)
                cr.stroke()
            cr.set_source_rgba(shorts[0], shorts[1], shorts[2], a)
            cr.rectangle(-0.24 * u, hip[1] - 0.05 * u, 0.48 * u, 0.42 * u)
            cr.fill()
        # shirt with stripes in the state colour
        cr.save()
        top = sh[1] - 0.05 * u
        cr.rectangle(-0.27 * u, top, 0.54 * u, hip[1] - top + 0.05 * u)
        cr.clip_preserve()
        sc_ = lighten(c, 0.1)
        cr.set_source_rgba(sc_[0], sc_[1], sc_[2], a)
        cr.fill()
        cr.set_source_rgba(1, 1, 1, 0.6 * a)
        k = top
        while k < hip[1]:
            cr.rectangle(-0.3 * u, k + 0.1 * u, 0.6 * u, 0.06 * u)
            k += 0.17 * u
        cr.fill()
        cr.restore()
        # arms (+ item in the right hand)
        hands = []
        for ang, side in ((arm_l, -1), (arm_r, 1)):
            ax, ay = side * 0.2 * u, sh[1] + 0.08 * u
            hx, hy = ax + math.sin(ang) * 0.75 * u, ay + math.cos(ang) * 0.75 * u
            cr.set_source_rgba(skin[0], skin[1], skin[2], a)
            cr.set_line_width(0.15 * u)
            cr.move_to(ax, ay)
            cr.line_to(hx, hy)
            cr.stroke()
            hands.append((hx, hy))
        hx, hy = hands[1]
        if item == "hammer":
            ex, ey = hx + math.sin(item_angle) * 0.55 * u, hy + math.cos(item_angle) * 0.55 * u
            cr.set_source_rgba(0.45, 0.32, 0.18, a)
            cr.set_line_width(0.08 * u)
            cr.move_to(hx, hy)
            cr.line_to(ex, ey)
            cr.stroke()
            cr.save()
            cr.translate(ex, ey)
            cr.rotate(-item_angle)
            cr.rectangle(-0.15 * u, -0.08 * u, 0.3 * u, 0.16 * u)
            cr.restore()
            cr.set_source_rgba(0.35, 0.36, 0.4, a)
            cr.fill()
        elif item == "flag":
            pole_top = (hx + math.sin(item_angle) * 1.6 * u, hy - math.cos(item_angle) * 1.6 * u)
            cr.set_source_rgba(0.45, 0.32, 0.18, a)
            cr.set_line_width(0.07 * u)
            cr.move_to(hx, hy + 0.3 * u)
            cr.line_to(*pole_top)
            cr.stroke()
            wave = math.sin(t * 7)
            cr.move_to(*pole_top)
            cr.curve_to(pole_top[0] + 0.35 * u, pole_top[1] - 0.1 * u + wave * 0.08 * u,
                        pole_top[0] + 0.6 * u, pole_top[1] + 0.1 * u - wave * 0.08 * u,
                        pole_top[0] + 0.85 * u, pole_top[1] + wave * 0.06 * u)
            cr.line_to(pole_top[0] + 0.85 * u, pole_top[1] + 0.55 * u + wave * 0.06 * u)
            cr.curve_to(pole_top[0] + 0.6 * u, pole_top[1] + 0.65 * u - wave * 0.08 * u,
                        pole_top[0] + 0.35 * u, pole_top[1] + 0.45 * u + wave * 0.08 * u,
                        pole_top[0], pole_top[1] + 0.55 * u)
            cr.close_path()
            cr.set_source_rgba(c[0], c[1], c[2], a)
            cr.fill()
        # head, short beard, straw hat with a wide brim
        cr.set_source_rgba(skin[0], skin[1], skin[2], a)
        cr.arc(head[0], head[1], 0.3 * u, 0, TAU)
        cr.fill()
        beard = shade((0.42, 0.28, 0.16), L)
        cr.set_source_rgba(beard[0], beard[1], beard[2], a)
        cr.arc(head[0] + 0.05 * u, head[1] + 0.05 * u, 0.27 * u, 0.15 * math.pi, 0.85 * math.pi)
        cr.fill()
        cr.set_source_rgba(0.1, 0.08, 0.06, a)
        if lying:  # asleep: closed eye
            cr.set_line_width(0.04 * u)
            cr.move_to(head[0] + 0.08 * u, head[1] - 0.04 * u)
            cr.line_to(head[0] + 0.2 * u, head[1] - 0.04 * u)
            cr.stroke()
        else:
            cr.arc(head[0] + 0.15 * u, head[1] - 0.05 * u, 0.04 * u, 0, TAU)
            cr.fill()
        hat = shade(HAT, L)
        cr.set_source_rgba(hat[0], hat[1], hat[2], a)
        cr.save()
        cr.translate(head[0], head[1] - 0.2 * u)
        cr.scale(0.55 * u, 0.09 * u)
        cr.arc(0, 0, 1, 0, TAU)
        cr.restore()
        cr.fill()
        cr.move_to(head[0] - 0.24 * u, head[1] - 0.2 * u)
        cr.curve_to(head[0] - 0.22 * u, head[1] - 0.52 * u, head[0] + 0.22 * u, head[1] - 0.52 * u,
                    head[0] + 0.24 * u, head[1] - 0.2 * u)
        cr.close_path()
        cr.fill()
        cr.set_source_rgba(c[0] * 0.8, c[1] * 0.8, c[2] * 0.8, a)  # hat band in the state colour
        cr.rectangle(head[0] - 0.24 * u, head[1] - 0.3 * u, 0.48 * u, 0.07 * u)
        cr.fill()
        cr.restore()
        _ = s
        return hands

    def _working(self, cr, t, x, y, R, u, c, a, L):
        s = self.scene.s
        # raft at the water's edge
        rx, ry = x - R * 0.95, y + R * 0.02
        wood = shade((0.55, 0.40, 0.24), L)
        for k in range(5):
            cr.set_source_rgba(wood[0] * (0.85 + 0.04 * k), wood[1], wood[2], a)
            cr.save()
            cr.translate(rx - R * 0.2 + k * R * 0.1, ry)
            cr.scale(R * 0.045, R * 0.12)
            cr.arc(0, 0, 1, 0, TAU)
            cr.restore()
            cr.fill()
        swing = math.sin(t * 6 + self.seed)
        arm = 1.9 + 0.9 * swing           # raised ... down on the raft
        self._figure(cr, x - R * 0.42, y - R * 0.16, u, c, a, L, arm_l=0.5, arm_r=arm, face=-1.0,
                     item="hammer", item_angle=arm + 0.6, t=t)
        if swing < -0.95:  # hit: little sparks / chips
            for k in range(3):
                cr.set_source_rgba(1, 0.9, 0.6, 0.8 * a)
                cr.arc(rx + R * (0.05 + 0.05 * k), ry - R * (0.1 + 0.06 * k), 1.6 * s, 0, TAU)
                cr.fill()

    def _fishing(self, cr, t, x, y, R, u, c, a, L, rod=True):
        fx, fy = x + R * 0.05, y - R * 0.14
        if not rod:  # blocked: sitting with the head down
            self._figure(cr, fx, fy, u, c, a, L, sitting=True, arm_l=0.9, arm_r=0.9, t=t)
            return
        hands = self._figure(cr, fx, fy, u, c, a, L, sitting=True, arm_l=0.6, arm_r=1.4, t=t)
        hx, hy = fx + hands[1][0], fy + hands[1][1]
        tip = (hx + R * 0.9, hy - R * 0.55)
        cr.set_source_rgba(0.4, 0.3, 0.18, self.alpha)
        cr.set_line_width(max(1.0, u * 0.05))
        cr.move_to(hx, hy)
        cr.line_to(*tip)
        cr.stroke()
        bob = (x + R * 1.45, y + R * 0.22 + math.sin(t * 2.2 + self.seed) * R * 0.02)
        cr.set_source_rgba(0.9, 0.9, 0.9, 0.6 * self.alpha)
        cr.set_line_width(1.0)
        cr.move_to(*tip)
        cr.curve_to(tip[0] + R * 0.2, tip[1] + R * 0.2, bob[0] - R * 0.05, bob[1] - R * 0.25, *bob)
        cr.stroke()
        cr.set_source_rgba(0.95, 0.25, 0.2, self.alpha)
        cr.arc(bob[0], bob[1], u * 0.07, 0, TAU)
        cr.fill()

    def _waiting(self, cr, t, x, y, R, u, c, a, L):
        s = self.scene.s
        fire = (x - R * 0.62, y - R * 0.1)  # signal fire with smoke
        for k in range(3):
            h = R * (0.22 + 0.06 * math.sin(t * 9 + k * 2))
            cr.move_to(fire[0] - R * 0.08 + k * R * 0.06, fire[1])
            cr.line_to(fire[0] - R * 0.05 + k * R * 0.06, fire[1] - h)
            cr.line_to(fire[0] - R * 0.02 + k * R * 0.06, fire[1])
            cr.close_path()
            cr.set_source_rgba(1.0, 0.55 + 0.15 * k, 0.15, 0.9 * a)
            cr.fill()
        for k in range(4):
            f = ((t * 0.35) + k / 4) % 1.0
            cr.set_source_rgba(0.75, 0.75, 0.78, (1 - f) * 0.45 * a)
            cr.arc(fire[0] + math.sin(f * 4 + k) * R * 0.1, fire[1] - R * 0.3 - f * R * 1.3, R * (0.06 + 0.12 * f),
                   0, TAU)
            cr.fill()
        wave = math.sin(t * 5)
        self._figure(cr, x - R * 0.05, y - R * 0.12, u, c, a, L, arm_l=2.6 + 0.35 * wave, arm_r=2.9,
                     item="flag", item_angle=0.25 * wave, t=t)
        _ = s

    def _hammock(self, cr, t, x, y, R, u, c, a, L, palm_x, palm_y):
        s = self.scene.s
        post = (x - R * 0.55, y - R * 0.12)
        cr.set_source_rgba(0.45, 0.32, 0.2, a)
        cr.set_line_width(R * 0.05)
        cr.move_to(post[0], post[1])
        cr.line_to(post[0], post[1] - R * 0.55)
        cr.stroke()
        p1, p2 = (post[0], post[1] - R * 0.5), (palm_x - R * 0.02, palm_y - R * 0.5)
        sway = math.sin(t * 0.7) * R * 0.03
        mid = ((p1[0] + p2[0]) / 2 + sway, max(p1[1], p2[1]) + R * 0.22)
        hammock = shade((0.85, 0.80, 0.70), L)
        cr.set_source_rgba(hammock[0], hammock[1], hammock[2], a)
        cr.set_line_width(R * 0.12)
        cr.move_to(*p1)
        cr.curve_to(p1[0] + R * 0.1, mid[1], p2[0] - R * 0.1, mid[1], *p2)
        cr.stroke()
        self._figure(cr, mid[0] - u * 0.9, mid[1] - u * 0.25, u * 0.85, c, a, L, lying=True, face=1.0,
                     arm_l=0.3, arm_r=0.3, t=t)
        T = self.scene.text
        for k in range(3):
            f = (t * 0.3 + k / 3) % 1.0
            lay = T.layout(cr, "z", (13 + 11 * f) * s, Weight.BOLD)
            Text.show(cr, lay, mid[0] + R * (0.1 + 0.4 * f), mid[1] - R * (0.45 + 0.9 * f),
                      (1, 1, 1), 0.8 * math.sin(math.pi * f) * a)


class Archipelago(Scene):
    name = "archipelago"
    actor_class = Castaway
    label_bg = (0.02, 0.05, 0.08)
    empty_color = (1.0, 1.0, 1.0)

    def __init__(self, **kw):
        super().__init__(**kw)
        self.bg = None
        self.bg_key = None
        self.horizon = 0.0
        self.light = 1.0
        self.bottles = []
        self.stars = []
        self.ship = None
        self.next_ship = 20.0
        self.gulls = []
        self.next_gulls = 6.0
        self.pal = None

    def depth_scale(self, y):
        top, bottom = self.horizon + 60 * self.s, self.h - self.strip_h
        return lerp(0.6, 1.1, clamp((y - top) / max(1.0, bottom - top), 0.0, 1.0))

    def waiting_targets(self, waiting):
        return {}  # islands stay where they are

    def _slots(self, n):
        """Island positions for n islands: one row for up to 3, two staggered rows for more."""
        w, h, s = self.w, self.h, self.s
        top, bottom = self.horizon + 0.14 * h, h - self.strip_h - 0.07 * h
        if n <= 3:
            rows = [n]
        else:
            rows = [n // 2, n - n // 2]
        slots = []
        for r, count in enumerate(rows):
            y = lerp(top, bottom, (r + 0.5) / len(rows)) if len(rows) > 1 else lerp(top, bottom, 0.55)
            for i in range(count):
                x = w * (i + 0.5 + (0.25 if r % 2 else 0)) / (count + (0.5 if r % 2 else 0))
                dy = (0.06 * h * (1 if i % 2 else -1)) if len(rows) == 1 and count > 1 else 0
                slots.append((x, y + dy))
        _ = s
        return slots

    def update_world(self, dt):
        s, w, rng, t = self.s, self.w, self.rng, self.t
        live = sorted((a for a in self.actors.values() if not a.dying),
                      key=lambda a: (a.info.get("started", 0), a.info.get("id", "")))
        for a, slot in zip(live, self._slots(len(live))):
            a.slot = slot
        for b in self.bottles:
            b["age"] += dt
            b["x"] += b["vx"] * dt
        self.bottles = [b for b in self.bottles if b["age"] < b["life"]]
        if t > self.next_ship:
            self.next_ship = t + rng.uniform(60, 140)
            self.ship = {"x": -120 * s, "dir": 1}
        if self.ship:
            self.ship["x"] += 14 * s * dt
            if self.ship["x"] > w + 120 * s:
                self.ship = None
        if t > self.next_gulls:
            self.next_gulls = t + rng.uniform(15, 35)
            y0 = rng.uniform(self.h * 0.08, self.horizon * 0.7)
            self.gulls = [{"x": -40 * s - k * 45 * s, "y": y0 + k * 18 * s * (1 if k % 2 else -1),
                           "ph": rng.uniform(0, TAU)} for k in range(rng.randint(2, 4))]
        for g in self.gulls:
            g["x"] += 70 * s * dt
        self.gulls = [g for g in self.gulls if g["x"] < w + 60 * s]

    def on_tool(self, actor, name, index):
        side = -1 if self.rng.random() < 0.5 else 1
        x = actor.x + side * actor.radius * 1.05
        # different heights and a quicker drift, so the names of several bottles do not run together
        y = actor.y + actor.radius * (0.1 + 0.12 * self.rng.random() + 0.1 * index)
        self.bottles.append({"x": x, "y": y,
                             "vx": side * self.rng.uniform(30, 45) * self.s, "age": -index * 0.6, "life": 7.0,
                             "label": name, "c": actor.color, "ph": self.rng.uniform(0, TAU)})

    def setup_world(self):
        w, h, rng = self.w, self.h, self.rng
        self.horizon = h * 0.40
        self.stars = [[rng.uniform(0, w), rng.uniform(0, self.horizon), rng.uniform(0.2, 1.0), rng.uniform(0, TAU)]
                      for _ in range(int(220 * w / 1920))]
        self.bg_key = None
        for a in self.actors.values():
            a.x, a.y = w / 2, self.horizon + h * 0.3
            a.slot = (a.x, a.y)

    def _make_bg(self, hour):
        """Sky, sun or moon and sea for the current time - rebuilt once a minute."""
        w, h, s, hz = self.w, self.h, self.s, self.horizon
        sky_top, sky_low, sea_far, sea_near, light = palette(hour)
        self.light = light
        bg = cairo.ImageSurface(cairo.FORMAT_RGB24, w, h)
        cr = cairo.Context(bg)
        g = cairo.LinearGradient(0, 0, 0, hz)
        g.add_color_stop_rgb(0, *sky_top)
        g.add_color_stop_rgb(1, *sky_low)
        cr.set_source(g)
        cr.rectangle(0, 0, w, hz)
        cr.fill()
        if 6.0 <= hour <= 20.0:  # sun arc
            f = (hour - 6.0) / 14.0
            sx, sy, sr, col = w * (0.08 + 0.84 * f), hz - math.sin(math.pi * f) * h * 0.30, 40 * s, (1.0, 0.92, 0.7)
        else:  # moon arc through the night
            f = ((hour - 20.0) % 24) / 10.0
            sx, sy, sr, col = w * (0.08 + 0.84 * f), hz - math.sin(math.pi * f) * h * 0.30, 30 * s, (0.95, 0.95, 0.88)
        glow = cairo.RadialGradient(sx, sy, sr, sx, sy, sr * 6)
        glow.add_color_stop_rgba(0, col[0], col[1], col[2], 0.35)
        glow.add_color_stop_rgba(1, col[0], col[1], col[2], 0)
        cr.set_source(glow)
        cr.rectangle(0, 0, w, hz)
        cr.fill()
        cr.save()
        cr.rectangle(0, 0, w, hz)
        cr.clip()
        cr.set_source_rgb(*col)
        cr.arc(sx, sy, sr, 0, TAU)
        cr.fill()
        cr.restore()
        self.sun = (sx, sy, sr, col)
        g = cairo.LinearGradient(0, hz, 0, h)
        g.add_color_stop_rgb(0, *sea_far)
        g.add_color_stop_rgb(1, *sea_near)
        cr.set_source(g)
        cr.rectangle(0, hz, w, h - hz)
        cr.fill()
        cr.set_source_rgba(1, 1, 1, 0.25 * light)  # horizon line
        cr.rectangle(0, hz - 1 * s, w, 1.5 * s)
        cr.fill()
        self.bg = bg

    def draw_world(self, cr, t):
        s, w, h, hz = self.s, self.w, self.h, self.horizon
        hour = hour_now()
        key = int(hour * 60)
        if key != self.bg_key:
            self.bg_key = key
            self._make_bg(hour)
        L = self.light
        cr.set_source_surface(self.bg, 0, 0)
        cr.paint()
        if L < 0.6:  # stars at night
            vis = (0.6 - L) / 0.35
            for x, y, z, ph in self.stars:
                tw = 0.5 + 0.5 * math.sin(t * (0.8 + z * 2) + ph * 5)
                cr.set_source_rgba(0.95, 0.95, 1.0, vis * (0.2 + 0.6 * z) * (0.4 + 0.6 * tw))
                cr.arc(x, y * 0.95, (0.5 + z) * s, 0, TAU)
                cr.fill()
        sx, sy, sr, col = self.sun
        for k in range(14):  # glitter path of the sun / moon on the water
            f = k / 14
            yy = hz + 6 * s + f * f * (h - hz) * 0.8
            ww = sr * (0.4 + 2.5 * f) * (0.6 + 0.4 * math.sin(t * 2 + k * 1.7))
            cr.set_source_rgba(col[0], col[1], col[2], 0.25 * (1 - f) * (0.5 + 0.5 * L))
            cr.rectangle(sx - ww / 2, yy, ww, 2 * s + f * 3 * s)
            cr.fill()
        cr.set_line_width(1.2 * s)  # waves
        for k in range(18):
            f = (k + (t * 0.05) % 1) / 18
            yy = hz + 10 * s + f * f * (h - hz)
            amp = (2 + 6 * f) * s
            cr.move_to(0, yy)
            for xi in range(0, w + 41, 40):
                cr.line_to(xi, yy + math.sin(xi * 0.012 / (0.3 + f) + t * 0.8 + k) * amp)
            cr.set_source_rgba(1, 1, 1, (0.04 + 0.08 * f) * (0.4 + 0.6 * L))
            cr.stroke()
        if self.ship:  # a sailing ship far away on the horizon
            x = self.ship["x"]
            dark = (0.12 + 0.2 * L, 0.1 + 0.15 * L, 0.12 + 0.15 * L)
            cr.set_source_rgba(dark[0], dark[1], dark[2], 0.8)
            cr.move_to(x - 30 * s, hz - 2 * s)
            cr.line_to(x + 30 * s, hz - 2 * s)
            cr.line_to(x + 22 * s, hz + 5 * s)
            cr.line_to(x - 24 * s, hz + 5 * s)
            cr.close_path()
            cr.fill()
            cr.rectangle(x - 1 * s, hz - 48 * s, 2 * s, 46 * s)
            cr.fill()
            cr.set_source_rgba(0.85 * (0.4 + 0.6 * L), 0.82 * (0.4 + 0.6 * L), 0.75 * (0.4 + 0.6 * L), 0.85)
            for top, bot, wd in ((-46, -26, 20), (-24, -6, 26)):
                cr.move_to(x + 2 * s, hz + top * s)
                cr.line_to(x + 2 * s + wd * s, hz + bot * s)
                cr.line_to(x + 2 * s, hz + bot * s)
                cr.close_path()
                cr.fill()
        cr.set_source_rgba(0.95 * (0.3 + 0.7 * L), 0.95 * (0.3 + 0.7 * L), 0.95 * (0.3 + 0.7 * L), 0.85)
        cr.set_line_width(2 * s)
        for g in self.gulls:  # seagulls
            flap = math.sin(t * 6 + g["ph"]) * 5 * s
            cr.move_to(g["x"] - 12 * s, g["y"] - flap)
            cr.curve_to(g["x"] - 6 * s, g["y"] - 7 * s, g["x"] - 2 * s, g["y"] - 3 * s, g["x"], g["y"])
            cr.curve_to(g["x"] + 2 * s, g["y"] - 3 * s, g["x"] + 6 * s, g["y"] - 7 * s, g["x"] + 12 * s, g["y"] - flap)
            cr.stroke()
        for b in self.bottles:  # messages in bottles drifting away
            if b["age"] < 0:
                continue
            fade = min(1.0, b["age"] * 2) * min(1.0, (b["life"] - b["age"]) / 2)
            bx, by = b["x"], b["y"] + math.sin(t * 2.5 + b["ph"]) * 2 * s
            cr.save()
            cr.translate(bx, by)
            cr.rotate(0.35 + math.sin(t * 2 + b["ph"]) * 0.15)
            cr.set_source_rgba(0.35, 0.65, 0.45, 0.85 * fade)
            cr.rectangle(-11 * s, -4 * s, 16 * s, 8 * s)
            cr.fill()
            cr.rectangle(5 * s, -2 * s, 6 * s, 4 * s)
            cr.fill()
            cr.set_source_rgba(0.6, 0.45, 0.3, fade)
            cr.rectangle(10 * s, -2 * s, 3 * s, 4 * s)
            cr.fill()
            cr.set_source_rgba(0.95, 0.92, 0.8, 0.9 * fade)  # the rolled message inside
            cr.rectangle(-8 * s, -1.5 * s, 10 * s, 3 * s)
            cr.fill()
            cr.restore()
            lay = self.text.layout(cr, b["label"], 15 * s, Weight.SEMIBOLD)
            tw, th = lay.get_pixel_size()
            Text.show(cr, lay, bx - tw / 2, by - 14 * s - th, lighten(b["c"], 0.3), 0.95 * fade, shadow=0.7)
        for a in sorted(self.actors.values(), key=lambda a: a.y):
            a.draw(cr, t)
