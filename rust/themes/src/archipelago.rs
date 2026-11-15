//! Archipelago theme: every session is a small island with a castaway, in a wide ocean.
//!
//! The castaway wears a striped shirt in the state colour.
//! working = hammers away at a raft; every tool call is a message in a bottle drifting off with the tool name
//! idle    = sits under the palm and fishes      waiting = stands on the beach waving a flag by a signal fire
//! rest    = sleeps in a hammock
//! The sky follows the real time of day: sun arc, sunset, moon and stars at night. Now and then a sailing
//! ship passes on the horizon and seagulls fly by. (Set VIVARIUM_HOUR=22 to preview the night.)
use vivarium_core::prelude::*;

const SKIN: Rgb = (0.93, 0.74, 0.58);
const HAT: Rgb = (0.86, 0.72, 0.42);
const SHORTS: Rgb = (0.25, 0.32, 0.50);

/// sky top, sky horizon, sea near horizon, sea in front, daylight 0..1
type Palette = (Rgb, Rgb, Rgb, Rgb, f64);

// hour: sky top, sky horizon, sea near horizon, sea in front, daylight 0..1
const KEYS: [(f64, Palette); 8] = [
    (0.0, ((0.02, 0.03, 0.08), (0.06, 0.08, 0.16), (0.04, 0.07, 0.13), (0.01, 0.03, 0.06), 0.25)),
    (5.0, ((0.02, 0.03, 0.08), (0.06, 0.08, 0.16), (0.04, 0.07, 0.13), (0.01, 0.03, 0.06), 0.25)),
    (6.5, ((0.25, 0.30, 0.55), (0.98, 0.66, 0.46), (0.32, 0.36, 0.50), (0.08, 0.18, 0.28), 0.7)),
    (9.0, ((0.22, 0.52, 0.88), (0.72, 0.86, 0.98), (0.12, 0.48, 0.62), (0.02, 0.22, 0.36), 1.0)),
    (16.0, ((0.22, 0.52, 0.88), (0.72, 0.86, 0.98), (0.12, 0.48, 0.62), (0.02, 0.22, 0.36), 1.0)),
    (18.5, ((0.30, 0.30, 0.56), (1.00, 0.60, 0.36), (0.38, 0.32, 0.42), (0.08, 0.12, 0.22), 0.75)),
    (20.5, ((0.05, 0.06, 0.17), (0.22, 0.16, 0.30), (0.06, 0.08, 0.16), (0.02, 0.04, 0.08), 0.35)),
    (24.0, ((0.02, 0.03, 0.08), (0.06, 0.08, 0.16), (0.04, 0.07, 0.13), (0.01, 0.03, 0.06), 0.25)),
];

fn hour_now() -> f64 {
    if let Some(forced) = std::env::var("VIVARIUM_HOUR").ok().and_then(|v| v.trim().parse::<f64>().ok()) {
        return pymod(forced, 24.0);
    }
    local_hours()
}

fn palette(hour: f64) -> Palette {
    for pair in KEYS.windows(2) {
        let ((ha, a), (hb, b)) = (pair[0], pair[1]);
        if ha <= hour && hour <= hb {
            let f = (hour - ha) / (hb - ha).max(1e-6);
            return (lerp3(a.0, b.0, f), lerp3(a.1, b.1, f), lerp3(a.2, b.2, f), lerp3(a.3, b.3, f), lerp(a.4, b.4, f));
        }
    }
    KEYS[0].1
}

/// Scene colours get darker (and a bit bluer) at night.
fn shade(c: Rgb, light: f64) -> Rgb {
    let k = 0.35 + 0.65 * light;
    (c.0 * k, c.1 * k, c.2 * (k + (1.0 - k) * 0.25))
}

#[derive(Clone, Copy, PartialEq)]
enum Item {
    None,
    Hammer,
    Flag,
}

/// How the castaway is drawn (the keyword arguments of the Python `_figure`).
#[derive(Clone, Copy)]
struct Pose {
    sitting: bool,
    lying: bool,
    arm_l: f64,
    arm_r: f64,
    face: f64,
    item: Item,
    item_angle: f64,
}

impl Pose {
    fn new() -> Self {
        Pose { sitting: false, lying: false, arm_l: 0.2, arm_r: -0.2, face: 1.0, item: Item::None, item_angle: 0.0 }
    }
}

/// What the parts of one island share while it is drawn.
#[derive(Clone, Copy)]
struct Isle {
    /// half the width of the island
    r: f64,
    /// size unit of the castaway
    u: f64,
    c: Rgb,
    a: f64,
    /// daylight 0..1
    l: f64,
    s: f64,
}

#[derive(Clone)]
pub struct Castaway {
    b: Base,
    slot: (f64, f64),
}

struct Bottle {
    x: f64,
    y: f64,
    vx: f64,
    age: f64,
    life: f64,
    label: String,
    c: Rgb,
    ph: f64,
}

struct Gull {
    x: f64,
    y: f64,
    ph: f64,
}

pub struct Archipelago {
    bg: Option<Picture>,
    bg_key: Option<i64>,
    horizon: f64,
    light: f64,
    bottles: Vec<Bottle>,
    stars: Vec<[f64; 4]>,
    ship: Option<f64>,
    next_ship: f64,
    gulls: Vec<Gull>,
    next_gulls: f64,
    sun: (f64, f64, f64, Rgb),
    /// the size changed: all islands start again from the middle
    recenter: bool,
}

impl ActorT<Archipelago> for Castaway {
    const BASE_RADIUS: f64 = 140.0; // half the width of the island (labels go beside the island)

    fn new(base: Base, _env: &Env, _world: &mut Archipelago) -> Self {
        let slot = (base.x, base.y);
        Castaway { b: base, slot }
    }

    fn base(&self) -> &Base {
        &self.b
    }

    fn base_mut(&mut self) -> &mut Base {
        &mut self.b
    }

    fn mv(&mut self, env: &mut Env, world: &mut Archipelago, _others: &[Other], dt: f64, _t: f64, _target: Option<(f64, f64)>) {
        let k = (dt * 0.8).min(1.0); // islands glide to a new place when sessions come or go
        self.b.x = lerp(self.b.x, self.slot.0, k);
        self.b.y = lerp(self.b.y, self.slot.1, k);
        self.b.radius *= world.depth_scale(env, self.b.y);
    }

    /// Sub-agents are seagulls circling above the island.
    fn draw_companions(&self, cr: &mut Canvas, env: &Env, _world: &Archipelago, t: f64) {
        let b = &self.b;
        let n = b.info.subagents.len();
        if n == 0 || b.alpha < 0.05 {
            return;
        }
        let (s, r, c) = (env.s, b.radius, WORKING_COLOR);
        for i in 0..n {
            let fi = i as f64;
            let ang = t * 0.8 + fi * TAU / n as f64 + b.seed;
            let x = b.x + ang.cos() * r * 1.0;
            let y = b.y - r * 1.45 + ang.sin() * r * 0.22;
            let mut g = Gradient::radial(x, y, 0.0, x, y, 22.0 * s);
            g.stop(0.0, c, 0.35 * b.alpha);
            g.stop(1.0, c, 0.0);
            cr.set_source(&g);
            cr.circle(x, y, 22.0 * s);
            cr.fill();
            let flap = (t * 7.0 + fi * 2.0).sin() * 6.0 * s;
            let d = if ang.sin() > 0.0 { -1.0 } else { 1.0 }; // direction of flight around the circle
            cr.set_source_rgba(0.97, 0.97, 0.97, b.alpha);
            cr.set_line_width(2.6 * s);
            cr.move_to(x - 15.0 * s, y - flap);
            cr.curve_to(x - 8.0 * s, y - 9.0 * s, x - 3.0 * s, y - 4.0 * s, x, y);
            cr.curve_to(x + 3.0 * s, y - 4.0 * s, x + 8.0 * s, y - 9.0 * s, x + 15.0 * s, y - flap);
            cr.stroke();
            cr.set_source_rgba(1.0, 0.75, 0.2, b.alpha); // beak
            cr.circle(x + d * 2.0 * s, y + 1.0 * s, 1.6 * s);
            cr.fill();
        }
    }

    // ------------------------------------------------------------------ drawing
    fn draw(&self, cr: &mut Canvas, env: &Env, world: &Archipelago, t: f64) {
        let s = env.s;
        let b = &self.b;
        let a = b.alpha * b.flicker;
        if a < 0.01 {
            return;
        }
        let (r, c, st, l) = (b.radius, b.color, b.state(), world.light);
        let (x, y) = (b.x, b.y);
        // island: shadow in the water, foam ring, sand
        cr.ellipse(x, y + r * 0.08, r * 1.15, r * 0.3);
        cr.set_source_rgba(0.0, 0.0, 0.0, 0.18 * a);
        cr.fill();
        let foam = 1.0 + 0.04 * (t * 1.3 + b.seed).sin();
        cr.ellipse(x, y, r * 1.08 * foam, r * 0.26 * foam);
        cr.set_source_rgba(0.9, 0.97, 1.0, 0.45 * a * (0.5 + 0.5 * l));
        cr.set_line_width(2.5 * s);
        cr.stroke();
        let sand = shade((0.93, 0.82, 0.58), l);
        cr.save();
        cr.translate(x, y);
        cr.scale(r, r * 0.24);
        cr.arc(0.0, 0.0, 1.0, PI, TAU); // sand dome above the water line
        cr.restore();
        cr.save();
        cr.translate(x, y);
        cr.scale(r, r * 0.07);
        cr.arc(0.0, 0.0, 1.0, 0.0, PI);
        cr.restore();
        let mut g = Gradient::linear(x, y - r * 0.24, x, y + r * 0.07);
        g.stop(0.0, lighten(sand, 0.25), a);
        g.stop(1.0, (sand.0 * 0.8, sand.1 * 0.8, sand.2 * 0.8), a);
        cr.set_source(&g);
        cr.fill();
        if st == State::Waiting {
            cr.pulse_rings(x, y - r * 0.35, r * 0.45, c, a, t, s);
        }
        let u = r * 0.3; // size unit of the castaway
        let k = Isle { r, u, c, a, l, s };
        let (palm_x, palm_y) = (x + r * 0.38, y - r * 0.14);
        if st == State::Rest {
            self.hammock(cr, t, x, y, k, palm_x, palm_y);
        }
        self.palm(cr, t, palm_x, palm_y, k);
        match st {
            State::Working => self.working(cr, t, x, y, k),
            State::Waiting => self.waiting(cr, t, x, y, k),
            State::Rest => {}
            _ => self.fishing(cr, t, x, y, k, st != State::Blocked),
        }
    }
}

impl Castaway {
    fn palm(&self, cr: &mut Canvas, t: f64, px: f64, py: f64, k: Isle) {
        let (r, a, l, s) = (k.r, k.a, k.l, k.s);
        let seed = self.b.seed;
        let sway = (t * 0.8 + seed).sin() * r * 0.04;
        let (tx, ty) = (px - r * 0.25 + sway, py - r * 1.25);
        cr.set_line_cap(LineCap::Round);
        let trunk = shade((0.50, 0.36, 0.22), l);
        cr.set_rgba(trunk, a);
        cr.set_line_width(r * 0.09);
        cr.move_to(px, py);
        cr.curve_to(px + r * 0.05, py - r * 0.5, px - r * 0.05, py - r * 0.9, tx, ty);
        cr.stroke();
        cr.set_source_rgba(trunk.0 * 0.7, trunk.1 * 0.7, trunk.2 * 0.7, a); // rings on the trunk
        cr.set_line_width(1.2 * s);
        for i in 1..7 {
            let f = i as f64 / 7.0;
            let bx = lerp(px, tx, f) + (f * PI).sin() * r * 0.03;
            let by = lerp(py, ty, f);
            cr.move_to(bx - r * 0.045, by);
            cr.line_to(bx + r * 0.045, by - r * 0.02);
        }
        cr.stroke();
        let leaf = shade((0.20, 0.55, 0.22), l);
        for i in 0..7 {
            // fronds
            let fi = i as f64;
            let ang = -PI / 2.0 + (fi - 3.0) * 0.55 + (t * 1.1 + fi + seed).sin() * 0.06;
            let ln = r * (0.75 - (fi - 3.0).abs() * 0.05);
            let (ex, ey) = (tx + ang.cos() * ln, ty + ang.sin() * ln * 0.6 + ln * 0.35);
            let (mx, my) = (tx + ang.cos() * ln * 0.5, ty + ang.sin() * ln * 0.5 - r * 0.08);
            cr.move_to(tx, ty);
            cr.curve_to(mx, my - r * 0.06, ex, ey - r * 0.08, ex, ey);
            cr.curve_to(ex - ang.cos() * r * 0.1, ey + r * 0.02, mx, my + r * 0.06, tx, ty);
            cr.set_source_rgba(leaf.0 * (0.85 + 0.03 * fi), leaf.1, leaf.2, a);
            cr.fill();
        }
        cr.set_source_rgba(0.35 * l + 0.1, 0.22 * l + 0.05, 0.1, a); // coconuts
        for dx in [-0.06, 0.05] {
            cr.circle(tx + r * dx, ty + r * 0.06, r * 0.045);
            cr.fill();
        }
    }

    /// The castaway (own design: wide straw hat, striped shirt in the state colour).
    /// Arm angles: 0 = hanging down, pi = straight up; positive = forward (in facing direction).
    /// Returns the two hands in the figure's own coordinates.
    #[allow(clippy::too_many_arguments)]
    fn figure(&self, cr: &mut Canvas, fx: f64, fy: f64, u: f64, k: Isle, p: Pose, t: f64) -> [(f64, f64); 2] {
        let (c, a, l) = (k.c, k.a, k.l);
        let (skin, shorts) = (shade(SKIN, l), shade(SHORTS, l));
        cr.save();
        cr.translate(fx, fy);
        if p.lying {
            cr.rotate(-PI / 2.0 * p.face);
        }
        cr.scale(p.face, 1.0);
        cr.set_line_cap(LineCap::Round);
        let (hip, sh, head);
        if p.sitting {
            hip = (0.0, -0.38 * u);
            sh = (0.0, -1.22 * u);
            head = (0.0, -1.66 * u);
            cr.set_rgba(shorts, a);
            cr.set_line_width(0.3 * u);
            cr.move_to(hip.0, hip.1);
            cr.line_to(0.62 * u, -0.38 * u);
            cr.stroke();
            cr.set_rgba(skin, a);
            cr.set_line_width(0.2 * u);
            cr.move_to(0.62 * u, -0.38 * u);
            cr.line_to(0.7 * u, 0.0);
            cr.stroke();
        } else {
            hip = (0.0, -0.95 * u);
            sh = (0.0, -1.8 * u);
            head = (0.0, -2.24 * u);
            for dx in [-0.12, 0.12] {
                cr.set_rgba(skin, a);
                cr.set_line_width(0.18 * u);
                cr.move_to(dx * u, hip.1 + 0.35 * u);
                cr.line_to(dx * u * 1.3, 0.0);
                cr.stroke();
            }
            cr.set_rgba(shorts, a);
            cr.rectangle(-0.24 * u, hip.1 - 0.05 * u, 0.48 * u, 0.42 * u);
            cr.fill();
        }
        // shirt with stripes in the state colour
        cr.save();
        let top = sh.1 - 0.05 * u;
        cr.rectangle(-0.27 * u, top, 0.54 * u, hip.1 - top + 0.05 * u);
        cr.clip_preserve();
        cr.set_rgba(lighten(c, 0.1), a);
        cr.fill();
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.6 * a);
        let mut ky = top;
        while ky < hip.1 {
            cr.rectangle(-0.3 * u, ky + 0.1 * u, 0.6 * u, 0.06 * u);
            ky += 0.17 * u;
        }
        cr.fill();
        cr.restore();
        // arms (+ item in the right hand)
        let mut hands = [(0.0, 0.0); 2];
        for (i, (ang, side)) in [(p.arm_l, -1.0), (p.arm_r, 1.0)].into_iter().enumerate() {
            let (ax, ay) = (side * 0.2 * u, sh.1 + 0.08 * u);
            let (hx, hy) = (ax + ang.sin() * 0.75 * u, ay + ang.cos() * 0.75 * u);
            cr.set_rgba(skin, a);
            cr.set_line_width(0.15 * u);
            cr.move_to(ax, ay);
            cr.line_to(hx, hy);
            cr.stroke();
            hands[i] = (hx, hy);
        }
        let (hx, hy) = hands[1];
        let ia = p.item_angle;
        match p.item {
            Item::Hammer => {
                let (ex, ey) = (hx + ia.sin() * 0.55 * u, hy + ia.cos() * 0.55 * u);
                cr.set_source_rgba(0.45, 0.32, 0.18, a);
                cr.set_line_width(0.08 * u);
                cr.move_to(hx, hy);
                cr.line_to(ex, ey);
                cr.stroke();
                cr.save();
                cr.translate(ex, ey);
                cr.rotate(-ia);
                cr.rectangle(-0.15 * u, -0.08 * u, 0.3 * u, 0.16 * u);
                cr.restore();
                cr.set_source_rgba(0.35, 0.36, 0.4, a);
                cr.fill();
            }
            Item::Flag => {
                let pt = (hx + ia.sin() * 1.6 * u, hy - ia.cos() * 1.6 * u); // top of the pole
                cr.set_source_rgba(0.45, 0.32, 0.18, a);
                cr.set_line_width(0.07 * u);
                cr.move_to(hx, hy + 0.3 * u);
                cr.line_to(pt.0, pt.1);
                cr.stroke();
                let wave = (t * 7.0).sin();
                cr.move_to(pt.0, pt.1);
                cr.curve_to(
                    pt.0 + 0.35 * u,
                    pt.1 - 0.1 * u + wave * 0.08 * u,
                    pt.0 + 0.6 * u,
                    pt.1 + 0.1 * u - wave * 0.08 * u,
                    pt.0 + 0.85 * u,
                    pt.1 + wave * 0.06 * u,
                );
                cr.line_to(pt.0 + 0.85 * u, pt.1 + 0.55 * u + wave * 0.06 * u);
                cr.curve_to(
                    pt.0 + 0.6 * u,
                    pt.1 + 0.65 * u - wave * 0.08 * u,
                    pt.0 + 0.35 * u,
                    pt.1 + 0.45 * u + wave * 0.08 * u,
                    pt.0,
                    pt.1 + 0.55 * u,
                );
                cr.close_path();
                cr.set_rgba(c, a);
                cr.fill();
            }
            Item::None => {}
        }
        // head, short beard, straw hat with a wide brim
        cr.set_rgba(skin, a);
        cr.circle(head.0, head.1, 0.3 * u);
        cr.fill();
        let beard = shade((0.42, 0.28, 0.16), l);
        cr.set_rgba(beard, a);
        cr.new_sub_path();
        cr.arc(head.0 + 0.05 * u, head.1 + 0.05 * u, 0.27 * u, 0.15 * PI, 0.85 * PI);
        cr.fill();
        cr.set_source_rgba(0.1, 0.08, 0.06, a);
        if p.lying {
            // asleep: closed eye
            cr.set_line_width(0.04 * u);
            cr.move_to(head.0 + 0.08 * u, head.1 - 0.04 * u);
            cr.line_to(head.0 + 0.2 * u, head.1 - 0.04 * u);
            cr.stroke();
        } else {
            cr.circle(head.0 + 0.15 * u, head.1 - 0.05 * u, 0.04 * u);
            cr.fill();
        }
        let hat = shade(HAT, l);
        cr.set_rgba(hat, a);
        cr.ellipse(head.0, head.1 - 0.2 * u, 0.55 * u, 0.09 * u);
        cr.fill();
        cr.move_to(head.0 - 0.24 * u, head.1 - 0.2 * u);
        cr.curve_to(
            head.0 - 0.22 * u,
            head.1 - 0.52 * u,
            head.0 + 0.22 * u,
            head.1 - 0.52 * u,
            head.0 + 0.24 * u,
            head.1 - 0.2 * u,
        );
        cr.close_path();
        cr.fill();
        cr.set_source_rgba(c.0 * 0.8, c.1 * 0.8, c.2 * 0.8, a); // hat band in the state colour
        cr.rectangle(head.0 - 0.24 * u, head.1 - 0.3 * u, 0.48 * u, 0.07 * u);
        cr.fill();
        cr.restore();
        hands
    }

    fn working(&self, cr: &mut Canvas, t: f64, x: f64, y: f64, k: Isle) {
        let (r, u, a, l, s) = (k.r, k.u, k.a, k.l, k.s);
        // raft at the water's edge
        let (rx, ry) = (x - r * 0.95, y + r * 0.02);
        let wood = shade((0.55, 0.40, 0.24), l);
        for i in 0..5 {
            let fi = i as f64;
            cr.set_source_rgba(wood.0 * (0.85 + 0.04 * fi), wood.1, wood.2, a);
            cr.ellipse(rx - r * 0.2 + fi * r * 0.1, ry, r * 0.045, r * 0.12);
            cr.fill();
        }
        let swing = (t * 6.0 + self.b.seed).sin();
        let arm = 1.9 + 0.9 * swing; // raised ... down on the raft
        let pose = Pose { arm_l: 0.5, arm_r: arm, face: -1.0, item: Item::Hammer, item_angle: arm + 0.6, ..Pose::new() };
        self.figure(cr, x - r * 0.42, y - r * 0.16, u, k, pose, t);
        if swing < -0.95 {
            // hit: little sparks / chips
            for i in 0..3 {
                let fi = i as f64;
                cr.set_source_rgba(1.0, 0.9, 0.6, 0.8 * a);
                cr.circle(rx + r * (0.05 + 0.05 * fi), ry - r * (0.1 + 0.06 * fi), 1.6 * s);
                cr.fill();
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn fishing(&self, cr: &mut Canvas, t: f64, x: f64, y: f64, k: Isle, rod: bool) {
        let (r, u) = (k.r, k.u);
        let alpha = self.b.alpha;
        let (fx, fy) = (x + r * 0.05, y - r * 0.14);
        if !rod {
            // blocked: sitting with the head down
            self.figure(cr, fx, fy, u, k, Pose { sitting: true, arm_l: 0.9, arm_r: 0.9, ..Pose::new() }, t);
            return;
        }
        let hands = self.figure(cr, fx, fy, u, k, Pose { sitting: true, arm_l: 0.6, arm_r: 1.4, ..Pose::new() }, t);
        let (hx, hy) = (fx + hands[1].0, fy + hands[1].1);
        let tip = (hx + r * 0.9, hy - r * 0.55);
        cr.set_source_rgba(0.4, 0.3, 0.18, alpha);
        cr.set_line_width((u * 0.05).max(1.0));
        cr.move_to(hx, hy);
        cr.line_to(tip.0, tip.1);
        cr.stroke();
        let bob = (x + r * 1.45, y + r * 0.22 + (t * 2.2 + self.b.seed).sin() * r * 0.02);
        cr.set_source_rgba(0.9, 0.9, 0.9, 0.6 * alpha);
        cr.set_line_width(1.0);
        cr.move_to(tip.0, tip.1);
        cr.curve_to(tip.0 + r * 0.2, tip.1 + r * 0.2, bob.0 - r * 0.05, bob.1 - r * 0.25, bob.0, bob.1);
        cr.stroke();
        cr.set_source_rgba(0.95, 0.25, 0.2, alpha);
        cr.circle(bob.0, bob.1, u * 0.07);
        cr.fill();
    }

    fn waiting(&self, cr: &mut Canvas, t: f64, x: f64, y: f64, k: Isle) {
        let (r, u, a) = (k.r, k.u, k.a);
        let fire = (x - r * 0.62, y - r * 0.1); // signal fire with smoke
        for i in 0..3 {
            let fi = i as f64;
            let h = r * (0.22 + 0.06 * (t * 9.0 + fi * 2.0).sin());
            cr.move_to(fire.0 - r * 0.08 + fi * r * 0.06, fire.1);
            cr.line_to(fire.0 - r * 0.05 + fi * r * 0.06, fire.1 - h);
            cr.line_to(fire.0 - r * 0.02 + fi * r * 0.06, fire.1);
            cr.close_path();
            cr.set_source_rgba(1.0, 0.55 + 0.15 * fi, 0.15, 0.9 * a);
            cr.fill();
        }
        for i in 0..4 {
            let fi = i as f64;
            let f = pymod(t * 0.35 + fi / 4.0, 1.0);
            cr.set_source_rgba(0.75, 0.75, 0.78, (1.0 - f) * 0.45 * a);
            cr.circle(fire.0 + (f * 4.0 + fi).sin() * r * 0.1, fire.1 - r * 0.3 - f * r * 1.3, r * (0.06 + 0.12 * f));
            cr.fill();
        }
        let wave = (t * 5.0).sin();
        let pose = Pose { arm_l: 2.6 + 0.35 * wave, arm_r: 2.9, item: Item::Flag, item_angle: 0.25 * wave, ..Pose::new() };
        self.figure(cr, x - r * 0.05, y - r * 0.12, u, k, pose, t);
    }

    #[allow(clippy::too_many_arguments)]
    fn hammock(&self, cr: &mut Canvas, t: f64, x: f64, y: f64, k: Isle, palm_x: f64, palm_y: f64) {
        let (r, u, a, l, s) = (k.r, k.u, k.a, k.l, k.s);
        let post = (x - r * 0.55, y - r * 0.12);
        cr.set_source_rgba(0.45, 0.32, 0.2, a);
        cr.set_line_width(r * 0.05);
        cr.move_to(post.0, post.1);
        cr.line_to(post.0, post.1 - r * 0.55);
        cr.stroke();
        let (p1, p2) = ((post.0, post.1 - r * 0.5), (palm_x - r * 0.02, palm_y - r * 0.5));
        let sway = (t * 0.7).sin() * r * 0.03;
        let mid = ((p1.0 + p2.0) / 2.0 + sway, p1.1.max(p2.1) + r * 0.22);
        let hammock = shade((0.85, 0.80, 0.70), l);
        cr.set_rgba(hammock, a);
        cr.set_line_width(r * 0.12);
        cr.move_to(p1.0, p1.1);
        cr.curve_to(p1.0 + r * 0.1, mid.1, p2.0 - r * 0.1, mid.1, p2.0, p2.1);
        cr.stroke();
        let pose = Pose { lying: true, face: 1.0, arm_l: 0.3, arm_r: 0.3, ..Pose::new() };
        self.figure(cr, mid.0 - u * 0.9, mid.1 - u * 0.25, u * 0.85, k, pose, t);
        for i in 0..3 {
            let f = pymod(t * 0.3 + i as f64 / 3.0, 1.0);
            let lay = text::layout("z", (13.0 + 11.0 * f) * s, weight::BOLD, None);
            let (zx, zy) = (mid.0 + r * (0.1 + 0.4 * f), mid.1 - r * (0.45 + 0.9 * f));
            text::show(cr, &lay, zx, zy, (1.0, 1.0, 1.0), 0.8 * (PI * f).sin() * a, 0.0);
        }
    }
}

impl Archipelago {
    fn depth_scale(&self, env: &Env, y: f64) -> f64 {
        let (top, bottom) = (self.horizon + 60.0 * env.s, env.h - env.strip_h);
        lerp(0.6, 1.1, clamp((y - top) / (bottom - top).max(1.0), 0.0, 1.0))
    }

    /// Island positions for n islands: one row for up to 3, two staggered rows for more.
    fn slots(&self, env: &Env, n: usize) -> Vec<(f64, f64)> {
        let (w, h) = (env.w, env.h);
        let (top, bottom) = (self.horizon + 0.14 * h, h - env.strip_h - 0.07 * h);
        let rows: Vec<usize> = if n <= 3 { vec![n] } else { vec![n / 2, n - n / 2] };
        let mut slots = Vec::with_capacity(n);
        for (r, &count) in rows.iter().enumerate() {
            let y = if rows.len() > 1 { lerp(top, bottom, (r as f64 + 0.5) / rows.len() as f64) } else { lerp(top, bottom, 0.55) };
            let odd = r % 2 == 1;
            for i in 0..count {
                let x = w * (i as f64 + 0.5 + if odd { 0.25 } else { 0.0 }) / (count as f64 + if odd { 0.5 } else { 0.0 });
                let dy = if rows.len() == 1 && count > 1 { 0.06 * h * if i % 2 == 1 { 1.0 } else { -1.0 } } else { 0.0 };
                slots.push((x, y + dy));
            }
        }
        slots
    }

    /// Sky, sun or moon and sea for the current time - rebuilt once a minute.
    fn make_bg(&mut self, env: &Env, hour: f64) {
        let (w, h, s, hz) = (env.w, env.h, env.s, self.horizon);
        let (sky_top, sky_low, sea_far, sea_near, light) = palette(hour);
        self.light = light;
        let (f, sr, col);
        if (6.0..=20.0).contains(&hour) {
            // sun arc
            f = (hour - 6.0) / 14.0;
            sr = 40.0 * s;
            col = (1.0, 0.92, 0.7);
        } else {
            // moon arc through the night
            f = pymod(hour - 20.0, 24.0) / 10.0;
            sr = 30.0 * s;
            col = (0.95, 0.95, 0.88);
        }
        let (sx, sy) = (w * (0.08 + 0.84 * f), hz - (PI * f).sin() * h * 0.30);
        self.sun = (sx, sy, sr, col);
        self.bg = Some(Picture::record(w, h, |cr| {
            cr.set_source_rgb(0.0, 0.0, 0.0);
            cr.rectangle(0.0, 0.0, w, h);
            cr.fill();
            let mut g = Gradient::linear(0.0, 0.0, 0.0, hz);
            g.stop(0.0, sky_top, 1.0);
            g.stop(1.0, sky_low, 1.0);
            cr.set_source(&g);
            cr.rectangle(0.0, 0.0, w, hz);
            cr.fill();
            let mut glow = Gradient::radial(sx, sy, sr, sx, sy, sr * 6.0);
            glow.stop(0.0, col, 0.35);
            glow.stop(1.0, col, 0.0);
            cr.set_source(&glow);
            cr.rectangle(0.0, 0.0, w, hz);
            cr.fill();
            cr.save();
            cr.rectangle(0.0, 0.0, w, hz);
            cr.clip();
            cr.set_rgb(col);
            cr.circle(sx, sy, sr);
            cr.fill();
            cr.restore();
            let mut g = Gradient::linear(0.0, hz, 0.0, h);
            g.stop(0.0, sea_far, 1.0);
            g.stop(1.0, sea_near, 1.0);
            cr.set_source(&g);
            cr.rectangle(0.0, hz, w, h - hz);
            cr.fill();
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.25 * light); // horizon line
            cr.rectangle(0.0, hz - 1.0 * s, w, 1.5 * s);
            cr.fill();
        }));
    }
}

impl Theme for Archipelago {
    type Actor = Castaway;
    const NAME: &'static str = "archipelago";
    const LABEL_BG: Rgb = (0.02, 0.05, 0.08);
    const EMPTY_COLOR: Rgb = (1.0, 1.0, 1.0);

    fn new(_env: &Env) -> Self {
        Archipelago {
            bg: None,
            bg_key: None,
            horizon: 0.0,
            light: 1.0,
            bottles: Vec::new(),
            stars: Vec::new(),
            ship: None,
            next_ship: 20.0,
            gulls: Vec::new(),
            next_gulls: 6.0,
            sun: (0.0, 0.0, 0.0, (1.0, 1.0, 1.0)),
            recenter: false,
        }
    }

    fn waiting_targets(&self, _env: &Env, _waiting: &[&Castaway]) -> Vec<(f64, f64)> {
        Vec::new() // islands stay where they are
    }

    fn update_world(&mut self, env: &mut Env, actors: &mut [Castaway], dt: f64) {
        let (s, w, t) = (env.s, env.w, env.t);
        if self.recenter {
            self.recenter = false;
            for a in actors.iter_mut() {
                a.b.x = w / 2.0;
                a.b.y = self.horizon + env.h * 0.3;
                a.slot = (a.b.x, a.b.y);
            }
        }
        let mut live: Vec<&mut Castaway> = actors.iter_mut().filter(|a| !a.b.dying).collect();
        live.sort_by(|a, b| a.b.info.started.total_cmp(&b.b.info.started).then_with(|| a.b.info.id.cmp(&b.b.info.id)));
        let slots = self.slots(env, live.len());
        for (a, slot) in live.into_iter().zip(slots) {
            a.slot = slot;
        }
        for b in &mut self.bottles {
            b.age += dt;
            b.x += b.vx * dt;
        }
        self.bottles.retain(|b| b.age < b.life);
        if t > self.next_ship {
            self.next_ship = t + env.rng.uniform(60.0, 140.0);
            self.ship = Some(-120.0 * s);
        }
        if let Some(x) = &mut self.ship {
            *x += 14.0 * s * dt;
            if *x > w + 120.0 * s {
                self.ship = None;
            }
        }
        if t > self.next_gulls {
            self.next_gulls = t + env.rng.uniform(15.0, 35.0);
            let y0 = env.rng.uniform(env.h * 0.08, self.horizon * 0.7);
            self.gulls = (0..env.rng.randint(2, 4))
                .map(|k| Gull {
                    x: -40.0 * s - k as f64 * 45.0 * s,
                    y: y0 + k as f64 * 18.0 * s * if k % 2 == 1 { 1.0 } else { -1.0 },
                    ph: env.rng.uniform(0.0, TAU),
                })
                .collect();
        }
        for g in &mut self.gulls {
            g.x += 70.0 * s * dt;
        }
        self.gulls.retain(|g| g.x < w + 60.0 * s);
    }

    fn on_tool(&mut self, env: &mut Env, actor: &mut Castaway, name: &str, index: usize) {
        let side = if env.rng.random() < 0.5 { -1.0 } else { 1.0 };
        let b = &actor.b;
        let x = b.x + side * b.radius * 1.05;
        // different heights and a quicker drift, so the names of several bottles do not run together
        let y = b.y + b.radius * (0.1 + 0.12 * env.rng.random() + 0.1 * index as f64);
        self.bottles.push(Bottle {
            x,
            y,
            vx: side * env.rng.uniform(30.0, 45.0) * env.s,
            age: -(index as f64) * 0.6,
            life: 7.0,
            label: name.to_string(),
            c: b.color,
            ph: env.rng.uniform(0.0, TAU),
        });
    }

    fn setup_world(&mut self, env: &mut Env) {
        let (w, h) = (env.w, env.h);
        // islands that are already there start again from the middle (new ones get their own place)
        self.recenter = self.horizon != 0.0;
        self.horizon = h * 0.40;
        let rng = &env.rng;
        self.stars = (0..(220.0 * w / 1920.0) as usize)
            .map(|_| [rng.uniform(0.0, w), rng.uniform(0.0, self.horizon), rng.uniform(0.2, 1.0), rng.uniform(0.0, TAU)])
            .collect();
        self.bg_key = None;
    }

    fn draw_world(&mut self, cr: &mut Canvas, env: &Env, actors: &[Castaway], t: f64) {
        let (s, w, h, hz) = (env.s, env.w, env.h, self.horizon);
        let hour = hour_now();
        let key = (hour * 60.0) as i64;
        if self.bg_key != Some(key) {
            self.bg_key = Some(key);
            self.make_bg(env, hour);
        }
        let l = self.light;
        if let Some(bg) = &self.bg {
            cr.picture(bg, 0.0, 0.0, 1.0);
        }
        if l < 0.6 {
            // stars at night
            let vis = (0.6 - l) / 0.35;
            for &[x, y, z, ph] in &self.stars {
                let tw = 0.5 + 0.5 * (t * (0.8 + z * 2.0) + ph * 5.0).sin();
                cr.set_source_rgba(0.95, 0.95, 1.0, vis * (0.2 + 0.6 * z) * (0.4 + 0.6 * tw));
                cr.circle(x, y * 0.95, (0.5 + z) * s);
                cr.fill();
            }
        }
        let (sx, _sy, sr, col) = self.sun;
        for k in 0..14 {
            // glitter path of the sun / moon on the water
            let f = k as f64 / 14.0;
            let yy = hz + 6.0 * s + f * f * (h - hz) * 0.8;
            let ww = sr * (0.4 + 2.5 * f) * (0.6 + 0.4 * (t * 2.0 + k as f64 * 1.7).sin());
            cr.set_rgba(col, 0.25 * (1.0 - f) * (0.5 + 0.5 * l));
            cr.rectangle(sx - ww / 2.0, yy, ww, 2.0 * s + f * 3.0 * s);
            cr.fill();
        }
        cr.set_line_width(1.2 * s); // waves
        for k in 0..18 {
            let kf = k as f64;
            let f = (kf + pymod(t * 0.05, 1.0)) / 18.0;
            let yy = hz + 10.0 * s + f * f * (h - hz);
            let amp = (2.0 + 6.0 * f) * s;
            cr.move_to(0.0, yy);
            let mut xi = 0.0;
            while xi < w + 41.0 {
                cr.line_to(xi, yy + (xi * 0.012 / (0.3 + f) + t * 0.8 + kf).sin() * amp);
                xi += 40.0;
            }
            cr.set_source_rgba(1.0, 1.0, 1.0, (0.04 + 0.08 * f) * (0.4 + 0.6 * l));
            cr.stroke();
        }
        if let Some(x) = self.ship {
            // a sailing ship far away on the horizon
            cr.set_source_rgba(0.12 + 0.2 * l, 0.1 + 0.15 * l, 0.12 + 0.15 * l, 0.8);
            cr.move_to(x - 30.0 * s, hz - 2.0 * s);
            cr.line_to(x + 30.0 * s, hz - 2.0 * s);
            cr.line_to(x + 22.0 * s, hz + 5.0 * s);
            cr.line_to(x - 24.0 * s, hz + 5.0 * s);
            cr.close_path();
            cr.fill();
            cr.rectangle(x - 1.0 * s, hz - 48.0 * s, 2.0 * s, 46.0 * s);
            cr.fill();
            let m = 0.4 + 0.6 * l;
            cr.set_source_rgba(0.85 * m, 0.82 * m, 0.75 * m, 0.85);
            for (top, bot, wd) in [(-46.0, -26.0, 20.0), (-24.0, -6.0, 26.0)] {
                cr.move_to(x + 2.0 * s, hz + top * s);
                cr.line_to(x + 2.0 * s + wd * s, hz + bot * s);
                cr.line_to(x + 2.0 * s, hz + bot * s);
                cr.close_path();
                cr.fill();
            }
        }
        let m = 0.95 * (0.3 + 0.7 * l);
        cr.set_source_rgba(m, m, m, 0.85);
        cr.set_line_width(2.0 * s);
        for g in &self.gulls {
            // seagulls
            let flap = (t * 6.0 + g.ph).sin() * 5.0 * s;
            cr.move_to(g.x - 12.0 * s, g.y - flap);
            cr.curve_to(g.x - 6.0 * s, g.y - 7.0 * s, g.x - 2.0 * s, g.y - 3.0 * s, g.x, g.y);
            cr.curve_to(g.x + 2.0 * s, g.y - 3.0 * s, g.x + 6.0 * s, g.y - 7.0 * s, g.x + 12.0 * s, g.y - flap);
            cr.stroke();
        }
        for b in &self.bottles {
            // messages in bottles drifting away
            if b.age < 0.0 {
                continue;
            }
            let fade = (b.age * 2.0).min(1.0) * ((b.life - b.age) / 2.0).min(1.0);
            let (bx, by) = (b.x, b.y + (t * 2.5 + b.ph).sin() * 2.0 * s);
            cr.save();
            cr.translate(bx, by);
            cr.rotate(0.35 + (t * 2.0 + b.ph).sin() * 0.15);
            cr.set_source_rgba(0.35, 0.65, 0.45, 0.85 * fade);
            cr.rectangle(-11.0 * s, -4.0 * s, 16.0 * s, 8.0 * s);
            cr.fill();
            cr.rectangle(5.0 * s, -2.0 * s, 6.0 * s, 4.0 * s);
            cr.fill();
            cr.set_source_rgba(0.6, 0.45, 0.3, fade);
            cr.rectangle(10.0 * s, -2.0 * s, 3.0 * s, 4.0 * s);
            cr.fill();
            cr.set_source_rgba(0.95, 0.92, 0.8, 0.9 * fade); // the rolled message inside
            cr.rectangle(-8.0 * s, -1.5 * s, 10.0 * s, 3.0 * s);
            cr.fill();
            cr.restore();
            let lay = text::layout(&b.label, 15.0 * s, weight::SEMIBOLD, None);
            let (tw, th) = lay.get_pixel_size();
            text::show(cr, &lay, bx - tw / 2.0, by - 14.0 * s - th, lighten(b.c, 0.3), 0.95 * fade, 0.7);
        }
        let mut order: Vec<&Castaway> = actors.iter().collect();
        order.sort_by(|a, b| a.b.y.total_cmp(&b.b.y));
        for a in order {
            a.draw(cr, env, self, t);
        }
    }
}
