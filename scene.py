"""Shared building blocks for all themes.

A theme is a Scene subclass with an Actor subclass (one actor per Claude Code session).
The base classes handle session bookkeeping, state colours, tool-call detection, labels,
the "needs your input" banner, the bottom status strip with clock and the wake hint, so
every theme reads the same way:
  working = teal, idle = blue, waiting ("needs you") = amber, blocked = red.

Theme hooks:  Scene.setup_world / update_world / draw_world / on_tool,  Actor.move / draw
"""
import math
import random
import sys
import time

import cairo  # noqa: F401  (re-exported for themes)

try:  # Linux: Pango gives the best text; elsewhere (Windows, macOS) Cairo's own text is used
    import gi

    gi.require_version("Pango", "1.0")
    gi.require_version("PangoCairo", "1.0")
    from gi.repository import Pango, PangoCairo  # noqa: E402
    HAVE_PANGO = True
except (ImportError, ValueError):
    HAVE_PANGO = False

if HAVE_PANGO:
    FONT, MONO = "Ubuntu Sans", "Ubuntu Sans Mono"
elif sys.platform == "darwin":
    FONT, MONO = "HelveticaNeue", "Menlo-Regular"
else:
    FONT, MONO = "Segoe UI", "Consolas"
TAU = math.tau


class Weight:
    """Font weights used by the themes (same numbers as Pango.Weight)."""
    LIGHT, NORMAL, SEMIBOLD, BOLD, HEAVY = 300, 400, 600, 700, 900

COLORS = {
    "working": (0.30, 0.96, 0.84),
    "idle": (0.42, 0.60, 1.00),
    "waiting": (1.00, 0.70, 0.22),
    "blocked": (1.00, 0.30, 0.36),
    "rest": (0.62, 0.62, 0.78),  # the resting creature shown when no session is running
    "background": (0.76, 0.55, 1.00),  # turn finished, a background task (e.g. test suite) still runs
}
LABELS = {"working": "working", "idle": "idle", "waiting": "needs you", "blocked": "blocked",
          "background": "background"}
SIZE = {"working": 1.0, "idle": 0.9, "waiting": 1.45, "blocked": 0.95, "rest": 0.9}
REST_ID = "__rest__"
ORDER = {"waiting": 0, "blocked": 1, "working": 2, "background": 2, "idle": 3}


def look(info):
    """State for colour and label. A session running a background task moves like a working one
    (its state is "working") but is shown in its own colour as "background"."""
    return "background" if info.get("background") else info.get("state", "idle")
CLAUDE_ORANGE = (0.85, 0.47, 0.34)


def lerp(a, b, t):
    return a + (b - a) * t


def lerp3(c1, c2, t):
    return tuple(lerp(x, y, t) for x, y in zip(c1, c2))


def lighten(c, f):
    return lerp3(c, (1.0, 1.0, 1.0), f)


def clamp(v, lo, hi):
    return max(lo, min(hi, v))


def angle_lerp(a, b, t):
    d = (b - a + math.pi) % TAU - math.pi
    return a + d * t


def fmt_duration(sec):
    sec = max(0, int(sec))
    if sec < 60:
        return f"{sec}s"
    m, s = divmod(sec, 60)
    if m < 60:
        return f"{m}m {s:02d}s"
    h, m = divmod(m, 60)
    return f"{h}h {m:02d}m"


def rounded_rect(cr, x, y, w, h, r):
    cr.new_sub_path()
    cr.arc(x + w - r, y + r, r, -math.pi / 2, 0)
    cr.arc(x + w - r, y + h - r, r, 0, math.pi / 2)
    cr.arc(x + r, y + h - r, r, math.pi / 2, math.pi)
    cr.arc(x + r, y + r, r, math.pi, 1.5 * math.pi)
    cr.close_path()


def pulse_rings(cr, x, y, R, c, a, t, s):
    """Expanding rings used to mark a session that needs you."""
    for k in range(2):
        f = ((t * 0.6) + k * 0.5) % 1.0
        cr.set_source_rgba(c[0], c[1], c[2], (1 - f) * 0.55 * a)
        cr.set_line_width(3 * s * (1 - f) + 0.5)
        cr.arc(x, y, R * (1.2 + 2.0 * f), 0, TAU)
        cr.stroke()


_GLOW_SPRITES = {}


def glow(cr, x, y, r, c, alpha, inner=0.0):
    """Soft radial glow: colour c, opacity `alpha` in the centre fading linearly to 0 at radius r.
    `inner` (0..1, fraction of r) keeps the opacity flat up to that radius before it fades.
    Worth it for big round glows; small flat ellipses (glow on the ground) are cheaper to fill directly.

    Same picture as a two-stop RadialGradient + arc + fill, but the gradient is rendered once into a
    small sprite per (colour, size) and then only blitted - about 15x cheaper than filling a gradient.
    The sprite size is rounded to steps of 6 px and scaled by the (tiny) remainder."""
    if alpha <= 0.004 or r < 1:
        return
    rq = max(6, int(round(r / 6.0)) * 6)
    key = (round(c[0], 2), round(c[1], 2), round(c[2], 2), rq, round(inner, 2))
    sp = _GLOW_SPRITES.get(key)
    if sp is None:
        if len(_GLOW_SPRITES) > 128:  # colours can animate; never let the cache grow without bound
            _GLOW_SPRITES.clear()
        sp = cairo.ImageSurface(cairo.FORMAT_ARGB32, 2 * rq, 2 * rq)
        sc = cairo.Context(sp)
        g = cairo.RadialGradient(rq, rq, 0, rq, rq, rq)
        g.add_color_stop_rgba(0, key[0], key[1], key[2], 1.0)
        if key[4] > 0:
            g.add_color_stop_rgba(key[4], key[0], key[1], key[2], 1.0)
        g.add_color_stop_rgba(1, key[0], key[1], key[2], 0.0)
        sc.set_source(g)
        sc.paint()
        _GLOW_SPRITES[key] = sp
    k = r / rq
    cr.save()
    cr.translate(x, y)
    cr.scale(k, k)
    cr.set_source_surface(sp, -rq, -rq)
    cr.paint_with_alpha(alpha)
    cr.restore()


class PangoText:
    """Text via Pango (Linux)."""

    def __init__(self):
        self._fonts = {}

    def _font(self, size, weight, family):
        key = (int(round(size)), weight, family)
        f = self._fonts.get(key)
        if f is None:
            f = Pango.FontDescription()
            f.set_family(family)
            f.set_absolute_size(max(1, key[0]) * Pango.SCALE)
            f.set_weight(Pango.Weight(int(weight)))
            self._fonts[key] = f
        return f

    def layout(self, cr, text, size, weight=Weight.NORMAL, max_width=None, family=FONT):
        lay = PangoCairo.create_layout(cr)
        lay.set_font_description(self._font(size, weight, family))
        if max_width:
            lay.set_width(int(max_width * Pango.SCALE))
            lay.set_ellipsize(Pango.EllipsizeMode.END)
        lay.set_text(text, -1)
        return lay

    @staticmethod
    def show(cr, lay, x, y, rgb, alpha, shadow=0.0):
        if shadow > 0:
            cr.move_to(x + 1, y + 2)
            cr.set_source_rgba(0, 0, 0, shadow * alpha)
            PangoCairo.show_layout(cr, lay)
        cr.move_to(x, y)
        cr.set_source_rgba(rgb[0], rgb[1], rgb[2], alpha)
        PangoCairo.show_layout(cr, lay)
        cr.new_path()  # drop the current point, or the next arc gets connected to the text


class _ToyLayout:
    __slots__ = ("text", "face", "bold", "size", "ascent", "width", "height", "mask")

    def __init__(self, text, face, bold, size, ascent, width, height):
        self.text, self.face, self.bold, self.size = text, face, bold, size
        self.ascent, self.width, self.height = ascent, width, height
        self.mask = None  # the rendered text, made on first use

    def get_pixel_size(self):
        return int(self.width + 0.5), int(self.height + 0.5)


class ToyText:
    """Text via Cairo's built-in font support (Windows, macOS) - same interface as PangoText."""
    if sys.platform == "darwin":  # Cairo's Quartz backend finds fonts by PostScript name
        FACES = {Weight.LIGHT: ("HelveticaNeue-Light", False), Weight.NORMAL: ("HelveticaNeue", False),
                 Weight.SEMIBOLD: ("HelveticaNeue-Medium", False), Weight.BOLD: ("HelveticaNeue-Bold", False),
                 Weight.HEAVY: ("HelveticaNeue-Bold", False)}
        SYMBOLS = "AppleSymbols"
    else:
        FACES = {Weight.LIGHT: ("Segoe UI Light", False), Weight.NORMAL: ("Segoe UI", False),
                 Weight.SEMIBOLD: ("Segoe UI Semibold", False), Weight.BOLD: ("Segoe UI", True),
                 Weight.HEAVY: ("Segoe UI Black", False)}
        SYMBOLS = "Segoe UI Symbol"

    _faces = {}

    def __init__(self):
        self._cache = {}

    @classmethod
    def _select(cls, cr, face, bold, size):
        key = (face, bold)
        ff = cls._faces.get(key)
        if ff is None:
            ff = cls._faces[key] = cairo.ToyFontFace(face, cairo.FONT_SLANT_NORMAL,
                                                     cairo.FONT_WEIGHT_BOLD if bold else cairo.FONT_WEIGHT_NORMAL)
        cr.set_font_face(ff)
        cr.set_font_size(size)

    @classmethod
    def _mask(cls, lay):
        """Text rendered once into an alpha mask and reused every frame (Cairo text on Windows is slow).
        The mask lives on the layout, and layouts belong to one scene, so threads never share one."""
        if lay.mask is None:
            m = cairo.ImageSurface(cairo.FORMAT_A8, max(1, int(lay.width) + 6), max(1, int(lay.height) + 6))
            c = cairo.Context(m)
            cls._select(c, lay.face, lay.bold, lay.size)
            c.move_to(3, 3 + lay.ascent)
            c.show_text(lay.text)
            lay.mask = m
        return lay.mask

    def layout(self, cr, text, size, weight=Weight.NORMAL, max_width=None, family=FONT):
        key = (text, round(size, 1), weight, max_width and round(max_width), family)
        lay = self._cache.get(key)
        if lay is not None:
            return lay
        if family == FONT:
            face, bold = self.FACES.get(weight, (FONT, weight >= Weight.BOLD))
        else:
            face, bold = family, weight >= Weight.SEMIBOLD
        if "✳" in text:  # the Claude asterisk is not in the normal UI font
            face = self.SYMBOLS
        self._select(cr, face, bold, size)
        ascent, descent = cr.font_extents()[:2]
        width = cr.text_extents(text).x_advance
        if max_width and width > max_width:  # ellipsize like Pango does
            while text and width > max_width:
                text = text[:-1]
                width = cr.text_extents(text.rstrip() + "…").x_advance
            text = text.rstrip() + "…"
        lay = _ToyLayout(text, face, bold, size, ascent, width, ascent + descent)
        if len(self._cache) > 3000:
            self._cache.clear()
        self._cache[key] = lay
        return lay

    @staticmethod
    def show(cr, lay, x, y, rgb, alpha, shadow=0.0):
        m = ToyText._mask(lay)
        if shadow > 0:
            cr.set_source_rgba(0, 0, 0, shadow * alpha)
            cr.mask_surface(m, x + 1 - 3, y + 2 - 3)
        cr.set_source_rgba(rgb[0], rgb[1], rgb[2], alpha)
        cr.mask_surface(m, x - 3, y - 3)
        cr.new_path()


Text = PangoText if HAVE_PANGO else ToyText


class Actor:
    """One Claude Code session on screen."""
    BASE_RADIUS = 50

    def __init__(self, scene, info, x, y):
        self.scene = scene
        self.info = info
        self.x, self.y = x, y
        self.vx = self.vy = 0.0
        self.rng = scene.rng
        self.seed = self.rng.uniform(0, 100)
        self.heading = self.rng.uniform(0, TAU)
        self.phase = self.rng.uniform(0, TAU)
        st = info.get("state", "idle")
        self.color = COLORS.get(look(info), COLORS["idle"])
        self.size_mul = SIZE.get(st, 1.0)
        self.alpha = 0.0
        self.dying = False
        self.seen = set()
        self.first = True
        self.radius = self.BASE_RADIUS * scene.s * self.size_mul
        self.flicker = 1.0

    @property
    def state(self):
        return self.info.get("state", "idle")

    def update(self, dt, t, target):
        st = self.state
        self.color = lerp3(self.color, COLORS.get(look(self.info), COLORS["idle"]), min(1.0, dt * 2.0))
        breathe = 1 + (0.06 * math.sin(t * 3.0) if st == "waiting" else 0)
        self.size_mul = lerp(self.size_mul, SIZE.get(st, 1.0) * breathe, min(1.0, dt * 1.5))
        self.alpha = lerp(self.alpha, 0.0 if self.dying else (0.8 if st == "rest" else 1.0), min(1.0, dt * 1.2))
        self.radius = self.BASE_RADIUS * self.scene.s * self.size_mul
        self.flicker = (0.55 + 0.45 * self.rng.random()) if st == "blocked" else 1.0
        self.move(dt, t, target)
        new = []
        for ev in self.info.get("tool_events") or []:
            if ev[0] not in self.seen:
                self.seen.add(ev[0])
                if not self.first:
                    new.append(ev)
        self.first = False
        for i, ev in enumerate(new[-3:]):
            self.scene.on_tool(self, ev[1], i)

    # --- theme hooks
    def move(self, dt, t, target):
        pass

    def draw(self, cr, t):
        pass

    # --- shared motion: wander inside a box, avoid the others, swim to target when waiting
    def wander(self, dt, t, target, speed, turn, box, spacing=2.8):
        s = self.scene.s
        left, top, right, bottom = box
        self.heading += (math.sin(t * 0.31 + self.seed) * 0.9 + math.sin(t * 0.17 + self.seed * 2.3) * 0.6) \
            * dt * turn
        dx, dy = math.cos(self.heading) * speed, math.sin(self.heading) * speed * 0.7
        if target is not None:
            ox, oy = target[0] - self.x, target[1] - self.y
            d = math.hypot(ox, oy) or 1.0
            sp = min(speed * 2.0, d * 1.2)
            dx = ox / d * sp + math.cos(t * 0.8 + self.seed) * s * 18
            dy = oy / d * sp + math.sin(t * 1.1 + self.seed) * s * 12
        px = py = 0.0
        if self.x < left:
            px += (left - self.x) * 3
        if self.x > right:
            px -= (self.x - right) * 3
        if self.y < top:
            py += (top - self.y) * 3
        if self.y > bottom:
            py -= (self.y - bottom) * 3
        for o in self.scene.actors.values():
            if o is self or o.dying:
                continue
            ox, oy = self.x - o.x, self.y - o.y
            d = math.hypot(ox, oy)
            md = (self.radius + o.radius) * spacing
            if 0.01 < d < md:
                f = (md - d) * 2.0
                px += ox / d * f
                py += oy / d * f
        dx += px
        dy += py
        if (px or py) and target is None:
            self.heading = angle_lerp(self.heading, math.atan2(dy, dx), min(1.0, dt * 1.5))
        k = min(1.0, dt * 1.2)
        self.vx = lerp(self.vx, dx, k)
        self.vy = lerp(self.vy, dy, k)
        self.x += self.vx * dt
        self.y += self.vy * dt

    # --- sub-agents: small companions orbiting the creature
    COMPANION_SCALE = 0.42

    def companion_spots(self, t):
        n = len(self.info.get("subagents") or [])
        R = self.radius
        for i in range(n):
            ang = t * 0.6 + i * TAU / n + self.seed
            yield (self.x + math.cos(ang) * R * 1.9, self.y + math.sin(ang) * R * 0.55 - R * 0.15, ang)

    def draw_companions(self, cr, t):
        """Default: a shrunken copy of the creature itself in the working colour (baby jellyfish, lambs,
        moons, ...). Themes can override this."""
        if self.alpha < 0.05 or not self.info.get("subagents"):
            return
        saved = self.__dict__.copy()
        try:
            for x, y, ang in self.companion_spots(t):
                self.x, self.y = x, y
                self.vx, self.vy = -math.sin(ang) * 40 * self.scene.s, 0.0
                self.radius = saved["radius"] * self.COMPANION_SCALE
                self.color = COLORS["working"]
                self.info = {"state": "working", "id": saved["info"].get("id")}
                for attr, val in (("trail", []), ("tongue", 0.0), ("hop", 0.0), ("male", 0.0), ("graze", 0.0),
                                  ("dig", 0.0), ("upright", 0.0), ("flicker", 1.0)):
                    if attr in saved:
                        setattr(self, attr, val)
                if "face" in saved:
                    self.face = 1.0 if self.vx >= 0 else -1.0
                self.draw(cr, t)
        except Exception:  # a theme detail must never break the screensaver
            pass
        finally:
            self.__dict__.clear()
            self.__dict__.update(saved)

    # --- shared label
    def label_layout(self, cr, show_text):
        """Build the label text. Returns (lines, sizes, width, height) or None."""
        if self.alpha < 0.05 or self.state == "rest":
            return None
        sc = self.scene
        if not (0 <= self.x <= sc.w and 0 <= self.y <= sc.h - sc.strip_h):
            return None  # off screen (flying themes wrap around the edges)
        scene = self.scene
        s, T, info = scene.s, scene.text, self.info
        st = self.state
        vis = look(info)
        c = COLORS.get(vis, COLORS["idle"])
        maxw = 380 * s
        lines = [(T.layout(cr, info.get("name", "?"), 21 * s, Weight.BOLD, maxw), (1, 1, 1), 0.95)]
        status = f"● {LABELS.get(vis, vis)} · {fmt_duration(time.time() - info.get('since', time.time()))}"
        lines.append((T.layout(cr, status, 16 * s, Weight.SEMIBOLD, maxw), c, 1.0))
        if show_text:
            detail = ""
            if st == "waiting":
                detail = f"waiting for {info['waiting_for']}" if info.get("waiting_for") else "waiting for your input"
            elif vis == "background" and info.get("bg_task"):
                detail = f"running: {info['bg_task']}"  # plain text: the arrow symbol is missing in some fonts
            elif st == "working" and info.get("tool"):
                detail = f"{info['tool']}  {info.get('tool_detail', '')}".strip()
            elif info.get("last_text"):
                detail = f"“{info['last_text']}”"
            if detail:
                lines.append((T.layout(cr, detail, 14 * s, Weight.NORMAL, maxw), (0.85, 0.92, 1.0), 0.8))
        subs = info.get("subagents") or []
        if subs:
            names = ", ".join(a["description"] or a["type"] for a in subs)
            lines.append((T.layout(cr, f"{len(subs)} sub-agent{'s' if len(subs) > 1 else ''}: {names}", 14 * s,
                                   Weight.NORMAL, maxw), lighten(COLORS["working"], 0.3), 0.85))
        if info.get("cwd"):
            lines.append((T.layout(cr, info["cwd"], 13 * s, Weight.NORMAL, maxw), (0.7, 0.8, 0.9), 0.55))

        sizes = [lay.get_pixel_size() for lay, _, _ in lines]
        w = max(sz[0] for sz in sizes)
        h = sum(sz[1] for sz in sizes) + 3 * s * (len(lines) - 1)
        return lines, sizes, w, h

    def label_candidates(self, pw, ph):
        """Possible top-left corners for the label box, best first."""
        R = self.radius
        right, left = self.x + R * 1.5, self.x - R * 1.5 - pw
        y = self.y - R * 0.9
        return [(right, y), (left, y), (right, y + ph * 0.8), (left, y + ph * 0.8),
                (right, y - ph * 0.8), (left, y - ph * 0.8), (self.x - pw / 2, self.y + R * 1.8)]

    def draw_label(self, cr, lab, lx, ly):
        scene = self.scene
        s, a = scene.s, self.alpha
        c = COLORS.get(look(self.info), COLORS["idle"])
        lines, sizes, w, h = lab
        pad = 12 * s
        bg = scene.label_bg
        cr.set_source_rgba(bg[0], bg[1], bg[2], 0.6 * a)
        rounded_rect(cr, lx, ly, w + pad * 2, h + pad * 2, 10 * s)
        cr.fill()
        cr.set_source_rgba(c[0], c[1], c[2], 0.75 * a)
        cr.rectangle(lx, ly + 8 * s, 3 * s, h + pad * 2 - 16 * s)
        cr.fill()
        yy = ly + pad
        for (lay, rgb, la), sz in zip(lines, sizes):
            Text.show(cr, lay, lx + pad + 4 * s, yy, rgb, la * a)
            yy += sz[1] + 3 * s


class Walker(Actor):
    """An animal walking on a ground plane (pasture, seabed, forest).

    y is the depth: further down = closer = bigger. Idle animals keep stopping to graze/rest,
    working ones walk briskly, waiting ones come to the front. Provides:
      self.step   gait phase (advances with walking speed)
      self.face   -1..1 smooth facing (right = +1)
      self.graze  0..1 (head down)
      self.hop    0..1 little jump after a tool call
    """
    SPEED = {"working": 75, "idle": 22, "waiting": 45, "blocked": 0}
    TURN = {"working": 1.0, "idle": 0.5, "waiting": 0.5, "blocked": 0.1}

    def __init__(self, scene, info, x, y):
        super().__init__(scene, info, x, y)
        self.step = 0.0
        self.face = 1.0
        self.graze = 0.0
        self.hop = 0.0
        self.moving = 0.0

    def update(self, dt, t, target):
        super().update(dt, t, target)
        self.radius *= self.depth_scale()

    def companion_spots(self, t):
        """On the ground the young ones trot along behind the parent, like ducklings."""
        n = len(self.info.get("subagents") or [])
        R, back = self.radius, -1.0 if self.face >= 0 else 1.0
        for i in range(n):
            wob = math.sin(t * 3 + i * 1.7 + self.seed) * R * 0.08
            yield (self.x + back * R * (1.25 + i * 0.75) + wob, self.y + R * (0.25 + 0.18 * (i % 2)), 0.0)

    def depth_scale(self):
        sc = self.scene
        top, bottom = sc.ground_top, sc.h - sc.strip_h
        return lerp(0.65, 1.15, clamp((self.y - top) / max(1.0, bottom - top), 0.0, 1.0))

    def move(self, dt, t, target):
        sc, s, st = self.scene, self.scene.s, self.state
        speed = 0.0 if st == "rest" else self.SPEED.get(st, 20) * s
        if st == "idle":  # walk a bit, stop and graze, walk again
            speed *= clamp(math.sin(t * 0.21 + self.seed) * 1.6, 0.0, 1.0)
        r = self.radius
        self.wander(dt, t, target, speed, self.TURN.get(st, 0.5),
                    (90 * s + r, sc.ground_top + r * 0.3, sc.w - 90 * s - r, sc.h - sc.strip_h - r * 1.1),
                    spacing=2.2)
        self.vy *= 0.97  # depth changes slowly
        sp = math.hypot(self.vx, self.vy)
        self.moving = clamp(sp / (25 * s), 0.0, 1.0)
        self.step += dt * sp / max(1.0, r * 0.35)
        if target is not None and math.hypot(target[0] - self.x, target[1] - self.y) < 80 * s:
            self.face = lerp(self.face, 1.0, min(1.0, dt * 2))  # arrived: stand still, no flipping
        elif abs(self.vx) > 12 * s:
            self.face = lerp(self.face, 1.0 if self.vx > 0 else -1.0, min(1.0, dt * 4))
        grazing = st == "idle" and sp < 10 * s
        self.graze = lerp(self.graze, 1.0 if grazing else 0.0, min(1.0, dt * 1.5))
        self.hop = max(0.0, self.hop - dt * 2.5)


class Scene:
    name = "base"
    actor_class = Actor
    label_bg = (0.01, 0.04, 0.08)
    empty_color = (0.7, 0.85, 1.0)
    waiting_row = 0.42   # where sessions that need you gather (fraction of the height)
    ground_top = 0.0     # for Walker themes

    def __init__(self, show_text=True, seed=None, decor=False):
        self.rng = random.Random(seed)
        self.show_text = show_text
        self.decor = decor  # extra monitor: creatures only, no labels/banner/strip/panel
        self.text = Text()
        self.t = 0.0
        self.w = self.h = 0
        self.s = 1.0
        self.strip_h = 64
        self.sessions = []
        self.actors = {}
        self.hint = ""
        self.hint_until = 0.0
        self.floaters = []
        self.system = None  # CPU/memory history from sysmon, or None = no panel

    def set_system(self, data):
        self.system = data

    def spawn_floater(self, x, y, label, color, style="text", vy=40.0, life=4.0):
        """Rising tool-call label. style: text | bubble | firefly"""
        self.floaters.append({"x0": x, "x": x, "y": y, "label": label, "c": color, "style": style,
                              "vy": vy * self.s, "life": life, "age": 0.0, "ph": self.rng.uniform(0, TAU)})

    # ------------------------------------------------------------------ data
    def set_sessions(self, sessions):
        self.sessions = list(sessions)
        live = set()
        # no session running -> one resting creature (no label, not in the status strip)
        infos = self.sessions or [{"id": REST_ID, "state": "rest", "name": "", "since": time.time()}]
        for info in infos:
            sid = info["id"]
            live.add(sid)
            a = self.actors.get(sid)
            if a is None:
                x, y = self._start_point(info) if self.w else (400, 300)
                a = self.actors[sid] = self.actor_class(self, info, x, y)
                a.placed = bool(self.w)  # without a size yet: placed in draw() once it is known
            else:
                a.info = info
                a.dying = False
        for sid, a in self.actors.items():
            if sid not in live:
                a.dying = True

    def show_hint(self, text, seconds=4.0):
        self.hint = text
        self.hint_until = self.t + seconds

    def spawn_point(self):
        return self.rng.uniform(self.w * 0.2, self.w * 0.8), self.rng.uniform(self.h * 0.25, self.h * 0.6)

    def rest_point(self):
        """Where the resting creature lies: front and centre on ground themes."""
        if self.ground_top:
            return self.w * self.rng.uniform(0.3, 0.42), self.h - self.strip_h - 150 * self.s
        return self.spawn_point()

    def _start_point(self, info):
        return self.rest_point() if info.get("state") == "rest" else self.spawn_point()

    def waiting_targets(self, waiting):
        n = len(waiting)
        return {id(a): (self.w / 2 + (i - (n - 1) / 2) * self.w * 0.28, self.h * self.waiting_row)
                for i, a in enumerate(waiting)}

    def sorted_actors(self):
        return sorted(self.actors.values(), key=lambda a: (a.state == "waiting", a.y))

    # ------------------------------------------------------------------ theme hooks
    def setup_world(self):
        pass

    def update_world(self, dt):
        pass

    def draw_world(self, cr, t):
        for a in self.sorted_actors():
            a.draw(cr, t)

    def on_tool(self, actor, name, index):
        pass

    # ------------------------------------------------------------------ main loop
    def update(self, dt):
        if not self.w:
            return
        dt = clamp(dt, 0.0, 0.1)
        self.t += dt
        waiting = sorted((a for a in self.actors.values() if a.state == "waiting" and not a.dying),
                         key=lambda a: a.info.get("id"))
        targets = self.waiting_targets(waiting)
        for sid, a in list(self.actors.items()):
            a.update(dt, self.t, targets.get(id(a)))
            if a.dying and a.alpha < 0.02:
                del self.actors[sid]
        for f in self.floaters:
            f["age"] += dt
            f["y"] -= f["vy"] * dt
            f["x"] = f["x0"] + math.sin(f["age"] * 2.0 + f["ph"]) * 10 * self.s
        self.floaters = [f for f in self.floaters if f["age"] < f["life"]]
        self.update_world(dt)

    def draw(self, cr, w, h):
        if (w, h) != (self.w, self.h):
            self.w, self.h = w, h
            self.s = clamp(min(h / 1200, w / 1600), 0.4, 3.0)
            self.strip_h = 64 * self.s
            self.setup_world()
            for a in self.actors.values():
                if not getattr(a, "placed", True):
                    a.x, a.y = self._start_point(a.info)
                    a.placed = True
        t, s = self.t, self.s
        self.draw_world(cr, t)
        for a in self.sorted_actors():
            a.draw_companions(cr, t)
        self._draw_floaters(cr, t)
        if self.decor:
            return
        self._draw_labels(cr)
        self._draw_sysmon(cr, t)
        self._draw_banner(cr, t)
        if not self.sessions:
            lay = self.text.layout(cr, "No Claude Code sessions running", 28 * s, Weight.LIGHT)
            tw, _ = lay.get_pixel_size()
            x = w / 2 - tw / 2 + math.sin(t * 0.13) * w * 0.25
            y = h * 0.45 + math.sin(t * 0.21) * h * 0.18
            Text.show(cr, lay, x, y, self.empty_color, 0.5)
        self._draw_strip(cr, t)
        self._draw_hint(cr, t)

    # ------------------------------------------------------------------ shared overlays
    def _draw_labels(self, cr):
        """Place labels so they do not cover each other; sessions that need you get the best spot.
        Each actor keeps its last spot while it is free, so labels do not jump around."""
        s = self.s
        pad, gap = 12 * s, 8 * s
        placed = [self._sysmon_rect()] if self.system else []  # keep labels off the monitor panel
        lo_x, lo_y = 20 * s, 20 * s
        for a in sorted(self.actors.values(), key=lambda a: (ORDER.get(a.state, 9), a.info.get("id", ""))):
            lab = a.label_layout(cr, self.show_text)
            if lab is None:
                continue
            pw, ph = lab[2] + pad * 2, lab[3] + pad * 2
            hi_x, hi_y = self.w - pw - 20 * s, self.h - self.strip_h - ph - 10 * s
            cands = a.label_candidates(pw, ph)
            prev = getattr(a, "label_choice", 0)
            order = [prev] + [i for i in range(len(cands)) if i != prev]
            choice = None
            for i in order:
                x, y = clamp(cands[i][0], lo_x, hi_x), clamp(cands[i][1], lo_y, hi_y)
                if all(x + pw + gap <= bx or bx + bw + gap <= x or y + ph + gap <= by or by + bh + gap <= y
                       for bx, by, bw, bh in placed):
                    choice = (i, x, y)
                    break
            if choice is None:
                choice = (0, clamp(cands[0][0], lo_x, hi_x), clamp(cands[0][1], lo_y, hi_y))
            a.label_choice = choice[0]
            placed.append((choice[1], choice[2], pw, ph))
            # Glide to a new spot instead of jumping. The offset to the creature is smoothed (not the
            # absolute position), so the label still follows the creature exactly while it moves.
            tx, ty = choice[1] - a.x, choice[2] - a.y
            last = getattr(a, "label_off", None)
            dtl = self.t - getattr(a, "label_t", self.t)
            a.label_t = self.t
            if last is None or dtl < 0 or dtl > 0.5:
                ox, oy = tx, ty                      # first frame or after a pause: no animation
            else:
                k = 1.0 - math.exp(-dtl * 5.0)       # ~90 % of the way in half a second
                ox, oy = lerp(last[0], tx, k), lerp(last[1], ty, k)
            a.label_off = (ox, oy)
            a.draw_label(cr, lab, clamp(a.x + ox, lo_x, hi_x), clamp(a.y + oy, lo_y, hi_y))

    SYS_W, SYS_H = 300, 192

    def _sysmon_rect(self):
        s = self.s
        ox = math.sin(self.t / 53) * 8 * s  # slow drift against burn-in
        oy = math.sin(self.t / 41) * 5 * s
        w, h = self.SYS_W * s, self.SYS_H * s
        return self.w - w - 28 * s + ox, 24 * s + oy, w, h

    _sys_cache = None  # (key, surface) - the panel only changes when a new sample arrives

    def _draw_sysmon(self, cr, t):
        """Small task-manager panel: CPU % and memory GB with a 30-minute graph each.
        Rendered into a surface once per new sample and then blitted (at whole pixels) every frame."""
        d = self.system
        if not d or not d.get("cpu"):
            return
        x, y, w, h = self._sysmon_rect()
        key = (d.get("time"), len(d["cpu"]), d["cpu"][-1], d["mem"][-1] if d.get("mem") else 0, self.s)
        if self._sys_cache is None or self._sys_cache[0] != key:
            surf = cairo.ImageSurface(cairo.FORMAT_ARGB32, int(w) + 4, int(h) + 4)
            self._paint_sysmon(cairo.Context(surf), 2, 2, w, h, d)
            self._sys_cache = (key, surf)
        cr.set_source_surface(self._sys_cache[1], round(x) - 2, round(y) - 2)
        cr.paint()

    def _paint_sysmon(self, cr, x, y, w, h, d):
        s, T = self.s, self.text
        cr.set_source_rgba(0.02, 0.03, 0.06, 0.62)
        rounded_rect(cr, x, y, w, h, 12 * s)
        cr.fill()
        cr.set_source_rgba(1, 1, 1, 0.10)
        cr.set_line_width(1 * s)
        rounded_rect(cr, x, y, w, h, 12 * s)
        cr.stroke()
        pad = 14 * s
        gw, gh = w - 2 * pad, 46 * s
        n_max = 30 * 60 // max(1, d.get("interval", 5))
        gib = 1024 ** 3
        mem_total = d.get("mem_total") or 1
        rows = [
            ("CPU", f"{d['cpu'][-1]:.0f} %", d["cpu"], 100.0, (0.55, 0.85, 1.0)),
            ("Memory", f"{d['mem'][-1] / gib:.1f} / {mem_total / gib:.1f} GB", d["mem"], mem_total,
             (0.78, 0.66, 1.0)),
        ]
        yy = y + pad - 2 * s
        for name, value, series, vmax, col in rows:
            lab = T.layout(cr, name, 13 * s, Weight.SEMIBOLD)
            val = T.layout(cr, value, 15 * s, Weight.BOLD)
            lw, lh = lab.get_pixel_size()
            vw, vh = val.get_pixel_size()
            Text.show(cr, lab, x + pad, yy + (vh - lh) / 2, (0.75, 0.8, 0.9), 0.8)
            Text.show(cr, val, x + w - pad - vw, yy, col, 0.95)
            gy = yy + vh + 4 * s
            self._sparkline(cr, x + pad, gy, gw, gh, series, vmax, n_max, col)
            yy = gy + gh + 10 * s
        foot = T.layout(cr, "last 30 min", 11 * s)
        Text.show(cr, foot, x + pad, yy - 6 * s, (0.7, 0.75, 0.85), 0.45)

    @staticmethod
    def _sparkline(cr, x, y, w, h, series, vmax, n_max, col):
        cr.set_source_rgba(1, 1, 1, 0.05)  # graph background + 50 % line
        cr.rectangle(x, y, w, h)
        cr.fill()
        cr.set_source_rgba(1, 1, 1, 0.08)
        cr.set_line_width(1)
        cr.move_to(x, y + h / 2)
        cr.line_to(x + w, y + h / 2)
        cr.stroke()
        n = len(series)
        if n < 2 or vmax <= 0:
            return
        step = w / max(1, n_max - 1)  # newest sample at the right edge; a short history fills from the right
        pts = [(x + w - (n - 1 - i) * step, y + h - clamp(v / vmax, 0.0, 1.0) * h) for i, v in enumerate(series)]
        cr.move_to(pts[0][0], y + h)
        for p in pts:
            cr.line_to(*p)
        cr.line_to(pts[-1][0], y + h)
        cr.close_path()
        cr.set_source_rgba(col[0], col[1], col[2], 0.18)
        cr.fill()
        cr.move_to(*pts[0])
        for p in pts[1:]:
            cr.line_to(*p)
        cr.set_source_rgba(col[0], col[1], col[2], 0.9)
        cr.set_line_width(1.5)
        cr.set_line_join(cairo.LINE_JOIN_ROUND)
        cr.stroke()

    def _draw_floaters(self, cr, t):
        s = self.s
        for f in self.floaters:
            fade = min(1.0, f["age"] * 3) * min(1.0, (f["life"] - f["age"]) / 1.2)
            x, y, c = f["x"], f["y"], f["c"]
            tx = x + 8 * s
            if f["style"] == "bubble":
                r = 10 * s
                cr.set_source_rgba(0.7, 0.9, 1.0, 0.08 * fade)
                cr.arc(x, y, r, 0, TAU)
                cr.fill_preserve()
                cr.set_source_rgba(0.75, 0.92, 1.0, 0.5 * fade)
                cr.set_line_width(max(1.0, 1.2 * s))
                cr.stroke()
                cr.set_source_rgba(1, 1, 1, 0.6 * fade)
                cr.arc(x - r * 0.35, y - r * 0.35, r * 0.25, 0, TAU)
                cr.fill()
                tx = x + r + 6 * s
            elif f["style"] == "firefly":
                blink = 0.6 + 0.4 * math.sin(f["age"] * 6 + f["ph"])
                g = cairo.RadialGradient(x, y, 0, x, y, 14 * s)
                g.add_color_stop_rgba(0, 1.0, 0.95, 0.6, 0.9 * fade * blink)
                g.add_color_stop_rgba(1, 1.0, 0.9, 0.4, 0)
                cr.set_source(g)
                cr.arc(x, y, 14 * s, 0, TAU)
                cr.fill()
                tx = x + 14 * s
            if not f["label"]:
                continue  # plain bubble (air stone, sleepy crab)
            lay = self.text.layout(cr, f["label"], 15 * s, Weight.SEMIBOLD)
            th = lay.get_pixel_size()[1]
            Text.show(cr, lay, tx, y - th / 2, lighten(c, 0.3), 0.95 * fade, shadow=0.7)

    def _draw_banner(self, cr, t):
        waiting = [a for a in self.actors.values() if a.state == "waiting" and not a.dying]
        if not waiting:
            return
        s = self.s
        if len(waiting) == 1:
            msg = f"{waiting[0].info.get('name', 'A session')} needs your input"
        else:
            msg = f"{len(waiting)} sessions need your input"
        lay = self.text.layout(cr, msg, 34 * s, Weight.BOLD)
        tw, th = lay.get_pixel_size()
        x = self.w / 2 - tw / 2 + math.sin(t * 0.11) * 40 * s
        y = 60 * s + math.sin(t * 0.4) * 8 * s
        px, py = 28 * s, 12 * s
        cr.set_source_rgba(0.02, 0.02, 0.04, 0.55)
        rounded_rect(cr, x - px, y - py, tw + 2 * px, th + 2 * py, (th + 2 * py) / 2)
        cr.fill()
        c = COLORS["waiting"]
        cr.set_source_rgba(c[0], c[1], c[2], 0.35 + 0.25 * math.sin(t * 3.0))
        cr.set_line_width(2 * s)
        rounded_rect(cr, x - px, y - py, tw + 2 * px, th + 2 * py, (th + 2 * py) / 2)
        cr.stroke()
        Text.show(cr, lay, x, y, c, 0.8 + 0.2 * math.sin(t * 3.0), shadow=0.5)

    def _draw_strip(self, cr, t):
        w, h, s, T = self.w, self.h, self.s, self.text
        sh = self.strip_h
        g = cairo.LinearGradient(0, h - sh * 1.4, 0, h)
        g.add_color_stop_rgba(0, 0, 0, 0, 0)
        g.add_color_stop_rgba(0.35, 0, 0.01, 0.02, 0.55)
        g.add_color_stop_rgba(1, 0, 0.01, 0.02, 0.75)
        cr.set_source(g)
        cr.rectangle(0, h - sh * 1.4, w, sh * 1.4)
        cr.fill()
        ox = math.sin(t / 61) * 12 * s  # slow drift so nothing burns in
        oy = math.sin(t / 47) * 4 * s
        cy = h - sh / 2 + oy

        clock = T.layout(cr, time.strftime("%H:%M"), 26 * s, Weight.BOLD)
        date = T.layout(cr, time.strftime("%a %d %b"), 14 * s)
        cw, chh = clock.get_pixel_size()
        dw, dh = date.get_pixel_size()
        rx = w - 36 * s + ox
        Text.show(cr, clock, rx - cw, cy - chh / 2, (0.9, 0.95, 1.0), 0.85)
        Text.show(cr, date, rx - cw - dw - 14 * s, cy - dh / 2, (0.7, 0.8, 0.9), 0.6)

        x = 36 * s + ox
        # Claude asterisk drawn as lines (no symbol font needed on any platform), then the name
        ar = 7 * s
        cr.set_source_rgba(CLAUDE_ORANGE[0], CLAUDE_ORANGE[1], CLAUDE_ORANGE[2], 0.85)
        cr.set_line_width(2 * s)
        cr.set_line_cap(cairo.LINE_CAP_ROUND)
        for k in range(4):
            a = k * math.pi / 4
            cr.move_to(x + ar + math.cos(a) * ar, cy + math.sin(a) * ar)
            cr.line_to(x + ar - math.cos(a) * ar, cy - math.sin(a) * ar)
        cr.stroke()
        x += 2 * ar + 7 * s
        brand = T.layout(cr, "Claude Code", 15 * s, Weight.SEMIBOLD)
        bw_, bh_ = brand.get_pixel_size()
        Text.show(cr, brand, x, cy - bh_ / 2, CLAUDE_ORANGE, 0.85)
        x += bw_ + 34 * s
        limit = rx - cw - dw - 50 * s
        infos = sorted(self.sessions, key=lambda i: (ORDER.get(i.get("state"), 9), i.get("name", "")))
        for info in infos:
            st = look(info)
            c = COLORS.get(st, COLORS["idle"])
            name = T.layout(cr, info.get("name", "?"), 16 * s, Weight.BOLD, 220 * s)
            n_sub = len(info.get("subagents") or [])
            extra = f" · +{n_sub} agent{'s' if n_sub > 1 else ''}" if n_sub else ""
            lab = T.layout(cr, f"{LABELS.get(st, st)} · {fmt_duration(time.time() - info.get('since', time.time()))}{extra}",
                           14 * s)
            nw, nh = name.get_pixel_size()
            lw, lh = lab.get_pixel_size()
            if x + 20 * s + nw + 8 * s + lw > limit:
                more = T.layout(cr, "…", 16 * s)
                Text.show(cr, more, x, cy - more.get_pixel_size()[1] / 2, (1, 1, 1), 0.6)
                break
            pr = 6 * s * ((1 + 0.35 * math.sin(t * 5)) if st == "waiting" else 1)
            cr.set_source_rgba(c[0], c[1], c[2], 0.95)
            cr.arc(x + 6 * s, cy, pr, 0, TAU)
            cr.fill()
            x += 20 * s
            Text.show(cr, name, x, cy - nh / 2, (1, 1, 1), 0.92)
            x += nw + 8 * s
            Text.show(cr, lab, x, cy - lh / 2, c, 0.9)
            x += lw + 34 * s

    def _draw_hint(self, cr, t):
        if not self.hint or t >= self.hint_until:
            return
        s = self.s
        a = min(1.0, (self.hint_until - t) / 1.0)
        lay = self.text.layout(cr, self.hint, 22 * s, Weight.SEMIBOLD)
        tw, th = lay.get_pixel_size()
        pw, ph = tw + 48 * s, th + 24 * s
        x, y = self.w / 2 - pw / 2, self.h * 0.78 - ph / 2
        cr.set_source_rgba(0.01, 0.04, 0.08, 0.75 * a)
        rounded_rect(cr, x, y, pw, ph, ph / 2)
        cr.fill()
        cr.set_source_rgba(0.6, 0.85, 1.0, 0.5 * a)
        cr.set_line_width(1.5 * s)
        rounded_rect(cr, x, y, pw, ph, ph / 2)
        cr.stroke()
        Text.show(cr, lay, x + 24 * s, y + 12 * s, (0.9, 0.96, 1.0), a)
