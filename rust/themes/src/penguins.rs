//! Penguins theme: a colony on a big ice floe drifting through a polar night.
//!
//! working = waddles about briskly and now and then slides on its belly with a little snow spray; every
//!           tool call makes the penguin hop and flap, and a fish leaps out of the nearest fishing hole
//!           (or the sea behind the floe) with the tool name rising
//! idle    = stands, looks around and preens, shuffles a few steps now and then
//! waiting = comes to the front, stands tall, flaps both flippers and calls with its beak up, pulses
//! blocked = stands hunched with its head down and flippers hanging, flickers
//! rest    = one penguin asleep on its belly, breathing slowly, "z z z"
//! Bobble hat and scarf are in the state colour; plumage, beak and feet stay natural.
//! Sub-agents are grey downy chicks trotting behind their parent.
use vivarium_core::prelude::*;

const INK: Rgb = (0.09, 0.10, 0.15);
const BELLY: Rgb = (0.97, 0.98, 1.0);
const ORANGE: Rgb = (0.99, 0.60, 0.14);
const DOWN: Rgb = (0.66, 0.69, 0.75);
const SNOW: Rgb = (0.95, 0.97, 1.0);
const ICE_SHADE: Rgb = (0.42, 0.62, 0.82);
const WATER: Rgb = (0.03, 0.14, 0.26);

/// Outline of the floe's top surface, clockwise from the far left corner.
/// Far and near edge: (x as a fraction of the width, y offset in px at scale 1);
/// the two sides: (x fraction, depth 0 = far edge .. 1 = near edge).
const FAR: [(f64, f64); 10] = [
    (0.115, 8.0),
    (0.19, -2.0),
    (0.27, 3.0),
    (0.36, -5.0),
    (0.47, -1.0),
    (0.56, -7.0),
    (0.66, -2.0),
    (0.75, -8.0),
    (0.83, -3.0),
    (0.895, -9.0),
];
const RIGHT: [(f64, f64); 5] = [(0.935, 0.12), (0.952, 0.30), (0.966, 0.52), (0.957, 0.75), (0.942, 0.90)];
const NEAR: [(f64, f64); 9] = [
    (0.915, -8.0),
    (0.82, -2.0),
    (0.71, -7.0),
    (0.60, 1.0),
    (0.50, -3.0),
    (0.39, 4.0),
    (0.28, 0.0),
    (0.17, 5.0),
    (0.09, -3.0),
];
const LEFT: [(f64, f64); 5] = [(0.055, 0.86), (0.038, 0.66), (0.048, 0.45), (0.064, 0.25), (0.085, 0.10)];
/// The part of the outline whose thick edge faces the viewer (indices into the outline).
const EDGE: std::ops::Range<usize> = 12..25;

/// How long a tool-call fish is in the air.
const LEAP: f64 = 1.05;

#[derive(Clone, Copy, PartialEq)]
enum Eyes {
    Open,
    Shut,
    Sad,
}

/// How a penguin holds itself; angles in radians, lengths in units of the radius.
struct Pose {
    /// 0 = upright .. 1 = on the belly
    lie: f64,
    /// waddle: body rocking about the feet
    lean: f64,
    /// head rotation (negative = beak up)
    head: f64,
    head_dy: f64,
    /// flippers: 0 = hanging down, larger = raised
    near: f64,
    far: f64,
    /// 0 = beak shut .. 1 = wide open
    beak: f64,
    eyes: Eyes,
    /// vertical stretch
    tall: f64,
    step: f64,
    mv: f64,
    /// -1..1, scarf end swinging
    sway: f64,
}

#[derive(Clone)]
pub struct Penguin {
    b: Base,
    /// 0 = upright .. 1 = sliding on the belly
    lie: f64,
    /// seconds the current slide still lasts
    slide_t: f64,
    next_slide: f64,
    /// 1 -> 0 after a tool call: flippers flapping
    flap: f64,
    blink_at: f64,
    /// drawn as a downy chick (sub-agent companion)
    chick: bool,
    /// screen width and scale, for placing the label
    screen: (f64, f64),
}

/// A fish leaping after a tool call.
struct Fish {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    /// negative while it still waits its turn
    age: f64,
    /// water level it falls back to
    y0: f64,
    len: f64,
    c: Rgb,
}

/// Where the fixed things are (all in pixels).
#[derive(Clone, Copy, Default)]
struct Geo {
    /// horizon
    hz: f64,
    /// far and near edge of the floe's top surface (before tilt and jitter)
    yt: f64,
    yf: f64,
    /// fishing holes: (x, y, half width)
    holes: [(f64, f64, f64); 2],
}

/// One vertical ray of an aurora curtain.
struct Ray {
    x0: f64,
    x1: f64,
    /// lower edge at both ends
    y0: f64,
    y1: f64,
    hgt: f64,
    /// brightness along the band (fades out at both ends)
    amp: f64,
    ph: f64,
}

pub struct Penguins {
    sky: Option<Picture>,
    floe: Option<Picture>,
    geo: Geo,
    outline: Vec<(f64, f64)>,
    rays: Vec<Ray>,
    /// wave lines on the open water: (x, y, half length, phase)
    waves: Vec<(f64, f64, f64, f64)>,
    /// drifting bits of ice: (x 0..1, y, size, speed, phase)
    chunks: Vec<(f64, f64, f64, f64, f64)>,
    /// snow flakes: (x 0..1, phase, speed, size)
    snow: Vec<(f64, f64, f64, f64)>,
    fish: Vec<Fish>,
    /// splashes: (x, y, age, size)
    splashes: Vec<(f64, f64, f64, f64)>,
}

/// Cheap repeatable random number 0..1 from a number.
fn hash(n: f64) -> f64 {
    ((n * 12.9898).sin() * 43758.5453).rem_euclid(1.0)
}

impl ActorT<Penguins> for Penguin {
    const BASE_RADIUS: f64 = 78.0;
    const WALKER: bool = true;
    const COMPANION_SCALE: f64 = 0.46;

    fn new(base: Base, env: &Env, _world: &mut Penguins) -> Self {
        Penguin {
            b: base,
            lie: 0.0,
            slide_t: 0.0,
            next_slide: env.t + env.rng.uniform(3.0, 10.0),
            flap: 0.0,
            blink_at: env.t + env.rng.uniform(1.0, 4.0),
            chick: false,
            screen: (env.w, env.s),
        }
    }

    fn base(&self) -> &Base {
        &self.b
    }

    fn base_mut(&mut self) -> &mut Base {
        &mut self.b
    }

    fn companion_reset(&mut self) {
        self.chick = true;
        self.lie = 0.0;
        self.slide_t = 0.0;
        self.flap = 0.0;
        self.b.moving = self.b.moving.max(0.35); // chicks always patter a little
    }

    fn mv(&mut self, env: &mut Env, world: &mut Penguins, others: &[Other], dt: f64, t: f64, target: Option<(f64, f64)>) {
        let st = self.b.state();
        self.screen = (env.w, env.s);
        // working: every now and then a stretch of tobogganing
        if st == State::Working && target.is_none() {
            if self.slide_t > 0.0 {
                self.slide_t -= dt;
                if self.slide_t <= 0.0 {
                    self.next_slide = t + env.rng.uniform(8.0, 16.0);
                }
            } else if t > self.next_slide {
                self.slide_t = env.rng.uniform(2.8, 4.2);
                // off across the floe, towards the side with more room
                let across = if self.b.x < env.w / 2.0 { 0.0 } else { PI };
                self.b.heading = across + env.rng.uniform(-0.5, 0.5);
            }
        } else {
            self.slide_t = 0.0;
        }
        let sliding = self.slide_t > 0.0;
        self.lie = lerp(self.lie, if sliding { 1.0 } else { 0.0 }, (dt * 5.0).min(1.0));
        let (speed, turn) = if sliding { (230.0, 0.35) } else { (78.0, 1.0) };
        self.b.walker_move(env, others, dt, t, target, [speed, 20.0, 55.0, 0.0], [turn, 0.5, 0.5, 0.1]);
        world.keep_on_floe(&mut self.b, env, dt, self.lie);
        if self.b.vx.abs() <= 12.0 * env.s && target.is_none() {
            // standing: finish the turn instead of staying edge-on
            let side = if self.b.face < 0.0 { -1.0 } else { 1.0 };
            self.b.face = lerp(self.b.face, side, (dt * 3.0).min(1.0));
        }
        self.flap = (self.flap - dt * 1.5).max(0.0);
        if t > self.blink_at + 0.2 {
            self.blink_at = t + env.rng.uniform(2.0, 6.0);
        }
    }

    /// The usual places, but never one that the screen edge would push back over the penguin.
    fn label_candidates(&self, pw: f64, ph: f64) -> Vec<(f64, f64)> {
        let b = &self.b;
        let r = b.radius;
        let (w, s) = self.screen;
        let (right, left) = (b.x + r * 1.5, b.x - r * 1.5 - pw);
        let y = b.y - r * 0.9;
        let mut v = Vec::new();
        for dy in [0.0, ph * 0.8, -ph * 0.8] {
            if right + pw <= w - 20.0 * s {
                v.push((right, y + dy));
            }
            if left >= 20.0 * s {
                v.push((left, y + dy));
            }
        }
        v.push((b.x - pw / 2.0, b.y + r * 1.8));
        v
    }

    fn draw(&self, cr: &mut Canvas, env: &Env, _world: &Penguins, t: f64) {
        let s = env.s;
        let b = &self.b;
        let (r, c, st) = (b.radius, b.color, b.state());
        if st == State::Rest {
            self.draw_asleep(cr, s, t, r, (b.alpha / 0.8).min(1.0));
            return;
        }
        let a = b.alpha * b.flicker;
        if a < 0.01 {
            return;
        }
        let lie = self.lie;
        let hop = if b.hop > 0.0 { (PI * b.hop).sin() * r * 0.4 } else { 0.0 };
        let gy = b.y + r * 0.95;
        let lift = 1.0 - 0.3 * hop / (r * 0.4).max(1.0);
        ground(cr, b.x, gy, r, c, if st != State::Idle { 0.62 } else { 0.42 } * a, (0.62 + 0.5 * lie) * lift, a);
        if st == State::Waiting {
            cr.pulse_rings(b.x, b.y - r * 0.1, r, c, a, t, s);
        }
        // a plump bird turns with a quick flip, it never gets thin
        let dir = if b.face < 0.0 { b.face.min(-0.75) } else { b.face.max(0.75) };
        if lie > 0.05 && !self.chick {
            self.draw_spray(cr, t, r, gy, dir.signum(), a * lie);
        }
        cr.save();
        cr.translate(b.x, b.y - hop);
        cr.scale(dir, 1.0);
        cr.scale(r, r);
        if self.chick {
            chick(cr, b.step, b.moving, c, a);
        } else {
            penguin(cr, &self.pose(t), c, a);
        }
        cr.restore();
    }
}

impl Penguin {
    fn pose(&self, t: f64) -> Pose {
        let b = &self.b;
        let (mv, lie) = (b.moving, self.lie);
        let swing = b.step.sin();
        let blink = self.blink_at < t && t < self.blink_at + 0.2;
        let mut p = Pose {
            lie,
            lean: swing * 0.13 * mv * (1.0 - lie),
            head: 0.0,
            head_dy: 0.0,
            near: 0.18,
            far: 0.30,
            beak: 0.0,
            eyes: if blink { Eyes::Shut } else { Eyes::Open },
            tall: 1.0,
            step: b.step,
            mv,
            sway: (t * 2.0 + b.seed).sin() * (0.3 + 0.7 * mv),
        };
        match b.state() {
            State::Working => {
                // flippers held out for balance
                p.near = 0.34 + swing * 0.22 * mv;
                p.far = 0.42 - swing * 0.22 * mv;
                p.head = (b.step * 2.0).sin() * 0.04 * mv;
            }
            State::Idle => {
                // standing still: look around, then preen under the flipper
                let pre = b.graze * (t * 0.7 + b.seed).sin().max(0.0);
                p.head = pre * 0.7 + (1.0 - pre) * (t * 0.45 + b.seed * 3.0).sin() * 0.16;
                p.near = 0.18 + pre * (0.55 + 0.2 * (t * 7.0).sin());
            }
            State::Waiting => {
                let f = (t * 9.0 + b.seed).sin();
                p.near = 1.25 + 0.55 * f;
                p.far = 1.15 + 0.55 * f;
                p.head = -0.42 + 0.05 * (t * 3.0 + b.seed).sin();
                p.beak = 0.55 + 0.45 * (t * 5.0 + b.seed).sin();
                p.tall = 1.05;
            }
            State::Blocked => {
                p.head = 0.55;
                p.head_dy = 0.05;
                p.near = 0.04;
                p.far = 0.08;
                p.tall = 0.95;
                p.eyes = Eyes::Sad;
            }
            State::Rest => {}
        }
        if self.flap > 0.0 {
            let k = (self.flap * PI).sin();
            let w = (self.flap * 20.0).sin();
            p.near = lerp(p.near, 1.35 + 0.5 * w, k);
            p.far = lerp(p.far, 1.25 + 0.5 * w, k);
            p.beak = p.beak.max(k * 0.6);
        }
        // on the belly: head up, flipper paddling
        p.head -= 1.15 * lie;
        p.near = lerp(p.near, 0.55 + 0.3 * (t * 10.0 + b.seed).sin(), lie);
        p.far = lerp(p.far, 0.05, lie);
        p
    }

    /// Snow thrown up behind a sliding penguin, and the track it leaves.
    fn draw_spray(&self, cr: &mut Canvas, t: f64, r: f64, gy: f64, side: f64, a: f64) {
        let b = &self.b;
        let k = a * (0.3 + 0.7 * b.moving);
        cr.set_rgba(ICE_SHADE, 0.45 * k);
        cr.set_line_width(r * 0.05);
        cr.set_line_cap(LineCap::Round);
        for dy in [-0.05, 0.05] {
            cr.move_to(b.x - side * r * 0.6, gy + dy * r);
            cr.line_to(b.x - side * r * 2.4, gy + dy * r);
        }
        cr.stroke();
        for (col, off, al) in [(ICE_SHADE, 0.035, 0.55), (SNOW, 0.0, 0.95)] {
            for i in 0..6 {
                let f = (t * 2.3 + i as f64 / 6.0 + b.seed) % 1.0;
                let px = b.x - side * r * (0.75 + 1.0 * f + 0.1 * (i % 2) as f64);
                let py = gy - r * (0.04 + 1.5 * f * (1.0 - f) * (0.3 + 0.12 * (i % 3) as f64)) + off * r;
                cr.set_rgba(col, al * (1.0 - f) * k);
                cr.circle(px, py, r * (0.05 + 0.09 * f));
                cr.fill();
            }
        }
    }

    /// Resting: asleep on the belly, head on the ice.
    fn draw_asleep(&self, cr: &mut Canvas, s: f64, t: f64, r: f64, a: f64) {
        let b = &self.b;
        if a < 0.01 {
            return;
        }
        let side = if b.face >= 0.0 { 1.0 } else { -1.0 };
        let breath = (t * 1.3 + b.seed).sin();
        ground(cr, b.x, b.y + r * 0.95, r, b.color, 0.3 * a, 1.15, a);
        let p = Pose {
            lie: 1.0,
            lean: 0.0,
            head: -0.72 + breath * 0.02,
            head_dy: 0.0,
            near: 0.16 + breath * 0.05,
            far: 0.05,
            beak: 0.0,
            eyes: Eyes::Shut,
            tall: 1.0,
            step: 0.6,
            mv: 0.0,
            sway: 0.0,
        };
        cr.save();
        cr.translate(b.x, b.y - (0.5 + 0.5 * breath) * r * 0.02);
        cr.scale(side, 1.0);
        cr.scale(r, r);
        penguin(cr, &p, b.color, a);
        cr.restore();
        // z z z
        for k in 0..3 {
            let f = (t * 0.3 + k as f64 / 3.0) % 1.0;
            let lay = text::layout("z", (14.0 + 14.0 * f) * s, weight::BOLD, None);
            let zx = b.x + side * r * (0.75 + 0.5 * f) + (f * 6.0).sin() * 4.0 * s;
            let zy = b.y - r * 0.2 - f * r * 1.4;
            text::show(cr, &lay, zx, zy, (0.22, 0.38, 0.70), 0.95 * (PI * f).sin() * a, 0.0);
        }
    }
}

/// Glow in the state colour on the ice and the shadow under the feet. `wide` = shadow half width in R.
#[allow(clippy::too_many_arguments)]
fn ground(cr: &mut Canvas, x: f64, gy: f64, r: f64, c: Rgb, glow: f64, wide: f64, a: f64) {
    let mut g = Gradient::radial(x, gy, 0.0, x, gy, r * 1.8);
    g.stop(0.0, c, glow);
    g.stop(0.45, c, glow * 0.6);
    g.stop(1.0, c, 0.0);
    cr.save();
    cr.translate(x, gy);
    cr.scale(1.0, 0.3);
    cr.translate(-x, -gy);
    cr.set_source(&g);
    cr.circle(x, gy, r * 1.8);
    cr.fill();
    cr.restore();
    cr.ellipse(x, gy, r * wide, r * 0.11);
    cr.set_source_rgba(0.04, 0.14, 0.30, 0.38 * a);
    cr.fill();
}

/// A flipper hanging down from (0, 0).
fn flipper(cr: &mut Canvas) {
    cr.move_to(-0.08, 0.0);
    cr.curve_to(-0.17, 0.20, -0.13, 0.52, 0.0, 0.64);
    cr.curve_to(0.11, 0.50, 0.11, 0.20, 0.08, 0.0);
    cr.close_path();
}

/// The penguin in units of its radius, facing right: feet at y = +0.94, bobble at about -1.05.
fn penguin(cr: &mut Canvas, p: &Pose, c: Rgb, a: f64) {
    let rim = lighten(c, 0.15);
    cr.set_line_cap(LineCap::Round);
    cr.set_line_join(LineJoin::Round);
    // lying down = tipped forward about the feet and moved so that the belly rests on the ice
    cr.translate(-0.75 * p.lie, -0.40 * p.lie);
    cr.translate(0.0, 0.94);
    cr.rotate(p.lean + 1.36 * p.lie);
    cr.scale(1.0, p.tall);
    cr.translate(0.0, -0.94);
    // feet, alternating
    cr.set_rgba(ORANGE, a);
    for k in 0..2 {
        let ph = p.step + k as f64 * PI;
        let fx = if k == 0 { -0.14 } else { 0.22 } + ph.cos() * 0.07 * p.mv;
        cr.ellipse(fx, 0.925 - ph.sin().max(0.0) * 0.08 * p.mv, 0.20, 0.065);
    }
    cr.fill();
    // far flipper, peeking out in front of the belly
    cr.save();
    cr.translate(0.30, -0.02);
    cr.rotate(-p.far);
    flipper(cr);
    cr.set_rgba(INK, a);
    cr.fill();
    cr.restore();
    // tail tip and body
    cr.move_to(-0.40, 0.66);
    cr.line_to(-0.68, 0.92);
    cr.line_to(-0.30, 0.90);
    cr.close_path();
    cr.set_rgba(INK, a);
    cr.fill();
    cr.move_to(0.0, -0.42);
    cr.curve_to(0.34, -0.42, 0.54, 0.10, 0.52, 0.48);
    cr.curve_to(0.50, 0.80, 0.28, 0.94, 0.0, 0.94);
    cr.curve_to(-0.30, 0.94, -0.50, 0.80, -0.50, 0.46);
    cr.curve_to(-0.52, 0.08, -0.32, -0.42, 0.0, -0.42);
    cr.close_path();
    cr.fill_preserve();
    cr.set_rgba(rim, 0.6 * a); // rim light in the state colour
    cr.set_line_width(0.04);
    cr.stroke();
    // white belly with a little shading
    cr.move_to(0.14, -0.30);
    cr.curve_to(0.40, -0.26, 0.50, 0.14, 0.48, 0.48);
    cr.curve_to(0.46, 0.78, 0.28, 0.90, 0.06, 0.90);
    cr.curve_to(-0.16, 0.90, -0.24, 0.70, -0.22, 0.44);
    cr.curve_to(-0.20, 0.10, -0.10, -0.30, 0.14, -0.30);
    cr.close_path();
    cr.set_rgba(BELLY, a);
    cr.fill();
    cr.set_rgba((0.70, 0.80, 0.92), 0.5 * a);
    cr.ellipse(0.10, 0.74, 0.26, 0.12);
    cr.fill();
    // scarf end hanging down the back
    let sx = -0.50 - 0.07 * p.sway - 0.10 * p.mv;
    cr.set_line_width(0.15);
    cr.set_rgba(darken(c, 0.25), a);
    cr.move_to(-0.30, -0.33);
    cr.quad_to(-0.44, -0.16, sx, 0.08);
    cr.stroke();
    cr.set_rgba(lighten(c, 0.55), a);
    cr.set_line_cap(LineCap::Butt);
    cr.move_to(lerp(-0.30, sx, 0.80), lerp(-0.33, 0.08, 0.72));
    cr.line_to(lerp(-0.30, sx, 0.90), lerp(-0.33, 0.08, 0.86));
    cr.stroke();
    cr.set_line_cap(LineCap::Round);
    // near flipper
    cr.save();
    cr.translate(-0.17, -0.06);
    cr.rotate(p.near);
    flipper(cr);
    cr.set_source_rgba(0.17, 0.19, 0.28, a);
    cr.fill_preserve();
    cr.set_rgba(rim, 0.55 * a);
    cr.set_line_width(0.03);
    cr.stroke();
    cr.restore();
    // head
    cr.save();
    cr.translate(0.04, -0.36 + p.head_dy);
    cr.rotate(p.head);
    head(cr, p, c, rim, a);
    cr.restore();
    // scarf around the neck
    cr.set_rgba(darken(c, 0.45), a);
    cr.set_line_width(0.215);
    cr.move_to(-0.30, -0.36);
    cr.quad_to(0.06, -0.20, 0.39, -0.35);
    cr.stroke();
    cr.set_rgba(c, a);
    cr.set_line_width(0.165);
    cr.move_to(-0.30, -0.36);
    cr.quad_to(0.06, -0.20, 0.39, -0.35);
    cr.stroke();
    cr.circle(-0.31, -0.34, 0.105);
    cr.fill();
}

/// Head with hat around the neck joint at (0, 0), facing right.
fn head(cr: &mut Canvas, p: &Pose, c: Rgb, rim: Rgb, a: f64) {
    let (hx, hy, hr) = (0.02, -0.24, 0.34);
    // beak: two mandibles opening about the hinge
    for (ang, col, pts) in [
        (-0.38 * p.beak, ORANGE, [(0.0, -0.10), (0.36, 0.0), (0.0, 0.0)]),
        (0.38 * p.beak, darken(ORANGE, 0.18), [(0.0, 0.0), (0.30, 0.01), (0.0, 0.07)]),
    ] {
        cr.save();
        cr.translate(hx + 0.28, hy + 0.05);
        cr.rotate(ang);
        cr.move_to(pts[0].0, pts[0].1);
        cr.line_to(pts[1].0, pts[1].1);
        cr.line_to(pts[2].0, pts[2].1);
        cr.close_path();
        cr.set_rgba(col, a);
        cr.fill();
        cr.restore();
    }
    cr.set_rgba(INK, a);
    cr.circle(hx, hy, hr);
    cr.fill_preserve();
    cr.set_rgba(rim, 0.6 * a);
    cr.set_line_width(0.04);
    cr.stroke();
    // white cheek, blush, eye
    cr.set_rgba(BELLY, a);
    cr.ellipse(hx + 0.13, hy + 0.06, 0.19, 0.17);
    cr.fill();
    cr.set_source_rgba(1.0, 0.55, 0.50, 0.55 * a);
    cr.circle(hx + 0.20, hy + 0.14, 0.05);
    cr.fill();
    let (ex, ey) = (hx + 0.15, hy + 0.01);
    cr.set_rgba(INK, a);
    match p.eyes {
        Eyes::Open => {
            cr.circle(ex, ey, 0.052);
            cr.fill();
            cr.set_source_rgba(1.0, 1.0, 1.0, a);
            cr.circle(ex + 0.018, ey - 0.018, 0.018);
            cr.fill();
        }
        Eyes::Shut => {
            cr.set_line_width(0.032);
            cr.new_sub_path();
            cr.arc(ex, ey - 0.01, 0.05, 0.2, PI - 0.2);
            cr.stroke();
        }
        Eyes::Sad => {
            cr.set_line_width(0.032);
            cr.new_sub_path();
            cr.arc(ex, ey + 0.04, 0.05, PI + 0.3, TAU - 0.3);
            cr.move_to(ex - 0.07, ey - 0.03);
            cr.line_to(ex + 0.06, ey - 0.07);
            cr.stroke();
        }
    }
    // bobble hat: cap over the top of the head, light brim, bobble
    let (a0, a1) = (PI + 0.25, TAU - 0.45);
    let cap = hr + 0.02;
    cr.new_sub_path();
    cr.arc(hx, hy, cap, a0, a1);
    cr.close_path();
    cr.set_rgba(c, a);
    cr.fill_preserve();
    cr.set_rgba(darken(c, 0.4), a);
    cr.set_line_width(0.03);
    cr.stroke();
    cr.set_rgba(lighten(c, 0.6), a);
    cr.set_line_width(0.085);
    cr.move_to(hx + a0.cos() * cap, hy + a0.sin() * cap);
    cr.line_to(hx + a1.cos() * cap, hy + a1.sin() * cap);
    cr.stroke();
    cr.set_rgba(lighten(c, 0.45), a);
    cr.circle(hx - 0.04, hy - cap - 0.04, 0.10);
    cr.fill();
}

/// A downy chick in units of its radius, facing right (feet at y = +0.94).
fn chick(cr: &mut Canvas, step: f64, mv: f64, c: Rgb, a: f64) {
    cr.set_line_cap(LineCap::Round);
    cr.translate(0.0, 0.94);
    cr.rotate(step.sin() * 0.16 * mv);
    cr.translate(0.0, -0.94);
    cr.set_source_rgba(0.22, 0.23, 0.30, a);
    for k in 0..2 {
        let ph = step + k as f64 * PI;
        let fx = if k == 0 { -0.14 } else { 0.20 } + ph.cos() * 0.07 * mv;
        cr.ellipse(fx, 0.925 - ph.sin().max(0.0) * 0.08 * mv, 0.18, 0.06);
    }
    cr.fill();
    // fluffy body: one ellipse with tufts of down around it
    cr.set_rgba(DOWN, a);
    cr.ellipse(0.0, 0.40, 0.52, 0.54);
    for k in 0..7 {
        let ang = 0.5 + k as f64 * 0.72;
        cr.circle(ang.cos() * 0.47, 0.40 + ang.sin() * 0.49, 0.12);
    }
    cr.fill();
    cr.set_rgba(lighten(DOWN, 0.5), a);
    cr.ellipse(0.10, 0.46, 0.32, 0.40);
    cr.fill();
    // stubby wing
    cr.save();
    cr.translate(-0.20, 0.16);
    cr.rotate(0.25 + step.sin() * 0.3 * mv);
    cr.set_rgba(darken(DOWN, 0.22), a);
    cr.ellipse(0.0, 0.22, 0.10, 0.25);
    cr.fill();
    cr.restore();
    // head: dark cap, white face
    cr.set_rgba(INK, a);
    cr.circle(0.06, -0.30, 0.40);
    cr.fill_preserve();
    cr.set_rgba(c, 0.6 * a);
    cr.set_line_width(0.045);
    cr.stroke();
    cr.set_rgba(BELLY, a);
    cr.ellipse(0.17, -0.24, 0.26, 0.25);
    cr.fill();
    cr.set_source_rgba(0.20, 0.21, 0.27, a);
    cr.move_to(0.40, -0.27);
    cr.line_to(0.62, -0.20);
    cr.line_to(0.40, -0.14);
    cr.close_path();
    cr.fill();
    cr.set_rgba(INK, a);
    cr.circle(0.22, -0.30, 0.058);
    cr.fill();
    cr.set_source_rgba(1.0, 1.0, 1.0, a);
    cr.circle(0.24, -0.32, 0.02);
    cr.fill();
    // little scarf in the working colour
    cr.set_rgba(c, a);
    cr.set_line_width(0.16);
    cr.move_to(-0.28, 0.02);
    cr.quad_to(0.08, 0.16, 0.40, 0.03);
    cr.stroke();
    cr.move_to(-0.28, 0.02);
    cr.line_to(-0.42, 0.26);
    cr.stroke();
}

/// A small fish around (0, 0) heading right.
fn fish(cr: &mut Canvas, len: f64, back: Rgb, a: f64) {
    cr.set_rgba(back, a);
    cr.move_to(-0.36 * len, 0.0);
    cr.line_to(-0.62 * len, -0.20 * len);
    cr.line_to(-0.54 * len, 0.0);
    cr.line_to(-0.62 * len, 0.20 * len);
    cr.close_path();
    cr.fill();
    cr.set_source_rgba(0.84, 0.91, 0.96, a);
    cr.ellipse(0.0, 0.0, 0.42 * len, 0.17 * len);
    cr.fill();
    cr.set_rgba(back, a);
    cr.move_to(-0.42 * len, 0.0);
    cr.curve_to(-0.28 * len, -0.23 * len, 0.28 * len, -0.23 * len, 0.42 * len, 0.0);
    cr.close_path();
    cr.fill();
    cr.set_source_rgba(0.05, 0.07, 0.12, a);
    cr.circle(0.27 * len, 0.03 * len, 0.035 * len);
    cr.fill();
}

/// Rings and drops where something broke the water; `f` = 0..1 age.
fn splash(cr: &mut Canvas, x: f64, y: f64, size: f64, f: f64) {
    cr.set_source_rgba(0.85, 0.95, 1.0, 0.75 * (1.0 - f));
    cr.set_line_width((size * 0.07).max(1.0));
    for k in 0..2 {
        let g = (f - k as f64 * 0.25).max(0.0);
        if g > 0.0 {
            cr.ellipse(x, y, size * (0.25 + 1.1 * g), size * (0.08 + 0.33 * g));
        }
    }
    cr.stroke();
    for k in 0..4 {
        let u = k as f64 - 1.5;
        cr.circle(x + u * size * 0.5 * f, y - size * (2.4 * f - 2.8 * f * f) * (1.0 - 0.15 * u.abs()), size * 0.07);
    }
    cr.fill();
}

// ---------------------------------------------------------------------- the static scene

/// Corner points of a shape.
type Pts = &'static [(f64, f64)];

/// A distant iceberg standing on (x, y); `kind` 0 = peaked, 1 = tabular. `haze` blends it into the distance.
#[allow(clippy::too_many_arguments)]
fn berg(cr: &mut Canvas, x: f64, y: f64, w: f64, h: f64, kind: usize, flip: bool, haze: f64) {
    let far = (0.42, 0.68, 0.76);
    let lit = lerp3((0.80, 0.92, 0.96), far, haze);
    let shade = lerp3((0.36, 0.58, 0.76), far, haze);
    let (outline, dark): (Pts, Pts) = if kind == 0 {
        (
            &[(-0.5, 0.0), (-0.40, -0.42), (-0.22, -0.58), (-0.08, -1.0), (0.10, -0.78), (0.22, -0.88), (0.40, -0.34), (0.5, 0.0)],
            &[(-0.08, -1.0), (0.02, -0.52), (-0.06, 0.0), (0.5, 0.0), (0.40, -0.34), (0.22, -0.88), (0.10, -0.78)],
        )
    } else {
        (
            &[(-0.5, 0.0), (-0.47, -0.78), (-0.12, -1.0), (0.40, -0.88), (0.5, 0.0)],
            &[(-0.12, -1.0), (-0.04, -0.60), (-0.10, 0.0), (0.5, 0.0), (0.40, -0.88)],
        )
    };
    let fx = if flip { -w } else { w };
    // reflection, squashed and faint
    for (pts, col, al) in [(outline, lit, 0.20), (dark, shade, 0.20)] {
        for (k, p) in pts.iter().enumerate() {
            let (px, py) = (x + p.0 * fx, y - p.1 * h * 0.55);
            if k == 0 { cr.move_to(px, py) } else { cr.line_to(px, py) }
        }
        cr.close_path();
        cr.set_rgba(col, al * (1.0 - haze * 0.5));
        cr.fill();
    }
    for (pts, col) in [(outline, lit), (dark, shade)] {
        for (k, p) in pts.iter().enumerate() {
            let (px, py) = (x + p.0 * fx, y + p.1 * h);
            if k == 0 { cr.move_to(px, py) } else { cr.line_to(px, py) }
        }
        cr.close_path();
        cr.set_rgb(col);
        cr.fill();
    }
    cr.set_source_rgba(0.9, 0.98, 1.0, 0.45); // waterline
    cr.rectangle(x - w * 0.52, y - h * 0.012, w * 1.04, h * 0.024 + 1.0);
    cr.fill();
}

/// A snow mound lying on (x, y).
fn mound(cr: &mut Canvas, x: f64, y: f64, w: f64, h: f64) {
    cr.set_rgba(ICE_SHADE, 0.35);
    cr.ellipse(x + w * 0.12, y, w * 1.1, h * 0.22);
    cr.fill();
    cr.move_to(x - w, y);
    cr.curve_to(x - w * 0.7, y - h * 1.25, x + w * 0.4, y - h * 1.35, x + w, y);
    cr.curve_to(x + w * 0.4, y + h * 0.16, x - w * 0.5, y + h * 0.16, x - w, y);
    cr.close_path();
    cr.set_rgb(SNOW);
    cr.fill();
    cr.move_to(x + w * 0.1, y - h * 0.86);
    cr.curve_to(x + w * 0.5, y - h * 0.7, x + w * 0.5, y - h * 0.1, x + w, y);
    cr.curve_to(x + w * 0.5, y + h * 0.14, x, y + h * 0.14, x - w * 0.3, y + h * 0.10);
    cr.curve_to(x + w * 0.2, y - h * 0.1, x + w * 0.3, y - h * 0.5, x + w * 0.1, y - h * 0.86);
    cr.close_path();
    cr.set_rgba(ICE_SHADE, 0.30);
    cr.fill();
}

/// A block of ice standing on (x, y) (middle of its front bottom edge).
fn block(cr: &mut Canvas, x: f64, y: f64, w: f64, h: f64, skew: f64) {
    let d = h * 0.45; // depth of the top face
    cr.set_rgba(ICE_SHADE, 0.38);
    cr.ellipse(x + w * 0.15, y + h * 0.04, w * 0.75, h * 0.16);
    cr.fill();
    cr.set_source_rgb(0.50, 0.74, 0.90); // front
    cr.rectangle(x - w / 2.0, y - h, w, h);
    cr.fill();
    cr.set_source_rgb(0.36, 0.60, 0.82); // side
    cr.move_to(x + w / 2.0, y - h);
    cr.line_to(x + w / 2.0 + skew, y - h - d);
    cr.line_to(x + w / 2.0 + skew, y - d);
    cr.line_to(x + w / 2.0, y);
    cr.close_path();
    cr.fill();
    cr.set_rgb(SNOW); // top
    cr.move_to(x - w / 2.0, y - h);
    cr.line_to(x - w / 2.0 + skew, y - h - d);
    cr.line_to(x + w / 2.0 + skew, y - h - d);
    cr.line_to(x + w / 2.0, y - h);
    cr.close_path();
    cr.fill();
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.5);
    cr.rectangle(x - w * 0.38, y - h * 0.82, w * 0.12, h * 0.6);
    cr.fill();
}

/// A fishing hole: the far wall of the ice, the dark water, a rim of snow.
fn hole(cr: &mut Canvas, x: f64, y: f64, rx: f64) {
    let ry = rx * 0.36;
    cr.set_rgba(SNOW, 0.9);
    cr.ellipse(x, y, rx * 1.16, ry * 1.22);
    cr.fill();
    cr.set_source_rgb(0.42, 0.70, 0.88);
    cr.ellipse(x, y, rx, ry);
    cr.fill();
    cr.save();
    cr.ellipse(x, y, rx, ry);
    cr.clip();
    cr.set_rgb(WATER);
    cr.ellipse(x, y + ry * 0.42, rx, ry);
    cr.fill();
    cr.set_source_rgba(0.5, 0.85, 0.95, 0.35);
    cr.ellipse(x - rx * 0.2, y + ry * 0.62, rx * 0.42, ry * 0.10);
    cr.fill();
    cr.restore();
    // shovelled snow next to it
    mound(cr, x + rx * 1.35, y + ry * 0.2, rx * 0.42, rx * 0.30);
}

fn paint_sky_and_sea(cr: &mut Canvas, env: &Env, geo: &Geo) {
    let (w, h, s, hz) = (env.w, env.h, env.s, geo.hz);
    let rng = &env.rng;
    let mut g = Gradient::linear(0.0, 0.0, 0.0, hz);
    g.add_color_stop_rgb(0.0, 0.02, 0.04, 0.13);
    g.add_color_stop_rgb(0.50, 0.04, 0.13, 0.30);
    g.add_color_stop_rgb(0.84, 0.12, 0.36, 0.50);
    g.add_color_stop_rgb(1.0, 0.46, 0.74, 0.78);
    cr.set_source(&g);
    cr.rectangle(0.0, 0.0, w, hz + 2.0);
    cr.fill();
    for _ in 0..90 {
        let f = rng.random();
        cr.set_source_rgba(0.9, 0.95, 1.0, rng.uniform(0.35, 0.95) * (1.0 - f * 0.8));
        cr.circle(rng.uniform(0.0, w), f * hz * 0.72, rng.uniform(0.7, 1.9) * s);
        cr.fill();
    }
    // last light of the low sun behind the horizon
    cr.save();
    cr.rectangle(0.0, 0.0, w, hz);
    cr.clip();
    cr.translate(w * 0.30, hz);
    cr.scale(1.0, 0.24);
    cr.glow(0.0, 0.0, w * 0.42, (1.0, 0.86, 0.66), 0.42, 0.0);
    cr.restore();
    // the sea
    let mut g = Gradient::linear(0.0, hz, 0.0, h);
    g.add_color_stop_rgb(0.0, 0.22, 0.52, 0.60);
    g.add_color_stop_rgb(0.07, 0.08, 0.31, 0.43);
    g.add_color_stop_rgb(0.40, 0.04, 0.18, 0.31);
    g.add_color_stop_rgb(1.0, 0.02, 0.08, 0.17);
    cr.set_source(&g);
    cr.rectangle(0.0, hz, w, h - hz);
    cr.fill();
    // far ice shelf along the horizon
    let n = 48;
    let mut top = 12.0;
    cr.move_to(0.0, hz + 3.0 * s);
    for k in 0..=n {
        let f = k as f64 / n as f64;
        if k % 3 == 0 {
            top = (7.0 + 13.0 * rng.random()) * (0.55 + 0.45 * (f * 7.0 + 1.0).sin().abs());
        }
        cr.line_to(f * w, hz - top * s);
        cr.line_to((f + 0.8 / n as f64) * w, hz - (top - rng.uniform(-2.0, 2.0)) * s);
    }
    cr.line_to(w, hz + 3.0 * s);
    cr.close_path();
    let mut g = Gradient::linear(0.0, hz - 20.0 * s, 0.0, hz + 3.0 * s);
    g.add_color_stop_rgb(0.0, 0.74, 0.90, 0.94);
    g.add_color_stop_rgb(1.0, 0.36, 0.62, 0.74);
    cr.set_source(&g);
    cr.fill();
    cr.set_source_rgba(0.80, 0.95, 1.0, 0.30);
    cr.rectangle(0.0, hz + 2.0 * s, w, 2.0 * s);
    cr.fill();
    // icebergs, the far ones first
    for (u, dy, bw, bh, kind, flip) in [
        (0.555, 7.0, 64.0, 30.0, 0, false),
        (0.415, 12.0, 120.0, 56.0, 1, false),
        (0.905, 16.0, 150.0, 58.0, 1, true),
        (0.125, 30.0, 200.0, 124.0, 0, false),
        (0.715, 38.0, 250.0, 150.0, 0, true),
    ] {
        berg(cr, u * w, hz + dy * s, bw * s, bh * s, kind, flip, clamp(0.62 - dy / 60.0, 0.0, 0.6));
    }
    // glints on the water, finer towards the horizon
    cr.set_line_cap(LineCap::Round);
    for _ in 0..110 {
        let f = rng.random();
        let y = hz + 6.0 * s + f * f * (h - hz);
        let len = lerp(8.0, 80.0, f) * s * rng.uniform(0.5, 1.2);
        let x = rng.uniform(0.0, w);
        cr.set_source_rgba(0.55, 0.88, 0.95, rng.uniform(0.05, 0.16));
        cr.set_line_width(lerp(1.0, 2.6, f) * s);
        cr.move_to(x - len, y);
        cr.line_to(x + len, y);
        cr.stroke();
    }
    cr.set_line_cap(LineCap::Butt);
}

impl Penguins {
    fn layout(env: &Env) -> Geo {
        let (w, h, s) = (env.w, env.h, env.s);
        let yt = env.ground_top + 16.0 * s;
        let yf = h - env.strip_h - 118.0 * s;
        let d = yf - yt;
        Geo { hz: h * 0.40, yt, yf, holes: [(w * 0.215, yt + d * 0.20, 40.0 * s), (w * 0.865, yt + d * 0.52, 50.0 * s)] }
    }

    /// Far and near edge at fraction `u` of the width: the floe lies slightly tilted.
    fn edges(geo: &Geo, s: f64, u: f64) -> (f64, f64) {
        (geo.yt + lerp(6.0, -8.0, u) * s, geo.yf + lerp(8.0, -10.0, u) * s)
    }

    /// Point on the floe: `u` = fraction of the width, `f` = 0 far edge .. 1 near edge.
    fn at(geo: &Geo, env: &Env, u: f64, f: f64) -> (f64, f64) {
        let (yt, yf) = Self::edges(geo, env.s, u);
        (u * env.w, lerp(yt, yf, f))
    }

    /// Thickness of the ice showing above the water at fraction `u` of the width.
    fn thick(env: &Env, x: f64) -> f64 {
        lerp(54.0, 36.0, x / env.w.max(1.0)) * env.s
    }

    fn build_outline(geo: &Geo, env: &Env) -> Vec<(f64, f64)> {
        let (w, s) = (env.w, env.s);
        let mut out = Vec::with_capacity(29);
        out.extend(FAR.iter().map(|&(u, dy)| (u * w, Self::edges(geo, s, u).0 + dy * s)));
        out.extend(RIGHT.iter().map(|&(u, f)| Self::at(geo, env, u, f)));
        out.extend(NEAR.iter().map(|&(u, dy)| (u * w, Self::edges(geo, s, u).1 + dy * s)));
        out.extend(LEFT.iter().map(|&(u, f)| Self::at(geo, env, u, f)));
        out
    }

    /// Left and right end of the floe's top surface at height `y`.
    fn span(&self, y: f64) -> (f64, f64) {
        let o = &self.outline;
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        for i in 0..o.len() {
            let (p, q) = (o[i], o[(i + 1) % o.len()]);
            if (p.1 <= y) != (q.1 <= y) {
                let x = p.0 + (q.0 - p.0) * (y - p.1) / (q.1 - p.1);
                lo = lo.min(x);
                hi = hi.max(x);
            }
        }
        if lo > hi { (-1e9, 1e9) } else { (lo, hi) }
    }

    /// Penguins stay on the ice and walk around the fishing holes.
    fn keep_on_floe(&self, b: &mut Base, env: &Env, dt: f64, lie: f64) {
        let s = env.s;
        let r = b.radius * b.depth_scale(env);
        let k = (dt * 6.0).min(1.0);
        let hi = self.geo.yf - 24.0 * s;
        let fy = b.y + r * 0.95;
        if fy > hi {
            b.y -= (fy - hi) * k;
            b.vy = b.vy.min(0.0);
            b.heading = angle_lerp(b.heading, -PI / 2.0, k * 0.3);
        }
        let lo = self.geo.yt + 30.0 * s;
        if fy < lo {
            b.y += (lo - fy) * k;
            b.vy = b.vy.max(0.0);
        }
        let fy = clamp(b.y + r * 0.95, self.geo.yt + 12.0 * s, hi);
        let (xl, xr) = self.span(fy);
        let m = r * (1.0 + 0.7 * lie) + 24.0 * s;
        if b.x < xl + m {
            b.x += (xl + m - b.x) * k;
            b.vx = b.vx.max(0.0);
            b.heading = angle_lerp(b.heading, 0.0, k * 0.6); // turn back towards the middle
        } else if b.x > xr - m {
            b.x -= (b.x - (xr - m)) * k;
            b.vx = b.vx.min(0.0);
            b.heading = angle_lerp(b.heading, PI, k * 0.6);
        }
        for &(hx, hy, hr) in &self.geo.holes {
            let (dx, dy) = (b.x - hx, (fy - hy) / 0.4);
            let d = hypot(dx, dy);
            let md = hr * 1.15 + r * 0.55;
            if 0.01 < d && d < md {
                let push = (md - d) * k;
                b.x += dx / d * push;
                b.y += dy / d * push * 0.4;
            }
        }
    }

    fn paint_floe(&self, cr: &mut Canvas, env: &Env) {
        let (w, h, s) = (env.w, env.h, env.s);
        let geo = &self.geo;
        let o = &self.outline;
        let rng = &env.rng;
        let th = |x: f64| Self::thick(env, x);
        let trace = |cr: &mut Canvas, dy: f64, grow: f64| {
            for (k, p) in o.iter().enumerate() {
                let x = w / 2.0 + (p.0 - w / 2.0) * grow;
                if k == 0 { cr.move_to(x, p.1 + dy) } else { cr.line_to(x, p.1 + dy) }
            }
            cr.close_path();
        };
        // shadow in the water and the pale ice below the surface
        trace(cr, 54.0 * s + 34.0 * s, 1.012);
        cr.set_source_rgba(0.0, 0.04, 0.10, 0.42);
        cr.fill();
        trace(cr, 54.0 * s + 12.0 * s, 1.02);
        cr.set_source_rgba(0.20, 0.62, 0.72, 0.22);
        cr.fill();
        // the thick edge above the waterline, facet by facet, and its reflection
        for i in EDGE {
            let (p, q) = (o[i], o[i + 1]);
            let (tp, tq) = (th(p.0), th(q.0));
            let v = hash(i as f64 * 3.7);
            let ym = (p.1 + q.1) / 2.0;
            let mut g = Gradient::linear(0.0, ym + tp, 0.0, ym + tp * 1.9);
            g.add_color_stop_rgba(0.0, 0.42, 0.76, 0.88, 0.34);
            g.add_color_stop_rgba(1.0, 0.42, 0.76, 0.88, 0.0);
            cr.set_source(&g);
            cr.move_to(p.0, p.1 + tp);
            cr.line_to(q.0, q.1 + tq);
            cr.line_to(q.0, q.1 + tq * 1.9);
            cr.line_to(p.0, p.1 + tp * 1.9);
            cr.close_path();
            cr.fill();
            let mut g = Gradient::linear(0.0, ym, 0.0, ym + tp);
            g.stop(0.0, lerp3((0.56, 0.80, 0.93), (0.74, 0.91, 0.98), v), 1.0);
            g.stop(0.55, lerp3((0.30, 0.62, 0.82), (0.42, 0.72, 0.88), v), 1.0);
            g.stop(1.0, lerp3((0.10, 0.40, 0.60), (0.18, 0.52, 0.70), v), 1.0);
            cr.set_source(&g);
            cr.move_to(p.0, p.1 - 1.0);
            cr.line_to(q.0, q.1 - 1.0);
            cr.line_to(q.0, q.1 + tq);
            cr.line_to(p.0, p.1 + tp);
            cr.close_path();
            cr.fill();
            // a few frozen streaks down the face
            let n = ((p.0 - q.0).abs() / (46.0 * s)) as usize;
            cr.set_source_rgba(0.85, 0.96, 1.0, 0.28);
            cr.set_line_width(2.0 * s);
            for _ in 0..n {
                let f = rng.uniform(0.1, 0.9);
                let (x, y) = (lerp(p.0, q.0, f), lerp(p.1, q.1, f));
                cr.move_to(x, y + 2.0 * s);
                cr.line_to(x, y + lerp(tp, tq, f) * rng.uniform(0.3, 0.8));
            }
            cr.stroke();
        }
        // foam along the waterline
        cr.set_source_rgba(0.90, 0.98, 1.0, 0.60);
        cr.set_line_width(2.5 * s);
        cr.set_line_join(LineJoin::Round);
        for p in &o[EDGE.start..=EDGE.end] {
            cr.line_to(p.0, p.1 + th(p.0));
        }
        cr.stroke();
        // the snowy top
        let mut g = Gradient::linear(0.0, geo.yt, 0.0, geo.yf);
        g.add_color_stop_rgb(0.0, 0.60, 0.75, 0.90);
        g.add_color_stop_rgb(0.35, 0.74, 0.85, 0.95);
        g.add_color_stop_rgb(1.0, 0.88, 0.94, 0.99);
        cr.set_source(&g);
        trace(cr, 0.0, 1.0);
        cr.fill();
        cr.save();
        trace(cr, 0.0, 1.0);
        cr.clip();
        // wind-blown drifts: long soft patches, lighter and darker
        for k in 0..16 {
            let f = rng.uniform(0.05, 0.95);
            let (x, y) = Self::at(geo, env, rng.uniform(0.08, 0.92), f);
            let dk = lerp(0.6, 1.1, f);
            if k % 2 == 0 {
                cr.set_source_rgba(1.0, 1.0, 1.0, 0.30);
            } else {
                cr.set_rgba(ICE_SHADE, 0.14);
            }
            cr.ellipse(x, y, rng.uniform(90.0, 230.0) * s * dk, rng.uniform(6.0, 13.0) * s * dk);
            cr.fill();
        }
        // faint glow of the aurora on the far half
        let mut g = Gradient::linear(0.0, geo.yt, 0.0, lerp(geo.yt, geo.yf, 0.5));
        g.add_color_stop_rgba(0.0, 0.30, 0.90, 0.70, 0.16);
        g.add_color_stop_rgba(1.0, 0.30, 0.90, 0.70, 0.0);
        cr.set_source(&g);
        cr.rectangle(0.0, geo.yt - 20.0 * s, w, (geo.yf - geo.yt) * 0.5 + 20.0 * s);
        cr.fill();
        // cracks
        let cracks: [&[(f64, f64)]; 7] = [
            &[(0.36, -0.05), (0.385, 0.10), (0.352, 0.20), (0.392, 0.31), (0.372, 0.40)],
            &[(0.352, 0.20), (0.305, 0.25)],
            &[(0.60, 1.05), (0.622, 0.88), (0.662, 0.80), (0.650, 0.68)],
            &[(0.662, 0.80), (0.712, 0.77)],
            &[(0.03, 0.56), (0.085, 0.535), (0.125, 0.585), (0.175, 0.57)],
            &[(0.752, -0.05), (0.742, 0.09), (0.772, 0.17)],
            &[(0.97, 0.80), (0.90, 0.83), (0.87, 0.90)],
        ];
        cr.set_line_join(LineJoin::Round);
        // each stretch gets a kink in the middle so nothing looks ruled
        let mut paths: Vec<Vec<(f64, f64)>> = Vec::new();
        for line in cracks {
            let mut pts: Vec<(f64, f64)> = Vec::new();
            for &(u, f) in line {
                let p = Self::at(geo, env, u, f);
                if let Some(&q) = pts.last() {
                    let j = hypot(p.0 - q.0, p.1 - q.1) * 0.10;
                    pts.push(((p.0 + q.0) / 2.0 + rng.uniform(-j, j), (p.1 + q.1) / 2.0 + rng.uniform(-j, j) * 0.6));
                }
                pts.push(p);
            }
            paths.push(pts);
        }
        for (off, lw, col, al) in [(1.8 * s, 1.4, (1.0, 1.0, 1.0), 0.60), (0.0, 2.0, (0.20, 0.44, 0.70), 0.70)] {
            cr.set_rgba(col, al);
            cr.set_line_width(lw * s);
            for pts in &paths {
                for (k, p) in pts.iter().enumerate() {
                    if k == 0 { cr.move_to(p.0, p.1 + off) } else { cr.line_to(p.0, p.1 + off) }
                }
            }
            cr.stroke();
        }
        cr.restore();
        // crisp lips of snow: far edge thin, near edge bright
        cr.set_source_rgba(0.90, 0.97, 1.0, 0.55);
        cr.set_line_width(1.5 * s);
        trace(cr, 0.0, 1.0);
        cr.stroke();
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.95);
        cr.set_line_width(3.5 * s);
        for p in &o[EDGE.start..=EDGE.end] {
            cr.line_to(p.0, p.1);
        }
        cr.stroke();
        // things lying on the ice, far ones first
        for (u, f, kind, size) in [
            (0.45, 0.02, 0, 1.3),
            (0.305, 0.05, 0, 1.0),
            (0.555, 0.06, 1, 1.0),
            (0.63, 0.03, 0, 1.2),
            (0.845, 0.10, 1, 1.25),
            (0.80, 0.14, 0, 0.8),
            (0.125, 0.28, 0, 1.1),
            (0.93, 0.40, 1, 0.9),
            (0.085, 0.60, 1, 1.1),
            (0.115, 0.66, 0, 0.9),
            (0.925, 0.84, 0, 1.2),
        ] {
            let (x, y) = Self::at(geo, env, u, f);
            let dk = lerp(0.6, 1.1, f) * size * s;
            if kind == 0 {
                mound(cr, x, y, 46.0 * dk, 22.0 * dk);
            } else {
                block(cr, x, y, 44.0 * dk, 30.0 * dk, 14.0 * dk * if u < 0.5 { 1.0 } else { -1.0 });
            }
        }
        for &(x, y, rx) in &geo.holes {
            hole(cr, x, y, rx);
        }
        // dark corners
        let mut v = Gradient::radial(w / 2.0, h * 0.55, h * 0.45, w / 2.0, h * 0.55, hypot(w, h) * 0.62);
        v.add_color_stop_rgba(0.0, 0.0, 0.02, 0.07, 0.0);
        v.add_color_stop_rgba(1.0, 0.0, 0.02, 0.07, 0.45);
        cr.set_source(&v);
        cr.rectangle(0.0, 0.0, w, h);
        cr.fill();
    }

    /// Aurora curtains: fixed rays whose brightness wanders.
    fn draw_aurora(&self, cr: &mut Canvas, t: f64) {
        for r in &self.rays {
            let a = r.amp * (0.50 + 0.30 * (r.ph + t * 0.55).sin() + 0.20 * (r.ph * 0.37 - t * 0.21).sin());
            if a < 0.01 {
                continue;
            }
            let yb = r.y0.max(r.y1);
            let mut g = Gradient::linear(0.0, yb - r.hgt, 0.0, yb);
            g.add_color_stop_rgba(0.0, 0.45, 0.35, 0.95, 0.0);
            g.add_color_stop_rgba(0.45, 0.25, 0.80, 0.80, 0.30 * a);
            g.add_color_stop_rgba(0.90, 0.35, 1.0, 0.65, a);
            g.add_color_stop_rgba(1.0, 0.35, 1.0, 0.65, 0.0);
            cr.set_source(&g);
            cr.move_to(r.x0, r.y0);
            cr.line_to(r.x1, r.y1);
            cr.line_to(r.x1, r.y1 - r.hgt);
            cr.line_to(r.x0, r.y0 - r.hgt);
            cr.close_path();
            cr.fill();
        }
    }

    /// Things on the open water: wave lines, drifting ice, a fin far out, a fish jumping.
    fn draw_sea(&self, cr: &mut Canvas, env: &Env, t: f64) {
        let (w, s, hz) = (env.w, env.s, self.geo.hz);
        let band = (self.geo.yt - hz).max(1.0);
        // an orca's fin passing far out
        let p = (t + 10.0).rem_euclid(48.0);
        if p < 24.0 {
            let n = ((t + 10.0) / 48.0).floor();
            let dir = if n.rem_euclid(2.0) < 1.0 { 1.0 } else { -1.0 };
            let f = p / 24.0;
            let x = w * (0.5 + dir * lerp(-0.34, 0.30, f));
            let y = hz + band * 0.30;
            let rise = clamp((PI * f).sin() * 2.5, 0.0, 1.0) * (0.55 + 0.45 * (t * 0.9).sin().max(-0.4));
            let (fw, fh) = (26.0 * s, 30.0 * s);
            cr.save();
            cr.rectangle(x - fw * 2.0, y - fh * 1.3, fw * 4.0, fh * 1.3);
            cr.clip();
            cr.translate(x, y + (1.0 - rise) * fh);
            cr.scale(dir, 1.0);
            cr.move_to(-fw * 0.9, fh * 0.2);
            cr.quad_to(-fw * 0.2, -fh * 0.1, -fw * 0.05, -fh * 0.55);
            cr.quad_to(fw * 0.02, -fh * 0.95, -fw * 0.22, -fh);
            cr.quad_to(fw * 0.42, -fh * 0.75, fw * 0.55, 0.0);
            cr.line_to(fw * 0.9, fh * 0.2);
            cr.close_path();
            cr.set_source_rgba(0.02, 0.05, 0.09, 0.95);
            cr.fill();
            cr.restore();
            cr.set_source_rgba(0.85, 0.96, 1.0, 0.45 * rise);
            cr.set_line_width(1.6 * s);
            cr.move_to(x - dir * fw * 0.6, y + 1.0 * s);
            cr.line_to(x - dir * fw * 2.6, y + 3.0 * s);
            cr.move_to(x + dir * fw * 0.7, y + 1.0 * s);
            cr.line_to(x - dir * fw * 0.2, y + 4.0 * s);
            cr.stroke();
        }
        // wave lines
        cr.set_line_cap(LineCap::Round);
        for &(x, y, len, ph) in &self.waves {
            let xx = x + (t * 0.3 + ph).sin() * len * 0.35;
            cr.set_source_rgba(0.62, 0.90, 0.96, 0.10 + 0.20 * (0.5 + 0.5 * (t * 0.8 + ph).sin()));
            cr.set_line_width((len * 0.045).max(1.0));
            cr.move_to(xx - len, y);
            cr.quad_to(xx, y - len * 0.10, xx + len, y);
            cr.stroke();
        }
        cr.set_line_cap(LineCap::Butt);
        // bits of ice drifting past
        for &(x0, y, size, speed, ph) in &self.chunks {
            let x = ((x0 + t * speed).rem_euclid(1.2) - 0.1) * w;
            let y = y + (t * 0.8 + ph).sin() * 1.5 * s;
            cr.set_source_rgba(0.32, 0.62, 0.80, 0.9);
            cr.move_to(x - size, y);
            cr.line_to(x + size, y);
            cr.line_to(x + size * 0.8, y + size * 0.32);
            cr.line_to(x - size * 0.85, y + size * 0.30);
            cr.close_path();
            cr.fill();
            cr.set_rgba(SNOW, 0.95);
            cr.move_to(x - size, y);
            cr.line_to(x - size * 0.5, y - size * (0.25 + 0.2 * hash(ph)));
            cr.line_to(x + size * 0.55, y - size * 0.30);
            cr.line_to(x + size, y);
            cr.close_path();
            cr.fill();
        }
        // now and then a fish jumps
        let cyc = t / 6.5;
        let (n, u) = (cyc.floor(), cyc.fract() / 0.15);
        if u < 1.3 {
            let d = 0.25 + 0.75 * hash(n * 3.3);
            let x0 = w * (0.08 + 0.84 * hash(n * 7.1));
            let y0 = hz + band * (0.12 + 0.74 * d);
            let k = lerp(0.55, 1.0, d) * s;
            let dir = if hash(n * 1.7) < 0.5 { -1.0 } else { 1.0 };
            let reach = 54.0 * k * dir;
            if u < 1.0 {
                let (x, y) = (x0 + reach * (u - 0.5), y0 - 4.0 * u * (1.0 - u) * 46.0 * k);
                cr.save();
                cr.translate(x, y);
                cr.scale(dir, 1.0);
                cr.rotate(((2.0 * u - 1.0) * 4.0 * 46.0).atan2(54.0));
                fish(cr, 22.0 * k, (0.25, 0.42, 0.58), 1.0);
                cr.restore();
            }
            for (at, x) in [(0.0, x0 - reach * 0.5), (1.0, x0 + reach * 0.5)] {
                let f = (u - at) / 0.3 * 0.15 / 0.15;
                if (0.0..1.0).contains(&f) {
                    splash(cr, x, y0, 12.0 * k, f);
                }
            }
        }
    }
}

impl Theme for Penguins {
    type Actor = Penguin;
    const NAME: &'static str = "penguins";
    const LABEL_BG: Rgb = (0.01, 0.04, 0.11);
    const EMPTY_COLOR: Rgb = (0.80, 0.92, 1.0);
    const WAITING_ROW: f64 = 0.74;

    fn new(_env: &Env) -> Self {
        Penguins {
            sky: None,
            floe: None,
            geo: Geo::default(),
            outline: Vec::new(),
            rays: Vec::new(),
            waves: Vec::new(),
            chunks: Vec::new(),
            snow: Vec::new(),
            fish: Vec::new(),
            splashes: Vec::new(),
        }
    }

    fn spawn_point(&self, env: &Env) -> (f64, f64) {
        (env.rng.uniform(env.w * 0.2, env.w * 0.8), env.rng.uniform(env.ground_top + 40.0 * env.s, self.geo.yf - 120.0 * env.s))
    }

    fn rest_point(&self, env: &Env) -> (f64, f64) {
        (env.w * env.rng.uniform(0.36, 0.48), self.geo.yf - 150.0 * env.s)
    }

    fn on_tool(&mut self, env: &mut Env, actor: &mut Penguin, name: &str, index: usize) {
        let s = env.s;
        actor.flap = 1.0;
        actor.slide_t = 0.0;
        actor.b.hop = 1.0;
        let b = &actor.b;
        // the fish leaps from the nearest fishing hole, or from the sea behind the floe
        let fy = b.y + b.radius * 0.95;
        let mut from = (clamp(b.x, env.w * 0.16, env.w * 0.86), self.geo.yt - 16.0 * s, 0.0);
        let mut best = hypot(from.0 - b.x, (from.1 - fy) * 1.6);
        for &(hx, hy, hr) in &self.geo.holes {
            let d = hypot(hx - b.x, (hy - fy) * 1.6);
            if d < best {
                best = d;
                from = (hx, hy + hr * 0.2, hr);
            }
        }
        let (x0, y0, hr) = from;
        let depth = clamp((y0 - self.geo.yt) / (self.geo.yf - self.geo.yt).max(1.0), 0.0, 1.0);
        let k = lerp(0.7, 1.1, depth) * s;
        // out of a hole it comes down where it went up; in the open sea it leaps along
        let reach = if hr > 0.0 { env.rng.uniform(-0.3, 0.3) * hr } else { env.rng.uniform(60.0, 100.0) * k * if env.rng.chance(0.5) { 1.0 } else { -1.0 } };
        self.fish.push(Fish {
            x: x0 - reach / 2.0,
            y: y0,
            vx: reach / LEAP,
            vy: -0.5 * 1000.0 * k * LEAP,
            age: -(index as f64) * 0.18,
            y0,
            len: 40.0 * k,
            c: b.color,
        });
        env.spawn_floater(b.x, b.y - b.radius * 1.3 - index as f64 * 26.0 * s, name, b.color, FloatStyle::Text, 40.0, 3.5);
    }

    fn setup_world(&mut self, env: &mut Env) {
        let (w, h, s) = (env.w, env.h, env.s);
        env.ground_top = h * 0.565;
        self.geo = Self::layout(env);
        let geo = self.geo;
        let env: &Env = env;
        let rng = &env.rng;
        self.outline = Self::build_outline(&geo, env);
        self.sky = Some(Picture::record(w, h, |cr| paint_sky_and_sea(cr, env, &geo)));
        self.floe = Some(Picture::record(w, h, |cr| self.paint_floe(cr, env)));
        // aurora: two curtains of rays hanging over the horizon
        self.rays.clear();
        for (xa, xb, base, wave, hgt, ph) in [(0.02, 0.60, 0.50, 0.09, 0.40, 0.0), (0.34, 0.99, 0.68, 0.06, 0.30, 2.1)] {
            let n = 24;
            let y = |f: f64| geo.hz * (base + wave * (f * 5.2 + ph).sin() + wave * 0.4 * (f * 11.0 + ph).sin());
            for i in 0..n {
                let (f0, f1) = (i as f64 / n as f64, (i + 1) as f64 / n as f64);
                let fm = (f0 + f1) / 2.0;
                self.rays.push(Ray {
                    x0: lerp(xa, xb, f0) * w,
                    x1: lerp(xa, xb, f1) * w + 0.5,
                    y0: y(f0),
                    y1: y(f1),
                    hgt: geo.hz * hgt * (0.6 + 0.4 * (fm * 9.0 + ph).sin()) * rng.uniform(0.85, 1.15),
                    amp: 0.5 * (PI * fm).sin().powf(0.7),
                    ph: i as f64 * 0.7 + ph * 3.0 + rng.uniform(0.0, 0.7),
                });
            }
        }
        // waves only where the water is open
        self.waves.clear();
        let foot = geo.yf + 54.0 * s;
        while self.waves.len() < 26 {
            let f = rng.random();
            let (x, y) = (rng.uniform(0.0, w), geo.hz + 10.0 * s + f * (h - env.strip_h - geo.hz - 10.0 * s));
            let len = lerp(12.0, 46.0, f) * s;
            let (xl, xr) = self.span(clamp(y, geo.yt + 12.0 * s, geo.yf - 14.0 * s));
            let on_ice = y > geo.yt - 14.0 * s && y < foot + 10.0 * s && x > xl - len * 1.5 && x < xr + len * 1.5;
            if !on_ice {
                self.waves.push((x, y, len, rng.uniform(0.0, TAU)));
            }
        }
        self.chunks = (0..6)
            .map(|_| {
                let f = rng.uniform(0.1, 0.9);
                (rng.random() * 1.2, lerp(geo.hz + 14.0 * s, geo.yt - 26.0 * s, f), lerp(7.0, 20.0, f) * s, rng.uniform(0.0015, 0.004), rng.uniform(0.0, TAU))
            })
            .collect();
        self.chunks.sort_by(|a, b| a.1.total_cmp(&b.1));
        self.snow = (0..46).map(|_| (rng.random(), rng.random(), rng.uniform(0.03, 0.07), rng.uniform(1.2, 2.8))).collect();
        self.fish.clear();
        self.splashes.clear();
    }

    fn update_world(&mut self, _env: &mut Env, _actors: &mut [Penguin], dt: f64) {
        for f in &mut self.fish {
            let before = f.age;
            f.age += dt;
            if f.age < 0.0 {
                continue;
            }
            if before <= 0.0 {
                self.splashes.push((f.x, f.y0, 0.0, f.len * 0.55));
            }
            f.vy += 1000.0 * (f.len / 40.0) * dt;
            f.x += f.vx * dt;
            f.y += f.vy * dt;
            if f.age >= LEAP {
                self.splashes.push((f.x, f.y0, 0.0, f.len * 0.6));
            }
        }
        self.fish.retain(|f| f.age < LEAP);
        for sp in &mut self.splashes {
            sp.2 += dt;
        }
        self.splashes.retain(|sp| sp.2 < 0.7);
    }

    fn draw_world(&mut self, cr: &mut Canvas, env: &Env, actors: &[Penguin], t: f64) {
        let (w, h, s) = (env.w, env.h, env.s);
        if let Some(sky) = &self.sky {
            cr.picture(sky, 0.0, 0.0, 1.0);
        }
        self.draw_aurora(cr, t);
        self.draw_sea(cr, env, t);
        if let Some(floe) = &self.floe {
            cr.picture(floe, 0.0, 0.0, 1.0);
        }
        // the water in the fishing holes moves a little
        for (k, &(x, y, rx)) in self.geo.holes.iter().enumerate() {
            let f = (t * 0.35 + k as f64 * 0.5) % 1.0;
            cr.set_source_rgba(0.60, 0.90, 0.98, 0.45 * (PI * f).sin());
            cr.set_line_width(1.5 * s);
            cr.ellipse(x, y + rx * 0.20, rx * (0.2 + 0.5 * f), rx * 0.36 * (0.14 + 0.36 * f));
            cr.stroke();
        }
        for a in sorted_actors(actors) {
            a.draw(cr, env, self, t);
        }
        for f in self.fish.iter().filter(|f| f.age >= 0.0) {
            cr.save();
            cr.translate(f.x, f.y);
            if f.vx < 0.0 {
                cr.scale(-1.0, 1.0);
            }
            cr.rotate(f.vy.atan2(f.vx.abs().max(f.len * 2.2)));
            fish(cr, f.len, lerp3((0.22, 0.38, 0.55), f.c, 0.55), 1.0);
            cr.restore();
        }
        for &(x, y, age, size) in &self.splashes {
            splash(cr, x, y, size, age / 0.7);
        }
        // light snowfall over everything
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.75);
        for &(x, ph, sp, size) in &self.snow {
            let f = (t * sp + ph) % 1.0;
            cr.circle(x * w + (t * 0.7 + ph * 20.0).sin() * 16.0 * s, f * (h + 10.0 * s) - 5.0 * s, size * s);
        }
        cr.fill();
    }
}
