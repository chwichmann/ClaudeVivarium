//! Cockroaches theme: cartoon roaches under the kitchen cupboards at night, having a toy-weapon battle.
//!
//! working    = scurries about, takes pot shots at the others with a toy weapon, digs a tunnel now and
//!              then (down a fresh hole, a bump travels along the floor, up again somewhere else) and
//!              grabs a quick bite; every tool call fires one shot (the weapon depends on the tool) and
//!              lets the tool name rise
//! background = like working, a little calmer
//! idle       = mostly eats and grooms its antennae; shoots back only when it was hit
//! waiting    = comes to the front, stands upright and waves a white toothpick flag and both antennae
//! blocked    = lies on its back, legs wiggling slowly, the helmet beside it; flickers
//! rest       = one roach asleep in a matchbox bed, "z z z"
//! The roaches stay brown; the state colour is the junk helmet (half a nutshell, a bottle cap or an
//! acorn cap), a rim light on the back and a glow on the ground.
//! Weapons: pea shooter, rubber band on a matchstick, cork popgun, pipette squirter, cream spoon,
//! pepper shaker, bubble wand. Being hit is harmless: stars, a jump, a tumble onto the back, a shake,
//! a face full of cream, a sneeze or a short flight in a soap bubble.
//! Sub-agents are pale little nymphs scuttling behind; they wait by the hole while the parent digs.
use vivarium_core::prelude::*;

const SHELL: Rgb = (0.50, 0.26, 0.10);
const BELLY: Rgb = (0.82, 0.60, 0.34);
const HEADC: Rgb = (0.64, 0.37, 0.16);
const LEGC: Rgb = (0.25, 0.12, 0.05);
const PALE_SHELL: Rgb = (0.84, 0.66, 0.42);
const PALE_BELLY: Rgb = (0.94, 0.82, 0.60);
const PALE_HEAD: Rgb = (0.90, 0.74, 0.50);
const PALE_LEG: Rgb = (0.52, 0.36, 0.20);
const INK: Rgb = (0.16, 0.07, 0.04);
const EARTH: Rgb = (0.36, 0.25, 0.15);
const EARTH_D: Rgb = (0.15, 0.10, 0.06);
const EARTH_L: Rgb = (0.50, 0.37, 0.23);
/// the kitchen light falling through the mouse hole
const LIGHT: Rgb = (1.0, 0.80, 0.46);
const STEEL: Rgb = (0.50, 0.51, 0.54);

const PEA: usize = 0;
const BAND: usize = 1;
const CORK: usize = 2;
const PIPETTE: usize = 3;
const SPOON: usize = 4;
const PEPPER: usize = 5;
const BUBBLE: usize = 6;
const WEAPONS: usize = 7;

/// seconds of the stance: the shot goes off at AIM_FIRE
const AIM_FIRE: f64 = 0.60;
const AIM_END: f64 = 1.45;
/// how long each hit reaction lasts (by weapon)
const HIT_DUR: [f64; WEAPONS] = [1.7, 0.9, 2.6, 1.6, 2.2, 1.9, 2.3];
const DIG_DOWN: f64 = 1.9;
const POP_UP: f64 = 0.9;
const MOUND_LIFE: f64 = 26.0;
const RIDGE_LIFE: f64 = 5.0;
const FOOD_N: usize = 7;
/// seconds of nibbling one piece lasts
const FOOD_SECONDS: f64 = 15.0;
const DROP_T: f64 = 0.55;
const MAX_BITS: usize = 260;
/// where the front legs hold a weapon (units of the radius)
const WPN: P = (0.42, -0.02);

type P = (f64, f64);


fn smooth(x: f64) -> f64 {
    let x = clamp(x, 0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

fn mixp(a: P, b: P, k: f64) -> P {
    (lerp(a.0, b.0, k), lerp(a.1, b.1, k))
}

fn sign(v: f64) -> f64 {
    if v >= 0.0 { 1.0 } else { -1.0 }
}

/// The joint of a limb from `a` to `b` with two parts of length `len`, bending to the side `bend`.
fn joint(a: P, b: P, len: f64, bend: f64) -> P {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let d = hypot(dx, dy).max(1e-6);
    let half = (d / 2.0).min(len);
    let off = (len * len - half * half).max(0.0).sqrt() * bend;
    ((a.0 + b.0) / 2.0 - dy / d * off, (a.1 + b.1) / 2.0 + dx / d * off)
}

/// Walker depth factor at the height `y` (the same curve as `Base::depth_scale`).
fn depth_k(env: &Env, y: f64) -> f64 {
    let (top, bottom) = (env.ground_top, env.h - env.strip_h);
    lerp(0.65, 1.15, clamp((y - top) / (bottom - top).max(1.0), 0.0, 1.0))
}

/// The same tool always fires the same weapon.
fn weapon_of(name: &str) -> usize {
    match name {
        "Read" | "NotebookRead" => PEA,
        "Grep" | "Glob" | "LS" => BAND,
        "Bash" | "BashOutput" | "KillShell" => CORK,
        "Edit" | "MultiEdit" | "NotebookEdit" => PIPETTE,
        "Write" | "TodoWrite" => SPOON,
        "WebFetch" | "WebSearch" => PEPPER,
        "Agent" | "Task" => BUBBLE,
        _ => name.bytes().fold(2_166_136_261u32, |h, b| (h ^ b as u32).wrapping_mul(16_777_619)) as usize % WEAPONS,
    }
}

/// Speed of a projectile (px/s at scale 1) and the height of its arc as a fraction of the distance.
fn flight(kind: usize) -> (f64, f64) {
    match kind {
        PEA => (1000.0, 0.04),
        BAND => (850.0, 0.03),
        CORK => (640.0, 0.14),
        PIPETTE => (560.0, 0.20),
        SPOON => (430.0, 0.50),
        PEPPER => (330.0, 0.06),
        _ => (210.0, 0.10),
    }
}

// --------------------------------------------------------------------------- the roach

/// Where the body parts are, in units of the radius, facing right, feet on the ground at y = +0.95.
/// Legs: 0-2 the near front, middle and hind leg, 3-5 the same on the far side.
#[derive(Clone, Copy)]
struct Pose {
    /// where thorax and abdomen meet
    c: P,
    /// 0 = level on all sixes, PI/2 = upright
    lean: f64,
    /// the whole roach turned around `c` (PI = on its back)
    roll: f64,
    /// head turned down (+) or up (-)
    nod: f64,
    feet: [P; 6],
    /// which way the knees bend
    kb: [f64; 6],
    /// antenna tips, relative to the head
    ant: [P; 2],
    /// 0..1 open mouth
    mouth: f64,
}

impl Pose {
    fn mix(&self, o: &Pose, k: f64) -> Pose {
        Pose {
            c: mixp(self.c, o.c, k),
            lean: lerp(self.lean, o.lean, k),
            roll: lerp(self.roll, o.roll, k),
            nod: lerp(self.nod, o.nod, k),
            feet: std::array::from_fn(|i| mixp(self.feet[i], o.feet[i], k)),
            kb: std::array::from_fn(|i| lerp(self.kb[i], o.kb[i], k)),
            ant: [mixp(self.ant[0], o.ant[0], k), mixp(self.ant[1], o.ant[1], k)],
            mouth: lerp(self.mouth, o.mouth, k),
        }
    }

    /// On all sixes: a quick tripod gait, standing when `mv` is 0.
    fn scurry(step: f64, mv: f64, t: f64, seed: f64) -> Pose {
        let bob = (step * 2.0).sin() * 0.012 * mv;
        let feet = std::array::from_fn(|k| {
            let (side, j) = ((k / 3) as f64, k % 3);
            let ph = step + if (j == 1) != (k >= 3) { PI } else { 0.0 };
            ([0.52, 0.0, -0.44][j] + side * 0.07 + 0.12 * ph.sin() * mv, 0.95 - ph.cos().max(0.0) * 0.07 * mv - side * 0.03)
        });
        let (w1, w2) = ((t * 2.3 + seed).sin(), (t * 3.1 + seed * 1.7).sin());
        Pose {
            c: (0.0, 0.58 + bob),
            lean: 0.08 + 0.05 * mv,
            roll: 0.0,
            nod: 0.0,
            feet,
            kb: [-1.0, 1.0, 1.0, -1.0, 1.0, 1.0],
            ant: [(0.44 + 0.07 * w1, -0.52 + 0.05 * w2 + 0.08 * mv), (0.66 + 0.05 * w2, -0.36 + 0.07 * w1 + 0.08 * mv)],
            mouth: 0.0,
        }
    }

    /// Standing on the hind legs, the other four held in front.
    fn upright(t: f64, seed: f64) -> Pose {
        let br = (t * 1.6 + seed).sin() * 0.01;
        let w = (t * 2.6 + seed).sin();
        Pose {
            c: (-0.02, 0.27 + br),
            lean: 1.2,
            roll: 0.0,
            nod: 0.0,
            feet: [(0.36, 0.08 + br), (0.37, 0.36), (0.20, 0.95), (0.42, 0.04 + br), (0.42, 0.30), (0.02, 0.95)],
            kb: [1.0, 1.0, -0.7, 1.0, 1.0, -0.7],
            ant: [(-0.04 + 0.06 * w, -0.62), (0.38 + 0.06 * w, -0.58)],
            mouth: 0.0,
        }
    }

    /// Head down at a piece of food, mandibles going.
    fn eat(t: f64, seed: f64) -> Pose {
        let ch = (t * 11.0 + seed).sin();
        let mut p = Pose::scurry(0.0, 0.0, t, seed);
        p.c = (0.0, 0.60);
        p.lean = -0.12;
        p.nod = 0.38 + 0.10 * ch;
        p.mouth = 0.5 + 0.5 * ch;
        p.feet[0] = (0.50, 0.95 - ch.max(0.0) * 0.04);
        p.ant = [(0.36, -0.30 + 0.04 * ch), (0.52, -0.20 - 0.04 * ch)];
        p
    }

    /// Sitting up and pulling one antenna through the front legs.
    fn groom(t: f64, seed: f64) -> Pose {
        let pull = 0.5 + 0.5 * (t * 3.2 + seed).sin();
        let tip = mixp((0.30, -0.26), (0.36, 0.14), pull);
        let head = (0.25, 0.13);
        let hand = (head.0 + tip.0 * 0.85, head.1 + tip.1 * 0.85 + 0.03);
        Pose {
            c: (-0.06, 0.44),
            lean: 0.72,
            roll: 0.0,
            nod: 0.18,
            feet: [hand, (0.34, 0.95), (0.02, 0.95), (hand.0 + 0.05, hand.1 + 0.06), (0.42, 0.93), (-0.10, 0.94)],
            kb: [1.0, 0.3, 1.0, 1.0, 0.3, 1.0],
            ant: [tip, (0.34, -0.50 + 0.04 * (t * 2.1 + seed).sin())],
            mouth: 0.0,
        }
    }

    /// Waiting: upright, waving the flag, the other legs and both antennae, calling.
    fn wave(t: f64, seed: f64) -> Pose {
        let mut p = Pose::upright(t, seed);
        let sw = (t * 6.0 + seed).sin();
        let hop = sw.abs() * 0.04;
        let fl = (t * 9.0 + seed).sin() * 0.10;
        p.c.1 -= hop;
        p.lean = 1.28 + 0.05 * sw;
        p.feet[0] = (0.36 + 0.10 * sw, -0.30 - hop);
        p.feet[3] = (-0.06 - 0.10 * sw, -0.28 - hop);
        p.feet[1] = (0.42, 0.22 + fl);
        p.feet[4] = (0.44, 0.30 - fl);
        p.kb[3] = -1.0;
        p.ant = [(-0.20 + 0.22 * (t * 7.0 + seed).sin(), -0.62), (0.40 + 0.22 * (t * 7.0 + seed + 1.2).sin(), -0.60)];
        p.mouth = 0.5 + 0.5 * (t * 8.0).sin();
        p.nod = -0.12;
        p
    }

    /// On its back, legs wiggling in the air (`speed` 1 = frantic, less for the blocked one).
    fn back(t: f64, seed: f64, speed: f64) -> Pose {
        let feet = std::array::from_fn(|k| {
            let (side, j) = ((k / 3) as f64, k % 3);
            let ph = t * speed * [7.0, 6.1, 5.3][j] + seed + k as f64 * 1.9;
            ([0.40, 0.06, -0.30][j] + side * 0.06 + 0.10 * ph.sin(), 1.04 + 0.07 * (ph * 1.3).cos() - side * 0.03)
        });
        Pose {
            c: (0.0, 0.68),
            lean: 0.0,
            roll: PI,
            nod: 0.0,
            feet,
            kb: [-1.0, 1.0, 1.0, -1.0, 1.0, 1.0],
            ant: [(0.50, -0.20 + 0.03 * (t * 2.0 * speed + seed).sin()), (0.62, -0.12)],
            mouth: 0.0,
        }
    }

    /// Head down, tail up, the front legs scrabbling at the ground.
    fn dig(t: f64, seed: f64) -> Pose {
        let ph = t * 24.0 + seed;
        let mut p = Pose::scurry(0.0, 0.0, t, seed);
        p.c = (0.0, 0.50 + 0.012 * ph.sin());
        p.lean = -0.42;
        p.nod = 0.35;
        p.feet[0] = (0.50 + 0.13 * ph.sin(), 0.93 - 0.08 * ph.cos().max(0.0));
        p.feet[3] = (0.56 - 0.13 * ph.sin(), 0.90 - 0.08 * (-ph.cos()).max(0.0));
        p.feet[1] = (0.10 + 0.08 * (ph * 0.5).sin(), 0.95);
        p.ant = [(0.26, -0.46), (0.08, -0.50)];
        p
    }

    /// Out of the hole again: upright with the front legs thrown up.
    fn cheer(t: f64, seed: f64) -> Pose {
        let mut p = Pose::upright(t, seed);
        let w = (t * 14.0 + seed).sin() * 0.04;
        p.feet[0] = (0.40, -0.26 + w);
        p.feet[3] = (-0.08, -0.28 - w);
        p.kb[3] = -1.0;
        p.mouth = 1.0;
        p.nod = -0.2;
        p
    }

    /// Asleep on its belly, antennae drooping.
    fn nap(t: f64, seed: f64) -> Pose {
        let br = (t * 1.3 + seed).sin();
        let mut p = Pose::scurry(0.0, 0.0, 0.0, 0.0);
        p.c.1 += 0.012 * br;
        p.nod = 0.22 + 0.03 * br;
        p.ant = [(0.46, 0.30 + 0.02 * br), (0.60, 0.20 + 0.02 * br)];
        p
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Eyes {
    Open,
    Shut,
    /// "> <"
    Squeeze,
    /// pupils going round
    Dizzy,
    /// startled: tiny pupils
    Wide,
}

/// How a roach is drawn besides its pose.
struct Looks {
    /// state colour (helmet and rim light)
    c: Rgb,
    a: f64,
    /// opacity of the rim light (flickers when blocked)
    glow: f64,
    eyes: Eyes,
    /// where the pupils look (-1..1)
    look: P,
    /// which junk helmet it wears, if it has it on
    helmet: Option<usize>,
    /// a nymph: pale, no wings
    pale: bool,
    frown: bool,
    /// 0..1 whipped cream in the face
    cream: f64,
    t: f64,
}

/// where the legs leave the body (along the body, towards the belly) and how long their two parts are
const ATT: [P; 3] = [(0.20, 0.13), (0.04, 0.17), (-0.14, 0.17)];
const LEGS: [f64; 3] = [0.22, 0.24, 0.30];
/// the antennae are drawn this much longer than the pose says
const ANT: f64 = 1.35;

/// The junk helmet around the head centre (0, 0): 0 half a nutshell, 1 a bottle cap, 2 an acorn cap.
fn helmet(cr: &mut Canvas, q: f64, kind: usize, c: Rgb, a: f64) {
    let dark = darken(c, 0.38);
    cr.set_line_cap(LineCap::Round);
    if kind == 1 {
        // bottle cap: flat top, crimped edge
        cr.move_to(-0.27 * q, -0.13 * q);
        cr.line_to(-0.21 * q, -0.32 * q);
        cr.quad_to(0.0, -0.37 * q, 0.21 * q, -0.32 * q);
        cr.line_to(0.27 * q, -0.13 * q);
        for i in 0..8 {
            let x = 0.27 - (i as f64 + 0.5) * 0.0675;
            cr.line_to(x * q, -0.075 * q);
            cr.line_to((x - 0.03375) * q, -0.13 * q);
        }
        cr.close_path();
        cr.set_rgba(c, a);
        cr.fill();
        cr.set_rgba(dark, 0.8 * a);
        cr.set_line_width(0.028 * q);
        cr.move_to(-0.235 * q, -0.185 * q);
        cr.line_to(0.235 * q, -0.185 * q);
        cr.stroke();
    } else {
        cr.move_to(-0.28 * q, -0.11 * q);
        cr.curve_to(-0.28 * q, -0.47 * q, 0.28 * q, -0.47 * q, 0.28 * q, -0.11 * q);
        cr.quad_to(0.0, -0.07 * q, -0.28 * q, -0.11 * q);
        cr.close_path();
        cr.set_rgba(c, a);
        cr.fill();
        cr.set_rgba(dark, 0.75 * a);
        cr.set_line_width(0.02 * q);
        if kind == 0 {
            // the ridges of the nutshell
            for k in [-1.0, 0.0, 1.0] {
                cr.move_to(k * 0.03 * q, -0.375 * q);
                cr.quad_to(k * 0.17 * q, -0.30 * q, k * 0.16 * q, -0.10 * q);
            }
        } else {
            // the scales of the acorn cap and its stalk
            for k in -3..=3 {
                let x = k as f64 * 0.075;
                cr.move_to((x - 0.05) * q, -0.13 * q);
                cr.line_to((x + 0.03) * q, -0.24 * q);
                cr.line_to((x - 0.03) * q, -0.33 * q);
            }
            cr.move_to(0.0, -0.37 * q);
            cr.line_to(0.03 * q, -0.45 * q);
        }
        cr.stroke();
        cr.set_rgba(dark, a);
        cr.set_line_width(0.035 * q);
        cr.move_to(-0.28 * q, -0.11 * q);
        cr.quad_to(0.0, -0.07 * q, 0.28 * q, -0.11 * q);
        cr.stroke();
    }
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.5 * a);
    cr.ellipse(-0.11 * q, -0.275 * q, 0.07 * q, 0.03 * q);
    cr.fill();
}

/// The head around (0, 0), looking to the right; `q` is the radius of the roach.
fn head(cr: &mut Canvas, q: f64, lk: &Looks, mouth: f64) {
    let a = lk.a;
    let hc = if lk.pale { PALE_HEAD } else { HEADC };
    cr.set_rgba(hc, a);
    cr.ellipse(0.0, 0.0, 0.25 * q, 0.235 * q);
    cr.fill();
    cr.set_rgba(lighten(hc, 0.18), a);
    cr.ellipse(0.08 * q, 0.105 * q, 0.15 * q, 0.105 * q);
    cr.fill();
    // big eyes: the far one a little smaller
    let eyes = [(0.172, 0.0, 0.068, 0.095), (0.035, 0.005, 0.095, 0.112)];
    cr.set_line_cap(LineCap::Round);
    cr.set_line_join(LineJoin::Round);
    match lk.eyes {
        Eyes::Shut => {
            cr.set_rgba(INK, a);
            cr.set_line_width(0.028 * q);
            for (x, y, rx, _) in eyes {
                cr.new_sub_path();
                cr.arc(x * q, (y - 0.02) * q, rx * 0.8 * q, 0.25, PI - 0.25);
            }
            cr.stroke();
        }
        Eyes::Squeeze => {
            cr.set_rgba(INK, a);
            cr.set_line_width(0.03 * q);
            for (i, (x, y, rx, _)) in eyes.into_iter().enumerate() {
                let d = if i == 0 { -1.0 } else { 1.0 };
                cr.move_to((x - d * rx * 0.55) * q, (y - 0.05) * q);
                cr.line_to((x + d * rx * 0.45) * q, y * q);
                cr.line_to((x - d * rx * 0.55) * q, (y + 0.05) * q);
            }
            cr.stroke();
        }
        _ => {
            cr.set_source_rgba(1.0, 1.0, 0.97, a);
            for (x, y, rx, ry) in eyes {
                cr.ellipse(x * q, y * q, rx * q, ry * q);
            }
            cr.fill();
            let size = if lk.eyes == Eyes::Wide { 0.30 } else { 0.52 };
            for (i, (x, y, rx, _)) in eyes.into_iter().enumerate() {
                let (px, py) = match lk.eyes {
                    Eyes::Dizzy => {
                        let ang = lk.t * 11.0 + i as f64 * PI;
                        (ang.cos() * rx * 0.36, ang.sin() * rx * 0.42)
                    }
                    Eyes::Wide => (0.0, 0.0),
                    _ => (rx * (0.24 + lk.look.0 * 0.22), rx * (0.05 + lk.look.1 * 0.36)),
                };
                cr.set_rgba(INK, a);
                cr.circle((x + px) * q, (y + py) * q, rx * size * q);
                cr.fill();
                cr.set_source_rgba(1.0, 1.0, 1.0, 0.95 * a);
                cr.circle((x + px - rx * 0.16) * q, (y + py - rx * 0.2) * q, rx * 0.17 * q);
                cr.fill();
            }
        }
    }
    // mouth: a smile, a long face, or open
    cr.set_rgba(INK, a);
    if mouth > 0.15 {
        cr.ellipse(0.135 * q, 0.15 * q, 0.055 * q, (0.018 + 0.045 * mouth) * q);
        cr.fill();
    } else {
        cr.set_line_width(0.024 * q);
        cr.new_sub_path();
        if lk.frown {
            cr.arc(0.135 * q, 0.215 * q, 0.065 * q, PI + 0.55, TAU - 0.55);
        } else {
            cr.arc(0.125 * q, 0.095 * q, 0.07 * q, 0.35, PI - 0.7);
        }
        cr.stroke();
    }
    cr.set_source_rgba(1.0, 0.52, 0.42, 0.38 * a);
    cr.ellipse(-0.05 * q, 0.105 * q, 0.05 * q, 0.033 * q);
    cr.fill();
    if let Some(kind) = lk.helmet {
        helmet(cr, q, kind, lk.c, a);
    }
    if lk.cream > 0.02 {
        // a dollop of whipped cream right in the face
        cr.set_source_rgba(1.0, 0.99, 0.95, clamp(lk.cream * 2.5, 0.0, 1.0) * a);
        let k = 0.6 + 0.4 * lk.cream;
        for (x, y, rr) in [(0.10, 0.0, 0.15), (0.0, 0.07, 0.10), (0.19, 0.09, 0.09), (0.05, -0.12, 0.08), (0.12, 0.20 + 0.10 * (1.0 - lk.cream), 0.05)] {
            cr.circle(x * q, y * q, rr * k * q);
        }
        cr.fill();
    }
}

/// Abdomen and thorax in the body's own frame: x along the body towards the head, y towards the belly.
fn body(cr: &mut Canvas, r: f64, lk: &Looks) {
    let a = lk.a;
    let (shell, belly) = if lk.pale { (PALE_SHELL, PALE_BELLY) } else { (SHELL, BELLY) };
    // abdomen: an egg pointing backwards, the belly side first
    cr.move_to(0.06 * r, -0.25 * r);
    cr.curve_to(-0.30 * r, -0.30 * r, -0.74 * r, -0.16 * r, -0.76 * r, 0.02 * r);
    cr.curve_to(-0.72 * r, 0.20 * r, -0.30 * r, 0.29 * r, 0.06 * r, 0.24 * r);
    cr.close_path();
    cr.set_rgba(belly, a);
    cr.fill();
    cr.set_rgba(darken(belly, 0.32), 0.8 * a);
    cr.set_line_width(0.02 * r);
    for i in 0..4 {
        let x = -0.10 - 0.15 * i as f64;
        cr.move_to((x + 0.02) * r, 0.05 * r);
        cr.quad_to((x - 0.04) * r, 0.15 * r, (x - 0.01) * r, (0.25 - 0.025 * i as f64) * r);
    }
    cr.stroke();
    // wing covers over the back (a nymph has none: a striped back instead)
    cr.move_to(0.06 * r, -0.25 * r);
    cr.curve_to(-0.30 * r, -0.30 * r, -0.74 * r, -0.16 * r, -0.78 * r, 0.03 * r);
    if lk.pale {
        cr.curve_to(-0.55 * r, -0.02 * r, -0.20 * r, -0.06 * r, 0.06 * r, -0.06 * r);
    } else {
        cr.curve_to(-0.55 * r, 0.10 * r, -0.20 * r, 0.07 * r, 0.06 * r, 0.05 * r);
    }
    cr.close_path();
    cr.set_rgba(shell, a);
    cr.fill();
    if lk.pale {
        cr.set_rgba(darken(shell, 0.25), 0.7 * a);
        cr.set_line_width(0.022 * r);
        for i in 0..4 {
            let x = -0.08 - 0.15 * i as f64;
            cr.move_to(x * r, (-0.25 + 0.02 * i as f64) * r);
            cr.line_to((x - 0.03) * r, -0.06 * r);
        }
        cr.stroke();
    } else {
        cr.set_rgba(darken(shell, 0.4), 0.75 * a);
        cr.set_line_width(0.018 * r);
        cr.move_to(0.04 * r, -0.07 * r);
        cr.quad_to(-0.40 * r, -0.10 * r, -0.74 * r, 0.0);
        cr.stroke();
        cr.set_source_rgba(1.0, 0.92, 0.75, 0.42 * a);
        cr.set_line_width(0.04 * r);
        cr.move_to(-0.10 * r, -0.195 * r);
        cr.quad_to(-0.36 * r, -0.225 * r, -0.58 * r, -0.13 * r);
        cr.stroke();
    }
    // rim light in the state colour along the back
    cr.move_to(0.04 * r, -0.265 * r);
    cr.curve_to(-0.30 * r, -0.315 * r, -0.74 * r, -0.175 * r, -0.775 * r, 0.0);
    cr.set_rgba(lk.c, 0.9 * lk.glow);
    cr.set_line_width(0.045 * r);
    cr.stroke();
    // the shield behind the head
    cr.set_rgba(lighten(shell, 0.10), a);
    cr.ellipse(0.13 * r, -0.03 * r, 0.21 * r, 0.26 * r);
    cr.fill();
    cr.set_source_rgba(1.0, 0.92, 0.75, 0.30 * a);
    cr.ellipse(0.10 * r, -0.17 * r, 0.10 * r, 0.045 * r);
    cr.fill();
}

/// The roach in local coordinates (facing right, feet at y = +0.95 R).
fn figure(cr: &mut Canvas, p: &Pose, r: f64, lk: &Looks) {
    let a = lk.a;
    if a < 0.01 {
        return;
    }
    let (u, n) = ((p.lean.cos(), -p.lean.sin()), (p.lean.sin(), p.lean.cos()));
    let at = |bx: f64, by: f64| (p.c.0 + u.0 * bx + n.0 * by, p.c.1 + u.1 * bx + n.1 * by);
    let legc = if lk.pale { PALE_LEG } else { LEGC };
    cr.save();
    cr.translate(p.c.0 * r, p.c.1 * r);
    cr.rotate(p.roll);
    cr.translate(-p.c.0 * r, -p.c.1 * r);
    cr.set_line_cap(LineCap::Round);
    cr.set_line_join(LineJoin::Round);
    let legs = |cr: &mut Canvas, far: bool| {
        for j in 0..3 {
            let k = if far { j + 3 } else { j };
            let a0 = if far { at(ATT[j].0 + 0.05, ATT[j].1 - 0.06) } else { at(ATT[j].0, ATT[j].1) };
            let kn = joint(a0, p.feet[k], LEGS[j], p.kb[k]);
            cr.move_to(a0.0 * r, a0.1 * r);
            cr.line_to(kn.0 * r, kn.1 * r);
            cr.line_to(p.feet[k].0 * r, p.feet[k].1 * r);
        }
    };
    legs(cr, true);
    cr.set_rgba(darken(legc, 0.3), a);
    cr.set_line_width(0.045 * r);
    cr.stroke();
    cr.save();
    cr.translate(p.c.0 * r, p.c.1 * r);
    cr.rotate(-p.lean);
    body(cr, r, lk);
    cr.restore();
    legs(cr, false);
    cr.set_rgba(legc, a);
    cr.set_line_width(0.05 * r);
    cr.stroke();
    // the head stays level whatever the body does; the long antennae start under the helmet
    let hp = at(0.44, -0.03);
    cr.save();
    cr.translate(hp.0 * r, hp.1 * r);
    cr.set_rgba(legc, a);
    let tips = p.ant.map(|tip| (tip.0 * ANT, tip.1 * ANT));
    for (i, tip) in tips.iter().enumerate() {
        let base = (0.04 + 0.11 * i as f64, -0.16);
        cr.move_to(base.0 * r, base.1 * r);
        cr.quad_to((base.0 + tip.0) * 0.5 * r - 0.06 * r, (base.1.min(tip.1) - 0.18) * r, tip.0 * r, tip.1 * r);
    }
    cr.set_line_width(0.026 * r);
    cr.stroke();
    for tip in &tips {
        cr.circle(tip.0 * r, tip.1 * r, 0.03 * r);
    }
    cr.fill();
    cr.rotate(p.nod);
    head(cr, r, lk, p.mouth);
    cr.restore();
    cr.restore();
}

/// The toy weapon in its own frame: held at (0, 0), pointing along +x. `charge` 0..1 builds up before
/// the shot, `fired` = the projectile has left.
fn weapon(cr: &mut Canvas, kind: usize, q: f64, charge: f64, fired: bool, a: f64) {
    cr.set_line_cap(LineCap::Round);
    match kind {
        PEA => {
            // a striped drinking straw
            cr.set_source_rgba(0.97, 0.96, 0.92, a);
            cr.set_line_width(0.08 * q);
            cr.move_to(-0.12 * q, 0.0);
            cr.line_to(0.62 * q, 0.0);
            cr.stroke();
            cr.set_line_cap(LineCap::Butt);
            cr.set_source_rgba(0.90, 0.26, 0.22, a);
            cr.set_line_width(0.032 * q);
            for i in 0..6 {
                let x = -0.08 + i as f64 * 0.125;
                cr.move_to(x * q, -0.04 * q);
                cr.line_to((x + 0.045) * q, 0.04 * q);
            }
            cr.stroke();
        }
        BAND => {
            // a matchstick with a rubber band hooked over its head
            cr.set_source_rgba(0.88, 0.74, 0.50, a);
            cr.set_line_width(0.058 * q);
            cr.move_to(0.22 * q, 0.24 * q);
            cr.line_to(0.22 * q, -0.28 * q);
            cr.stroke();
            cr.set_source_rgba(0.80, 0.22, 0.16, a);
            cr.ellipse(0.22 * q, -0.31 * q, 0.052 * q, 0.068 * q);
            cr.fill();
            if !fired {
                let pull = (-0.12 - 0.24 * charge) * q;
                cr.set_source_rgba(0.98, 0.55, 0.66, a);
                cr.set_line_width(0.034 * q);
                cr.move_to(0.22 * q, -0.30 * q);
                cr.line_to(pull, 0.04 * q);
                cr.line_to(0.22 * q, -0.20 * q);
                cr.stroke();
            }
        }
        CORK => {
            // popgun: a green tube, a plunger at the back, the cork in the muzzle
            let push = if fired { 0.16 } else { 0.0 };
            cr.set_source_rgba(0.62, 0.42, 0.24, a);
            cr.set_line_width(0.045 * q);
            cr.move_to((-0.36 + push) * q, 0.0);
            cr.line_to(-0.10 * q, 0.0);
            cr.stroke();
            cr.circle((-0.36 + push) * q, 0.0, 0.055 * q);
            cr.fill();
            cr.set_line_cap(LineCap::Butt);
            cr.set_source_rgba(0.40, 0.68, 0.44, a);
            cr.set_line_width(0.17 * q);
            cr.move_to(-0.13 * q, 0.0);
            cr.line_to(0.42 * q, 0.0);
            cr.stroke();
            cr.set_source_rgba(0.24, 0.46, 0.30, a);
            cr.set_line_width(0.19 * q);
            cr.move_to(0.36 * q, 0.0);
            cr.line_to(0.42 * q, 0.0);
            cr.move_to(-0.13 * q, 0.0);
            cr.line_to(-0.08 * q, 0.0);
            cr.stroke();
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.35 * a);
            cr.set_line_width(0.025 * q);
            cr.move_to(-0.04 * q, -0.05 * q);
            cr.line_to(0.32 * q, -0.05 * q);
            cr.stroke();
            if !fired {
                cr.set_source_rgba(0.84, 0.66, 0.42, a);
                cr.rounded_rect(0.40 * q, -0.06 * q, 0.15 * q, 0.12 * q, 0.03 * q);
                cr.fill();
            }
        }
        PIPETTE => {
            // glass pipette with a rubber bulb
            let sq = if fired { 0.5 } else { 1.0 - 0.12 * charge };
            cr.set_source_rgba(0.92, 0.44, 0.26, a);
            cr.ellipse(-0.12 * q, 0.0, 0.135 * q, 0.12 * sq * q);
            cr.fill();
            let glass = |cr: &mut Canvas, k: f64| {
                cr.move_to(-0.02 * q, -0.05 * k * q);
                cr.line_to(0.42 * q, -0.04 * k * q);
                cr.line_to(0.62 * q, -0.014 * q);
                cr.line_to(0.62 * q, 0.014 * q);
                cr.line_to(0.42 * q, 0.04 * k * q);
                cr.line_to(-0.02 * q, 0.05 * k * q);
                cr.close_path();
            };
            glass(cr, 1.0);
            cr.set_source_rgba(0.82, 0.93, 1.0, 0.5 * a);
            cr.fill();
            if !fired {
                glass(cr, 0.6);
                cr.set_source_rgba(0.45, 0.74, 1.0, 0.85 * a);
                cr.fill();
            }
            glass(cr, 1.0);
            cr.set_source_rgba(0.92, 0.98, 1.0, 0.8 * a);
            cr.set_line_width(0.014 * q);
            cr.stroke();
        }
        SPOON => {
            // plastic spoon loaded with whipped cream
            cr.set_source_rgba(0.94, 0.94, 0.96, a);
            cr.set_line_width(0.055 * q);
            cr.move_to(-0.06 * q, 0.0);
            cr.line_to(0.60 * q, 0.0);
            cr.stroke();
            cr.ellipse(0.72 * q, 0.0, 0.15 * q, 0.09 * q);
            cr.fill();
            cr.set_source_rgba(0.74, 0.75, 0.80, a);
            cr.ellipse(0.72 * q, -0.012 * q, 0.11 * q, 0.05 * q);
            cr.fill();
            if !fired {
                cr.set_source_rgba(1.0, 0.99, 0.95, a);
                for (x, y, rr) in [(0.72, -0.07, 0.09), (0.65, -0.04, 0.06), (0.79, -0.04, 0.06), (0.72, -0.16, 0.04)] {
                    cr.circle(x * q, y * q, rr * q);
                }
                cr.fill();
            }
        }
        PEPPER => {
            // glass shaker with a metal top
            cr.set_source_rgba(0.82, 0.88, 0.90, 0.6 * a);
            cr.rounded_rect(0.0, -0.10 * q, 0.32 * q, 0.20 * q, 0.04 * q);
            cr.fill();
            cr.set_source_rgba(0.30, 0.22, 0.16, a);
            cr.rounded_rect(0.02 * q, -0.03 * q, 0.28 * q, 0.11 * q, 0.03 * q);
            cr.fill();
            cr.set_source_rgba(0.80, 0.80, 0.83, a);
            cr.rounded_rect(0.30 * q, -0.115 * q, 0.11 * q, 0.23 * q, 0.035 * q);
            cr.fill();
            cr.set_source_rgba(0.25, 0.25, 0.28, a);
            for y in [-0.06, 0.0, 0.06] {
                cr.circle(0.385 * q, y * q, 0.014 * q);
            }
            cr.fill();
        }
        _ => {
            // bubble wand: a bubble grows on the ring
            cr.set_source_rgba(0.98, 0.80, 0.26, a);
            cr.set_line_width(0.042 * q);
            cr.move_to(-0.10 * q, 0.0);
            cr.line_to(0.40 * q, 0.0);
            cr.stroke();
            cr.set_line_width(0.036 * q);
            cr.circle(0.50 * q, 0.0, 0.10 * q);
            cr.stroke();
            if !fired {
                let br = (0.07 + 0.15 * charge) * q;
                bubble(cr, 0.55 * q + br * 0.7, 0.0, br, a);
            }
        }
    }
}

/// A soap bubble.
fn bubble(cr: &mut Canvas, x: f64, y: f64, rad: f64, a: f64) {
    cr.set_source_rgba(0.70, 0.90, 1.0, 0.14 * a);
    cr.circle(x, y, rad);
    cr.fill();
    cr.set_source_rgba(0.86, 0.97, 1.0, 0.75 * a);
    cr.set_line_width((rad * 0.07).max(1.0));
    cr.circle(x, y, rad);
    cr.stroke();
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.8 * a);
    cr.ellipse(x - rad * 0.38, y - rad * 0.42, rad * 0.20, rad * 0.11);
    cr.fill();
}

/// A little star with four points.
fn star(cr: &mut Canvas, x: f64, y: f64, rad: f64, rot: f64) {
    for i in 0..8 {
        let ang = rot + i as f64 * PI / 4.0;
        let rr = if i % 2 == 0 { rad } else { rad * 0.38 };
        cr.line_to(x + ang.cos() * rr, y + ang.sin() * rr);
    }
    cr.close_path();
    cr.new_sub_path();
}

// --------------------------------------------------------------------------- world state

/// Who can be shot at (rebuilt every frame from the actors).
struct Mark {
    index: u64,
    x: f64,
    y: f64,
    ok: bool,
}

/// A projectile on its way.
struct Shot {
    kind: usize,
    from: u64,
    /// 0 = nobody: a pot shot at a spot on the floor
    target: u64,
    x0: f64,
    y0: f64,
    tx: f64,
    ty: f64,
    age: f64,
    dur: f64,
    /// height of the arc
    arc: f64,
    /// radius of the shooter: the size of everything
    z: f64,
}

impl Shot {
    fn pos(&self, f: f64) -> P {
        (lerp(self.x0, self.tx, f), lerp(self.y0, self.ty, f) - self.arc * 4.0 * f * (1.0 - f))
    }
}

/// Where a projectile came down. The roaches look here to find out that they were hit.
struct Impact {
    x: f64,
    y: f64,
    /// the ground below
    gy: f64,
    t0: f64,
    kind: usize,
    from: u64,
    z: f64,
}

/// Flying dirt, crumbs, drops, puffs and stars.
struct Bit {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    age: f64,
    life: f64,
    c: Rgb,
    size: f64,
    /// share of gravity
    grav: f64,
    /// 0 a lump, 1 a puff that grows and fades, 2 a star
    kind: u8,
    /// lumps stay lying at this height
    floor: f64,
    lying: bool,
}

/// The heap around a burrow hole; it flattens with age.
struct Mound {
    x: f64,
    y: f64,
    /// radius of the roach that dug it
    k: f64,
    born: f64,
}

struct Food {
    id: u64,
    kind: usize,
    x: f64,
    /// where it lies on the ground
    y: f64,
    /// pixels per unit
    k: f64,
    /// which side is eaten (+1 = right)
    side: f64,
    /// 1 = whole, 0 = gone
    amount: f64,
    /// one picture per stage: a quarter left .. whole
    pics: Vec<Picture>,
    /// when it dropped in
    born: f64,
    landed: bool,
    /// who is at it (index, when it last said so)
    claim: (u64, f64),
}

/// half the width of each kind of food in its own units, its size and the colour of its crumbs
const FOOD_HW: [f64; 8] = [70.0, 60.0, 48.0, 70.0, 44.0, 40.0, 64.0, 56.0];
const FOOD_SIZE: [f64; 8] = [1.25, 1.3, 1.3, 1.2, 1.0, 1.05, 1.15, 1.0];
const FOOD_CRUMB: [Rgb; 8] = [
    (1.0, 0.82, 0.32),
    (0.90, 0.70, 0.40),
    (0.90, 0.22, 0.22),
    (0.84, 0.60, 0.32),
    (1.0, 1.0, 1.0),
    (0.66, 0.80, 0.32),
    (0.96, 0.80, 0.38),
    (0.86, 0.66, 0.40),
];
/// the picture of a piece of food: size in units and where its ground point is
const FOOD_BOX: (f64, f64, f64, f64) = (210.0, 150.0, 105.0, 122.0);

impl Food {
    fn stage(&self) -> usize {
        ((self.amount * 4.0).ceil() as usize).clamp(1, 4)
    }

    /// How far the eaten edge is from the middle, in pixels.
    fn front(&self) -> f64 {
        FOOD_HW[self.kind] * [-0.45, 0.0, 0.5, 1.0][self.stage() - 1] * self.k
    }
}

/// One piece of food around its ground point (0, 0), in its own units.
/// 0 cheese, 1 biscuit, 2 strawberry, 3 pizza crust, 4 sugar cube, 5 grape, 6 potato chip, 7 breadcrumbs.
fn food_shape(cr: &mut Canvas, kind: usize) {
    let poly = |cr: &mut Canvas, pts: &[P], c: Rgb| {
        for p in pts {
            cr.line_to(p.0, p.1);
        }
        cr.close_path();
        cr.set_rgb(c);
        cr.fill();
    };
    match kind {
        0 => {
            poly(cr, &[(-70.0, 0.0), (70.0, 0.0), (70.0, -64.0), (-70.0, -20.0)], (0.96, 0.74, 0.22));
            poly(cr, &[(-70.0, -20.0), (70.0, -64.0), (44.0, -80.0), (-78.0, -31.0)], (1.0, 0.87, 0.42));
            cr.set_source_rgb(0.80, 0.56, 0.12);
            for (x, y, rx, ry) in [(22.0, -28.0, 13.0, 11.0), (-24.0, -11.0, 8.0, 6.5), (50.0, -12.0, 7.0, 6.0), (52.0, -46.0, 6.5, 6.5), (-48.0, -9.0, 5.0, 4.0)] {
                cr.ellipse(x, y, rx, ry);
            }
            cr.fill();
            cr.set_source_rgb(0.90, 0.70, 0.26);
            cr.ellipse(8.0, -57.0, 10.0, 4.0);
            cr.ellipse(-38.0, -36.0, 7.0, 3.0);
            cr.fill();
        }
        1 => {
            cr.set_source_rgb(0.80, 0.56, 0.28);
            for i in 0..16 {
                let ang = i as f64 * TAU / 16.0;
                cr.circle(ang.cos() * 53.0, -56.0 + ang.sin() * 53.0, 9.0);
            }
            cr.circle(0.0, -56.0, 54.0);
            cr.fill();
            cr.set_source_rgb(0.93, 0.74, 0.44);
            cr.circle(0.0, -56.0, 48.0);
            cr.fill();
            cr.set_source_rgb(0.66, 0.44, 0.20);
            for (x, y) in [(0.0, -56.0), (-22.0, -72.0), (22.0, -72.0), (-22.0, -40.0), (22.0, -40.0), (0.0, -86.0), (0.0, -26.0)] {
                cr.circle(x, y, 4.0);
            }
            cr.fill();
            cr.set_source_rgba(1.0, 0.95, 0.75, 0.4);
            cr.ellipse(-16.0, -84.0, 20.0, 7.0);
            cr.fill();
        }
        2 => {
            cr.save();
            cr.rotate(0.22);
            cr.move_to(0.0, 0.0);
            cr.curve_to(-54.0, -30.0, -58.0, -86.0, -24.0, -94.0);
            cr.curve_to(-8.0, -100.0, 8.0, -100.0, 24.0, -94.0);
            cr.curve_to(58.0, -86.0, 54.0, -30.0, 0.0, 0.0);
            cr.close_path();
            cr.set_source_rgb(0.88, 0.15, 0.17);
            cr.fill();
            cr.set_source_rgba(1.0, 0.75, 0.70, 0.35);
            cr.ellipse(-20.0, -64.0, 10.0, 18.0);
            cr.fill();
            cr.set_source_rgb(1.0, 0.86, 0.42);
            for (x, y) in [(-4.0, -24.0), (-22.0, -40.0), (14.0, -42.0), (-32.0, -62.0), (0.0, -60.0), (30.0, -64.0), (-14.0, -80.0), (16.0, -82.0)] {
                cr.ellipse(x, y, 2.6, 3.8);
            }
            cr.fill();
            cr.set_source_rgb(0.28, 0.58, 0.22);
            for k in -2..=2 {
                let x = k as f64 * 15.0;
                cr.move_to(x - 9.0, -96.0);
                cr.line_to(x * 1.5, -78.0 + (k as f64).abs() * 3.0);
                cr.line_to(x + 9.0, -96.0);
                cr.close_path();
            }
            cr.rectangle(-3.0, -110.0, 6.0, 16.0);
            cr.fill();
            cr.restore();
        }
        3 => {
            // a crust standing like an arch, a rest of sauce and cheese on the inside
            cr.move_to(-70.0, 0.0);
            cr.arc(0.0, 0.0, 70.0, PI, TAU);
            cr.line_to(40.0, 0.0);
            cr.arc_negative(0.0, 0.0, 40.0, TAU, PI);
            cr.close_path();
            cr.set_source_rgb(0.82, 0.57, 0.28);
            cr.fill();
            cr.set_line_cap(LineCap::Butt);
            cr.set_line_width(9.0);
            cr.set_source_rgb(0.98, 0.84, 0.46);
            cr.new_sub_path();
            cr.arc(0.0, 0.0, 44.5, PI, TAU);
            cr.stroke();
            cr.set_line_width(5.0);
            cr.set_source_rgb(0.78, 0.24, 0.13);
            cr.new_sub_path();
            cr.arc(0.0, 0.0, 51.0, PI, TAU);
            cr.stroke();
            cr.set_source_rgba(0.55, 0.32, 0.12, 0.7);
            for i in 0..7 {
                let ang = PI + (i as f64 + 0.5) * PI / 7.0;
                cr.circle(ang.cos() * 62.0, ang.sin() * 62.0, 3.5 + (i % 3) as f64);
            }
            cr.fill();
            cr.set_source_rgba(1.0, 0.92, 0.70, 0.35);
            cr.set_line_width(4.0);
            cr.new_sub_path();
            cr.arc(0.0, 0.0, 66.0, PI + 0.5, PI + 1.3);
            cr.stroke();
        }
        4 => {
            poly(cr, &[(-44.0, -60.0), (8.0, -72.0), (44.0, -58.0), (-8.0, -46.0)], (1.0, 1.0, 1.0));
            poly(cr, &[(-44.0, -60.0), (-8.0, -46.0), (-8.0, 2.0), (-44.0, -10.0)], (0.87, 0.89, 0.95));
            poly(cr, &[(-8.0, -46.0), (44.0, -58.0), (44.0, -8.0), (-8.0, 2.0)], (0.73, 0.77, 0.88));
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.9);
            for (x, y) in [(-30.0, -36.0), (-18.0, -14.0), (10.0, -30.0), (30.0, -40.0), (22.0, -14.0), (-36.0, -20.0)] {
                cr.circle(x, y, 1.8);
            }
            cr.fill();
        }
        5 => {
            cr.set_source_rgb(0.42, 0.30, 0.14);
            cr.set_line_width(5.0);
            cr.move_to(3.0, -76.0);
            cr.line_to(11.0, -94.0);
            cr.stroke();
            cr.set_source_rgb(0.44, 0.62, 0.18);
            cr.circle(0.0, -40.0, 40.0);
            cr.fill();
            cr.set_source_rgb(0.62, 0.78, 0.28);
            cr.circle(-4.0, -44.0, 34.0);
            cr.fill();
            cr.set_source_rgba(1.0, 1.0, 0.9, 0.55);
            cr.ellipse(-16.0, -58.0, 10.0, 6.0);
            cr.fill();
        }
        6 => {
            cr.save();
            cr.translate(0.0, -40.0);
            cr.rotate(-0.16);
            cr.set_source_rgb(0.86, 0.64, 0.24);
            cr.ellipse(0.0, 0.0, 64.0, 35.0);
            cr.fill();
            cr.set_source_rgb(0.98, 0.83, 0.42);
            cr.ellipse(-3.0, -5.0, 56.0, 26.0);
            cr.fill();
            cr.set_source_rgba(0.80, 0.56, 0.18, 0.8);
            cr.set_line_width(3.0);
            for k in -2..=2 {
                let x = k as f64 * 19.0;
                cr.move_to(x - 4.0, -24.0 + (k as f64).abs() * 4.0);
                cr.quad_to(x + 6.0, -4.0, x - 4.0, 16.0 - (k as f64).abs() * 4.0);
            }
            cr.stroke();
            cr.restore();
        }
        _ => {
            let lumps = [(-30.0, -15.0, 18.0), (-4.0, -23.0, 24.0), (26.0, -15.0, 17.0), (45.0, -8.0, 9.0), (-49.0, -7.0, 8.0), (8.0, -46.0, 13.0)];
            cr.set_source_rgb(0.78, 0.57, 0.32);
            for (x, y, rr) in lumps {
                cr.circle(x, y, rr);
            }
            cr.fill();
            cr.set_source_rgb(0.93, 0.77, 0.52);
            for (x, y, rr) in lumps {
                cr.circle(x - rr * 0.14, y - rr * 0.2, rr * 0.72);
            }
            cr.fill();
            cr.set_source_rgba(0.60, 0.40, 0.20, 0.7);
            for (x, y, rr) in lumps {
                cr.circle(x + rr * 0.2, y + rr * 0.1, rr * 0.12);
            }
            cr.fill();
        }
    }
}

/// A piece of food with `stage` quarters left (4 = whole), its ground point at FOOD_BOX.2/3 * k.
fn paint_food(cr: &mut Canvas, kind: usize, k: f64, stage: usize, side: f64) {
    let hw = FOOD_HW[kind];
    let front = hw * [-0.45, 0.0, 0.5, 1.0][stage - 1];
    cr.translate(FOOD_BOX.2 * k, FOOD_BOX.3 * k);
    cr.scale(side * k, k);
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.34);
    cr.ellipse((front - hw) / 2.0 + 4.0, 2.0, (front + hw) / 2.0 * 1.08 + 5.0, 8.0);
    cr.fill();
    if stage < 4 {
        // crumbs where it was eaten, then the bite marks: a row of scallops along the eaten edge
        cr.set_rgb(FOOD_CRUMB[kind]);
        for i in 0..(4 - stage) * 4 {
            let f = i as f64;
            cr.circle(front + 12.0 + (f * 7.3).sin().abs() * (hw - front), 2.0 + (f * 3.1).sin() * 5.0, 2.2 + (f * 1.7).sin().abs() * 2.0);
        }
        cr.fill();
        cr.rectangle(-150.0, -140.0, 150.0 + front + 7.0, 170.0);
        cr.clip();
        cr.set_fill_rule(FillRule::EvenOdd);
        for set in 0..2 {
            cr.rectangle(-150.0, -140.0, 300.0, 170.0);
            for i in (set..7).step_by(2) {
                cr.circle(front + 6.0, -8.0 - 27.0 * i as f64, 16.0);
            }
            cr.clip();
        }
        cr.set_fill_rule(FillRule::Winding);
    }
    food_shape(cr, kind);
}

// --------------------------------------------------------------------------- the actor

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Ground,
    /// scrabbling and sinking into a fresh hole
    DigDown { t0: f64 },
    /// tunnelling: only a bump shows
    Under { t0: f64, dur: f64, from: P, to: P },
    PopUp { t0: f64 },
}

/// A nymph (sub-agent) with its own place and gait.
#[derive(Clone)]
struct Kid {
    x: f64,
    y: f64,
    face: f64,
    step: f64,
    moving: f64,
}

#[derive(Clone)]
pub struct Roach {
    b: Base,
    pose: Pose,
    mode: Mode,
    /// number of the world layout the mode belongs to (a resize makes a new floor)
    epoch: u64,
    helmet: usize,
    next_dig: f64,
    /// 0..1 how far down the hole it is
    sink: f64,
    /// where it went down (the nymphs wait there)
    hole: P,
    /// distance since the last ridge it left while tunnelling
    trail: f64,
    weapon: usize,
    /// seconds into the shooting stance (< 0 = not shooting)
    aim_t: f64,
    aim: P,
    aim_at: u64,
    fired: bool,
    recoil: f64,
    next_shot: f64,
    /// when it shoots just for fun
    next_whim: f64,
    /// a tool call wants a shot with this weapon
    pending: Option<usize>,
    /// seconds into a hit reaction (< 0 = none)
    hit: f64,
    hit_kind: usize,
    /// impacts older than this were already looked at
    hit_seen: f64,
    /// who hit it, and until when it bears a grudge
    grudge: Option<(u64, f64)>,
    /// off the ground (jump, bubble), in units of the radius
    lift: f64,
    /// the piece of food it is going to or eating
    meal: Option<u64>,
    eat_left: f64,
    meal_until: f64,
    next_meal: f64,
    at_table: bool,
    groom: f64,
    /// 0..1 arrived at the front and standing up to wave
    up: f64,
    kids: Vec<Kid>,
}

/// Standing still: let the walking values run out.
fn stand(b: &mut Base, dt: f64) {
    b.vx = 0.0;
    b.vy = 0.0;
    b.moving = lerp(b.moving, 0.0, (dt * 8.0).min(1.0));
    b.graze = lerp(b.graze, 0.0, (dt * 1.5).min(1.0));
    b.hop = (b.hop - dt * 2.5).max(0.0);
}

impl Roach {
    fn dig_gap(&self, env: &Env) -> f64 {
        match self.b.look() {
            Look::Idle => env.rng.uniform(70.0, 140.0),
            Look::Background => env.rng.uniform(24.0, 40.0),
            _ => env.rng.uniform(20.0, 40.0),
        }
    }

    fn meal_gap(&self, env: &Env) -> f64 {
        match self.b.look() {
            Look::Idle => env.rng.uniform(4.0, 9.0),
            Look::Background => env.rng.uniform(10.0, 18.0),
            _ => env.rng.uniform(12.0, 22.0),
        }
    }

    fn whim_gap(&self, env: &Env) -> f64 {
        if self.b.look() == Look::Background { env.rng.uniform(9.0, 17.0) } else { env.rng.uniform(7.0, 13.0) }
    }

    /// Never off the floor or behind the status strip, whoever pushes.
    fn keep_in(&mut self, env: &Env, rk: f64) {
        let b = &mut self.b;
        b.x = clamp(b.x, rk * 0.8, env.w - rk * 0.8);
        // the one on its back lies lower in its own space, so it needs more room at the bottom
        let room = if b.state() == State::Blocked { 1.35 } else { 1.02 };
        b.y = clamp(b.y, env.ground_top - rk * 0.2, env.h - env.strip_h - rk * room);
    }

    /// Forget every running gag (state change, resize, drawing a nymph).
    fn calm_down(&mut self) {
        self.aim_t = -1.0;
        self.pending = None;
        self.hit = -1.0;
        self.grudge = None;
        self.meal = None;
        self.at_table = false;
        self.recoil = 0.0;
        self.lift = 0.0;
    }

    /// The angle the weapon points at, in the roach's own (right-facing) frame.
    fn aim_angle(&self) -> f64 {
        let b = &self.b;
        let d = sign(self.aim.0 - b.x);
        let r = b.radius.max(1.0);
        let (dx, dy) = ((self.aim.0 - b.x) * d / r - WPN.0, (self.aim.1 - b.y) / r - WPN.1);
        let up = match self.weapon {
            CORK => 0.18,
            PIPETTE => 0.28,
            PEPPER | BUBBLE => 0.12,
            _ => 0.04,
        };
        clamp(dy.atan2(dx.max(0.3)) - up, -0.95, 0.40)
    }

    /// How far the spoon has swung (it flicks forward when the shot goes off).
    fn spoon_angle(&self) -> f64 {
        lerp(-2.45, -0.75, smooth((self.aim_t - AIM_FIRE + 0.05) / 0.10)) - 0.25 * smooth(self.aim_t / AIM_FIRE)
    }

    /// The shooting stance.
    fn aim_pose(&self, t: f64) -> Pose {
        let mut p = Pose::upright(t, self.b.seed);
        let ang = if self.weapon == SPOON { self.spoon_angle() } else { self.aim_angle() };
        let rot = |x: f64, y: f64| (WPN.0 + x * ang.cos() - y * ang.sin(), WPN.1 + x * ang.sin() + y * ang.cos());
        let charge = clamp(self.aim_t / AIM_FIRE, 0.0, 1.0);
        match self.weapon {
            BAND => {
                p.feet[0] = rot(0.22, 0.16);
                p.feet[3] = if self.fired { rot(0.0, 0.0) } else { rot(-0.12 - 0.24 * charge, 0.04) };
            }
            SPOON => {
                p.feet[0] = rot(0.10, 0.0);
                p.feet[3] = rot(-0.04, 0.0);
            }
            _ => {
                p.feet[0] = rot(0.16, 0.05);
                p.feet[3] = rot(-0.03, 0.06);
            }
        }
        let rc = self.recoil;
        p.c.0 -= 0.08 * rc;
        p.lean += 0.10 * rc;
        p.nod = clamp(ang * 0.35, -0.3, 0.2) - 0.12 * rc;
        if matches!(self.weapon, PEA | BUBBLE) && !self.fired {
            p.mouth = 0.4;
        }
        p.ant = [(-0.10 - 0.12 * rc, -0.60), (0.28 - 0.12 * rc, -0.62)];
        p
    }

    /// Start the stance: turn to another roach, or to a spot on the floor when nobody can be teased.
    fn start_shot(&mut self, env: &Env, world: &Cockroaches, kind: usize, prefer: Option<u64>, t: f64) {
        let s = env.s;
        let b = &self.b;
        let cands: Vec<&Mark> = world
            .marks
            .iter()
            .filter(|m| {
                let d = hypot(m.x - b.x, m.y - b.y);
                m.ok && m.index != b.index && d > 170.0 * s && d < 1600.0 * s
            })
            .collect();
        let pick = prefer
            .and_then(|p| cands.iter().find(|m| m.index == p).copied())
            .or_else(|| if cands.is_empty() { None } else { Some(cands[env.rng.index(cands.len())]) });
        match pick {
            Some(m) => {
                self.aim = (m.x, m.y);
                self.aim_at = m.index;
            }
            None => {
                let d = if b.x < env.w / 2.0 { 1.0 } else { -1.0 };
                self.aim = (clamp(b.x + d * env.rng.uniform(300.0, 620.0) * s, 90.0 * s, env.w - 90.0 * s), b.y + env.rng.uniform(-50.0, 50.0) * s);
                self.aim_at = 0;
            }
        }
        self.weapon = kind;
        self.aim_t = 0.0;
        self.fired = false;
        self.next_shot = t + env.rng.uniform(3.4, 4.6);
        self.next_whim = t + self.whim_gap(env);
    }

    /// Everything a roach does while it is above ground. Returns the pose to move towards.
    #[allow(clippy::too_many_arguments)]
    fn on_ground(&mut self, env: &mut Env, world: &mut Cockroaches, others: &[Other], dt: f64, t: f64, target: Option<P>, rk: f64, fresh: f64) -> Pose {
        let s = env.s;
        let st = self.b.state();
        let seed = self.b.seed;
        let idx = self.b.index;
        let active = matches!(st, State::Working | State::Idle) && !self.b.dying;
        self.sink = 0.0;
        if st != State::Waiting {
            self.up = 0.0;
        }
        if !active {
            // waiting, blocked, resting: no battle, no errands
            self.calm_down();
            // nobody pushes the one that needs you away from the front
            self.b.walker_move(env, if target.is_some() { &[] } else { others }, dt, t, target, [96.0, 24.0, 64.0, 0.0], [1.1, 0.6, 0.5, 0.1]);
            self.keep_in(env, rk);
            let there = target.is_some_and(|(tx, ty)| hypot(tx - self.b.x, ty - self.b.y) < 110.0 * s);
            self.up = lerp(self.up, if there { 1.0 } else { 0.0 }, (dt * 3.0).min(1.0));
            let b = &self.b;
            return match st {
                State::Waiting => Pose::scurry(b.step, b.moving, t, seed).mix(&Pose::wave(t, seed), smooth(self.up)),
                State::Blocked => Pose::back(t, seed, 0.32),
                State::Rest => Pose::nap(t, seed),
                _ => Pose::scurry(b.step, b.moving, t, seed),
            };
        }
        // did something land on it?
        if self.hit < 0.0 {
            let (x, y) = (self.b.x, self.b.y);
            if let Some(imp) = world.impacts.iter().find(|i| i.t0 >= fresh && i.from != idx && hypot(i.x - x, i.y - y) < rk * 1.15) {
                self.hit = 0.0;
                self.hit_kind = imp.kind;
                self.grudge = Some((imp.from, t + 7.0));
                self.aim_t = -1.0;
                self.pending = None;
                self.meal = None;
                self.at_table = false;
                self.next_shot = self.next_shot.min(t + HIT_DUR[imp.kind] + 0.4);
            }
        }
        if self.hit >= 0.0 {
            return self.react(env, world, dt, t, rk);
        }
        // the shooting stance
        if self.aim_t >= 0.0 {
            stand(&mut self.b, dt);
            if !self.fired {
                if let Some(m) = world.marks.iter().find(|m| m.index == self.aim_at && m.ok) {
                    self.aim = (m.x, m.y);
                }
            }
            let d = sign(self.aim.0 - self.b.x);
            self.b.face = lerp(self.b.face, d, (dt * 12.0).min(1.0));
            let before = self.aim_t;
            self.aim_t += dt;
            if before < AIM_FIRE && self.aim_t >= AIM_FIRE {
                self.fired = true;
                self.recoil = 1.0;
                let ang = self.aim_angle();
                let reach = [0.64, 0.0, 0.52, 0.64, 0.0, 0.46, 0.72][self.weapon];
                let loc = match self.weapon {
                    BAND => (WPN.0 + 0.22, WPN.1 - 0.28),
                    SPOON => (WPN.0 + 0.30, WPN.1 - 0.72),
                    _ => (WPN.0 + reach * ang.cos(), WPN.1 + reach * ang.sin()),
                };
                let muzzle = (self.b.x + d * loc.0 * rk, self.b.y + loc.1 * rk);
                world.fire(env, self.weapon, idx, self.aim_at, muzzle, self.aim, rk);
            }
            if self.aim_t >= AIM_END {
                self.aim_t = -1.0;
            }
            return self.aim_pose(t);
        }
        // take a shot: a tool call, a return shot, or just for fun (idle ones only shoot back)
        if target.is_none() && t >= self.next_shot {
            let grudge = self.grudge.filter(|g| t <= g.1).map(|g| g.0);
            let kind = if let Some(k) = self.pending.take() {
                Some(k)
            } else if st == State::Idle {
                grudge.map(|_| env.rng.index(WEAPONS))
            } else if t >= self.next_whim {
                Some(env.rng.index(WEAPONS))
            } else {
                None
            };
            if let Some(k) = kind {
                self.start_shot(env, world, k, grudge, t);
                self.grudge = None;
                stand(&mut self.b, dt);
                return self.aim_pose(t);
            }
        }
        self.pending = None;
        // dig a tunnel
        if t >= self.next_dig && !self.at_table {
            self.meal = None;
            self.mode = Mode::DigDown { t0: t };
            self.hole = (self.b.x, self.b.y + rk * 0.95);
            world.mound(self.hole.0, self.hole.1, rk, t);
            stand(&mut self.b, dt);
            return Pose::dig(t, seed);
        }
        // something to eat: the nearest piece nobody else is at
        if self.meal.is_none() && t >= self.next_meal {
            let (x, y) = (self.b.x, self.b.y);
            let near = world
                .foods
                .iter()
                .filter(|f| f.landed && f.amount > 0.04 && (f.claim.0 == idx || t - f.claim.1 > 0.6))
                .min_by(|a, b| hypot(a.x - x, (a.y - y) * 1.6).total_cmp(&hypot(b.x - x, (b.y - y) * 1.6)));
            match near {
                Some(f) => {
                    self.meal = Some(f.id);
                    self.meal_until = t + 14.0;
                    self.eat_left = match self.b.look() {
                        Look::Idle => env.rng.uniform(5.0, 9.0),
                        Look::Background => env.rng.uniform(3.0, 5.0),
                        _ => env.rng.uniform(2.5, 4.0),
                    };
                }
                None => self.next_meal = t + 4.0,
            }
        }
        if let Some(id) = self.meal {
            let found = world.foods.iter_mut().find(|f| f.id == id && f.amount > 0.0).map(|f| {
                f.claim = (idx, t);
                (f.x, f.y, f.side, f.front(), f.kind)
            });
            let Some((fx, fy, side, front, kind)) = found else {
                self.meal = None;
                self.at_table = false;
                self.next_meal = t + self.meal_gap(env);
                return Pose::scurry(self.b.step, self.b.moving, t, seed);
            };
            // stand at the eaten side, a hair in front of the piece
            let rf = self.b.radius * depth_k(env, fy - rk * 0.95);
            let spot = (fx + side * (front + rf * 0.60), fy + 4.0 * s - rf * 0.95);
            let (dx, dy) = (spot.0 - self.b.x, spot.1 - self.b.y);
            let dist = hypot(dx, dy);
            if dist > if self.at_table { 34.0 * s } else { 10.0 * s } {
                self.at_table = false;
                let pace = if st == State::Idle { 110.0 } else { 170.0 };
                let sp = (pace * s).min(dist * 3.0 + 12.0 * s);
                let b = &mut self.b;
                b.vx = dx / dist * sp;
                b.vy = dy / dist * sp;
                b.x += b.vx * dt;
                b.y += b.vy * dt;
                b.moving = lerp(b.moving, clamp(sp / (25.0 * s), 0.0, 1.0), (dt * 8.0).min(1.0));
                b.step += dt * sp / (rk * 0.35).max(1.0);
                b.hop = (b.hop - dt * 2.5).max(0.0);
                if dx.abs() > 8.0 * s {
                    b.face = lerp(b.face, sign(dx), (dt * 6.0).min(1.0));
                }
                if t > self.meal_until {
                    self.meal = None;
                    self.next_meal = t + self.meal_gap(env);
                }
                return Pose::scurry(self.b.step, self.b.moving, t, seed);
            }
            self.at_table = true;
            stand(&mut self.b, dt);
            self.b.face = lerp(self.b.face, -side, (dt * 8.0).min(1.0));
            self.eat_left -= dt;
            let mut gone = false;
            if let Some(f) = world.foods.iter_mut().find(|f| f.id == id) {
                f.amount -= dt / FOOD_SECONDS;
                gone = f.amount <= 0.0;
            }
            if env.rng.chance(dt * 9.0) {
                let (mx, my) = (self.b.x - side * rk * 0.66, self.b.y + rk * 0.72);
                world.bit(Bit {
                    x: mx + env.rng.uniform(-0.08, 0.08) * rk,
                    y: my,
                    vx: env.rng.uniform(-60.0, 60.0) * s,
                    vy: -env.rng.uniform(40.0, 150.0) * s,
                    age: 0.0,
                    life: env.rng.uniform(0.8, 1.6),
                    c: FOOD_CRUMB[kind],
                    size: env.rng.uniform(1.6, 3.2) * rk / 116.0,
                    grav: 1.0,
                    kind: 0,
                    floor: self.b.y + rk * (0.95 + env.rng.uniform(0.0, 0.08)),
                    lying: false,
                });
            }
            if self.eat_left <= 0.0 || gone {
                self.meal = None;
                self.at_table = false;
                self.next_meal = t + self.meal_gap(env);
            }
            return Pose::eat(t, seed);
        }
        // nothing special: scurry about; idle ones stop to groom their antennae
        let pace = if self.b.look() == Look::Background { 64.0 } else { 98.0 };
        // roaches standing still (eating, aiming) do not step aside, so keep only a small distance
        let r0 = self.b.radius;
        let near: Vec<Other> = others.iter().map(|o| Other { radius: (r0 + o.radius) * 0.5 - r0, ..*o }).collect();
        self.b.walker_move(env, &near, dt, t, None, [pace, 24.0, 64.0, 0.0], [1.1, 0.6, 0.5, 0.1]);
        self.keep_in(env, rk);
        let grooming = st == State::Idle && self.b.graze > 0.6;
        self.groom = lerp(self.groom, if grooming { 1.0 } else { 0.0 }, (dt * 3.0).min(1.0));
        let b = &self.b;
        Pose::scurry(b.step, b.moving, t, seed).mix(&Pose::groom(t, seed), smooth(self.groom))
    }

    /// The reaction to a hit; which one depends on the weapon.
    fn react(&mut self, env: &mut Env, world: &mut Cockroaches, dt: f64, t: f64, rk: f64) -> Pose {
        let s = env.s;
        let seed = self.b.seed;
        stand(&mut self.b, dt);
        let dur = HIT_DUR[self.hit_kind];
        let before = self.hit;
        self.hit += dt;
        let u = clamp(self.hit / dur, 0.0, 1.0);
        let crossed = |at: f64| before < dur * at && before + dt >= dur * at;
        let (x, y) = (self.b.x, self.b.y);
        let d = sign(self.b.face);
        let wt = smooth(u * 10.0) * smooth((1.0 - u) * 6.0);
        let base = Pose::scurry(0.0, 0.0, t, seed);
        let pose = match self.hit_kind {
            PEA => {
                // sees stars: sways on its hind legs
                let sw = (t * 7.0 + seed).sin();
                let mut p = Pose::upright(t, seed);
                p.c.0 += 0.08 * sw;
                p.lean += 0.14 * sw;
                p.nod = 0.15 * (t * 5.0).cos();
                p.feet[0] = (0.30 + 0.08 * sw, 0.24);
                p.feet[3] = (0.36 + 0.08 * sw, 0.16);
                p.ant = [(-0.12 + 0.14 * sw, -0.50), (0.40 + 0.14 * sw, -0.44)];
                base.mix(&p, wt)
            }
            BAND => {
                // a startled leap, all legs out
                self.lift = (PI * u).sin() * 0.85;
                let mut p = base;
                for (k, f) in p.feet.iter_mut().enumerate() {
                    *f = (f.0 * 1.3, 0.84 + 0.04 * (t * 30.0 + k as f64).sin());
                }
                p.ant = [(0.10, -0.72), (0.42, -0.70)];
                p.mouth = 1.0;
                p
            }
            CORK => {
                // knocked onto its back, legs going, then rolls over again
                self.lift = (PI * clamp(u * 6.0, 0.0, 1.0)).sin() * 0.3;
                let down = smooth(u * 12.0) * smooth((1.0 - u) * 7.0);
                base.mix(&Pose::back(t, seed, 1.0), down)
            }
            PIPETTE => {
                // soaked: shakes itself like a dog
                let mut p = base;
                let sh = wt * (1.0 - u);
                p.c.0 += 0.05 * sh * (t * 46.0).sin();
                p.lean = 0.08 + 0.16 * sh * (t * 31.0).sin();
                p.ant = [(0.40 + 0.2 * sh * (t * 40.0).sin(), -0.40), (0.60 + 0.2 * sh * (t * 43.0).cos(), -0.24)];
                if u < 0.75 && env.rng.chance(dt * 26.0) {
                    let ang = env.rng.uniform(-PI, 0.0);
                    let sp = env.rng.uniform(120.0, 300.0) * s;
                    world.bit(Bit {
                        x: x + env.rng.uniform(-0.5, 0.5) * rk,
                        y: y + rk * 0.4,
                        vx: ang.cos() * sp,
                        vy: ang.sin() * sp,
                        age: 0.0,
                        life: env.rng.uniform(0.4, 0.8),
                        c: (0.60, 0.82, 1.0),
                        size: env.rng.uniform(1.8, 3.4) * rk / 116.0,
                        grav: 1.0,
                        kind: 0,
                        floor: y + rk,
                        lying: false,
                    });
                }
                p
            }
            SPOON => {
                // cream all over the face: sits up and wipes it off
                let mut p = Pose::upright(t, seed);
                let wipe = (t * 12.0 + seed).sin();
                p.lean = 1.0;
                p.feet[0] = (0.38 + 0.07 * wipe, -0.10 - 0.06 * wipe);
                p.feet[3] = (0.30 - 0.07 * wipe, -0.02 + 0.06 * wipe);
                p.ant = [(-0.20, -0.30), (0.10, -0.52)];
                base.mix(&p, wt)
            }
            PEPPER => {
                // "ah ... ah ..." head back, then the sneeze throws it forward
                let mut p = Pose::upright(t, seed);
                if u < 0.5 {
                    let k = smooth(u / 0.5);
                    p.nod = -0.55 * k;
                    p.lean += 0.14 * k;
                    p.mouth = k;
                    p.ant = [(-0.25 * k, -0.60), (0.30 - 0.2 * k, -0.62)];
                } else {
                    let k = 1.0 - smooth((u - 0.5) / 0.3);
                    p.nod = 0.75 * k;
                    p.lean -= 0.45 * k;
                    p.c.0 += 0.12 * k;
                    p.mouth = k;
                    p.ant = [(0.30 * k, -0.55), (0.50 * k + 0.2, -0.45)];
                    self.lift = (PI * clamp((u - 0.5) / 0.2, 0.0, 1.0)).sin() * 0.18;
                }
                if crossed(0.5) {
                    for _ in 0..7 {
                        world.bit(Bit {
                            x: x + d * rk * 0.75,
                            y: y - rk * 0.05,
                            vx: d * env.rng.uniform(120.0, 320.0) * s,
                            vy: env.rng.uniform(-70.0, 70.0) * s,
                            age: 0.0,
                            life: env.rng.uniform(0.4, 0.8),
                            c: (0.92, 0.90, 0.84),
                            size: env.rng.uniform(5.0, 10.0) * rk / 116.0,
                            grav: 0.0,
                            kind: 1,
                            floor: 1e9,
                            lying: false,
                        });
                    }
                }
                base.mix(&p, wt)
            }
            _ => {
                // carried off in a soap bubble until it pops
                self.lift = if u < 0.72 { smooth(u / 0.4) * 0.95 + 0.04 * (t * 4.0).sin() } else { 0.95 * (1.0 - ((u - 0.72) / 0.16).min(1.0).powi(2)) };
                let mut p = base;
                if u < 0.86 {
                    for (k, f) in p.feet.iter_mut().enumerate() {
                        *f = (f.0 * 0.9 + 0.06 * (t * 12.0 + k as f64 * 2.1).sin(), 0.90 + 0.05 * (t * 12.0 + k as f64 * 2.1).cos());
                    }
                    p.ant = [(0.20, -0.66), (0.50, -0.60)];
                    p.mouth = 0.8;
                }
                if crossed(0.72) {
                    world.burst(env, 10, (x, y - rk * 0.6), (80.0, 260.0), (0.86, 0.97, 1.0), 2.4 * rk / 116.0, 0.2, 0, (0.25, 0.5), 1e9);
                }
                p
            }
        };
        if self.hit >= dur {
            self.hit = -1.0;
            self.lift = 0.0;
        }
        pose
    }

    /// The nymphs: behind the parent, or beside the hole while it is under ground.
    fn update_kids(&mut self, env: &Env, dt: f64, rk: f64) {
        let s = env.s;
        let n = self.b.info.subagents.len();
        self.kids.truncate(n);
        while self.kids.len() < n {
            self.kids.push(Kid { x: self.b.x, y: self.b.y, face: self.b.face, step: 0.0, moving: 0.0 });
        }
        let kr = rk * Self::COMPANION_SCALE;
        let back = if self.b.face >= 0.0 { -1.0 } else { 1.0 };
        let above = self.mode == Mode::Ground;
        let (px, py) = (self.b.x, self.b.y);
        for (i, k) in self.kids.iter_mut().enumerate() {
            let fi = i as f64;
            let goal = if above {
                (px + back * rk * (1.15 + fi * 0.62), py + rk * 0.95 - kr * 0.95 + rk * 0.10 * (i % 2) as f64 + 3.0 * s)
            } else {
                let side = if i % 2 == 0 { 1.0 } else { -1.0 };
                (self.hole.0 + side * (rk * 0.95 + (i / 2) as f64 * kr * 2.1), self.hole.1 - kr * 0.95 + kr * 0.3)
            };
            let (dx, dy) = (goal.0 - k.x, goal.1 - k.y);
            let d = hypot(dx, dy);
            if d > 4.0 * s {
                let sp = (240.0 * s).min(d * 4.0 + 10.0 * s);
                k.x += dx / d * sp * dt;
                k.y += dy / d * sp * dt;
                k.moving = lerp(k.moving, clamp(sp / (45.0 * s), 0.0, 1.0), (dt * 8.0).min(1.0));
                k.step += dt * sp / (kr * 0.35).max(1.0);
                if dx.abs() > 3.0 * s {
                    k.face = lerp(k.face, sign(dx), (dt * 8.0).min(1.0));
                }
            } else {
                k.moving = lerp(k.moving, 0.0, (dt * 6.0).min(1.0));
                if !above {
                    k.face = lerp(k.face, sign(px - k.x), (dt * 4.0).min(1.0)); // watch the bump
                }
            }
        }
    }

    /// Under ground: a travelling heap of earth, the helmet showing like a fin, the antenna tips out.
    fn draw_bump(&self, cr: &mut Canvas, t: f64, a: f64) {
        let b = &self.b;
        let (r, c) = (b.radius, b.color);
        let (x, foot) = (b.x, b.y + r * 0.95);
        cr.save();
        cr.translate(x, foot);
        cr.scale(1.0, 0.3);
        cr.glow(0.0, 0.0, r * 1.4, c, 0.42 * a, 0.0);
        cr.restore();
        let wob = (t * 15.0 + b.seed).sin();
        let (hw, hh) = (r * 0.66, r * (0.30 + 0.035 * wob));
        let d = sign(b.face);
        // antenna tips and helmet first, the heap covers their lower part
        cr.set_rgba(LEGC, a);
        cr.set_line_width(0.028 * r);
        cr.set_line_cap(LineCap::Round);
        for k in 0..2 {
            let kf = k as f64;
            let bx = x + d * r * (0.10 + 0.20 * kf);
            let tip = (bx + d * r * (0.16 + 0.07 * (t * 9.0 + kf * 2.0).sin()), foot - hh - r * (0.30 + 0.05 * (t * 11.0 + kf).cos()));
            cr.move_to(bx, foot - hh * 0.5);
            cr.quad_to(bx, tip.1, tip.0, tip.1);
        }
        cr.stroke();
        cr.save();
        cr.translate(x - d * r * 0.06, foot - hh * 0.62 + r * 0.02 * wob);
        helmet(cr, r * 0.92, self.helmet, c, a);
        cr.restore();
        cr.move_to(x - hw, foot + r * 0.05);
        cr.curve_to(x - hw * 0.62, foot - hh * 1.25, x + hw * 0.62, foot - hh * 1.25, x + hw, foot + r * 0.05);
        cr.close_path();
        cr.set_rgba(EARTH, a);
        cr.fill();
        cr.move_to(x - hw * 0.62, foot - hh * 0.42);
        cr.curve_to(x - hw * 0.3, foot - hh * 1.02, x + hw * 0.3, foot - hh * 1.02, x + hw * 0.55, foot - hh * 0.52);
        cr.quad_to(x, foot - hh * 0.70, x - hw * 0.62, foot - hh * 0.42);
        cr.set_rgba(EARTH_L, 0.9 * a);
        cr.fill();
        cr.set_rgba(EARTH_D, 0.7 * a);
        cr.set_line_width(0.022 * r);
        for k in [-1.0, 0.2, 1.0] {
            cr.move_to(x + k * hw * 0.42, foot - hh * 0.30);
            cr.line_to(x + k * hw * 0.52 + r * 0.03, foot - hh * 0.05);
        }
        cr.stroke();
    }

    /// Resting: asleep in the drawer of a matchbox, under a scrap of cloth, the head on a cotton ball.
    fn draw_asleep(&self, cr: &mut Canvas, s: f64, t: f64, r: f64, a: f64) {
        let b = &self.b;
        if a < 0.01 {
            return;
        }
        let r = r * 1.35;
        let (x, y, c) = (b.x, b.y - r * 0.20, b.color);
        // soft light and shadow under the bed
        cr.save();
        cr.translate(x, y + r * 1.02);
        cr.scale(1.0, 0.28);
        cr.glow(0.0, 0.0, r * 2.0, c, 0.30 * a, 0.0);
        cr.restore();
        cr.set_source_rgba(0.0, 0.0, 0.0, 0.40 * a);
        cr.ellipse(x + r * 0.04, y + r * 1.03, r * 1.22, r * 0.11);
        cr.fill();
        // back wall and inside of the drawer
        cr.set_source_rgba(0.50, 0.39, 0.25, a);
        cr.move_to(x - r * 1.06, y + r * 1.0);
        cr.line_to(x - r * 0.98, y + r * 0.38);
        cr.line_to(x + r * 1.10, y + r * 0.38);
        cr.line_to(x + r * 1.02, y + r * 1.0);
        cr.close_path();
        cr.fill();
        cr.set_source_rgba(0.27, 0.19, 0.12, a);
        cr.rectangle(x - r * 0.96, y + r * 0.45, r * 2.0, r * 0.5);
        cr.fill();
        // the cotton ball for a pillow
        cr.set_source_rgba(0.96, 0.95, 0.90, a);
        for (px, py, pr) in [(0.42, 0.72, 0.20), (0.64, 0.70, 0.22), (0.84, 0.74, 0.17), (0.54, 0.60, 0.15), (0.74, 0.60, 0.15)] {
            cr.circle(x + px * r, y + py * r, pr * r);
        }
        cr.fill();
        // the sleeper
        cr.save();
        cr.translate(x - r * 0.06, y - r * 0.02);
        let lk = Looks { c, a, glow: a, eyes: Eyes::Shut, look: (0.0, 0.0), helmet: Some(self.helmet), pale: false, frown: false, cream: 0.0, t };
        figure(cr, &self.pose, r, &lk);
        cr.restore();
        // the blanket over the back, hanging over the front wall
        let br = (t * 1.3 + b.seed).sin() * r * 0.012;
        let cloth = |cr: &mut Canvas| {
            cr.move_to(x - r * 0.92, y + r * 0.56);
            cr.curve_to(x - r * 0.80, y + r * 0.26 + br, x - r * 0.30, y + r * 0.20 + br, x + r * 0.04, y + r * 0.34 + br);
            cr.line_to(x + r * 0.12, y + r * 0.80);
            cr.quad_to(x - r * 0.40, y + r * 0.88, x - r * 0.90, y + r * 0.80);
            cr.close_path();
        };
        cloth(cr);
        cr.set_source_rgba(0.90, 0.86, 0.76, a);
        cr.fill();
        cr.set_source_rgba(0.50, 0.56, 0.68, 0.85 * a);
        cr.set_line_width(r * 0.035);
        for k in 0..5 {
            let f = k as f64 / 4.0;
            let sx = x - r * 0.80 + f * r * 0.80;
            cr.move_to(sx, y + r * (0.50 - 0.22 * (PI * (0.15 + f * 0.75)).sin()) + br);
            cr.line_to(sx + r * 0.03, y + r * 0.80);
        }
        cr.stroke();
        // front wall of the drawer
        cr.set_source_rgba(0.66, 0.53, 0.35, a);
        cr.rectangle(x - r * 1.06, y + r * 0.74, r * 2.08, r * 0.28);
        cr.fill();
        cr.set_source_rgba(0.80, 0.68, 0.48, a);
        cr.rectangle(x - r * 1.06, y + r * 0.74, r * 2.08, r * 0.045);
        cr.fill();
        // a corner of the blanket over the edge
        cr.move_to(x - r * 0.74, y + r * 0.74);
        cr.line_to(x - r * 0.10, y + r * 0.74);
        cr.quad_to(x - r * 0.30, y + r * 0.96, x - r * 0.64, y + r * 0.90);
        cr.close_path();
        cr.set_source_rgba(0.90, 0.86, 0.76, a);
        cr.fill();
        // z z z
        for i in 0..3 {
            let f = (t * 0.3 + i as f64 / 3.0) % 1.0;
            let lay = text::layout("z", (14.0 + 14.0 * f) * s, weight::BOLD, None);
            let zx = x + r * (0.55 + 0.45 * f) + (f * 6.0).sin() * 4.0 * s;
            let zy = y + r * 0.10 - f * r * 1.3;
            text::show(cr, &lay, zx, zy, (1.0, 0.97, 0.88), 0.8 * (PI * f).sin() * a, 0.4);
        }
    }
}

impl Roach {
    /// Nymphs: small, pale and wingless, with a working-colour rim light and glow.
    fn draw_kids(&self, cr: &mut Canvas, t: f64) {
        let b = &self.b;
        if b.alpha < 0.05 || self.kids.is_empty() {
            return;
        }
        let kr = b.radius * <Self as ActorT<Cockroaches>>::COMPANION_SCALE;
        let waiting = self.mode != Mode::Ground;
        for (i, k) in self.kids.iter().enumerate() {
            let seed = b.seed + i as f64 * 2.3;
            let foot = k.y + kr * 0.95;
            cr.set_rgba(WORKING_COLOR, 0.20 * b.alpha);
            cr.ellipse(k.x, foot, kr * 1.15, kr * 0.26);
            cr.fill();
            cr.set_source_rgba(0.0, 0.0, 0.0, 0.3 * b.alpha);
            cr.ellipse(k.x, foot, kr * 0.6, kr * 0.09);
            cr.fill();
            let mut p = Pose::scurry(k.step, k.moving, t * 1.6, seed);
            if waiting {
                // sitting up a little, looking for the parent
                let up = 1.0 - k.moving;
                p.lean += 0.30 * up;
                p.c.1 -= 0.06 * up;
            }
            let lk = Looks { c: WORKING_COLOR, a: b.alpha, glow: b.alpha, eyes: Eyes::Open, look: (0.3, 0.0), helmet: None, pale: true, frown: false, cream: 0.0, t };
            cr.save();
            cr.translate(k.x, k.y);
            cr.scale(sign(k.face) * k.face.abs().max(0.35), 1.0);
            figure(cr, &p, kr, &lk);
            cr.restore();
        }
    }
}

impl ActorT<Cockroaches> for Roach {
    const BASE_RADIUS: f64 = 116.0;
    const WALKER: bool = true;
    const COMPANION_SCALE: f64 = 0.42;

    fn new(base: Base, env: &Env, world: &mut Cockroaches) -> Self {
        let t = env.t;
        let idle = base.look() == Look::Idle;
        let pose = if base.state() == State::Rest { Pose::nap(0.0, base.seed) } else { Pose::scurry(0.0, 0.0, 0.0, base.seed) };
        Roach {
            pose,
            mode: Mode::Ground,
            epoch: world.epoch,
            helmet: (base.index % 3) as usize,
            next_dig: t + if idle { env.rng.uniform(50.0, 120.0) } else { env.rng.uniform(9.0, 30.0) },
            sink: 0.0,
            hole: (base.x, base.y),
            trail: 0.0,
            weapon: PEA,
            aim_t: -1.0,
            aim: (0.0, 0.0),
            aim_at: 0,
            fired: false,
            recoil: 0.0,
            next_shot: t + env.rng.uniform(1.5, 3.5),
            next_whim: t + env.rng.uniform(4.0, 10.0),
            pending: None,
            hit: -1.0,
            hit_kind: PEA,
            hit_seen: t,
            grudge: None,
            lift: 0.0,
            meal: None,
            eat_left: 0.0,
            meal_until: 0.0,
            next_meal: t + if idle { env.rng.uniform(1.0, 4.0) } else { env.rng.uniform(5.0, 16.0) },
            at_table: false,
            groom: 0.0,
            up: 0.0,
            kids: Vec::new(),
            b: base,
        }
    }

    fn base(&self) -> &Base {
        &self.b
    }

    fn base_mut(&mut self) -> &mut Base {
        &mut self.b
    }

    fn companion_reset(&mut self) {
        self.calm_down();
        self.mode = Mode::Ground;
        self.sink = 0.0;
        self.groom = 0.0;
        self.kids.clear();
        self.pose = Pose::scurry(self.b.step, self.b.moving, 0.0, self.b.seed);
    }

    fn companion_spots(&self, _env: &Env, _t: f64) -> Vec<(f64, f64, f64)> {
        self.kids.iter().map(|k| (k.x, k.y, if k.face >= 0.0 { -PI / 2.0 } else { PI / 2.0 })).collect()
    }

    /// The nymphs are drawn in depth order with their parent (see `draw_world`), not on top of everything.
    fn draw_companions(&self, _cr: &mut Canvas, _env: &Env, _world: &Cockroaches, _t: f64) {}

    fn mv(&mut self, env: &mut Env, world: &mut Cockroaches, others: &[Other], dt: f64, t: f64, target: Option<(f64, f64)>) {
        let s = env.s;
        let st = self.b.state();
        let seed = self.b.seed;
        if self.epoch != world.epoch {
            // the screen changed: the floor, its holes and the food are new
            self.epoch = world.epoch;
            self.calm_down();
            self.mode = Mode::Ground;
            self.sink = 0.0;
            self.kids.clear();
            self.b.x = clamp(self.b.x, 130.0 * s, env.w - 130.0 * s);
            self.b.y = clamp(self.b.y, env.ground_top + 40.0 * s, env.h - env.strip_h - 110.0 * s);
            self.b.vx = 0.0;
            self.b.vy = 0.0;
            self.next_dig = t + env.rng.uniform(10.0, 30.0);
        }
        let rk = self.b.radius * self.b.depth_scale(env);
        let active = matches!(st, State::Working | State::Idle) && !self.b.dying;
        self.recoil = (self.recoil - dt * 3.5).max(0.0);
        self.lift = 0.0;
        let fresh = self.hit_seen;
        self.hit_seen = t;
        if !active {
            // called away while digging: come up right here
            match self.mode {
                Mode::DigDown { .. } => self.mode = Mode::Ground,
                Mode::Under { .. } => {
                    self.mode = Mode::PopUp { t0: t };
                    world.mound(self.b.x, self.b.y + rk * 0.95, rk, t);
                }
                _ => {}
            }
        }
        let goal = match self.mode {
            Mode::Ground => self.on_ground(env, world, others, dt, t, target, rk, fresh),
            Mode::DigDown { t0 } => {
                stand(&mut self.b, dt);
                let u = t - t0;
                self.sink = smooth((u - 0.9) / (DIG_DOWN - 0.9));
                let d = sign(self.b.face);
                if env.rng.chance(dt * 26.0) {
                    // dirt flies out behind
                    world.bit(Bit {
                        x: self.b.x + d * rk * env.rng.uniform(-0.1, 0.4),
                        y: self.b.y + rk * 0.85,
                        vx: -d * env.rng.uniform(60.0, 260.0) * s,
                        vy: -env.rng.uniform(140.0, 340.0) * s,
                        age: 0.0,
                        life: env.rng.uniform(1.2, 2.4),
                        c: if env.rng.chance(0.5) { EARTH_L } else { EARTH },
                        size: env.rng.uniform(2.2, 5.0) * rk / 116.0,
                        grav: 1.0,
                        kind: 0,
                        floor: self.b.y + rk * (0.95 + env.rng.uniform(-0.04, 0.12)),
                        lying: false,
                    });
                }
                if u >= DIG_DOWN {
                    // tunnel to a spot some way off
                    let dir = if self.b.x < env.w * 0.3 {
                        1.0
                    } else if self.b.x > env.w * 0.7 {
                        -1.0
                    } else if env.rng.chance(0.5) {
                        1.0
                    } else {
                        -1.0
                    };
                    let tx = clamp(self.b.x + dir * env.rng.uniform(330.0, 660.0) * s, 170.0 * s, env.w - 170.0 * s);
                    let ty = env.rng.uniform(env.ground_top + 50.0 * s, env.h - env.strip_h - 140.0 * s);
                    let dist = hypot(tx - self.b.x, ty - self.b.y);
                    self.mode = Mode::Under { t0: t, dur: (dist / (200.0 * s)).max(1.3), from: (self.b.x, self.b.y), to: (tx, ty) };
                    self.sink = 1.0;
                    self.trail = 0.0;
                }
                Pose::dig(t, seed)
            }
            Mode::Under { t0, dur, from, to } => {
                let f = clamp((t - t0) / dur, 0.0, 1.0);
                let (nx, ny) = (lerp(from.0, to.0, f), lerp(from.1, to.1, f));
                self.trail += hypot(nx - self.b.x, ny - self.b.y);
                let b = &mut self.b;
                b.vx = (to.0 - from.0) / dur;
                b.vy = (to.1 - from.1) / dur;
                b.face = lerp(b.face, sign(b.vx), (dt * 6.0).min(1.0));
                b.x = nx;
                b.y = ny;
                b.moving = 1.0;
                b.step += dt * 9.0;
                b.hop = 0.0;
                self.sink = 1.0;
                let foot = ny + rk * 0.95;
                if self.trail > 30.0 * s {
                    self.trail = 0.0;
                    world.ridge(nx, foot, rk, t);
                }
                if env.rng.chance(dt * 12.0) {
                    // crumbs of earth jump where the bump passes
                    world.bit(Bit {
                        x: nx + env.rng.uniform(-0.5, 0.5) * rk,
                        y: foot - rk * 0.2,
                        vx: env.rng.uniform(-70.0, 70.0) * s,
                        vy: -env.rng.uniform(90.0, 220.0) * s,
                        age: 0.0,
                        life: env.rng.uniform(0.7, 1.4),
                        c: EARTH_L,
                        size: env.rng.uniform(1.8, 3.6) * rk / 116.0,
                        grav: 1.0,
                        kind: 0,
                        floor: foot + env.rng.uniform(0.0, 0.1) * rk,
                        lying: false,
                    });
                }
                if f >= 1.0 {
                    self.mode = Mode::PopUp { t0: t };
                    world.mound(nx, foot, rk, t);
                    world.burst(env, 20, (nx, foot - rk * 0.1), (160.0, 430.0), EARTH_L, 3.6 * rk / 116.0, 1.0, 0, (1.0, 2.2), foot + rk * 0.08);
                }
                Pose::dig(t, seed)
            }
            Mode::PopUp { t0 } => {
                stand(&mut self.b, dt);
                let u = (t - t0) / POP_UP;
                self.sink = 1.0 - smooth(u * 2.2);
                self.lift = (PI * clamp(u * 1.5 - 0.1, 0.0, 1.0)).sin() * 0.38;
                if u >= 1.0 {
                    self.mode = Mode::Ground;
                    self.sink = 0.0;
                    self.next_dig = t + self.dig_gap(env);
                    self.next_shot = self.next_shot.max(t + 0.8);
                }
                Pose::cheer(t, seed)
            }
        };
        self.pose = self.pose.mix(&goal, (dt * 11.0).min(1.0));
        self.update_kids(env, dt, rk);
    }

    fn draw(&self, cr: &mut Canvas, env: &Env, _world: &Cockroaches, t: f64) {
        let s = env.s;
        let b = &self.b;
        let (r, c, st) = (b.radius, b.color, b.state());
        if st == State::Rest {
            self.draw_asleep(cr, s, t, r, (b.alpha / 0.8).min(1.0));
            return;
        }
        let a = b.alpha;
        if a < 0.01 {
            return;
        }
        if matches!(self.mode, Mode::Under { .. }) {
            self.draw_bump(cr, t, a);
            return;
        }
        // blocked: the colour flickers hard, the body only a little (so the legs do not show through)
        let flick = if st == State::Blocked { b.flicker } else { 1.0 };
        let body_a = a * lerp(1.0, flick, 0.25);
        let foot = b.y + r * 0.95;
        let sink = self.sink;
        let lift = self.lift + if b.hop > 0.0 { (PI * b.hop).sin() * 0.24 } else { 0.0 };
        // glow on the ground + shadow
        cr.save();
        cr.translate(b.x, foot);
        cr.scale(1.0, 0.3);
        let strength = match st {
            State::Idle => 0.40,
            State::Blocked => 0.80,
            _ => 0.55,
        };
        cr.glow(0.0, 0.0, r * 1.75, c, strength * a * flick * (1.0 - 0.5 * sink), 0.0);
        cr.restore();
        let sh = (1.0 - 0.45 * lift.min(1.0)) * (1.0 - sink);
        cr.set_source_rgba(0.0, 0.0, 0.0, 0.42 * a * sh);
        cr.ellipse(b.x, foot, r * 0.66 * sh, r * 0.09 * sh);
        cr.fill();
        if st == State::Waiting {
            cr.pulse_rings(b.x, b.y - r * 0.1, r, c, a, t, s);
        }
        let d = sign(b.face);
        let sx = d * b.face.abs().max(0.6);
        let shrink = 1.0 - 0.38 * sink;
        let vis = 1.0 - smooth((sink - 0.72) / 0.28);
        let fa = body_a * vis;
        if fa < 0.01 {
            return;
        }
        let hit = self.hit >= 0.0;
        let hu = if hit { clamp(self.hit / HIT_DUR[self.hit_kind], 0.0, 1.0) } else { 0.0 };
        let blink = (t * 0.8 + b.seed).fract() < 0.045;
        let aiming = self.aim_t >= 0.0;
        let eyes = if hit {
            match self.hit_kind {
                PEA | CORK => Eyes::Dizzy,
                BAND | BUBBLE => Eyes::Wide,
                PIPETTE => Eyes::Squeeze,
                SPOON => if hu < 0.6 { Eyes::Squeeze } else { Eyes::Open },
                _ => if hu > 0.42 && hu < 0.72 { Eyes::Squeeze } else { Eyes::Open },
            }
        } else if st == State::Blocked {
            Eyes::Dizzy
        } else if blink || (aiming && self.recoil > 0.5) {
            Eyes::Shut
        } else {
            Eyes::Open
        };
        let look = if self.at_table {
            (0.2, 0.9)
        } else if aiming {
            (1.0, self.aim_angle())
        } else if st == State::Waiting {
            (0.2, -0.3)
        } else {
            (0.5 + 0.5 * (t * 0.7 + b.seed).sin(), 0.2 * (t * 1.1 + b.seed).sin())
        };
        let cream = if hit && self.hit_kind == SPOON { 1.0 - smooth((hu - 0.35) / 0.55) } else { 0.0 };
        let lk = Looks { c, a: fa, glow: a * flick * vis, eyes, look, helmet: (st != State::Blocked).then_some(self.helmet), pale: false, frown: st == State::Blocked, cream, t };
        cr.save();
        cr.translate(b.x, b.y - lift * r + sink * r * 0.80);
        cr.scale(sx * shrink, shrink);
        if st == State::Blocked {
            // the helmet lies beside it, rocking
            cr.save();
            cr.translate(-r * 1.02, r * 0.86);
            cr.rotate(PI + 0.18 * (t * 1.7 + b.seed).sin());
            helmet(cr, r, self.helmet, c, a * flick);
            cr.restore();
        }
        figure(cr, &self.pose, r, &lk);
        if aiming {
            // the weapon in the front legs
            let wa = fa * smooth(self.aim_t / 0.22) * smooth((AIM_END - self.aim_t) / 0.22);
            let charge = clamp(self.aim_t / AIM_FIRE, 0.0, 1.0);
            let mut ang = if self.weapon == SPOON { self.spoon_angle() } else { self.aim_angle() };
            if self.weapon == PEPPER && !self.fired {
                ang += 0.22 * (t * 34.0).sin() * charge;
            }
            cr.save();
            cr.translate((WPN.0 - 0.08 * self.recoil) * r, WPN.1 * r);
            cr.rotate(ang);
            weapon(cr, self.weapon, r, charge, self.fired, wa);
            cr.restore();
        }
        if st == State::Waiting && self.up > 0.3 {
            // a white flag: a toothpick with a scrap of paper
            let fa = fa * smooth((self.up - 0.3) / 0.4);
            let h = self.pose.feet[0];
            let sway = (t * 6.0 + b.seed).sin();
            let top = (h.0 + 0.10 * sway, h.1 - 0.74);
            cr.set_source_rgba(0.88, 0.74, 0.50, fa);
            cr.set_line_width(0.035 * r);
            cr.set_line_cap(LineCap::Round);
            cr.move_to((h.0 - 0.02) * r, (h.1 + 0.12) * r);
            cr.line_to(top.0 * r, top.1 * r);
            cr.stroke();
            let fl = (t * 9.0 + b.seed).sin() * 0.04;
            cr.move_to(top.0 * r, (top.1 + 0.02) * r);
            cr.quad_to((top.0 + 0.20) * r, (top.1 - 0.04 + fl) * r, (top.0 + 0.40) * r, (top.1 + 0.05 - fl) * r);
            cr.line_to((top.0 + 0.37) * r, (top.1 + 0.30 - fl) * r);
            cr.quad_to((top.0 + 0.18) * r, (top.1 + 0.22 + fl) * r, (top.0 + 0.02) * r, (top.1 + 0.28) * r);
            cr.close_path();
            cr.set_source_rgba(1.0, 0.99, 0.94, fa);
            cr.fill();
            cr.set_source_rgba(0.70, 0.72, 0.80, 0.6 * fa);
            cr.set_line_width(0.014 * r);
            for k in [0.10, 0.17] {
                cr.move_to((top.0 + 0.06) * r, (top.1 + k + fl * 0.3) * r);
                cr.line_to((top.0 + 0.32) * r, (top.1 + k + 0.03 - fl * 0.5) * r);
            }
            cr.stroke();
        }
        if hit {
            match self.hit_kind {
                PEA | CORK => {
                    // stars going round the head
                    let (ux, uy) = (self.pose.lean.cos(), -self.pose.lean.sin());
                    let hp = if self.pose.roll > 1.5 { (-0.44, 0.50) } else { (self.pose.c.0 + ux * 0.44, self.pose.c.1 + uy * 0.44 - 0.38) };
                    cr.set_source_rgba(1.0, 0.92, 0.40, fa * smooth(hu * 8.0) * smooth((1.0 - hu) * 5.0));
                    for k in 0..3 {
                        let ang = t * 5.0 + k as f64 * TAU / 3.0;
                        star(cr, (hp.0 + ang.cos() * 0.30) * r, (hp.1 + ang.sin() * 0.09) * r, r * 0.085, ang * 1.3);
                    }
                    cr.fill();
                }
                _ => {}
            }
        }
        cr.restore();
        if hit && self.hit_kind == BUBBLE && hu < 0.72 {
            bubble(cr, b.x, b.y - lift * r + r * 0.48, r * (0.98 + 0.03 * (t * 6.0).sin()), fa * smooth(hu * 12.0));
        }
    }
}

// --------------------------------------------------------------------------- the room

/// Where the fixed things of the room are.
#[derive(Clone, Copy, Default)]
struct Geo {
    /// middle of the mouse hole's sill, its width and height
    hole: P,
    hole_w: f64,
    hole_h: f64,
    /// underside of the cupboard floor
    beam: f64,
    /// the dripping end of the pipe, and the water in the bottle cap below
    drip: P,
    pool: P,
    /// where the spider lets itself down
    spider: f64,
}

/// A cobweb in a corner: spokes from (cx, cy) between the angles a0 and a1, sagging threads between.
#[allow(clippy::too_many_arguments)]
fn web(cr: &mut Canvas, cx: f64, cy: f64, a0: f64, a1: f64, rad: f64, spokes: usize, rings: usize, lw: f64, alpha: f64) {
    cr.set_source_rgba(0.86, 0.80, 0.68, alpha);
    cr.set_line_width(lw);
    let ray = |i: usize| {
        let ang = lerp(a0, a1, i as f64 / spokes as f64);
        (ang.cos(), ang.sin(), rad * (0.82 + 0.18 * (i as f64 * 2.4).sin().abs()))
    };
    for i in 0..=spokes {
        let (c, s, len) = ray(i);
        cr.move_to(cx, cy);
        cr.line_to(cx + c * len, cy + s * len);
    }
    for j in 1..=rings {
        let f = j as f64 / rings as f64 * 0.94;
        for i in 0..spokes {
            let (c0, s0, l0) = ray(i);
            let (c1, s1, l1) = ray(i + 1);
            let (p0, p1) = ((cx + c0 * l0 * f, cy + s0 * l0 * f), (cx + c1 * l1 * f, cy + s1 * l1 * f));
            cr.move_to(p0.0, p0.1);
            cr.quad_to(cx + ((p0.0 + p1.0) / 2.0 - cx) * 0.86, cy + ((p0.1 + p1.1) / 2.0 - cy) * 0.86, p1.0, p1.1);
        }
    }
    cr.stroke();
}

/// A patch where the plaster has come off and the bricks show.
fn bricks(cr: &mut Canvas, rng: &Rng, cx: f64, cy: f64, cols: i64, rows: i64, s: f64) {
    let (bw, bh, gap) = (88.0 * s, 36.0 * s, 6.0 * s);
    let (rx, ry) = ((cols as f64 * 0.5 + 0.1) * bw, (rows as f64 * 0.5 + 0.1) * bh);
    let ph = rng.uniform(0.0, TAU);
    // the ragged edge of the plaster
    let edge = |cr: &mut Canvas, grow: f64, dy: f64| {
        for i in 0..40 {
            let ang = i as f64 * TAU / 40.0;
            let k = 1.0 + 0.07 * (ang * 5.0 + ph).sin() + 0.05 * (ang * 11.0 + ph * 2.0).sin();
            cr.line_to(cx + ang.cos() * (rx * k + grow), cy + dy + ang.sin() * (ry * k + grow));
        }
        cr.close_path();
    };
    edge(cr, 5.0 * s, 4.0 * s);
    cr.set_source_rgba(1.0, 0.85, 0.65, 0.07); // the lit lower edge of the plaster
    cr.fill();
    cr.save();
    edge(cr, 0.0, 0.0);
    cr.set_source_rgb(0.075, 0.05, 0.04);
    cr.fill_preserve();
    cr.clip();
    for row in -1..=rows {
        for col in -1..=cols + 1 {
            let (fx, fy) = (col as f64 - cols as f64 / 2.0 - if row % 2 == 0 { 0.0 } else { 0.5 }, row as f64 - rows as f64 / 2.0);
            let v = rng.uniform(-0.05, 0.05);
            cr.set_source_rgb(0.31 + v, 0.15 + v * 0.6, 0.095 + v * 0.4);
            cr.rounded_rect(cx + fx * bw, cy + fy * bh, bw - gap, bh - gap, 3.0 * s);
            cr.fill();
            cr.set_source_rgba(1.0, 0.75, 0.55, 0.07);
            cr.rectangle(cx + fx * bw + 3.0 * s, cy + fy * bh + 2.0 * s, bw - gap - 6.0 * s, 3.0 * s);
            cr.fill();
        }
    }
    // the plaster throws a shadow on the bricks along the upper edge
    edge(cr, -3.0 * s, 9.0 * s);
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.5);
    cr.set_line_width(16.0 * s);
    cr.stroke();
    cr.restore();
}

/// A tipped tin can lying on its side, the open end to the right; (x, y) = lowest point of its middle.
fn tin_can(cr: &mut Canvas, x: f64, y: f64, k: f64) {
    let (len, rad) = (250.0 * k, 100.0 * k);
    let (x0, x1, top, cy) = (x - len / 2.0, x + len / 2.0, y - 2.0 * rad, y - rad);
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.42);
    cr.ellipse(x + 26.0 * k, y + 2.0 * k, len * 0.66, 17.0 * k);
    cr.fill();
    let mut g = Gradient::linear(0.0, top, 0.0, y);
    g.add_color_stop_rgb(0.0, 0.26, 0.26, 0.28);
    g.add_color_stop_rgb(0.22, 0.56, 0.56, 0.58);
    g.add_color_stop_rgb(0.5, 0.36, 0.36, 0.38);
    g.add_color_stop_rgb(1.0, 0.11, 0.10, 0.10);
    cr.set_source(&g);
    cr.ellipse(x0, cy, 30.0 * k, rad);
    cr.rectangle(x0, top, len, 2.0 * rad);
    cr.fill();
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.22);
    cr.set_line_width(3.0 * k);
    for i in 0..7 {
        let xx = x0 + (i as f64 + 0.4) * len / 7.0;
        cr.move_to(xx, top);
        cr.line_to(xx, y);
    }
    cr.stroke();
    // what is left of the paper label
    let (lx0, lx1) = (x0 + len * 0.10, x0 + len * 0.70);
    let mut g = Gradient::linear(0.0, top, 0.0, y);
    g.add_color_stop_rgb(0.0, 0.40, 0.33, 0.22);
    g.add_color_stop_rgb(0.25, 0.72, 0.63, 0.44);
    g.add_color_stop_rgb(0.55, 0.52, 0.44, 0.30);
    g.add_color_stop_rgb(1.0, 0.18, 0.14, 0.09);
    cr.set_source(&g);
    cr.move_to(lx0, top + 3.0 * k);
    cr.line_to(lx1, top + 3.0 * k);
    cr.line_to(lx1, y - 46.0 * k);
    cr.line_to(lx1 - 34.0 * k, y - 4.0 * k);
    cr.line_to(lx0, y - 4.0 * k);
    cr.close_path();
    cr.fill();
    cr.set_source_rgba(0.50, 0.17, 0.10, 0.85);
    cr.rectangle(lx0, top + 22.0 * k, lx1 - lx0, 13.0 * k);
    cr.rectangle(lx0, y - 40.0 * k, lx1 - lx0 - 30.0 * k, 11.0 * k);
    cr.fill();
    cr.set_source_rgba(0.62, 0.20, 0.12, 0.9);
    cr.circle((lx0 + lx1) / 2.0, cy - 6.0 * k, 40.0 * k);
    cr.fill();
    cr.set_source_rgba(0.26, 0.40, 0.18, 0.9);
    cr.ellipse((lx0 + lx1) / 2.0, cy - 44.0 * k, 16.0 * k, 6.0 * k);
    cr.fill();
    cr.set_source_rgba(1.0, 0.8, 0.7, 0.16);
    cr.ellipse((lx0 + lx1) / 2.0 - 12.0 * k, cy - 20.0 * k, 12.0 * k, 8.0 * k);
    cr.fill();
    // the open end with its bent lid
    cr.set_rgb(STEEL);
    cr.ellipse(x1, cy, 34.0 * k, rad);
    cr.fill();
    cr.set_source_rgb(0.05, 0.04, 0.035);
    cr.ellipse(x1 + 3.0 * k, cy, 27.0 * k, rad * 0.90);
    cr.fill();
    cr.set_source_rgba(0.75, 0.62, 0.45, 0.20);
    cr.set_line_width(3.0 * k);
    cr.new_sub_path();
    cr.save();
    cr.translate(x1 + 3.0 * k, cy);
    cr.scale(27.0 * k, rad * 0.90);
    cr.arc(0.0, 0.0, 0.92, 0.5, 1.9);
    cr.restore();
    cr.stroke();
    cr.save();
    cr.translate(x1 + 62.0 * k, top + 30.0 * k);
    cr.rotate(0.62);
    cr.set_source_rgb(0.40, 0.41, 0.44);
    cr.ellipse(0.0, 0.0, 24.0 * k, 86.0 * k);
    cr.fill();
    cr.set_source_rgba(1.0, 0.95, 0.85, 0.22);
    cr.ellipse(-6.0 * k, -14.0 * k, 9.0 * k, 52.0 * k);
    cr.fill();
    cr.restore();
}

/// A bottle cap lying on its back, full of drip water; (x, y) = lowest point.
fn bottle_cap(cr: &mut Canvas, x: f64, y: f64, k: f64) {
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.40);
    cr.ellipse(x + 8.0 * k, y + 1.0 * k, 88.0 * k, 12.0 * k);
    cr.fill();
    // crimped wall
    cr.move_to(x - 62.0 * k, y - 4.0 * k);
    let n = 12;
    for i in 0..=n {
        let f = i as f64 / n as f64;
        let xx = x - 78.0 * k + f * 156.0 * k;
        cr.line_to(xx, y - 36.0 * k - (f * PI).sin() * 3.0 * k);
        if i < n {
            cr.line_to(xx + 6.5 * k, y - 28.0 * k);
        }
    }
    cr.line_to(x + 62.0 * k, y - 4.0 * k);
    cr.quad_to(x, y + 6.0 * k, x - 62.0 * k, y - 4.0 * k);
    cr.close_path();
    let mut g = Gradient::linear(x - 78.0 * k, 0.0, x + 78.0 * k, 0.0);
    g.add_color_stop_rgb(0.0, 0.50, 0.38, 0.14);
    g.add_color_stop_rgb(0.35, 0.74, 0.60, 0.26);
    g.add_color_stop_rgb(1.0, 0.26, 0.19, 0.08);
    cr.set_source(&g);
    cr.fill();
    cr.set_source_rgb(0.20, 0.15, 0.08);
    cr.ellipse(x, y - 34.0 * k, 74.0 * k, 11.0 * k);
    cr.fill();
    cr.set_source_rgba(0.34, 0.50, 0.58, 0.9);
    cr.ellipse(x, y - 32.0 * k, 68.0 * k, 9.0 * k);
    cr.fill();
    cr.set_source_rgba(1.0, 0.86, 0.60, 0.35);
    cr.ellipse(x + 18.0 * k, y - 33.0 * k, 30.0 * k, 3.0 * k);
    cr.fill();
}

/// A dimpled steel thimble standing on (x, y).
fn thimble(cr: &mut Canvas, x: f64, y: f64, k: f64) {
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.40);
    cr.ellipse(x + 10.0 * k, y + 1.0 * k, 72.0 * k, 11.0 * k);
    cr.fill();
    cr.move_to(x - 56.0 * k, y);
    cr.line_to(x - 42.0 * k, y - 92.0 * k);
    cr.curve_to(x - 40.0 * k, y - 138.0 * k, x + 40.0 * k, y - 138.0 * k, x + 42.0 * k, y - 92.0 * k);
    cr.line_to(x + 56.0 * k, y);
    cr.quad_to(x, y + 10.0 * k, x - 56.0 * k, y);
    cr.close_path();
    let mut g = Gradient::linear(x - 56.0 * k, 0.0, x + 56.0 * k, 0.0);
    g.add_color_stop_rgb(0.0, 0.34, 0.35, 0.38);
    g.add_color_stop_rgb(0.32, 0.66, 0.67, 0.70);
    g.add_color_stop_rgb(1.0, 0.17, 0.17, 0.19);
    cr.set_source(&g);
    cr.fill();
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.26);
    for row in 0..5 {
        let yy = y - (34.0 + row as f64 * 19.0) * k;
        let half = (50.0 - row as f64 * 3.4) * k;
        for col in 0..7 {
            let f = (col as f64 + if row % 2 == 0 { 0.5 } else { 0.0 }) / 7.0 - 0.5;
            cr.circle(x + f * 1.8 * half, yy, 3.6 * k);
        }
    }
    cr.fill();
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.30);
    cr.set_line_width(3.0 * k);
    cr.move_to(x - 54.0 * k, y - 14.0 * k);
    cr.quad_to(x, y - 5.0 * k, x + 54.0 * k, y - 14.0 * k);
    cr.stroke();
}

/// A matchbox lying on (x, y) (bottom centre), the drawer pulled out a little.
fn matchbox(cr: &mut Canvas, x: f64, y: f64, k: f64) {
    let (bw, bh, dx, dy) = (230.0 * k, 78.0 * k, 30.0 * k, 24.0 * k);
    let x0 = x - bw / 2.0;
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.42);
    cr.ellipse(x + 30.0 * k, y + 2.0 * k, bw * 0.68, 14.0 * k);
    cr.fill();
    // the drawer sticks out on the right: cardboard, with the heads of the matches
    let px = 58.0 * k;
    cr.set_source_rgb(0.52, 0.41, 0.26);
    cr.rectangle(x0 + bw, y - bh + 7.0 * k, px, bh - 7.0 * k);
    cr.fill();
    cr.set_source_rgb(0.20, 0.14, 0.09);
    cr.move_to(x0 + bw, y - bh + 7.0 * k);
    cr.line_to(x0 + bw + px, y - bh + 7.0 * k);
    cr.line_to(x0 + bw + px + dx, y - bh - dy + 7.0 * k);
    cr.line_to(x0 + bw + dx, y - bh - dy + 7.0 * k);
    cr.close_path();
    cr.fill();
    for i in 0..4 {
        let f = i as f64;
        let (mx, my) = (x0 + bw + 10.0 * k + f * 8.0 * k, y - bh + 3.0 * k - f * 6.0 * k);
        cr.set_source_rgb(0.74, 0.60, 0.38);
        cr.set_line_width(5.0 * k);
        cr.set_line_cap(LineCap::Butt);
        cr.move_to(mx - 6.0 * k, my);
        cr.line_to(mx + 34.0 * k, my);
        cr.stroke();
        cr.set_source_rgb(0.62, 0.17, 0.12);
        cr.ellipse(mx + 38.0 * k, my, 7.0 * k, 4.6 * k);
        cr.fill();
    }
    // the sleeve: front with the striking strip, top with a picture
    cr.set_source_rgb(0.25, 0.30, 0.42);
    cr.rectangle(x0, y - bh, bw, bh);
    cr.fill();
    cr.set_source_rgb(0.16, 0.10, 0.07);
    cr.rectangle(x0, y - bh * 0.72, bw, bh * 0.50);
    cr.fill();
    cr.set_source_rgba(0.50, 0.36, 0.26, 0.5);
    for i in 0..40 {
        let f = i as f64;
        cr.circle(x0 + ((f * 0.618).fract()) * bw, y - bh * 0.70 + ((f * 0.377).fract()) * bh * 0.46, 1.4 * k);
    }
    cr.fill();
    cr.move_to(x0, y - bh);
    cr.line_to(x0 + bw, y - bh);
    cr.line_to(x0 + bw + dx, y - bh - dy);
    cr.line_to(x0 + dx, y - bh - dy);
    cr.close_path();
    cr.set_source_rgb(0.36, 0.42, 0.56);
    cr.fill();
    cr.move_to(x0 + bw * 0.22, y - bh - dy * 0.2);
    cr.line_to(x0 + bw * 0.80, y - bh - dy * 0.2);
    cr.line_to(x0 + bw * 0.80 + dx * 0.6, y - bh - dy * 0.8);
    cr.line_to(x0 + bw * 0.22 + dx * 0.6, y - bh - dy * 0.8);
    cr.close_path();
    cr.set_source_rgb(0.74, 0.66, 0.48);
    cr.fill();
    cr.set_source_rgb(0.70, 0.30, 0.12);
    cr.ellipse(x0 + bw * 0.54, y - bh - dy * 0.5, 18.0 * k, 5.0 * k);
    cr.fill();
}

/// A teaspoon leaning against the wall: handle end on the floor at (x, y), bowl up to the right.
fn teaspoon(cr: &mut Canvas, x: f64, y: f64, k: f64) {
    let (tx, ty) = (x + 96.0 * k, y - 430.0 * k);
    let (dx, dy) = (tx - x, ty - y);
    let d = hypot(dx, dy);
    let (nx, ny) = (-dy / d, dx / d);
    let shape = |cr: &mut Canvas, ox: f64| {
        cr.move_to(x + ox - nx * 13.0 * k, y - ny * 13.0 * k);
        cr.line_to(tx + ox - nx * 6.0 * k, ty - ny * 6.0 * k);
        cr.line_to(tx + ox + nx * 6.0 * k, ty + ny * 6.0 * k);
        cr.line_to(x + ox + nx * 13.0 * k, y + ny * 13.0 * k);
        cr.close_path();
        cr.save();
        cr.translate(tx + ox + dx / d * 62.0 * k, ty + dy / d * 62.0 * k);
        cr.rotate(dy.atan2(dx));
        cr.ellipse(0.0, 0.0, 70.0 * k, 44.0 * k);
        cr.restore();
    };
    shape(cr, 26.0 * k);
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.30); // its shadow on the wall
    cr.fill();
    shape(cr, 0.0);
    let mut g = Gradient::linear(x - 20.0 * k, 0.0, tx + 60.0 * k, 0.0);
    g.add_color_stop_rgb(0.0, 0.30, 0.31, 0.34);
    g.add_color_stop_rgb(0.5, 0.58, 0.59, 0.62);
    g.add_color_stop_rgb(1.0, 0.36, 0.37, 0.40);
    cr.set_source(&g);
    cr.fill();
    cr.save();
    cr.translate(tx + dx / d * 62.0 * k, ty + dy / d * 62.0 * k);
    cr.rotate(dy.atan2(dx));
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.25);
    cr.ellipse(4.0 * k, 4.0 * k, 54.0 * k, 31.0 * k);
    cr.fill();
    cr.set_source_rgba(1.0, 0.92, 0.75, 0.30);
    cr.ellipse(-14.0 * k, -12.0 * k, 26.0 * k, 9.0 * k);
    cr.fill();
    cr.restore();
    cr.set_source_rgba(1.0, 0.95, 0.85, 0.28);
    cr.set_line_width(3.0 * k);
    cr.move_to(x - nx * 6.0 * k, y - 20.0 * k);
    cr.line_to(tx - nx * 2.0 * k, ty);
    cr.stroke();
}

/// A wooden clothes peg leaning on the wall, its foot at (x, y).
fn clothes_peg(cr: &mut Canvas, x: f64, y: f64, k: f64) {
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.40);
    cr.ellipse(x - 30.0 * k, y + 2.0 * k, 96.0 * k, 11.0 * k);
    cr.fill();
    cr.save();
    cr.translate(x, y);
    cr.rotate(-1.08);
    for (side, c) in [(-1.0, (0.46, 0.33, 0.18)), (1.0, (0.62, 0.46, 0.26))] {
        cr.save();
        cr.rotate(side * 0.045);
        cr.set_rgb(c);
        cr.rounded_rect(0.0, side * 14.0 * k - 12.0 * k, 280.0 * k, 24.0 * k, 9.0 * k);
        cr.fill();
        cr.set_source_rgba(0.0, 0.0, 0.0, 0.16);
        cr.set_line_width(2.0 * k);
        cr.move_to(20.0 * k, side * 14.0 * k);
        cr.line_to(250.0 * k, side * 14.0 * k + 3.0 * k);
        cr.stroke();
        cr.restore();
    }
    // the spring
    cr.set_rgb(STEEL);
    cr.set_line_width(5.0 * k);
    cr.circle(128.0 * k, 0.0, 15.0 * k);
    cr.move_to(110.0 * k, -26.0 * k);
    cr.line_to(110.0 * k, 26.0 * k);
    cr.move_to(146.0 * k, -26.0 * k);
    cr.line_to(146.0 * k, 26.0 * k);
    cr.stroke();
    cr.restore();
}

/// The whole static room: wall, hole, pipe, floor and the giant things lying about.
fn paint_room(cr: &mut Canvas, env: &Env, geo: &Geo) {
    let (w, h, s, gt) = (env.w, env.h, env.s, env.ground_top);
    let rng = &env.rng;
    // the back wall: old plaster, darker towards the top
    let mut g = Gradient::linear(0.0, 0.0, 0.0, gt);
    g.add_color_stop_rgb(0.0, 0.085, 0.06, 0.05);
    g.add_color_stop_rgb(0.55, 0.20, 0.145, 0.105);
    g.add_color_stop_rgb(1.0, 0.16, 0.11, 0.075);
    cr.set_source(&g);
    cr.rectangle(0.0, 0.0, w, gt + 4.0 * s);
    cr.fill();
    for _ in 0..(70.0 * w / (1600.0 * s)) as usize {
        let v = rng.uniform(0.0, 1.0);
        cr.set_source_rgba(v, v * 0.8, v * 0.6, rng.uniform(0.02, 0.05));
        cr.ellipse(rng.uniform(0.0, w), rng.uniform(0.0, gt), rng.uniform(30.0, 130.0) * s, rng.uniform(14.0, 60.0) * s);
        cr.fill();
    }
    bricks(cr, rng, w * 0.615, gt * 0.36, 5, 6, s);
    bricks(cr, rng, w * 0.13, gt * 0.62, 4, 5, s);
    bricks(cr, rng, w * 0.90, gt * 0.66, 3, 5, s);
    // cracks in the plaster
    cr.set_source_rgba(0.04, 0.025, 0.02, 0.7);
    cr.set_line_width(2.2 * s);
    cr.set_line_cap(LineCap::Round);
    cr.set_line_join(LineJoin::Round);
    for (fx, fy) in [(0.30, 0.16), (0.48, 0.30), (0.76, 0.22), (0.20, 0.40)] {
        let (mut x, mut y) = (w * fx, gt * fy);
        cr.move_to(x, y);
        for _ in 0..7 {
            x += rng.uniform(-34.0, 34.0) * s;
            y += rng.uniform(22.0, 50.0) * s;
            cr.line_to(x, y);
        }
    }
    cr.stroke();
    // the glow of the kitchen light around the mouse hole
    let (hx, hw, hh) = (geo.hole.0, geo.hole_w, geo.hole_h);
    cr.glow(hx, gt - hh * 0.45, 520.0 * s, LIGHT, 0.26, 0.0);
    // the rusty pipe along the wall, turning down at its end
    let (px, py) = (geo.drip.0, geo.drip.1 - 92.0 * s);
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.32);
    cr.rectangle(0.0, py + 18.0 * s, px + 20.0 * s, 44.0 * s);
    cr.fill();
    cr.set_source_rgba(0.38, 0.17, 0.07, 0.40); // rust running down the wall
    cr.move_to(px - 16.0 * s, py + 80.0 * s);
    cr.line_to(px + 16.0 * s, py + 80.0 * s);
    cr.line_to(px + 6.0 * s, gt - 40.0 * s);
    cr.line_to(px - 6.0 * s, gt - 40.0 * s);
    cr.close_path();
    cr.fill();
    let mut g = Gradient::linear(0.0, py - 23.0 * s, 0.0, py + 23.0 * s);
    g.add_color_stop_rgb(0.0, 0.30, 0.15, 0.08);
    g.add_color_stop_rgb(0.3, 0.52, 0.28, 0.14);
    g.add_color_stop_rgb(1.0, 0.15, 0.07, 0.04);
    cr.set_source(&g);
    cr.rectangle(-10.0 * s, py - 23.0 * s, px + 10.0 * s, 46.0 * s);
    cr.fill();
    let mut g = Gradient::linear(px - 23.0 * s, 0.0, px + 23.0 * s, 0.0);
    g.add_color_stop_rgb(0.0, 0.34, 0.17, 0.09);
    g.add_color_stop_rgb(0.35, 0.52, 0.28, 0.14);
    g.add_color_stop_rgb(1.0, 0.15, 0.07, 0.04);
    cr.set_source(&g);
    cr.new_sub_path();
    cr.arc(px, py, 23.0 * s, -PI / 2.0, 0.0);
    cr.line_to(px + 23.0 * s, py + 80.0 * s);
    cr.line_to(px - 23.0 * s, py + 80.0 * s);
    cr.line_to(px - 23.0 * s, py);
    cr.close_path();
    cr.fill();
    cr.set_source_rgb(0.24, 0.115, 0.06);
    for fx in [0.25, 0.62] {
        cr.rounded_rect(px * fx - 9.0 * s, py - 29.0 * s, 18.0 * s, 58.0 * s, 4.0 * s);
    }
    cr.rounded_rect(px - 46.0 * s, py - 29.0 * s, 18.0 * s, 58.0 * s, 4.0 * s);
    cr.rounded_rect(px - 29.0 * s, py + 72.0 * s, 58.0 * s, 20.0 * s, 4.0 * s);
    cr.fill();
    cr.set_source_rgba(0.62, 0.72, 0.60, 0.22); // a bit of verdigris
    cr.ellipse(px * 0.44, py + 6.0 * s, 46.0 * s, 9.0 * s);
    cr.ellipse(px - 4.0 * s, py + 40.0 * s, 9.0 * s, 22.0 * s);
    cr.fill();
    // the underside of the cupboard
    let mut g = Gradient::linear(0.0, 0.0, 0.0, geo.beam);
    g.add_color_stop_rgb(0.0, 0.05, 0.03, 0.02);
    g.add_color_stop_rgb(1.0, 0.13, 0.08, 0.05);
    cr.set_source(&g);
    cr.rectangle(0.0, 0.0, w, geo.beam);
    cr.fill();
    cr.set_source_rgba(0.40, 0.27, 0.16, 0.5);
    cr.rectangle(0.0, geo.beam - 4.0 * s, w, 4.0 * s);
    cr.fill();
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.30);
    cr.rectangle(0.0, geo.beam, w, 16.0 * s);
    cr.fill();
    web(cr, 0.0, geo.beam, 0.0, PI / 2.0, 270.0 * s, 6, 6, 1.3 * s, 0.34);
    web(cr, w, geo.beam, PI / 2.0, PI, 230.0 * s, 5, 5, 1.3 * s, 0.30);
    web(cr, w * 0.52, geo.beam, 0.25, PI - 0.35, 110.0 * s, 5, 3, 1.1 * s, 0.22);
    // the mouse hole: the bright kitchen beyond
    let (x0, x1, top) = (hx - hw / 2.0, hx + hw / 2.0, gt - hh + hw / 2.0);
    let arch = |cr: &mut Canvas, grow: f64| {
        cr.move_to(x0 - grow, gt + 4.0 * s);
        cr.line_to(x0 - grow, top);
        cr.arc(hx, top, hw / 2.0 + grow, PI, TAU);
        cr.line_to(x1 + grow, gt + 4.0 * s);
        cr.close_path();
    };
    arch(cr, 13.0 * s);
    cr.set_source_rgb(0.07, 0.045, 0.03);
    cr.fill();
    let mut g = Gradient::radial(hx, gt - hh * 0.40, 0.0, hx, gt - hh * 0.40, hh * 0.85);
    g.add_color_stop_rgb(0.0, 1.0, 0.95, 0.74);
    g.add_color_stop_rgb(0.6, 1.0, 0.80, 0.42);
    g.add_color_stop_rgb(1.0, 0.86, 0.52, 0.20);
    cr.set_source(&g);
    arch(cr, 0.0);
    cr.fill();
    // far away: the kitchen floor and the legs of a chair
    cr.set_source_rgba(0.80, 0.50, 0.20, 0.42);
    cr.rectangle(x0, gt - hh * 0.20, hw, hh * 0.20 + 4.0 * s);
    cr.fill();
    cr.set_source_rgba(0.62, 0.34, 0.12, 0.50);
    for (fx, ww, top, len) in [(0.16, 0.085, 0.86, 0.80), (0.70, 0.05, 0.86, 0.70), (0.52, 0.035, 0.86, 0.66)] {
        cr.rectangle(x0 + hw * fx, gt - hh * top, hw * ww, hh * len);
    }
    cr.fill();
    // the floor: packed earth and dust
    let mut g = Gradient::linear(0.0, gt, 0.0, h);
    g.add_color_stop_rgb(0.0, 0.37, 0.26, 0.155);
    g.add_color_stop_rgb(0.45, 0.28, 0.19, 0.11);
    g.add_color_stop_rgb(1.0, 0.15, 0.10, 0.06);
    cr.set_source(&g);
    cr.move_to(0.0, h);
    let n = 48;
    for i in 0..=n {
        let f = i as f64 / n as f64;
        // the sill of the mouse hole stays level
        let wave = if (f * w - hx).abs() < hw * 0.6 { 0.0 } else { (f * 31.0).sin() * 3.0 + (f * 77.0).sin() * 1.5 };
        cr.line_to(f * w, gt + wave * s);
    }
    cr.line_to(w, h);
    cr.close_path();
    cr.fill();
    let mut g = Gradient::linear(0.0, gt, 0.0, gt + 60.0 * s);
    g.add_color_stop_rgba(0.0, 0.0, 0.0, 0.0, 0.42);
    g.add_color_stop_rgba(1.0, 0.0, 0.0, 0.0, 0.0);
    cr.set_source(&g);
    cr.rectangle(0.0, gt, w, 60.0 * s);
    cr.fill();
    let gh = h - gt;
    for _ in 0..(190.0 * w / (1600.0 * s)) as usize {
        let f = rng.random();
        let k = (0.5 + f) * s;
        let v = if rng.chance(0.5) { 0.0 } else { 1.0 };
        cr.set_source_rgba(v, v * 0.85, v * 0.6, rng.uniform(0.025, 0.07));
        cr.ellipse(rng.uniform(0.0, w), gt + f * gh, rng.uniform(16.0, 70.0) * k, rng.uniform(4.0, 12.0) * k);
        cr.fill();
    }
    // the light of the kitchen fanning out over the floor
    let mut g = Gradient::linear(0.0, gt, 0.0, gt + gh * 0.95);
    g.stop(0.0, LIGHT, 0.42);
    g.stop(0.35, LIGHT, 0.16);
    g.stop(1.0, LIGHT, 0.0);
    cr.set_source(&g);
    cr.move_to(x0, gt + 2.0 * s);
    cr.line_to(x1, gt + 2.0 * s);
    cr.line_to(x1 + gh * 1.15, h);
    cr.line_to(x0 - gh * 0.30, h);
    cr.close_path();
    cr.fill();
    cr.save();
    cr.translate(hx + 30.0 * s, gt + 34.0 * s);
    cr.scale(1.0, 0.26);
    cr.glow(0.0, 0.0, 330.0 * s, LIGHT, 0.30, 0.2);
    cr.restore();
    // pebbles, old mounds and burrow holes
    for _ in 0..(46.0 * w / (1600.0 * s)) as usize {
        let f = rng.random();
        let k = (0.5 + f) * s;
        let (x, y, pr) = (rng.uniform(0.0, w), gt + 12.0 * s + f * gh, rng.uniform(3.0, 8.0) * k);
        cr.set_source_rgba(0.0, 0.0, 0.0, 0.25);
        cr.ellipse(x + pr * 0.4, y + pr * 0.35, pr * 1.1, pr * 0.5);
        cr.fill();
        cr.set_source_rgb(0.42 + f * 0.06, 0.33, 0.24);
        cr.ellipse(x, y, pr, pr * 0.62);
        cr.fill();
        cr.set_source_rgba(1.0, 0.9, 0.7, 0.16);
        cr.ellipse(x - pr * 0.25, y - pr * 0.2, pr * 0.5, pr * 0.22);
        cr.fill();
    }
    for (fx, d, hole) in [(0.045, 0.30, true), (0.955, 0.50, true), (0.31, 0.10, false), (0.74, 0.06, true), (0.50, 0.03, false), (0.03, 0.72, false), (0.975, 0.82, false)] {
        let (x, y, k) = (w * fx, gt + d * (gh - env.strip_h), lerp(0.62, 1.1, d) * s);
        cr.set_rgb(darken(EARTH, 0.25));
        cr.ellipse(x + 8.0 * k, y + 6.0 * k, 78.0 * k, 16.0 * k);
        cr.fill();
        cr.set_rgb(EARTH);
        cr.move_to(x - 70.0 * k, y + 6.0 * k);
        cr.curve_to(x - 40.0 * k, y - 34.0 * k, x + 40.0 * k, y - 34.0 * k, x + 70.0 * k, y + 6.0 * k);
        cr.close_path();
        cr.fill();
        cr.set_rgba(EARTH_L, 0.55);
        cr.ellipse(x - 12.0 * k, y - 14.0 * k, 34.0 * k, 7.0 * k);
        cr.fill();
        if hole {
            cr.set_source_rgb(0.035, 0.022, 0.015);
            cr.ellipse(x + 4.0 * k, y - 2.0 * k, 34.0 * k, 12.0 * k);
            cr.fill();
        }
    }
    // the giant things along the wall
    let base = gt + 10.0 * s;
    tin_can(cr, w * 0.105, base + 8.0 * s, s);
    bottle_cap(cr, geo.pool.0, geo.pool.1 + 32.0 * s, s);
    thimble(cr, w * 0.545, base, s);
    matchbox(cr, w * 0.655, base + 6.0 * s, s);
    teaspoon(cr, w * 0.775, base + 4.0 * s, s);
    clothes_peg(cr, w * 0.945, base + 10.0 * s, s);
    // deep shadows in the corners
    let mut v = Gradient::radial(w * 0.46, h * 0.60, h * 0.42, w * 0.46, h * 0.60, hypot(w, h) * 0.66);
    v.add_color_stop_rgba(0.0, 0.02, 0.01, 0.0, 0.0);
    v.add_color_stop_rgba(1.0, 0.02, 0.01, 0.0, 0.66);
    cr.set_source(&v);
    cr.rectangle(0.0, 0.0, w, h);
    cr.fill();
}

pub struct Cockroaches {
    bg: Option<Picture>,
    /// counts the layouts: a resize makes a new floor
    epoch: u64,
    geo: Geo,
    foods: Vec<Food>,
    next_food: f64,
    food_id: u64,
    mounds: Vec<Mound>,
    /// the trail of a tunnel: x, y, radius of the digger, when
    ridges: Vec<(f64, f64, f64, f64)>,
    shots: Vec<Shot>,
    impacts: Vec<Impact>,
    bits: Vec<Bit>,
    marks: Vec<Mark>,
    /// motes in the light: x, y (fractions of the lit area), phase, speed
    motes: Vec<(f64, f64, f64, f64)>,
}

impl Cockroaches {
    fn bit(&mut self, b: Bit) {
        if self.bits.len() < MAX_BITS {
            self.bits.push(b);
        }
    }

    /// `n` bits flying apart from `at`; `speed` in px/s at scale 1, `life` in seconds.
    #[allow(clippy::too_many_arguments)]
    fn burst(&mut self, env: &Env, n: usize, at: P, speed: (f64, f64), c: Rgb, size: f64, grav: f64, kind: u8, life: (f64, f64), floor: f64) {
        for _ in 0..n {
            let ang = if grav > 0.5 { env.rng.uniform(-PI * 0.92, -PI * 0.08) } else { env.rng.uniform(0.0, TAU) };
            let sp = env.rng.uniform(speed.0, speed.1) * env.s;
            self.bit(Bit {
                x: at.0,
                y: at.1,
                vx: ang.cos() * sp,
                vy: ang.sin() * sp,
                age: 0.0,
                life: env.rng.uniform(life.0, life.1),
                c,
                size: size * env.rng.uniform(0.6, 1.3),
                grav,
                kind,
                floor: floor + env.rng.uniform(-0.3, 1.0) * size * 3.0,
                lying: false,
            });
        }
    }

    fn mound(&mut self, x: f64, y: f64, k: f64, t: f64) {
        if self.mounds.len() >= 14 {
            self.mounds.remove(0);
        }
        self.mounds.push(Mound { x, y, k, born: t });
    }

    fn ridge(&mut self, x: f64, y: f64, k: f64, t: f64) {
        if self.ridges.len() < 90 {
            self.ridges.push((x, y, k, t));
        }
    }

    /// A projectile leaves the weapon, with a little puff.
    #[allow(clippy::too_many_arguments)]
    fn fire(&mut self, env: &Env, kind: usize, from: u64, target: u64, muzzle: P, aim: P, z: f64) {
        let s = env.s;
        let dist = hypot(aim.0 - muzzle.0, aim.1 - muzzle.1);
        let (speed, arc) = flight(kind);
        if self.shots.len() < 24 {
            self.shots.push(Shot { kind, from, target, x0: muzzle.0, y0: muzzle.1, tx: aim.0, ty: aim.1, age: 0.0, dur: clamp(dist / (speed * s), 0.28, 3.4), arc: arc * dist, z });
        }
        let puff = match kind {
            PIPETTE => (0.62, 0.84, 1.0),
            PEPPER => (0.42, 0.34, 0.26),
            _ => (0.95, 0.92, 0.84),
        };
        for _ in 0..if kind == PEPPER { 6 } else { 3 } {
            self.bit(Bit {
                x: muzzle.0,
                y: muzzle.1,
                vx: sign(aim.0 - muzzle.0) * env.rng.uniform(20.0, 110.0) * s,
                vy: env.rng.uniform(-50.0, 30.0) * s,
                age: 0.0,
                life: env.rng.uniform(0.25, 0.5),
                c: puff,
                size: env.rng.uniform(4.0, 8.0) * z / 116.0,
                grav: 0.0,
                kind: 1,
                floor: 1e9,
                lying: false,
            });
        }
    }

    /// A projectile comes down: remember where (the roaches look there) and make a mess.
    fn land(&mut self, env: &Env, sh: &Shot) {
        let (x, y, z) = (sh.tx, sh.ty, sh.z);
        let gy = y + z * 0.95;
        let k = z / 116.0;
        if self.impacts.len() >= 16 {
            self.impacts.remove(0);
        }
        self.impacts.push(Impact { x, y, gy, t0: env.t, kind: sh.kind, from: sh.from, z });
        match sh.kind {
            PEA => {
                self.burst(env, 1, (x, y), (200.0, 300.0), (0.56, 0.82, 0.26), 6.0 * k, 1.0, 0, (1.6, 2.4), gy);
                self.burst(env, 4, (x, y), (120.0, 260.0), (1.0, 0.92, 0.40), 7.0 * k, 0.15, 2, (0.4, 0.7), 1e9);
            }
            BAND => {
                self.burst(env, 1, (x, y), (160.0, 260.0), (0.98, 0.55, 0.66), 5.0 * k, 1.0, 0, (1.6, 2.4), gy);
                self.burst(env, 3, (x, y), (120.0, 240.0), (1.0, 0.92, 0.40), 6.0 * k, 0.15, 2, (0.35, 0.6), 1e9);
            }
            CORK => {
                self.burst(env, 1, (x, y), (220.0, 320.0), (0.84, 0.66, 0.42), 8.0 * k, 1.0, 0, (1.8, 2.6), gy);
                self.burst(env, 5, (x, y), (140.0, 300.0), (1.0, 0.92, 0.40), 7.0 * k, 0.15, 2, (0.4, 0.7), 1e9);
                self.burst(env, 3, (x, y), (30.0, 90.0), (0.95, 0.92, 0.84), 10.0 * k, 0.0, 1, (0.3, 0.5), 1e9);
            }
            PIPETTE => self.burst(env, 14, (x, y), (120.0, 340.0), (0.60, 0.82, 1.0), 3.4 * k, 1.0, 0, (0.5, 0.9), gy),
            SPOON => self.burst(env, 11, (x, y), (100.0, 300.0), (1.0, 0.99, 0.95), 4.6 * k, 1.0, 0, (1.2, 2.4), gy),
            PEPPER => {
                self.burst(env, 7, (x, y), (20.0, 90.0), (0.44, 0.35, 0.27), 13.0 * k, 0.0, 1, (0.6, 1.1), 1e9);
                self.burst(env, 8, (x, y), (40.0, 150.0), (0.20, 0.15, 0.11), 1.8 * k, 0.3, 0, (0.6, 1.0), gy);
            }
            _ => self.burst(env, 10, (x, y), (90.0, 260.0), (0.86, 0.97, 1.0), 2.4 * k, 0.2, 0, (0.25, 0.5), 1e9),
        }
    }

    /// A new piece of food: lying there from the start (`initial`) or dropping in from above.
    fn drop_food(&mut self, env: &Env, initial: bool) {
        let (w, s, gt) = (env.w, env.s, env.ground_top);
        let gh = env.h - env.strip_h - gt - 24.0 * s;
        let mut place = None;
        for _ in 0..40 {
            let (x, d) = (env.rng.uniform(0.07, 0.93) * w, env.rng.uniform(0.07, 0.88));
            let y = gt + d * gh;
            // not on another piece, and not where the waiting roaches stand
            if self.foods.iter().any(|f| hypot(f.x - x, (f.y - y) * 2.2) < 250.0 * s) || ((x - w / 2.0).abs() < w * 0.25 && d > 0.48) {
                continue;
            }
            place = Some((x, y, d));
            break;
        }
        let Some((x, y, d)) = place else { return };
        // a kind that is not lying about already, if possible
        let mut kind = env.rng.index(8);
        for _ in 0..8 {
            if !self.foods.iter().any(|f| f.kind == kind) {
                break;
            }
            kind = (kind + 1) % 8;
        }
        let k = lerp(0.70, 1.22, d) * s * FOOD_SIZE[kind];
        let side = if x < w * 0.2 {
            1.0
        } else if x > w * 0.8 || env.rng.chance(0.5) {
            -1.0
        } else {
            1.0
        };
        let pics = (1..=4).map(|q| Picture::record(FOOD_BOX.0 * k, FOOD_BOX.1 * k, |cr| paint_food(cr, kind, k, q, side))).collect();
        self.food_id += 1;
        self.foods.push(Food { id: self.food_id, kind, x, y, k, side, amount: 1.0, pics, born: if initial { -100.0 } else { env.t }, landed: initial, claim: (0, -100.0) });
    }

    fn draw_food(&self, cr: &mut Canvas, f: &Food, t: f64) {
        let age = t - f.born;
        let off = if age < DROP_T {
            -(1.0 - (age / DROP_T).powi(2)) * (f.y + 160.0 * f.k)
        } else if age < DROP_T + 1.0 {
            let u = age - DROP_T;
            -(u * 10.0).sin().abs() * 34.0 * f.k * (-u * 4.5).exp()
        } else {
            0.0
        };
        cr.picture(&f.pics[f.stage() - 1], (f.x - FOOD_BOX.2 * f.k).round(), (f.y - FOOD_BOX.3 * f.k + off).round(), 1.0);
    }

    /// The far side of a burrow: the dark hole and the earth thrown up behind it.
    fn draw_mound_back(&self, cr: &mut Canvas, m: &Mound, t: f64) {
        let age = t - m.born;
        let flat = 1.0 - smooth((age - 5.0) / (MOUND_LIFE - 8.0));
        let a = clamp((MOUND_LIFE - age) / 3.0, 0.0, 1.0);
        let k = m.k;
        cr.set_rgba(darken(EARTH, 0.2), a);
        cr.ellipse(m.x, m.y - 0.02 * k, 0.74 * k, (0.10 + 0.14 * flat) * k);
        cr.fill();
        cr.set_rgba(EARTH, a);
        cr.ellipse(m.x - 0.05 * k, m.y - (0.04 + 0.10 * flat) * k, 0.60 * k, (0.07 + 0.12 * flat) * k);
        cr.fill();
        let open = 1.0 - smooth((age - 4.0) / 9.0);
        if open > 0.02 {
            cr.set_source_rgba(0.03, 0.02, 0.012, a);
            cr.ellipse(m.x, m.y + 0.01 * k, 0.46 * k * (0.4 + 0.6 * open), 0.14 * k * open);
            cr.fill();
        }
    }

    /// The near side: the heap in front, which hides a roach going down or coming up.
    fn draw_mound_front(&self, cr: &mut Canvas, m: &Mound, t: f64) {
        let age = t - m.born;
        let flat = 1.0 - smooth((age - 5.0) / (MOUND_LIFE - 8.0));
        let a = clamp((MOUND_LIFE - age) / 3.0, 0.0, 1.0);
        let k = m.k;
        cr.set_rgba(EARTH, a);
        cr.ellipse(m.x - 0.44 * k, m.y + 0.06 * k, 0.36 * k, (0.04 + 0.19 * flat) * k);
        cr.ellipse(m.x + 0.46 * k, m.y + 0.07 * k, 0.34 * k, (0.04 + 0.17 * flat) * k);
        cr.ellipse(m.x, m.y + 0.15 * k, 0.56 * k, (0.03 + 0.12 * flat) * k);
        cr.fill();
        cr.set_rgba(EARTH_L, 0.8 * a);
        cr.ellipse(m.x - 0.47 * k, m.y + (0.03 - 0.10 * flat) * k, 0.20 * k, (0.015 + 0.05 * flat) * k);
        cr.ellipse(m.x + 0.44 * k, m.y + (0.04 - 0.08 * flat) * k, 0.18 * k, (0.015 + 0.045 * flat) * k);
        cr.ellipse(m.x + 0.04 * k, m.y + (0.14 - 0.05 * flat) * k, 0.26 * k, (0.012 + 0.03 * flat) * k);
        cr.fill();
        cr.set_rgba(EARTH_D, 0.6 * a);
        for (dx, dy) in [(-0.62, 0.16), (-0.20, 0.24), (0.30, 0.22), (0.70, 0.15), (0.05, 0.27)] {
            cr.circle(m.x + dx * k, m.y + dy * k, 0.022 * k);
        }
        cr.fill();
    }

    fn draw_shot(&self, cr: &mut Canvas, sh: &Shot, t: f64) {
        let f = clamp(sh.age / sh.dur, 0.0, 1.0);
        let (x, y) = sh.pos(f);
        let z = sh.z;
        match sh.kind {
            PEA => {
                let (bx, by) = sh.pos((f - 0.06).max(0.0));
                cr.set_source_rgba(0.56, 0.82, 0.26, 0.35);
                cr.set_line_width(0.10 * z);
                cr.set_line_cap(LineCap::Round);
                cr.move_to(bx, by);
                cr.line_to(x, y);
                cr.stroke();
                cr.set_source_rgb(0.50, 0.76, 0.22);
                cr.circle(x, y, 0.085 * z);
                cr.fill();
                cr.set_source_rgba(1.0, 1.0, 0.8, 0.7);
                cr.circle(x - 0.025 * z, y - 0.03 * z, 0.028 * z);
                cr.fill();
            }
            BAND => {
                cr.save();
                cr.translate(x, y);
                cr.rotate(sh.age * 22.0);
                cr.set_source_rgb(0.98, 0.55, 0.66);
                cr.set_line_width(0.035 * z);
                cr.ellipse(0.0, 0.0, 0.14 * z, 0.06 * z);
                cr.stroke();
                cr.restore();
            }
            CORK => {
                cr.save();
                cr.translate(x, y);
                cr.rotate(sh.age * 10.0);
                cr.set_source_rgb(0.84, 0.66, 0.42);
                cr.rounded_rect(-0.085 * z, -0.06 * z, 0.17 * z, 0.12 * z, 0.03 * z);
                cr.fill();
                cr.set_source_rgba(0.55, 0.38, 0.20, 0.8);
                cr.circle(-0.03 * z, 0.0, 0.016 * z);
                cr.circle(0.04 * z, -0.02 * z, 0.014 * z);
                cr.fill();
                cr.restore();
            }
            PIPETTE => {
                // a string of drops
                cr.set_source_rgba(0.60, 0.82, 1.0, 0.9);
                for i in 0..6 {
                    let fi = f - i as f64 * 0.04;
                    if fi > 0.0 {
                        let (dx, dy) = sh.pos(fi);
                        cr.circle(dx, dy, (0.055 - 0.006 * i as f64) * z);
                    }
                }
                cr.fill();
                cr.set_source_rgba(1.0, 1.0, 1.0, 0.8);
                cr.circle(x - 0.015 * z, y - 0.02 * z, 0.018 * z);
                cr.fill();
            }
            SPOON => {
                let wob = (sh.age * 16.0).sin() * 0.02;
                cr.set_source_rgb(1.0, 0.99, 0.95);
                cr.ellipse(x, y, (0.12 + wob) * z, (0.11 - wob) * z);
                for i in 1..3 {
                    let (dx, dy) = sh.pos((f - i as f64 * 0.035).max(0.0));
                    cr.circle(dx, dy, (0.06 - 0.018 * i as f64) * z);
                }
                cr.fill();
                cr.set_source_rgba(0.80, 0.80, 0.86, 0.5);
                cr.ellipse(x + 0.03 * z, y + 0.04 * z, 0.06 * z, 0.035 * z);
                cr.fill();
            }
            PEPPER => {
                cr.set_source_rgba(0.50, 0.40, 0.30, 0.42);
                cr.circle(x, y, 0.21 * z);
                cr.circle(x - 0.15 * z, y + 0.04 * z, 0.14 * z);
                cr.circle(x + 0.12 * z, y - 0.06 * z, 0.12 * z);
                cr.fill();
                cr.set_source_rgba(0.16, 0.12, 0.09, 0.95);
                for i in 0..7 {
                    let ang = t * 7.0 + i as f64 * 0.9;
                    let rr = (0.05 + 0.02 * (i % 3) as f64) * z * 2.0;
                    cr.circle(x + ang.cos() * rr, y + (ang * 1.3).sin() * rr * 0.7, 0.016 * z);
                }
                cr.fill();
            }
            _ => bubble(cr, x, y + (sh.age * 5.0).sin() * 0.07 * z, (0.20 + 0.012 * (sh.age * 9.0).sin()) * z, 1.0),
        }
    }

    /// What an impact leaves lying on the floor.
    fn draw_stain(&self, cr: &mut Canvas, i: &Impact, t: f64) {
        let u = t - i.t0;
        let fade = clamp((3.0 - u) / 1.5, 0.0, 1.0);
        let grow = smooth(u * 5.0);
        match i.kind {
            PIPETTE => {
                cr.set_source_rgba(0.50, 0.72, 0.95, 0.34 * fade);
                cr.ellipse(i.x, i.gy, 0.62 * i.z * grow, 0.12 * i.z * grow);
                cr.fill();
            }
            SPOON => {
                cr.set_source_rgba(1.0, 0.99, 0.95, 0.85 * fade);
                cr.ellipse(i.x, i.gy, 0.34 * i.z * grow, 0.08 * i.z * grow);
                cr.ellipse(i.x - 0.30 * i.z, i.gy + 0.03 * i.z, 0.13 * i.z * grow, 0.04 * i.z * grow);
                cr.ellipse(i.x + 0.32 * i.z, i.gy - 0.02 * i.z, 0.11 * i.z * grow, 0.035 * i.z * grow);
                cr.fill();
            }
            _ => {}
        }
    }

    /// The moment of an impact: a flash, the "pop" of the cork, the ring of a burst bubble.
    fn draw_flash(&self, cr: &mut Canvas, i: &Impact, t: f64) {
        let u = t - i.t0;
        if u > 0.45 {
            return;
        }
        let z = i.z;
        if u < 0.16 && i.kind != PEPPER {
            cr.set_source_rgba(1.0, 1.0, 0.95, 0.7 * (1.0 - u / 0.16));
            cr.circle(i.x, i.y, z * (0.12 + u * 1.6));
            cr.fill();
        }
        match i.kind {
            CORK => {
                // "pop": a spiky burst
                let f = u / 0.45;
                let rad = z * (0.25 + 0.45 * smooth(f * 2.0));
                for k in 0..16 {
                    let ang = k as f64 * TAU / 16.0 + 0.2;
                    let rr = if k % 2 == 0 { rad } else { rad * 0.55 };
                    cr.line_to(i.x + ang.cos() * rr, i.y + ang.sin() * rr);
                }
                cr.close_path();
                cr.set_source_rgba(1.0, 0.93, 0.50, 0.85 * (1.0 - f));
                cr.fill();
            }
            BUBBLE => {
                let f = u / 0.45;
                cr.set_source_rgba(0.86, 0.97, 1.0, 0.8 * (1.0 - f));
                cr.set_line_width(0.03 * z);
                cr.circle(i.x, i.y, z * (0.22 + 0.5 * f));
                cr.stroke();
            }
            _ => {}
        }
    }

    /// The pipe drips into the bottle cap.
    fn draw_drip(&self, cr: &mut Canvas, s: f64, t: f64) {
        let (x, y0) = self.geo.drip;
        let y1 = self.geo.pool.1;
        let f = (t / 3.4).fract();
        cr.set_source_rgba(0.62, 0.80, 0.92, 0.85);
        if f < 0.55 {
            let g = f / 0.55;
            cr.ellipse(x, y0 + 3.0 * s * g, 3.2 * s * g, 5.0 * s * g);
            cr.fill();
        } else if f < 0.78 {
            let g = (f - 0.55) / 0.23;
            cr.ellipse(x, lerp(y0, y1, g * g), 3.0 * s, 6.0 * s);
            cr.fill();
        } else {
            let g = (f - 0.78) / 0.22;
            cr.set_source_rgba(0.80, 0.92, 1.0, 0.6 * (1.0 - g));
            cr.set_line_width(1.6 * s);
            cr.ellipse(x, y1, (8.0 + 40.0 * g) * s, (1.5 + 6.0 * g) * s);
            cr.stroke();
        }
    }

    /// A spider lets itself down from the cupboard now and then, has a look and climbs up again.
    fn draw_spider(&self, cr: &mut Canvas, s: f64, t: f64) {
        let f = (t / 37.0 + 0.72).fract();
        if f > 0.34 {
            return;
        }
        let len = smooth((f / 0.34 * PI).sin() * 1.7) * 290.0 * s + (t * 2.1).sin() * 4.0 * s;
        let (ax, ay) = (self.geo.spider, self.geo.beam);
        let (x, y) = (ax + (t * 0.9).sin() * 5.0 * s, ay + len);
        cr.set_source_rgba(0.86, 0.80, 0.68, 0.45);
        cr.set_line_width(1.2 * s);
        cr.move_to(ax, ay);
        cr.line_to(x, y);
        cr.stroke();
        cr.set_source_rgb(0.07, 0.045, 0.035);
        cr.set_line_width(1.9 * s);
        cr.set_line_cap(LineCap::Round);
        cr.set_line_join(LineJoin::Round);
        for side in [-1.0, 1.0] {
            for k in 0..4 {
                let kf = k as f64;
                let wig = (t * 5.0 + kf * 1.3 + side).sin() * 1.6 * s;
                cr.move_to(x, y + 2.0 * s);
                cr.line_to(x + side * (12.0 + kf) * s, y + (kf * 5.0 - 9.0) * s + wig);
                cr.line_to(x + side * (17.0 + kf * 1.5) * s, y + (kf * 7.0 - 3.0) * s + wig);
            }
        }
        cr.stroke();
        cr.ellipse(x, y + 6.0 * s, 8.5 * s, 10.0 * s);
        cr.circle(x, y - 5.0 * s, 5.5 * s);
        cr.fill();
        cr.set_source_rgba(1.0, 0.95, 0.8, 0.95);
        cr.circle(x - 2.2 * s, y - 6.0 * s, 1.5 * s);
        cr.circle(x + 2.2 * s, y - 6.0 * s, 1.5 * s);
        cr.fill();
    }

    /// A moth fluttering about the light.
    fn draw_moth(&self, cr: &mut Canvas, s: f64, t: f64) {
        let (hx, hy) = (self.geo.hole.0, self.geo.hole.1 - self.geo.hole_h * 0.75);
        let x = hx + (t * 0.83).sin() * 150.0 * s + (t * 2.9).sin() * 26.0 * s;
        let y = hy + (t * 1.17).cos() * 70.0 * s + (t * 3.7).sin() * 18.0 * s - 40.0 * s;
        let flap = (t * 26.0).sin().abs();
        cr.set_source_rgba(0.78, 0.70, 0.56, 0.95);
        for side in [-1.0, 1.0] {
            cr.move_to(x, y);
            cr.line_to(x + side * 13.0 * s * (0.3 + 0.7 * flap), y - 12.0 * s);
            cr.line_to(x + side * 15.0 * s * (0.3 + 0.7 * flap), y + 5.0 * s);
            cr.close_path();
        }
        cr.fill();
        cr.set_source_rgb(0.30, 0.22, 0.15);
        cr.ellipse(x, y, 2.4 * s, 7.0 * s);
        cr.fill();
    }
}

impl Theme for Cockroaches {
    type Actor = Roach;
    const NAME: &'static str = "cockroaches";
    const LABEL_BG: Rgb = (0.07, 0.04, 0.02);
    const EMPTY_COLOR: Rgb = (1.0, 0.93, 0.78);
    const WAITING_ROW: f64 = 0.78;

    fn new(_env: &Env) -> Self {
        Cockroaches {
            bg: None,
            epoch: 0,
            geo: Geo::default(),
            foods: Vec::new(),
            next_food: 0.0,
            food_id: 0,
            mounds: Vec::new(),
            ridges: Vec::new(),
            shots: Vec::new(),
            impacts: Vec::new(),
            bits: Vec::new(),
            marks: Vec::new(),
            motes: Vec::new(),
        }
    }

    fn spawn_point(&self, env: &Env) -> (f64, f64) {
        (env.rng.uniform(env.w * 0.2, env.w * 0.8), env.rng.uniform(env.ground_top + 40.0, env.h * 0.85))
    }

    fn on_tool(&mut self, env: &mut Env, actor: &mut Roach, name: &str, index: usize) {
        let t = env.t;
        let calm = actor.mode == Mode::Ground && actor.hit < 0.0 && actor.aim_t < 0.0;
        if calm && t >= actor.next_shot && actor.b.state() == State::Working {
            actor.pending = Some(weapon_of(name));
        } else if calm && !actor.at_table {
            actor.b.hop = 1.0;
        }
        let b = &actor.b;
        env.spawn_floater(b.x, b.y - b.radius * 1.15 - index as f64 * 26.0 * env.s, name, b.color, FloatStyle::Text, 40.0, 3.5);
    }

    fn setup_world(&mut self, env: &mut Env) {
        let (w, h, s) = (env.w, env.h, env.s);
        let gt = h * 0.60;
        env.ground_top = gt;
        self.epoch += 1; // the roaches notice and come out of their tunnels
        let beam = 46.0 * s;
        let pool = (w * 0.245, gt - 14.0 * s);
        self.geo = Geo { hole: (w * 0.395, gt), hole_w: 230.0 * s, hole_h: 236.0 * s, beam, drip: (pool.0, beam + 210.0 * s), pool, spider: w * 0.705 };
        let geo = self.geo;
        let env: &Env = env;
        self.bg = Some(Picture::record(w, h, |cr| paint_room(cr, env, &geo)));
        self.mounds.clear();
        self.ridges.clear();
        self.shots.clear();
        self.impacts.clear();
        self.bits.clear();
        self.marks.clear();
        self.foods.clear();
        for _ in 0..FOOD_N {
            self.drop_food(env, true);
        }
        self.next_food = env.t + 6.0;
        self.motes = (0..26).map(|_| (env.rng.random(), env.rng.random(), env.rng.uniform(0.0, TAU), env.rng.uniform(0.4, 1.0))).collect();
    }

    fn update_world(&mut self, env: &mut Env, actors: &mut [Roach], dt: f64) {
        let (s, t) = (env.s, env.t);
        let env: &Env = env;
        // who can be shot at: working and idle roaches above ground
        self.marks.clear();
        for a in actors.iter() {
            let b = &a.b;
            let ok = matches!(b.state(), State::Working | State::Idle) && !b.dying && b.alpha > 0.6 && a.mode == Mode::Ground;
            self.marks.push(Mark { index: b.index, x: b.x, y: b.y, ok });
        }
        // projectiles lean a little towards a moving target
        for sh in &mut self.shots {
            sh.age += dt;
            if let Some(m) = self.marks.iter().find(|m| m.index == sh.target && m.ok) {
                let k = (dt * 5.0).min(1.0);
                sh.tx = lerp(sh.tx, m.x, k);
                sh.ty = lerp(sh.ty, m.y, k);
            }
        }
        let mut i = 0;
        while i < self.shots.len() {
            if self.shots[i].age >= self.shots[i].dur {
                let sh = self.shots.swap_remove(i);
                self.land(env, &sh);
            } else {
                i += 1;
            }
        }
        self.impacts.retain(|i| t - i.t0 < 3.0);
        let g = 900.0 * s;
        for p in &mut self.bits {
            p.age += dt;
            if p.lying {
                continue;
            }
            p.vy += g * p.grav * dt;
            if p.grav < 0.5 {
                p.vx *= 1.0 - (dt * 2.5).min(1.0);
                p.vy *= 1.0 - (dt * 2.5).min(1.0);
            }
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            if p.vy > 0.0 && p.y >= p.floor {
                p.y = p.floor;
                p.lying = true;
            }
        }
        self.bits.retain(|p| p.age < p.life);
        self.mounds.retain(|m| t - m.born < MOUND_LIFE);
        self.ridges.retain(|r| t - r.3 < RIDGE_LIFE);
        // food: dust where a new piece lands, a new piece some time after one is gone
        let mut dust: Vec<(f64, f64, f64)> = Vec::new();
        for f in &mut self.foods {
            if !f.landed && t - f.born >= DROP_T {
                f.landed = true;
                dust.push((f.x, f.y, f.k));
            }
        }
        for (x, y, k) in dust {
            self.burst(env, 9, (x, y - 4.0 * k), (60.0, 190.0), (0.62, 0.50, 0.36), 12.0 * k, 0.0, 1, (0.5, 0.9), 1e9);
            self.burst(env, 8, (x, y - 4.0 * k), (120.0, 300.0), EARTH_L, 3.2 * k, 1.0, 0, (0.8, 1.6), y + 6.0 * k);
        }
        let before = self.foods.len();
        self.foods.retain(|f| f.amount > 0.0);
        if self.foods.len() < before {
            self.next_food = t + env.rng.uniform(4.0, 8.0);
        }
        if self.foods.len() < FOOD_N && t >= self.next_food {
            self.drop_food(env, false);
            self.next_food = t + env.rng.uniform(5.0, 9.0);
        }
    }

    fn draw_world(&mut self, cr: &mut Canvas, env: &Env, actors: &[Roach], t: f64) {
        let (h, s, gt) = (env.h, env.s, env.ground_top);
        if let Some(bg) = &self.bg {
            cr.picture(bg, 0.0, 0.0, 1.0);
        }
        self.draw_drip(cr, s, t);
        self.draw_spider(cr, s, t);
        self.draw_moth(cr, s, t);
        // dust motes drifting in the light of the hole
        let (hx, hw, hh) = (self.geo.hole.0, self.geo.hole_w, self.geo.hole_h);
        for &(fx, fy, ph, sp) in &self.motes {
            let fy = (fy + t * 0.012 * sp).fract();
            let y = gt - hh * 1.1 + fy * (hh * 1.1 + (h - gt) * 0.55);
            let spread = hw * 0.7 + (y - gt).max(0.0) * 1.1 + 60.0 * s;
            let x = hx + (fx - 0.42) * 2.0 * spread + (t * 0.3 * sp + ph).sin() * 22.0 * s;
            let tw = 0.5 + 0.5 * (t * 1.3 * sp + ph).sin();
            cr.set_rgba(LIGHT, (0.12 + 0.5 * tw) * (PI * fy).sin());
            cr.circle(x, y, (1.2 + 1.4 * tw) * s);
            cr.fill();
        }
        // the trail of a tunnel: a low ridge that sinks back
        for &(x, y, k, born) in &self.ridges {
            let f = (t - born) / RIDGE_LIFE;
            let up = 1.0 - smooth(f * 1.2);
            cr.set_rgba(darken(EARTH, 0.25), 0.55 * (1.0 - f));
            cr.ellipse(x, y + 0.04 * k, 0.34 * k, 0.10 * k);
            cr.fill();
            cr.set_rgba(EARTH_L, 0.75 * (1.0 - f));
            cr.ellipse(x, y - 0.03 * k * up, 0.26 * k, (0.03 + 0.07 * up) * k);
            cr.fill();
        }
        for i in &self.impacts {
            self.draw_stain(cr, i, t);
        }
        // food, burrows and roaches, back to front; the ones that need you on top
        let mut items: Vec<(f64, u8, usize)> = Vec::with_capacity(self.foods.len() + self.mounds.len() * 2 + actors.len());
        for (i, f) in self.foods.iter().enumerate() {
            items.push((f.y, 0, i));
        }
        for (i, m) in self.mounds.iter().enumerate() {
            items.push((m.y - 3.0 * s, 1, i));
            items.push((m.y + 3.0 * s, 2, i));
        }
        for (i, a) in actors.iter().enumerate() {
            let b = &a.b;
            items.push((b.y + b.radius * 0.95 + if b.state() == State::Waiting { 1e6 } else { 0.0 }, 3, i));
        }
        items.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (_, what, i) in items {
            match what {
                0 => self.draw_food(cr, &self.foods[i], t),
                1 => self.draw_mound_back(cr, &self.mounds[i], t),
                2 => self.draw_mound_front(cr, &self.mounds[i], t),
                _ => {
                    actors[i].draw(cr, env, self, t);
                    actors[i].draw_kids(cr, t);
                }
            }
        }
        for sh in &self.shots {
            self.draw_shot(cr, sh, t);
        }
        for p in &self.bits {
            let f = p.age / p.life;
            match p.kind {
                1 => {
                    cr.set_rgba(p.c, 0.42 * (1.0 - f));
                    cr.circle(p.x, p.y, p.size * (0.6 + 1.6 * f));
                }
                2 => {
                    cr.set_rgba(p.c, 1.0 - f * f);
                    star(cr, p.x, p.y, p.size * (1.0 - 0.4 * f), p.age * 6.0);
                }
                _ => {
                    cr.set_rgba(p.c, clamp((1.0 - f) * 4.0, 0.0, 1.0));
                    cr.circle(p.x, p.y, p.size);
                }
            }
            cr.fill();
        }
        for i in &self.impacts {
            self.draw_flash(cr, i, t);
        }
    }
}
