//! Halloween theme: funny glowing skeletons in a moonlit graveyard.
//!
//! working = walks about with a loose, bouncy gait and breaks into a goofy dance now and then; every
//!           tool call is a little trick (the skull pops off and spins, it tips its skull like a hat,
//!           or it juggles a bone) with a burst of sparks and the tool name rising
//! idle    = strolls a little, then sits on the ground with its legs out, taps a foot, scratches its
//!           skull and yawns
//! waiting = comes to the front, waves both arms overhead with a lantern, jaw chattering, pulses
//! blocked = stands slumped, skull hanging to the side, arms dangling, a bone dropped; flickers
//! rest    = one skeleton asleep beside an open grave, hands folded on the ribs, "z z z"
//! The bones are a pale tint of the state colour with a glow in the full colour around them.
//! Sub-agents are small ghosts floating behind their skeleton.
use vivarium_core::prelude::*;

/// silhouettes against the night sky
const NIGHT: Rgb = (0.045, 0.03, 0.085);
/// eye sockets, nose, the gaps between the teeth
const SOCKET: Rgb = (0.07, 0.04, 0.13);
const STONE: Rgb = (0.36, 0.36, 0.52);
const PUMPKIN: Rgb = (0.95, 0.47, 0.08);
const CANDLE: Rgb = (1.0, 0.82, 0.32);
const GHOST: Rgb = (0.86, 0.84, 1.0);
const WEB: Rgb = (0.82, 0.82, 1.0);

/// length of one arm / leg bone in units of the radius
const ARM: f64 = 0.30;
const LEG: f64 = 0.38;

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

/// The joint of a limb from `a` to `b` with two bones of length `len`, bending to the side `bend`.
fn joint(a: P, b: P, len: f64, bend: f64) -> P {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let d = hypot(dx, dy).max(1e-6);
    let half = (d / 2.0).min(len);
    let off = (len * len - half * half).max(0.0).sqrt() * bend;
    ((a.0 + b.0) / 2.0 - dy / d * off, (a.1 + b.1) / 2.0 + dx / d * off)
}

/// Where the joints are, in units of the radius, seen from the front, feet at y = +0.95.
/// Index 0 is the arm/leg on the left of the picture, 1 the one on the right.
#[derive(Clone, Copy)]
struct Pose {
    hip: P,
    /// middle between the shoulders
    chest: P,
    /// centre of the skull
    head: P,
    /// rotation of the skull
    tilt: f64,
    /// half distance between the shoulders
    sw: f64,
    hands: [P; 2],
    feet: [P; 2],
    /// which way elbows and knees bend (-1..1)
    eb: [f64; 2],
    kb: [f64; 2],
    /// 0..1 open jaw
    jaw: f64,
}

impl Pose {
    fn mix(&self, o: &Pose, k: f64) -> Pose {
        let two = |a: [P; 2], b: [P; 2]| [mixp(a[0], b[0], k), mixp(a[1], b[1], k)];
        let twof = |a: [f64; 2], b: [f64; 2]| [lerp(a[0], b[0], k), lerp(a[1], b[1], k)];
        Pose {
            hip: mixp(self.hip, o.hip, k),
            chest: mixp(self.chest, o.chest, k),
            head: mixp(self.head, o.head, k),
            tilt: lerp(self.tilt, o.tilt, k),
            sw: lerp(self.sw, o.sw, k),
            hands: two(self.hands, o.hands),
            feet: two(self.feet, o.feet),
            eb: twof(self.eb, o.eb),
            kb: twof(self.kb, o.kb),
            jaw: lerp(self.jaw, o.jaw, k),
        }
    }

    /// Bouncy, loose-limbed walk towards `dir` (-1..1); standing when `mv` is 0.
    fn walk(step: f64, mv: f64, dir: f64, t: f64, seed: f64) -> Pose {
        let bounce = step.sin().abs() * 0.075 * mv;
        let breath = (t * 1.5 + seed).sin() * 0.008;
        let foot = |k: f64, side: f64| {
            let ph = step + k * PI;
            (side * 0.11 + dir * 0.25 * ph.sin() * mv, 0.95 - ph.cos().max(0.0) * 0.16 * mv)
        };
        let hand = |k: f64, side: f64| {
            let ph = step + k * PI + PI;
            let dangle = (step * 2.0 + k * 1.3).sin() * 0.035 * mv;
            (side * (0.30 + 0.04 * mv) + dir * 0.26 * ph.sin() * mv + dangle, 0.15 - 0.13 * ph.sin().abs() * mv)
        };
        Pose {
            hip: (0.0, 0.20 - bounce),
            chest: (dir * 0.04 * mv, -0.42 - bounce * 1.2 + breath),
            head: (dir * 0.07, -0.86 - bounce * 1.7 + breath),
            tilt: dir * 0.06 + step.sin() * 0.10 * mv,
            sw: 0.22,
            hands: [hand(0.0, -1.0), hand(1.0, 1.0)],
            feet: [foot(0.0, -1.0), foot(1.0, 1.0)],
            eb: [dir * 0.6 + 0.3, dir * 0.6 - 0.3],
            kb: [-dir * 0.9 + 0.12, -dir * 0.9 - 0.12],
            jaw: 0.0,
        }
    }

    /// A goofy shuffle on the spot: hips swaying, one arm pointing up and the other down in turn.
    fn dance(t: f64, seed: f64) -> Pose {
        let beat = t * 6.5 + seed;
        let (sb, cb, c2) = (beat.sin(), beat.cos(), (beat * 2.0).cos());
        let u = smooth(0.5 + 0.9 * (beat * 0.5).sin());
        Pose {
            hip: (0.08 * sb, 0.26 + 0.03 * c2),
            chest: (-0.03 * sb, -0.37 + 0.025 * c2),
            head: (0.07 * sb, -0.81 + 0.035 * c2),
            tilt: 0.22 * sb,
            sw: 0.22,
            hands: [(-0.42 - 0.06 * cb, lerp(-0.86, 0.04, u)), (0.42 + 0.06 * cb, lerp(0.04, -0.86, u))],
            feet: [(-0.17 + 0.05 * sb, 0.95 - sb.max(0.0) * 0.11), (0.17 + 0.05 * sb, 0.95 - (-sb).max(0.0) * 0.11)],
            eb: [0.8, -0.8],
            kb: [0.9, -0.9],
            jaw: 0.25 + 0.25 * c2,
        }
    }

    /// Sitting on the ground, legs out towards `d` (+1/-1), leaning back on one hand; the other hand
    /// rests on the knee, scratches the skull (`scratch`) or covers a yawn (`yawn`).
    fn sit(t: f64, seed: f64, d: f64, scratch: f64, yawn: f64) -> Pose {
        let breath = (t * 1.2 + seed).sin() * 0.01;
        let tap = (t * 5.0 + seed).sin().max(0.0) * 0.08;
        let head = (-0.19 * d, -0.24 + breath - 0.03 * yawn);
        let mut front = (0.14 * d, 0.55);
        front = mixp(front, (head.0 + (0.25 + 0.04 * (t * 13.0).sin()) * d, head.1 - 0.06), scratch);
        front = mixp(front, (head.0 + 0.20 * d, head.1 + 0.22), yawn);
        let back = (-0.52 * d, 0.80);
        let (hands, eb) = if d > 0.0 { ([back, front], [0.5, 1.0]) } else { ([front, back], [-1.0, -0.5]) };
        Pose {
            hip: (-0.12 * d, 0.78),
            chest: (-0.24 * d, 0.20 + breath),
            head,
            tilt: -0.10 * d - 0.25 * d * yawn + 0.06 * d * scratch,
            sw: 0.14,
            hands,
            feet: [(0.60 * d, 0.93 - tap), (0.36 * d, 0.95)],
            eb,
            kb: [-d, -d],
            jaw: yawn,
        }
    }

    /// Waiting: both arms waving overhead, hopping from foot to foot, rattling, jaw chattering.
    fn wave(t: f64, seed: f64) -> Pose {
        let j = (t * 31.0 + seed).sin() * 0.012;
        let sw = (t * 5.0 + seed).sin();
        let hop = sw.abs() * 0.04;
        Pose {
            hip: (j, 0.20 - hop),
            chest: (j * 1.5, -0.42 - hop),
            head: (j * 2.0, -0.86 - hop),
            tilt: (t * 9.0 + seed).sin() * 0.07,
            sw: 0.22,
            hands: [(-0.43 + 0.13 * (t * 7.0 + seed).sin(), -0.93 - hop), (0.46 + 0.08 * (t * 7.0 + seed + 1.3).sin(), -0.88 - hop)],
            feet: [(-0.13, 0.95 - sw.max(0.0) * 0.06), (0.13, 0.95 - (-sw).max(0.0) * 0.06)],
            eb: [0.5, -0.5],
            kb: [0.3, -0.3],
            jaw: 0.5 + 0.5 * (t * 24.0).sin(),
        }
    }

    /// Blocked: knock-kneed, the skull hanging far to the side, arms dangling.
    fn slump(t: f64, seed: f64) -> Pose {
        let sway = (t * 0.8 + seed).sin() * 0.02;
        Pose {
            hip: (0.0, 0.27),
            chest: (0.04 + sway, -0.31),
            head: (0.20 + sway * 2.0, -0.60),
            tilt: 0.62 + sway * 3.0,
            sw: 0.20,
            hands: [(-0.18 + sway * 2.5, 0.28), (0.27 + sway * 2.5, 0.27)],
            feet: [(-0.16, 0.95), (0.16, 0.95)],
            eb: [0.1, -0.1],
            kb: [-0.7, 0.7],
            jaw: 0.0,
        }
    }

    /// Asleep on the back (drawn turned by a quarter): hands folded on the ribs, snoring.
    fn lie(t: f64, seed: f64) -> Pose {
        let breath = (t * 1.3 + seed).sin();
        Pose {
            hip: (0.0, 0.20),
            chest: (0.0, -0.42),
            head: (0.0, -0.86),
            tilt: 0.12,
            sw: 0.22,
            hands: [(-0.05, -0.20 + 0.012 * breath), (0.05, -0.15 + 0.012 * breath)],
            feet: [(-0.10, 0.95), (0.12, 0.92)],
            eb: [1.0, -1.0],
            kb: [0.2, -0.2],
            jaw: 0.25 + 0.2 * breath,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mood {
    Grin,
    Sad,
    Asleep,
}

/// How a skeleton is drawn besides its pose.
struct Looks {
    c: Rgb,
    a: f64,
    /// -1..1: which way it is heading (feet and eyes turn that way)
    dir: f64,
    mood: Mood,
    /// the skull lifted off the neck (tool-call tricks), in units of the radius, and turned
    skull_off: P,
    skull_rot: f64,
}

/// The skull around (0, 0); `q` is the radius of the skeleton.
fn skull(cr: &mut Canvas, q: f64, lk: &Looks, jaw: f64) {
    let (c, a) = (lk.c, lk.a);
    let bone = lighten(c, 0.62);
    // halo in the state colour
    cr.set_rgba(c, 0.12 * a);
    cr.circle(0.0, 0.03 * q, 0.44 * q);
    cr.fill();
    cr.set_rgba(c, 0.22 * a);
    cr.circle(0.0, 0.03 * q, 0.37 * q);
    cr.fill();
    let jo = jaw * 0.16 * q;
    if jaw > 0.03 {
        cr.set_rgba(SOCKET, a);
        cr.rounded_rect(-0.15 * q, 0.16 * q, 0.30 * q, 0.14 * q + jo, 0.05 * q);
        cr.fill();
    }
    // lower jaw
    cr.set_rgba(lighten(c, 0.55), a);
    cr.rounded_rect(-0.165 * q, 0.22 * q + jo, 0.33 * q, 0.16 * q, 0.07 * q);
    cr.fill();
    // cranium and cheeks
    cr.set_rgba(bone, a);
    cr.circle(0.0, -0.02 * q, 0.30 * q);
    cr.rounded_rect(-0.19 * q, 0.02 * q, 0.38 * q, 0.245 * q, 0.08 * q);
    cr.fill();
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.30 * a); // moonlight on the dome
    cr.ellipse(-0.10 * q, -0.20 * q, 0.10 * q, 0.055 * q);
    cr.fill();
    // toothy grin (or a long face)
    let bend = if lk.mood == Mood::Sad { -0.05 } else { 0.05 };
    cr.set_rgba(SOCKET, a);
    cr.set_line_cap(LineCap::Round);
    cr.set_line_width(0.022 * q);
    cr.move_to(-0.175 * q, (0.25 - bend * 0.5) * q);
    cr.quad_to(0.0, (0.25 + bend * 1.5) * q, 0.175 * q, (0.25 - bend * 0.5) * q);
    for k in -2..=2 {
        let x = k as f64 * 0.058 * q;
        let dip = bend * (1.0 - (k as f64 / 3.0).powi(2)) * q * 0.5;
        cr.move_to(x, 0.205 * q + dip);
        cr.line_to(x, 0.295 * q + dip);
        if jaw > 0.2 {
            cr.move_to(x, 0.235 * q + jo);
            cr.line_to(x, 0.29 * q + jo);
        }
    }
    cr.stroke();
    // eye sockets with small glowing pupils
    let ex = lk.dir * 0.02 * q;
    if lk.mood == Mood::Asleep {
        cr.set_line_width(0.035 * q);
        for side in [-1.0, 1.0] {
            cr.new_sub_path();
            cr.arc(side * 0.12 * q, -0.06 * q, 0.07 * q, 0.25, PI - 0.25);
        }
        cr.stroke();
    } else {
        for side in [-1.0, 1.0] {
            cr.save();
            cr.translate(side * 0.12 * q + ex, -0.04 * q);
            cr.rotate(side * if lk.mood == Mood::Sad { 0.35 } else { -0.12 });
            cr.ellipse(0.0, 0.0, 0.088 * q, 0.105 * q);
            cr.restore();
        }
        cr.fill();
        let (px, py) = if lk.mood == Mood::Sad { (0.0, 0.045) } else { (lk.dir * 0.03, 0.01) };
        for side in [-1.0, 1.0] {
            let (x, y) = (side * 0.12 * q + ex + px * q, (-0.04 + py) * q);
            cr.set_rgba(c, 0.45 * a);
            cr.circle(x, y, 0.062 * q);
            cr.fill();
            cr.set_rgba(lighten(c, 0.6), a);
            cr.circle(x, y, 0.032 * q);
            cr.fill();
        }
    }
    // nose and a crack in the dome
    cr.set_rgba(SOCKET, a);
    cr.move_to(ex, 0.075 * q);
    cr.line_to(ex - 0.034 * q, 0.14 * q);
    cr.line_to(ex, 0.122 * q);
    cr.line_to(ex + 0.034 * q, 0.14 * q);
    cr.close_path();
    cr.fill();
    cr.set_rgba(SOCKET, 0.6 * a);
    cr.set_line_width(0.014 * q);
    cr.move_to(0.13 * q, -0.29 * q);
    cr.line_to(0.10 * q, -0.22 * q);
    cr.line_to(0.15 * q, -0.19 * q);
    cr.line_to(0.13 * q, -0.15 * q);
    cr.stroke();
}

/// The skeleton in local coordinates (feet at y = +0.95 R).
fn figure(cr: &mut Canvas, p: &Pose, r: f64, lk: &Looks) {
    let (c, a) = (lk.c, lk.a);
    if a < 0.01 {
        return;
    }
    let bone = lighten(c, 0.62);
    // the trunk: `u` runs down the spine, `v` across it
    let (ux, uy) = {
        let (dx, dy) = (p.hip.0 - p.chest.0, p.hip.1 - p.chest.1);
        let d = hypot(dx, dy).max(1e-6);
        (dx / d, dy / d)
    };
    let (vx, vy) = (uy, -ux);
    let at = |d: f64, side: f64| (p.chest.0 + ux * d + vx * side, p.chest.1 + uy * d + vy * side);
    let sh = [at(0.0, -p.sw), at(0.0, p.sw)];
    let hw = p.sw * 0.48;
    let hips = [(p.hip.0 - vx * hw, p.hip.1 - vy * hw), (p.hip.0 + vx * hw, p.hip.1 + vy * hw)];
    let el = [joint(sh[0], p.hands[0], ARM, p.eb[0]), joint(sh[1], p.hands[1], ARM, p.eb[1])];
    let kn = [joint(hips[0], p.feet[0], LEG, p.kb[0]), joint(hips[1], p.feet[1], LEG, p.kb[1])];
    let neck = mixp(p.chest, p.head, 0.55);
    let trunk = [(neck, p.hip), (sh[0], sh[1]), (hips[0], kn[0]), (kn[0], p.feet[0]), (hips[1], kn[1]), (kn[1], p.feet[1])];
    let arms = [(sh[0], el[0]), (el[0], p.hands[0]), (sh[1], el[1]), (el[1], p.hands[1])];
    let wide = p.sw / 0.22;
    let ribs: Vec<(P, P, P)> = (0..4)
        .flat_map(|i| {
            let d = 0.075 + 0.085 * i as f64;
            let wr = [0.19, 0.215, 0.20, 0.16][i] * wide;
            [-1.0, 1.0].map(|side| (at(d, 0.0), at(d - 0.04, side * wr * 1.08), at(d + 0.075, side * wr)))
        })
        .collect();
    let lines = |cr: &mut Canvas, segs: &[(P, P)]| {
        for (p0, p1) in segs {
            cr.move_to(p0.0 * r, p0.1 * r);
            cr.line_to(p1.0 * r, p1.1 * r);
        }
    };
    let curves = |cr: &mut Canvas| {
        for (p0, p1, p2) in &ribs {
            cr.move_to(p0.0 * r, p0.1 * r);
            cr.quad_to(p1.0 * r, p1.1 * r, p2.0 * r, p2.1 * r);
        }
    };
    cr.set_line_cap(LineCap::Round);
    cr.set_line_join(LineJoin::Round);
    // the glow: all bones as one wide translucent stroke in the state colour, twice
    for (wd, al) in [(0.26, 0.14), (0.15, 0.28)] {
        lines(cr, &trunk);
        lines(cr, &arms);
        curves(cr);
        cr.set_rgba(c, al * a);
        cr.set_line_width(r * wd);
        cr.stroke();
    }
    // ribs, spine, legs
    cr.set_rgba(bone, a);
    curves(cr);
    cr.set_line_width(r * 0.055);
    cr.stroke();
    lines(cr, &trunk);
    cr.set_line_width(r * 0.085);
    cr.stroke();
    for k in 0..2 {
        cr.circle(kn[k].0 * r, kn[k].1 * r, r * 0.06);
        let side = if k == 0 { -1.0 } else { 1.0 };
        let toe = lk.dir * 0.07 + side * 0.035 * (1.0 - lk.dir.abs());
        cr.ellipse((p.feet[k].0 + toe) * r, (p.feet[k].1 - 0.005) * r, r * 0.11, r * 0.048);
    }
    cr.ellipse(p.hip.0 * r, p.hip.1 * r, r * (0.03 + 0.125 * wide), r * 0.085);
    cr.fill();
    cr.set_rgba(SOCKET, a);
    for side in [-1.0, 1.0] {
        cr.ellipse((p.hip.0 + side * 0.065 * wide) * r, (p.hip.1 + 0.012) * r, r * 0.035 * wide, r * 0.038);
    }
    cr.fill();
    // skull (the tricks lift it off the neck)
    cr.save();
    cr.translate((p.head.0 + lk.skull_off.0) * r, (p.head.1 + lk.skull_off.1) * r);
    cr.rotate(p.tilt + lk.skull_rot);
    skull(cr, r, lk, p.jaw);
    cr.restore();
    // arms in front, knobs at the joints
    cr.set_line_cap(LineCap::Round);
    cr.set_rgba(bone, a);
    lines(cr, &arms);
    cr.set_line_width(r * 0.078);
    cr.stroke();
    for k in 0..2 {
        cr.circle(sh[k].0 * r, sh[k].1 * r, r * 0.062);
        cr.circle(el[k].0 * r, el[k].1 * r, r * 0.055);
        cr.circle(p.hands[k].0 * r, p.hands[k].1 * r, r * 0.072);
    }
    cr.fill();
}

/// A loose bone with two knobs at each end, centred on (x, y).
fn loose_bone(cr: &mut Canvas, x: f64, y: f64, len: f64, ang: f64, c: Rgb, a: f64) {
    let (dx, dy) = (ang.cos() * len / 2.0, ang.sin() * len / 2.0);
    let (nx, ny) = (-ang.sin() * len * 0.09, ang.cos() * len * 0.09);
    cr.set_line_cap(LineCap::Round);
    for (col, wd, al) in [(c, 0.38, 0.25), (lighten(c, 0.72), 0.17, 1.0)] {
        cr.set_rgba(col, al * a);
        cr.set_line_width(len * wd);
        cr.move_to(x - dx, y - dy);
        cr.line_to(x + dx, y + dy);
        cr.stroke();
    }
    for e in [-1.0, 1.0] {
        for n in [-1.0, 1.0] {
            cr.circle(x + e * dx + n * nx, y + e * dy + n * ny, len * 0.105);
        }
    }
    cr.fill();
}

/// Small lantern hanging from (0, 0); its light is in the state colour.
fn lantern(cr: &mut Canvas, r: f64, c: Rgb, a: f64, flick: f64) {
    cr.glow(0.0, r * 0.20, r * 0.62, c, (0.34 + 0.12 * flick) * a, 0.0);
    cr.set_rgba(NIGHT, a);
    cr.set_line_width(r * 0.022);
    cr.new_sub_path();
    cr.arc(0.0, r * 0.08, r * 0.075, PI, TAU);
    cr.stroke();
    cr.rounded_rect(-r * 0.09, r * 0.08, r * 0.18, r * 0.24, r * 0.03);
    cr.fill();
    cr.set_rgba(lighten(c, 0.35 + 0.3 * flick), a);
    cr.rounded_rect(-r * 0.062, r * 0.115, r * 0.124, r * 0.17, r * 0.02);
    cr.fill();
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.85 * a);
    cr.ellipse(0.0, r * 0.215, r * 0.022, r * 0.045);
    cr.fill();
    cr.set_rgba(NIGHT, a);
    cr.move_to(0.0, r * 0.115);
    cr.line_to(0.0, r * 0.14);
    cr.rectangle(-r * 0.10, r * 0.30, r * 0.20, r * 0.03);
    cr.fill();
}

/// A little ghost around (x, y), `g` wide to each side, looking towards `look` (-1..1).
#[allow(clippy::too_many_arguments)]
fn ghost(cr: &mut Canvas, x: f64, y: f64, g: f64, c: Rgb, a: f64, t: f64, look: f64) {
    cr.set_rgba(c, 0.13 * a);
    cr.circle(x, y, g * 1.75);
    cr.fill();
    cr.set_rgba(c, 0.16 * a);
    cr.circle(x, y, g * 1.35);
    cr.fill();
    // dome and a wavy hem
    let hem = y + g * 0.95;
    cr.move_to(x - g, hem);
    cr.line_to(x - g, y - g * 0.15);
    cr.arc(x, y - g * 0.15, g, PI, TAU);
    cr.line_to(x + g, hem);
    for k in 0..4 {
        let x0 = x + g - (k as f64 + 0.5) * g * 0.5;
        let wave = (t * 4.0 + k as f64 * 1.6).sin() * g * 0.06;
        let dip = if k % 2 == 0 { -0.24 } else { 0.20 };
        cr.quad_to(x0 + wave, hem + dip * g, x0 - g * 0.25, hem);
    }
    cr.close_path();
    cr.set_rgba(lighten(c, 0.62), 0.92 * a);
    cr.fill();
    // stubby arms
    cr.ellipse(x - g * 1.0, y + g * 0.18, g * 0.24, g * 0.15);
    cr.ellipse(x + g * 1.0, y + g * 0.18, g * 0.24, g * 0.15);
    cr.fill();
    // face
    let fx = x + look * g * 0.14;
    cr.set_rgba(SOCKET, 0.9 * a);
    cr.ellipse(fx - g * 0.33, y - g * 0.28, g * 0.15, g * 0.21);
    cr.ellipse(fx + g * 0.33, y - g * 0.28, g * 0.15, g * 0.21);
    cr.ellipse(fx, y + g * 0.14, g * 0.13, g * (0.12 + 0.05 * (t * 2.0).sin()));
    cr.fill();
    cr.set_source_rgba(1.0, 0.6, 0.7, 0.4 * a);
    cr.circle(fx - g * 0.58, y - g * 0.02, g * 0.11);
    cr.circle(fx + g * 0.58, y - g * 0.02, g * 0.11);
    cr.fill();
}

/// A tombstone standing on (x, y) (bottom centre); `k` = pixels per unit (about 85 units high).
/// Shapes: 0 round top, 1 cross, 2 tilted with a crack, 3 big one with "RIP", 4 pointed.
fn tombstone(cr: &mut Canvas, x: f64, y: f64, k: f64, shape: usize, a: f64) {
    cr.set_source_rgba(0.0, 0.0, 0.02, 0.32 * a);
    cr.ellipse(x + 10.0 * k, y + 1.0 * k, 44.0 * k, 7.0 * k);
    cr.fill();
    cr.save();
    cr.translate(x, y);
    if shape == 2 {
        cr.rotate(0.17);
    }
    let (hw, top) = match shape {
        1 => (32.0, 98.0),
        2 => (26.0, 72.0),
        3 => (37.0, 94.0),
        4 => (22.0, 100.0),
        _ => (29.0, 82.0),
    };
    let outline = |cr: &mut Canvas| match shape {
        1 => {
            for (px, py) in [(-10.0, 0.0), (-10.0, 56.0), (-32.0, 56.0), (-32.0, 74.0), (-10.0, 74.0), (-10.0, 98.0), (10.0, 98.0), (10.0, 74.0), (32.0, 74.0), (32.0, 56.0), (10.0, 56.0), (10.0, 0.0)] {
                cr.line_to(px * k, -py * k);
            }
            cr.close_path();
            cr.rectangle(-21.0 * k, -11.0 * k, 42.0 * k, 11.0 * k);
        }
        4 => {
            for (px, py) in [(-22.0, 0.0), (-18.0, 80.0), (0.0, 100.0), (18.0, 80.0), (22.0, 0.0)] {
                cr.line_to(px * k, -py * k);
            }
            cr.close_path();
        }
        _ => {
            cr.move_to(-hw * k, 0.0);
            cr.line_to(-hw * k, -(top - hw) * k);
            cr.arc(0.0, -(top - hw) * k, hw * k, PI, TAU);
            cr.line_to(hw * k, 0.0);
            cr.close_path();
        }
    };
    // moonlit from the left
    let mut g = Gradient::linear(-hw * k, 0.0, hw * k, 0.0);
    g.stop(0.0, lighten(STONE, 0.28), a);
    g.stop(0.35, STONE, a);
    g.stop(1.0, darken(STONE, 0.45), a);
    cr.set_source(&g);
    outline(cr);
    cr.fill();
    cr.set_rgba(lighten(STONE, 0.5), 0.5 * a);
    cr.set_line_width(1.6 * k);
    cr.set_line_join(LineJoin::Round);
    outline(cr);
    cr.stroke();
    // inscription, crack, moss
    let ink = darken(STONE, 0.62);
    cr.set_line_cap(LineCap::Round);
    match shape {
        3 => {
            let lay = text::layout("RIP", 23.0 * k, weight::HEAVY, None);
            let (tw, _) = lay.get_pixel_size();
            text::show(cr, &lay, -tw / 2.0, -72.0 * k, ink, 0.85 * a, 0.0);
            cr.set_rgba(ink, 0.6 * a);
            cr.set_line_width(2.4 * k);
            for (yy, half) in [(-36.0, 20.0), (-26.0, 14.0)] {
                cr.move_to(-half * k, yy * k);
                cr.line_to(half * k, yy * k);
            }
            cr.stroke();
        }
        1 => {}
        _ => {
            cr.set_rgba(ink, 0.6 * a);
            cr.set_line_width(2.2 * k);
            for (yy, half) in [(-54.0, 13.0), (-44.0, 16.0), (-34.0, 10.0)] {
                cr.move_to(-half * k, yy * k);
                cr.line_to(half * k, yy * k);
            }
            if shape == 2 {
                cr.move_to(6.0 * k, -72.0 * k);
                cr.line_to(0.0, -62.0 * k);
                cr.line_to(9.0 * k, -56.0 * k);
            }
            cr.stroke();
        }
    }
    cr.set_source_rgba(0.20, 0.36, 0.26, 0.55 * a);
    cr.ellipse(-hw * 0.45 * k, -5.0 * k, hw * 0.5 * k, 6.0 * k);
    cr.ellipse(hw * 0.5 * k, -3.0 * k, hw * 0.3 * k, 4.0 * k);
    cr.fill();
    cr.restore();
    // grass at the foot
    cr.set_source_rgba(0.13, 0.20, 0.24, a);
    cr.set_line_width(2.0 * k);
    for i in 0..9 {
        let f = i as f64 / 8.0 - 0.5;
        let bx = x + f * hw * 2.6 * k;
        cr.move_to(bx, y + 3.0 * k);
        cr.line_to(bx + (f * 9.0 + (i as f64 * 2.3).sin() * 4.0) * k, y - (9.0 + (i * 5 % 7) as f64) * k);
    }
    cr.stroke();
}

/// A jack-o'-lantern standing on (x, y) (bottom centre), about 76 units wide and 60 high.
/// `face`: 0 jagged grin, 1 surprised, 2 goofy with one tooth.
fn pumpkin(cr: &mut Canvas, x: f64, y: f64, k: f64, face: usize, a: f64) {
    let cy = y - 27.0 * k;
    cr.set_source_rgba(0.0, 0.0, 0.02, 0.35 * a);
    cr.ellipse(x + 5.0 * k, y, 42.0 * k, 7.0 * k);
    cr.fill();
    // stem
    cr.set_source_rgba(0.26, 0.34, 0.14, a);
    cr.set_line_width(7.0 * k);
    cr.set_line_cap(LineCap::Round);
    cr.move_to(x, cy - 22.0 * k);
    cr.quad_to(x + 1.0 * k, cy - 34.0 * k, x + 9.0 * k, cy - 37.0 * k);
    cr.stroke();
    // body out of three lobes
    cr.set_rgba(darken(PUMPKIN, 0.22), a);
    cr.ellipse(x - 19.0 * k, cy, 19.0 * k, 25.0 * k);
    cr.ellipse(x + 19.0 * k, cy, 19.0 * k, 25.0 * k);
    cr.fill();
    cr.set_rgba(PUMPKIN, a);
    cr.ellipse(x - 9.0 * k, cy, 17.0 * k, 26.5 * k);
    cr.ellipse(x + 9.0 * k, cy, 17.0 * k, 26.5 * k);
    cr.fill();
    cr.set_rgba(lighten(PUMPKIN, 0.16), a);
    cr.ellipse(x, cy, 13.0 * k, 27.0 * k);
    cr.fill();
    cr.set_source_rgba(1.0, 0.85, 0.6, 0.22 * a); // moonlight
    cr.ellipse(x - 20.0 * k, cy - 11.0 * k, 6.0 * k, 10.0 * k);
    cr.fill();
    // carved face, lit from inside
    cr.set_rgba(CANDLE, a);
    let tri = |cr: &mut Canvas, tx: f64, ty: f64, sz: f64| {
        cr.move_to(tx - sz, ty + sz * 0.7);
        cr.line_to(tx, ty - sz * 0.9);
        cr.line_to(tx + sz, ty + sz * 0.7);
        cr.close_path();
    };
    match face {
        1 => {
            cr.circle(x - 13.0 * k, cy - 7.0 * k, 6.5 * k);
            cr.circle(x + 13.0 * k, cy - 7.0 * k, 6.5 * k);
            cr.ellipse(x, cy + 12.0 * k, 8.0 * k, 9.0 * k);
        }
        2 => {
            cr.circle(x - 13.0 * k, cy - 8.0 * k, 8.0 * k);
            tri(cr, x + 14.0 * k, cy - 7.0 * k, 6.0 * k);
            cr.move_to(x - 23.0 * k, cy + 5.0 * k);
            cr.quad_to(x, cy + 30.0 * k, x + 23.0 * k, cy + 5.0 * k);
            cr.quad_to(x, cy + 14.0 * k, x - 23.0 * k, cy + 5.0 * k);
            cr.close_path();
        }
        _ => {
            tri(cr, x - 14.0 * k, cy - 7.0 * k, 7.5 * k);
            tri(cr, x + 14.0 * k, cy - 7.0 * k, 7.5 * k);
            cr.move_to(x - 24.0 * k, cy + 4.0 * k);
            cr.quad_to(x, cy + 30.0 * k, x + 24.0 * k, cy + 4.0 * k);
            for (px, py) in [(16.0, 11.0), (11.0, 6.0), (5.5, 12.0), (0.0, 7.0), (-5.5, 12.0), (-11.0, 6.0), (-16.0, 11.0)] {
                cr.line_to(x + px * k, cy + py * k);
            }
            cr.close_path();
        }
    }
    cr.fill();
    tri(cr, x, cy + 2.0 * k, 3.5 * k);
    cr.fill();
    if face == 2 {
        cr.set_rgba(lighten(PUMPKIN, 0.16), a); // the one tooth
        cr.rectangle(x - 9.0 * k, cy + 9.0 * k, 8.0 * k, 8.0 * k);
        cr.fill();
    }
}

/// A piece of spider web: spokes from (x, y) between the angles a0 and a1, joined by sagging threads.
#[allow(clippy::too_many_arguments)]
fn web(cr: &mut Canvas, x: f64, y: f64, a0: f64, a1: f64, rad: f64, spokes: usize, rings: usize, lw: f64, alpha: f64) {
    let ang = |i: usize| lerp(a0, a1, i as f64 / (spokes - 1).max(1) as f64);
    let len = |i: usize| rad * (1.0 + 0.10 * (i as f64 * 2.4).sin());
    for i in 0..spokes {
        cr.move_to(x, y);
        cr.line_to(x + ang(i).cos() * len(i), y + ang(i).sin() * len(i));
    }
    for j in 1..=rings {
        let f = j as f64 / rings as f64 * 0.94;
        for i in 0..spokes - 1 {
            let (r0, r1) = (len(i) * f, len(i + 1) * f);
            let am = (ang(i) + ang(i + 1)) / 2.0;
            let rm = (r0 + r1) / 2.0 * 0.86;
            cr.move_to(x + ang(i).cos() * r0, y + ang(i).sin() * r0);
            cr.quad_to(x + am.cos() * rm, y + am.sin() * rm, x + ang(i + 1).cos() * r1, y + ang(i + 1).sin() * r1);
        }
    }
    cr.set_rgba(WEB, alpha);
    cr.set_line_width(lw);
    cr.stroke();
}

/// One branch of a bare tree and everything growing out of it. Forks worth a web are noted in
/// `forks` as (x, y, angle 0, angle 1, length).
#[allow(clippy::too_many_arguments)]
fn branch(cr: &mut Canvas, rng: &Rng, x: f64, y: f64, ang: f64, len: f64, wid: f64, depth: usize, forks: &mut Vec<(f64, f64, f64, f64, f64)>) {
    let (ex, ey) = (x + ang.cos() * len, y + ang.sin() * len);
    let bend = rng.uniform(-0.22, 0.22) * len;
    cr.move_to(x, y);
    cr.quad_to((x + ex) / 2.0 - ang.sin() * bend, (y + ey) / 2.0 + ang.cos() * bend, ex, ey);
    cr.set_line_width(wid);
    cr.stroke();
    if depth == 0 {
        return;
    }
    let (a0, a1) = (ang - rng.uniform(0.32, 0.70), ang + rng.uniform(0.32, 0.70));
    let l = len * rng.uniform(0.66, 0.80);
    if depth >= 3 {
        forks.push((ex, ey, a0, a1, l));
    }
    // gnarled: the twigs bend back towards the horizontal
    let gnarl = |a: f64| a + (a + PI / 2.0).sin() * 0.16;
    branch(cr, rng, ex, ey, gnarl(a0), l, wid * 0.64, depth - 1, forks);
    branch(cr, rng, ex, ey, gnarl(a1), l * rng.uniform(0.8, 1.0), wid * 0.60, depth - 1, forks);
    if depth >= 2 && rng.chance(0.3) {
        branch(cr, rng, ex, ey, ang + rng.uniform(-0.15, 0.15), l * 0.7, wid * 0.45, depth - 1, forks);
    }
}

/// A bare gnarled tree `height` high standing on (x, y), with webs in two forks.
fn bare_tree(cr: &mut Canvas, rng: &Rng, x: f64, y: f64, height: f64, lean: f64, s: f64) {
    let wid = height * 0.075;
    cr.set_rgb(NIGHT);
    cr.set_line_cap(LineCap::Round);
    // roots
    cr.move_to(x - wid * 1.5, y + wid * 0.2);
    cr.quad_to(x - wid * 0.5, y - wid * 0.2, x - wid * 0.45, y - wid * 1.6);
    cr.line_to(x + wid * 0.45, y - wid * 1.6);
    cr.quad_to(x + wid * 0.5, y - wid * 0.2, x + wid * 1.6, y + wid * 0.2);
    cr.close_path();
    cr.fill();
    let mut forks = Vec::new();
    branch(cr, rng, x, y, -PI / 2.0 + lean, height / 2.7, wid, 5, &mut forks);
    // a knot hole and a little moonlight on the trunk
    cr.set_source_rgba(0.30, 0.26, 0.48, 0.35);
    cr.set_line_width(wid * 0.12);
    cr.move_to(x - wid * 0.36, y - wid * 0.6);
    cr.line_to(x - wid * 0.36 + lean * height * 0.2, y - height * 0.26);
    cr.stroke();
    forks.sort_by(|a, b| b.4.total_cmp(&a.4));
    for &(fx, fy, a0, a1, l) in forks.iter().take(2) {
        web(cr, fx, fy, a0, a1, l * 0.62, 4, 4, 1.1 * s, 0.38);
    }
}

/// Where the fixed things are (all in pixels).
#[derive(Clone, Copy, Default)]
struct Geo {
    /// moon: centre and radius
    moon: (f64, f64, f64),
    /// the gate in the fence, where the path begins
    gate: f64,
    /// bottom centre of the haunted house
    house: (f64, f64),
    /// where the spider's thread hangs from the big web
    thread: (f64, f64),
}

/// The lit windows of the house at scale 1 relative to its bottom centre: (x, y, w, h).
const WINDOWS: [(f64, f64, f64, f64); 3] = [(-52.0, -100.0, 22.0, 32.0), (52.0, -180.0, 20.0, 30.0), (-24.0, -150.0, 16.0, 16.0)];

fn paint_sky(cr: &mut Canvas, env: &Env, geo: &Geo) {
    let (w, s, gt) = (env.w, env.s, env.ground_top);
    let rng = &env.rng;
    let mut g = Gradient::linear(0.0, 0.0, 0.0, gt);
    g.add_color_stop_rgb(0.0, 0.025, 0.03, 0.11);
    g.add_color_stop_rgb(0.45, 0.09, 0.06, 0.24);
    g.add_color_stop_rgb(0.85, 0.30, 0.13, 0.40);
    g.add_color_stop_rgb(1.0, 0.52, 0.24, 0.40);
    cr.set_source(&g);
    cr.rectangle(0.0, 0.0, w, gt + 2.0);
    cr.fill();
    for _ in 0..(120.0 * w / (1600.0 * s)) as usize {
        let y = gt * rng.uniform(0.0, 0.8);
        cr.set_source_rgba(0.9, 0.9, 1.0, rng.uniform(0.2, 0.75) * (1.0 - y / gt));
        cr.circle(rng.uniform(0.0, w), y, rng.uniform(0.6, 1.5) * s);
        cr.fill();
    }
    // the full moon with its halo
    let (mx, my, mr) = geo.moon;
    cr.glow(mx, my, mr * 4.2, (0.62, 0.55, 0.95), 0.30, 0.0);
    cr.glow(mx, my, mr * 1.9, (0.95, 0.90, 0.85), 0.35, 0.5);
    let mut g = Gradient::radial(mx - mr * 0.3, my - mr * 0.3, mr * 0.1, mx, my, mr);
    g.add_color_stop_rgb(0.0, 1.0, 0.98, 0.88);
    g.add_color_stop_rgb(1.0, 0.93, 0.88, 0.74);
    cr.set_source(&g);
    cr.circle(mx, my, mr);
    cr.fill();
    cr.set_source_rgba(0.72, 0.66, 0.62, 0.38);
    for (dx, dy, rr) in [(-0.35, -0.30, 0.20), (0.25, -0.45, 0.12), (0.42, 0.10, 0.24), (-0.20, 0.38, 0.15), (-0.58, 0.12, 0.09), (0.05, 0.02, 0.08)] {
        cr.ellipse(mx + dx * mr, my + dy * mr, rr * mr, rr * mr * 0.9);
    }
    cr.fill();
    // thin clouds
    for (cx, cy, size, al) in [(mx + mr * 0.5, my + mr * 0.42, 1.0, 0.62), (w * 0.62, gt * 0.20, 1.2, 0.5), (w * 0.86, gt * 0.52, 0.9, 0.5), (w * 0.10, gt * 0.60, 0.8, 0.4)] {
        for (dx, dy, rx, ry) in [(0.0, 0.0, 170.0, 9.0), (-90.0, 9.0, 120.0, 7.0), (110.0, -8.0, 110.0, 6.0), (40.0, 14.0, 150.0, 6.0)] {
            cr.set_source_rgba(0.26, 0.19, 0.42, al);
            cr.ellipse(cx + dx * s * size, cy + dy * s * size, rx * s * size, ry * s * size);
            cr.fill();
        }
    }
    // far woods along the horizon
    cr.move_to(0.0, gt + 2.0);
    let n = (w / (14.0 * s)) as usize;
    for i in 0..=n {
        let x = i as f64 / n as f64 * w;
        let f = x / s;
        let up = 20.0 + 13.0 * (f * 0.011).sin() + 8.0 * (f * 0.043 + 1.0).sin() + rng.uniform(0.0, 9.0);
        cr.line_to(x, gt - up * s);
    }
    cr.line_to(w, gt + 2.0);
    cr.close_path();
    cr.set_source_rgb(0.19, 0.09, 0.27);
    cr.fill();
}

/// The crooked haunted house on its hill.
fn paint_house(cr: &mut Canvas, env: &Env, geo: &Geo) {
    let (s, gt) = (env.s, env.ground_top);
    let (hx, hy) = geo.house;
    // hill
    cr.move_to(hx - 430.0 * s, gt + 10.0 * s);
    cr.curve_to(hx - 250.0 * s, gt - 4.0 * s, hx - 190.0 * s, hy + 2.0 * s, hx - 70.0 * s, hy - 3.0 * s);
    cr.line_to(hx + 90.0 * s, hy - 3.0 * s);
    cr.curve_to(hx + 200.0 * s, hy, hx + 270.0 * s, gt - 4.0 * s, hx + 440.0 * s, gt + 10.0 * s);
    cr.close_path();
    cr.set_source_rgb(0.105, 0.065, 0.18);
    cr.fill();
    // the path up to the door
    cr.set_source_rgba(0.40, 0.26, 0.44, 0.8);
    cr.set_line_width(7.0 * s);
    cr.set_line_cap(LineCap::Round);
    cr.move_to(geo.gate, gt);
    cr.curve_to(geo.gate + 40.0 * s, gt - 40.0 * s, hx - 150.0 * s, hy + 50.0 * s, hx - 8.0 * s, hy);
    cr.stroke();
    cr.save();
    cr.translate(hx, hy);
    cr.scale(s, s);
    let poly = |cr: &mut Canvas, pts: &[(f64, f64)]| {
        for &(x, y) in pts {
            cr.line_to(x, y);
        }
        cr.close_path();
    };
    cr.set_rgb(NIGHT);
    poly(cr, &[(-72.0, 2.0), (-78.0, -118.0), (58.0, -124.0), (62.0, 2.0)]); // main block
    poly(cr, &[(-94.0, -112.0), (-16.0, -198.0), (76.0, -118.0)]); // its roof
    poly(cr, &[(38.0, 2.0), (33.0, -205.0), (87.0, -212.0), (93.0, 2.0)]); // tower
    poly(cr, &[(-118.0, 2.0), (-121.0, -66.0), (-70.0, -70.0), (-70.0, 2.0)]); // wing
    poly(cr, &[(-130.0, -62.0), (-76.0, -106.0), (-70.0, -64.0)]);
    poly(cr, &[(-58.0, -150.0), (-63.0, -200.0), (-43.0, -203.0), (-40.0, -166.0)]); // chimney
    cr.move_to(20.0, -200.0); // witch-hat roof of the tower
    cr.quad_to(52.0, -240.0, 76.0, -312.0);
    cr.quad_to(80.0, -250.0, 102.0, -207.0);
    cr.close_path();
    cr.fill();
    // moonlight along the roof edges
    cr.set_source_rgba(0.50, 0.42, 0.78, 0.55);
    cr.set_line_width(2.0);
    cr.move_to(-94.0, -112.0);
    cr.line_to(-16.0, -198.0);
    cr.move_to(20.0, -200.0);
    cr.quad_to(52.0, -240.0, 76.0, -312.0);
    cr.move_to(-130.0, -62.0);
    cr.line_to(-76.0, -106.0);
    cr.move_to(-78.0, -112.0);
    cr.line_to(-72.0, 0.0);
    cr.stroke();
    // door and dark windows
    cr.set_source_rgb(0.02, 0.012, 0.045);
    cr.rounded_rect(-24.0, -50.0, 30.0, 52.0, 14.0);
    cr.fill();
    cr.set_source_rgb(0.13, 0.09, 0.22);
    cr.rectangle(12.0, -100.0, 22.0, 32.0);
    cr.rectangle(55.0, -92.0, 18.0, 26.0);
    cr.rectangle(-106.0, -48.0, 16.0, 22.0);
    cr.fill();
    // lit windows
    for (x, y, ww, wh) in WINDOWS {
        cr.glow(x + ww / 2.0, y + wh / 2.0, 52.0, (1.0, 0.62, 0.20), 0.42, 0.0);
        cr.set_source_rgb(1.0, 0.74, 0.28);
        cr.rectangle(x, y, ww, wh);
        cr.fill();
        cr.set_rgb(NIGHT);
        cr.set_line_width(2.4);
        cr.move_to(x + ww / 2.0, y);
        cr.line_to(x + ww / 2.0, y + wh);
        cr.move_to(x, y + wh * 0.45);
        cr.line_to(x + ww, y + wh * 0.45);
        cr.stroke();
    }
    cr.restore();
}

fn paint_ground(cr: &mut Canvas, env: &Env, geo: &Geo) {
    let (w, h, s, gt) = (env.w, env.h, env.s, env.ground_top);
    let rng = &env.rng;
    let mut g = Gradient::linear(0.0, gt, 0.0, h);
    g.add_color_stop_rgb(0.0, 0.15, 0.14, 0.29);
    g.add_color_stop_rgb(0.35, 0.10, 0.11, 0.22);
    g.add_color_stop_rgb(1.0, 0.055, 0.06, 0.13);
    cr.set_source(&g);
    cr.move_to(0.0, h);
    let n = 40;
    for i in 0..=n {
        let f = i as f64 / n as f64;
        cr.line_to(f * w, gt + (f * 23.0).sin() * 3.0 * s);
    }
    cr.line_to(w, h);
    cr.close_path();
    cr.fill();
    // moonlight lying on the grass
    cr.save();
    cr.translate(geo.moon.0 + 120.0 * s, gt + (h - gt) * 0.42);
    cr.scale(1.0, 0.30);
    cr.glow(0.0, 0.0, 620.0 * s, (0.40, 0.42, 0.80), 0.16, 0.0);
    cr.restore();
    // the winding path from the gate to the front
    let path_at = |f: f64| {
        let y = lerp(gt + 4.0 * s, h, f.powf(1.25));
        let cx = geo.gate - (f * PI * 1.5).sin() * 105.0 * s * f.sqrt();
        (cx, y, lerp(15.0, 150.0, f.powf(1.3)) * s)
    };
    let n = 28;
    for i in 0..=n {
        let (cx, y, half) = path_at(i as f64 / n as f64);
        cr.line_to(cx - half, y);
    }
    for i in (0..=n).rev() {
        let (cx, y, half) = path_at(i as f64 / n as f64);
        cr.line_to(cx + half, y);
    }
    cr.close_path();
    let mut g = Gradient::linear(0.0, gt, 0.0, h);
    g.add_color_stop_rgb(0.0, 0.27, 0.22, 0.37);
    g.add_color_stop_rgb(1.0, 0.12, 0.105, 0.19);
    cr.set_source(&g);
    cr.fill();
    for _ in 0..46 {
        let f = rng.uniform(0.05, 1.0);
        let (cx, y, half) = path_at(f);
        cr.set_source_rgba(0.0, 0.0, 0.05, rng.uniform(0.10, 0.22));
        cr.ellipse(cx + rng.uniform(-0.85, 0.85) * half, y, (3.0 + 9.0 * f) * s, (1.2 + 3.0 * f) * s);
        cr.fill();
    }
    // tufts of grass
    cr.set_line_cap(LineCap::Round);
    for _ in 0..(150.0 * w / (1600.0 * s)) as usize {
        let f = rng.random();
        let (x, y) = (rng.uniform(0.0, w), lerp(gt + 8.0 * s, h, f));
        let k = (0.5 + f) * s;
        cr.set_source_rgba(0.20, 0.25, 0.42, rng.uniform(0.25, 0.6));
        cr.set_line_width(1.6 * k);
        for d in [-1.0, 0.0, 1.0] {
            cr.move_to(x + d * 3.0 * k, y);
            cr.line_to(x + d * 8.0 * k + rng.uniform(-2.0, 2.0) * k, y - rng.uniform(7.0, 13.0) * k);
        }
        cr.stroke();
    }
}

/// The wrought-iron fence along the back of the graveyard, with the open gate.
fn paint_fence(cr: &mut Canvas, env: &Env, geo: &Geo) {
    let (w, s) = (env.w, env.s);
    let rng = &env.rng;
    let base = env.ground_top + 7.0 * s;
    let gate = geo.gate;
    let mut bars: Vec<(f64, f64, f64)> = Vec::new();
    let mut x = -6.0 * s;
    while x < w + 10.0 * s {
        if (x - gate).abs() > 52.0 * s {
            let crooked = if rng.chance(0.12) { rng.uniform(-9.0, 9.0) } else { rng.uniform(-1.0, 1.0) };
            bars.push((x, crooked * s, rng.uniform(56.0, 66.0) * s));
        }
        x += 23.0 * s;
    }
    let trace = |cr: &mut Canvas| {
        for &(x, lean, hgt) in &bars {
            cr.move_to(x, base);
            cr.line_to(x + lean, base - hgt);
        }
        for (x0, x1) in [(-10.0 * s, gate - 50.0 * s), (gate + 50.0 * s, w + 10.0 * s)] {
            for yy in [14.0, 46.0] {
                cr.move_to(x0, base - yy * s);
                cr.line_to(x1, base - yy * s);
            }
        }
        // gate posts, arch and the two leaves standing ajar
        for side in [-1.0, 1.0] {
            let px = gate + side * 50.0 * s;
            for k in 1..=3 {
                let bx = px - side * k as f64 * 6.5 * s;
                cr.move_to(bx, base + k as f64 * 2.0 * s);
                cr.line_to(bx, base - (72.0 - k as f64 * 3.0) * s);
            }
            cr.move_to(px, base - 20.0 * s);
            cr.line_to(px - side * 20.0 * s, base - 14.0 * s);
            cr.move_to(px, base - 62.0 * s);
            cr.line_to(px - side * 20.0 * s, base - 56.0 * s);
        }
        cr.move_to(gate - 50.0 * s, base - 92.0 * s);
        cr.quad_to(gate, base - 150.0 * s, gate + 50.0 * s, base - 92.0 * s);
    };
    cr.set_line_cap(LineCap::Round);
    // a little moonlight on the left of every bar, then the iron itself
    cr.save();
    cr.translate(-1.6 * s, -0.8 * s);
    trace(cr);
    cr.set_source_rgba(0.50, 0.44, 0.80, 0.55);
    cr.set_line_width(3.0 * s);
    cr.stroke();
    cr.restore();
    trace(cr);
    cr.set_rgb(NIGHT);
    cr.set_line_width(3.4 * s);
    cr.stroke();
    for &(x, lean, hgt) in &bars {
        let (tx, ty) = (x + lean, base - hgt);
        cr.move_to(tx - 4.0 * s, ty);
        cr.line_to(tx, ty - 11.0 * s);
        cr.line_to(tx + 4.0 * s, ty);
        cr.line_to(tx, ty + 4.0 * s);
        cr.close_path();
    }
    cr.fill();
    for side in [-1.0, 1.0] {
        let px = gate + side * 50.0 * s;
        cr.set_source_rgba(0.50, 0.44, 0.80, 0.55);
        cr.rectangle(px - 6.5 * s, base - 96.0 * s, 11.0 * s, 100.0 * s);
        cr.fill();
        cr.set_rgb(NIGHT);
        cr.rectangle(px - 5.0 * s, base - 96.0 * s, 10.0 * s, 100.0 * s);
        cr.circle(px, base - 101.0 * s, 7.0 * s);
        cr.fill();
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Tomb(usize),
    Pumpkin(usize),
}

/// A tombstone or pumpkin standing in the graveyard; drawn in depth order with the skeletons.
struct Prop {
    x: f64,
    /// where it stands on the ground
    y: f64,
    /// pixels per unit
    k: f64,
    pic: Picture,
    /// candle light of a pumpkin
    glow: Option<Picture>,
    /// a ghost peeks out from behind this one now and then
    haunted: bool,
    ph: f64,
}

/// Sparks of a tool-call trick.
struct Spark {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    age: f64,
    life: f64,
    c: Rgb,
}

#[derive(Clone)]
pub struct Skeleton {
    b: Base,
    pose: Pose,
    /// 1 -> 0 after a tool call: the trick
    gag: f64,
    /// 0 skull pops off, 1 tips the skull like a hat, 2 juggles a bone
    trick: usize,
    /// 0..1 dancing instead of walking
    dance: f64,
    scratch: f64,
    yawn: f64,
}

impl ActorT<Halloween> for Skeleton {
    const BASE_RADIUS: f64 = 80.0;
    const WALKER: bool = true;

    fn new(base: Base, env: &Env, _world: &mut Halloween) -> Self {
        let pose = if base.state() == State::Rest { Pose::lie(0.0, base.seed) } else { Pose::walk(0.0, 0.0, 1.0, 0.0, base.seed) };
        Skeleton { b: base, pose, gag: 0.0, trick: env.rng.index(3), dance: 0.0, scratch: 0.0, yawn: 0.0 }
    }

    fn base(&self) -> &Base {
        &self.b
    }

    fn base_mut(&mut self) -> &mut Base {
        &mut self.b
    }

    fn companion_reset(&mut self) {
        self.gag = 0.0;
        self.dance = 0.0;
        self.scratch = 0.0;
        self.yawn = 0.0;
        self.pose = Pose::walk(self.b.step, self.b.moving, self.b.face, 0.0, self.b.seed);
    }

    /// Sub-agents are little ghosts floating along behind the skeleton.
    fn draw_companions(&self, cr: &mut Canvas, env: &Env, _world: &Halloween, t: f64) {
        let b = &self.b;
        if b.alpha < 0.05 || b.info.subagents.is_empty() {
            return;
        }
        let g = b.radius * Self::COMPANION_SCALE * 0.60;
        for (i, (x, y, _)) in self.companion_spots(env, t).into_iter().enumerate() {
            let ph = b.seed + i as f64 * 2.1;
            let fy = y - b.radius * 0.30 + (t * 2.2 + ph).sin() * g * 0.28;
            cr.set_rgba(WORKING_COLOR, 0.16 * b.alpha);
            cr.ellipse(x, y + b.radius * 0.72, g * 1.5, g * 0.3);
            cr.fill();
            ghost(cr, x, fy, g, WORKING_COLOR, b.alpha, t + ph, sign(b.x - x));
        }
    }

    fn mv(&mut self, env: &mut Env, _world: &mut Halloween, others: &[Other], dt: f64, t: f64, target: Option<(f64, f64)>) {
        let st = self.b.state();
        let seed = self.b.seed;
        // working: a few dance steps now and then
        let dancing = st == State::Working && self.gag <= 0.0 && (t * 0.26 + seed).sin() > 0.5;
        self.dance = lerp(self.dance, if dancing { 1.0 } else { 0.0 }, (dt * 2.5).min(1.0));
        let pace = 82.0 * (1.0 - 0.92 * self.dance);
        self.b.walker_move(env, others, dt, t, target, [pace, 22.0, 55.0, 0.0], [1.0, 0.5, 0.5, 0.1]);
        self.gag = (self.gag - dt * 0.72).max(0.0);
        // idle and sitting: scratch the skull, yawn
        let sitting = st == State::Idle && self.b.graze > 0.7;
        let yawning = sitting && (t * 0.33 + seed * 2.9).sin() > 0.80;
        let scratching = sitting && !yawning && (t * 0.41 + seed * 1.7).sin() > 0.55;
        self.yawn = lerp(self.yawn, if yawning { 1.0 } else { 0.0 }, (dt * 2.5).min(1.0));
        self.scratch = lerp(self.scratch, if scratching { 1.0 } else { 0.0 }, (dt * 4.0).min(1.0));
        let b = &self.b;
        let walk = Pose::walk(b.step, b.moving, b.face, t, seed);
        let goal = match st {
            State::Working => walk.mix(&Pose::dance(t, seed), smooth(self.dance)),
            State::Idle => walk.mix(&Pose::sit(t, seed, sign(b.face), self.scratch, self.yawn), smooth(b.graze * 1.3 - 0.15)),
            State::Waiting => {
                // the arms wave all the way to the front; the legs keep walking until it is there
                let mut p = Pose::wave(t, seed);
                p.feet = [mixp(p.feet[0], walk.feet[0], b.moving), mixp(p.feet[1], walk.feet[1], b.moving)];
                p.kb = [lerp(p.kb[0], walk.kb[0], b.moving), lerp(p.kb[1], walk.kb[1], b.moving)];
                p
            }
            State::Blocked => Pose::slump(t, seed),
            State::Rest => Pose::lie(t, seed),
        };
        self.pose = self.pose.mix(&goal, (dt * 10.0).min(1.0));
    }

    fn draw(&self, cr: &mut Canvas, env: &Env, _world: &Halloween, t: f64) {
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
        let hop = if b.hop > 0.0 { (PI * b.hop).sin() * r * 0.28 } else { 0.0 };
        // glow on the ground + shadow
        let gy = b.y + r * 0.97;
        cr.save();
        cr.translate(b.x, gy);
        cr.scale(1.0, 0.3);
        cr.glow(0.0, 0.0, r * 1.8, c, if st == State::Idle { 0.36 } else { 0.5 } * a, 0.0);
        cr.restore();
        cr.ellipse(b.x, gy, r * 0.5, r * 0.09);
        cr.set_source_rgba(0.0, 0.0, 0.03, 0.4 * a);
        cr.fill();
        if st == State::Waiting {
            cr.pulse_rings(b.x, b.y - r * 0.2, r, c, a, t, s);
        }
        let mut p = self.pose;
        let mut lk = Looks { c, a, dir: b.face, mood: if st == State::Blocked { Mood::Sad } else { Mood::Grin }, skull_off: (0.0, 0.0), skull_rot: 0.0 };
        // the tool-call trick
        let u = 1.0 - self.gag;
        let wt = if self.gag > 0.0 { (u * 7.0).min(1.0) * (self.gag * 5.0).min(1.0) } else { 0.0 };
        let mut flying: Option<(f64, f64, f64)> = None;
        if wt > 0.0 {
            let d = sign(b.face);
            match self.trick {
                0 => {
                    // the skull pops off, turns over once and drops back
                    let up = (PI * u).sin();
                    lk.skull_off = (0.0, -0.95 * up);
                    lk.skull_rot = d * TAU * smooth(u * 1.15) - p.tilt * wt;
                    p.hands = [mixp(p.hands[0], (-0.38, -0.80), wt), mixp(p.hands[1], (0.38, -0.80), wt)];
                    p.eb = [lerp(p.eb[0], 0.6, wt), lerp(p.eb[1], -0.6, wt)];
                    p.jaw = lerp(p.jaw, 0.8, wt);
                }
                1 => {
                    // tips its skull like a hat
                    let k = if d > 0.0 { 1 } else { 0 };
                    lk.skull_off = (d * 0.10 * wt, -0.22 * wt);
                    lk.skull_rot = d * 0.42 * wt;
                    p.hands[k] = mixp(p.hands[k], (p.head.0 + d * 0.38, p.head.1 - 0.20), wt);
                    p.eb[k] = lerp(p.eb[k], -d * 0.7, wt);
                }
                _ => {
                    // juggles a bone from hand to hand
                    let v = (u * 2.0).fract();
                    let from = if u < 0.5 { -1.0 } else { 1.0 };
                    flying = Some((from * lerp(0.52, -0.52, v), -0.44 - 4.4 * v * (1.0 - v), u * 16.0));
                    let lift = |k: f64| 0.06 * (u * TAU * 2.0 + k).sin();
                    p.hands = [mixp(p.hands[0], (-0.50, -0.36 + lift(0.0)), wt), mixp(p.hands[1], (0.50, -0.36 + lift(PI)), wt)];
                    p.eb = [lerp(p.eb[0], 0.9, wt), lerp(p.eb[1], -0.9, wt)];
                    p.head.1 -= 0.03 * wt;
                    p.jaw = lerp(p.jaw, 0.5, wt);
                }
            }
        }
        cr.save();
        cr.translate(b.x, b.y - hop);
        if st == State::Blocked {
            // it dropped a bone
            loose_bone(cr, -r * 0.52, r * 0.93, r * 0.30, 0.25, c, a);
        }
        if st == State::Waiting {
            // the lantern swings from the raised hand
            cr.save();
            cr.translate(p.hands[1].0 * r, p.hands[1].1 * r);
            cr.rotate((t * 7.0 + b.seed).sin() * 0.35);
            lantern(cr, r, c, a, 0.5 + 0.5 * (t * 11.0 + b.seed).sin());
            cr.restore();
        }
        figure(cr, &p, r, &lk);
        if let Some((fx, fy, rot)) = flying {
            loose_bone(cr, fx * r, fy * r, r * 0.30, rot, c, a * wt);
        }
        cr.restore();
    }
}

impl Skeleton {
    /// Resting: asleep on its back beside an open grave, hands folded, a pumpkin for a night light.
    fn draw_asleep(&self, cr: &mut Canvas, s: f64, t: f64, r: f64, a: f64) {
        let b = &self.b;
        if a < 0.01 {
            return;
        }
        // the little still life is drawn bigger than a walking skeleton
        let r = r * 1.4;
        let (x, y, c) = (b.x, b.y - r * 0.25, b.color);
        let k = r / 80.0;
        // the open grave with its mound of earth and the spade
        cr.set_source_rgba(0.015, 0.01, 0.04, 0.9 * a);
        cr.ellipse(x + r * 0.10, y + r * 0.08, r * 1.15, r * 0.20);
        cr.fill();
        cr.set_source_rgba(0.10, 0.08, 0.17, a);
        cr.ellipse(x + r * 0.10, y + r * 0.13, r * 1.02, r * 0.14);
        cr.fill();
        cr.set_source_rgba(0.40, 0.36, 0.50, a);
        cr.set_line_width(r * 0.05);
        cr.set_line_cap(LineCap::Round);
        cr.move_to(x + r * 1.50, y - r * 0.80);
        cr.line_to(x + r * 1.32, y - r * 0.12);
        cr.move_to(x + r * 1.40, y - r * 0.82);
        cr.line_to(x + r * 1.60, y - r * 0.78);
        cr.stroke();
        cr.set_source_rgba(0.27, 0.22, 0.36, a);
        cr.move_to(x + r * 0.78, y + r * 0.16);
        cr.quad_to(x + r * 1.25, y - r * 0.50, x + r * 1.78, y + r * 0.16);
        cr.close_path();
        cr.fill();
        cr.set_source_rgba(0.38, 0.32, 0.50, 0.7 * a);
        cr.ellipse(x + r * 1.12, y - r * 0.06, r * 0.10, r * 0.05);
        cr.ellipse(x + r * 1.38, y + r * 0.04, r * 0.08, r * 0.04);
        cr.fill();
        tombstone(cr, x - r * 1.42, y + r * 0.34, k * 1.1, 3, a);
        // soft light under the sleeper
        cr.set_rgba(c, 0.10 * a);
        cr.ellipse(x, y + r * 0.72, r * 1.5, r * 0.34);
        cr.fill();
        cr.set_source_rgba(0.0, 0.0, 0.03, 0.38 * a);
        cr.ellipse(x + r * 0.05, y + r * 0.86, r * 1.15, r * 0.10);
        cr.fill();
        cr.save();
        cr.translate(x + r * 0.02, y + r * 0.62);
        cr.rotate(-PI / 2.0);
        let lk = Looks { c, a, dir: 0.0, mood: Mood::Asleep, skull_off: (0.0, 0.0), skull_rot: 0.0 };
        figure(cr, &self.pose, r, &lk);
        cr.restore();
        // pumpkin with a flickering candle
        let flick = 0.6 + 0.25 * (t * 9.0).sin() + 0.15 * (t * 15.7).sin();
        let (px, py) = (x + r * 1.58, y + r * 1.06);
        cr.glow(px, py - 27.0 * k, 95.0 * k, (1.0, 0.6, 0.15), (0.24 + 0.10 * flick) * a, 0.0);
        pumpkin(cr, px, py, k * 0.9, 0, a);
        // z z z
        for i in 0..3 {
            let f = (t * 0.3 + i as f64 / 3.0) % 1.0;
            let lay = text::layout("z", (14.0 + 14.0 * f) * s, weight::BOLD, None);
            let zx = x - r * (0.75 - 0.5 * f) + (f * 6.0).sin() * 4.0 * s;
            let zy = y + r * 0.1 - f * r * 1.4;
            text::show(cr, &lay, zx, zy, (1.0, 1.0, 1.0), 0.8 * (PI * f).sin() * a, 0.4);
        }
    }
}

pub struct Halloween {
    bg: Option<Picture>,
    geo: Geo,
    props: Vec<Prop>,
    /// twinkling stars: (x, y, size, phase)
    stars: Vec<(f64, f64, f64, f64)>,
    /// will-o'-the-wisps: (centre x, centre y, reach, phase)
    wisps: Vec<(f64, f64, f64, f64)>,
    /// banks of mist: (y, phase, speed, opacity, in front of the skeletons)
    mist: Vec<(f64, f64, f64, f64, bool)>,
    fog: Option<Picture>,
    wisp: Option<Picture>,
    sparks: Vec<Spark>,
}

impl Halloween {
    fn layout(env: &Env) -> Geo {
        let (w, h, s, gt) = (env.w, env.h, env.s, env.ground_top);
        let rad = 330.0 * s;
        // on spoke 3 of 7 of the corner web, where ring 5 crosses it
        let (ta, tr) = (PI / 4.0, rad * (1.0 + 0.10 * (3.0_f64 * 2.4).sin()) * 0.94 * 5.0 / 7.0);
        Geo {
            moon: (w * 0.335, h * 0.205, 96.0 * s),
            gate: w * 0.47,
            house: (w * 0.665, gt - 76.0 * s),
            thread: (ta.cos() * tr, ta.sin() * tr),
        }
    }

    fn paint_night(cr: &mut Canvas, env: &Env, geo: &Geo) {
        let (w, h, s, gt) = (env.w, env.h, env.s, env.ground_top);
        let rng = &env.rng;
        paint_sky(cr, env, geo);
        paint_house(cr, env, geo);
        paint_ground(cr, env, geo);
        paint_fence(cr, env, geo);
        // two bare trees; the left one reaches across the moon
        let top = h * 0.27;
        bare_tree(cr, rng, w * 0.165, gt + 26.0 * s, gt - top, 0.10, s);
        bare_tree(cr, rng, w * 0.905, gt + 30.0 * s, gt - top - 30.0 * s, -0.12, s);
        // the big web in the corner
        web(cr, 0.0, 0.0, 0.0, PI / 2.0, 330.0 * s, 7, 7, 1.3 * s, 0.42);
        // dark corners
        let mut v = Gradient::radial(w / 2.0, h * 0.55, h * 0.45, w / 2.0, h * 0.55, hypot(w, h) * 0.62);
        v.add_color_stop_rgba(0.0, 0.01, 0.0, 0.04, 0.0);
        v.add_color_stop_rgba(1.0, 0.01, 0.0, 0.04, 0.5);
        cr.set_source(&v);
        cr.rectangle(0.0, 0.0, w, h);
        cr.fill();
    }

    /// Bats crossing the moon now and then.
    fn draw_bats(&self, cr: &mut Canvas, s: f64, t: f64) {
        let (mx, my, mr) = self.geo.moon;
        let round = (t / 17.0).floor();
        let f = (t - round * 17.0) / 6.0;
        if f >= 1.0 {
            return;
        }
        let dir = if round as i64 % 2 == 0 { 1.0 } else { -1.0 };
        cr.set_source_rgb(0.03, 0.015, 0.06);
        for k in 0..3 {
            let kf = k as f64;
            let ff = f * 1.15 - kf * 0.07;
            let x = mx + dir * (ff - 0.5) * mr * 9.0;
            let y = my + (ff * 9.0 + kf * 2.0).sin() * mr * 0.32 + (kf - 1.0) * mr * 0.42;
            let z = (15.0 - 2.5 * kf) * s;
            let flap = (t * 15.0 + kf * 2.0).sin();
            cr.ellipse(x, y, z * 0.24, z * 0.36);
            cr.circle(x - z * 0.14, y - z * 0.36, z * 0.10); // ears
            cr.circle(x + z * 0.14, y - z * 0.36, z * 0.10);
            for side in [-1.0, 1.0] {
                let tip = (x + side * z * 1.5, y - z * 0.75 * flap);
                cr.move_to(x, y - z * 0.25);
                cr.line_to(tip.0, tip.1);
                cr.quad_to(x + side * z * 1.1, y + z * (0.05 - 0.2 * flap), x + side * z * 0.95, y + z * (0.42 - 0.3 * flap));
                cr.quad_to(x + side * z * 0.65, y + z * 0.05, x + side * z * 0.45, y + z * 0.42);
                cr.quad_to(x + side * z * 0.3, y + z * 0.1, x, y + z * 0.3);
                cr.close_path();
            }
            cr.fill();
        }
    }

    /// The spider bobbing on its thread below the big web.
    fn draw_spider(&self, cr: &mut Canvas, s: f64, t: f64) {
        let (ax, ay) = self.geo.thread;
        let len = (150.0 + 42.0 * (t * 0.45).sin() + 5.0 * (t * 2.3).sin()) * s;
        let x = ax + (t * 0.9).sin() * 5.0 * s;
        let y = ay + len;
        cr.set_rgba(WEB, 0.5);
        cr.set_line_width(1.2 * s);
        cr.move_to(ax, ay);
        cr.line_to(x, y);
        cr.stroke();
        cr.set_source_rgb(0.03, 0.02, 0.06);
        cr.set_line_width(1.8 * s);
        cr.set_line_cap(LineCap::Round);
        cr.set_line_join(LineJoin::Round);
        for side in [-1.0, 1.0] {
            for k in 0..4 {
                let kf = k as f64;
                let wig = (t * 5.0 + kf * 1.3 + side).sin() * 1.5 * s;
                cr.move_to(x, y + 2.0 * s);
                cr.line_to(x + side * (12.0 + kf) * s, y + (kf * 5.0 - 9.0) * s + wig);
                cr.line_to(x + side * (17.0 + kf * 1.5) * s, y + (kf * 7.0 - 3.0) * s + wig);
            }
        }
        cr.stroke();
        cr.ellipse(x, y + 6.0 * s, 8.5 * s, 10.0 * s);
        cr.circle(x, y - 5.0 * s, 5.5 * s);
        cr.fill();
        cr.set_source_rgba(1.0, 0.75, 0.3, 0.95);
        cr.circle(x - 2.2 * s, y - 8.0 * s, 1.3 * s);
        cr.circle(x + 2.2 * s, y - 8.0 * s, 1.3 * s);
        cr.fill();
    }

    fn draw_prop(&self, cr: &mut Canvas, p: &Prop, t: f64) {
        if p.haunted {
            // a ghost rises from behind the stone, looks around and ducks again
            let f = (t * 0.045 + 0.57) % 1.0;
            if f < 0.24 {
                let up = smooth((f / 0.24 * PI).sin() * 1.8);
                let g = 21.0 * p.k;
                ghost(cr, p.x + 9.0 * p.k, p.y - 60.0 * p.k - up * 58.0 * p.k, g, GHOST, 0.9, t, (t * 1.3).sin());
            }
        }
        cr.picture(&p.pic, (p.x - 80.0 * p.k).round(), (p.y - 150.0 * p.k).round(), 1.0);
        if let Some(glow) = &p.glow {
            let flick = 0.6 + 0.25 * (t * 9.0 + p.ph).sin() + 0.15 * (t * 15.7 + p.ph * 2.0).sin();
            cr.picture(glow, (p.x - 110.0 * p.k).round(), (p.y - 137.0 * p.k).round(), 0.45 + 0.55 * flick);
        }
    }
}

impl Theme for Halloween {
    type Actor = Skeleton;
    const NAME: &'static str = "halloween";
    const LABEL_BG: Rgb = (0.05, 0.02, 0.10);
    const EMPTY_COLOR: Rgb = (0.86, 0.80, 1.0);
    const WAITING_ROW: f64 = 0.76;

    fn new(_env: &Env) -> Self {
        Halloween {
            bg: None,
            geo: Geo::default(),
            props: Vec::new(),
            stars: Vec::new(),
            wisps: Vec::new(),
            mist: Vec::new(),
            fog: None,
            wisp: None,
            sparks: Vec::new(),
        }
    }

    fn spawn_point(&self, env: &Env) -> (f64, f64) {
        (env.rng.uniform(env.w * 0.2, env.w * 0.8), env.rng.uniform(env.ground_top + 40.0, env.h * 0.85))
    }

    fn on_tool(&mut self, env: &mut Env, actor: &mut Skeleton, name: &str, index: usize) {
        let s = env.s;
        if actor.gag <= 0.05 {
            actor.trick = (actor.trick + 1 + env.rng.index(2)) % 3;
            actor.gag = 1.0;
            if actor.trick != 0 {
                actor.b.hop = 1.0;
            }
        }
        let b = &actor.b;
        let (x, y) = (b.x, b.y - b.radius * 0.95);
        for _ in 0..9 {
            let ang = env.rng.uniform(-PI, 0.0);
            let sp = env.rng.uniform(90.0, 230.0) * s;
            self.sparks.push(Spark {
                x: x + env.rng.uniform(-0.25, 0.25) * b.radius,
                y,
                vx: ang.cos() * sp,
                vy: ang.sin() * sp,
                age: 0.0,
                life: env.rng.uniform(0.5, 1.0),
                c: if env.rng.chance(0.3) { CANDLE } else { lighten(b.color, 0.4) },
            });
        }
        env.spawn_floater(b.x, b.y - b.radius * 1.5 - index as f64 * 26.0 * s, name, b.color, FloatStyle::Text, 40.0, 3.5);
    }

    fn setup_world(&mut self, env: &mut Env) {
        let (w, h, s) = (env.w, env.h, env.s);
        let gt = h * 0.63;
        env.ground_top = gt;
        self.geo = Self::layout(env);
        let geo = self.geo;
        let env: &Env = env;
        let rng = &env.rng;
        self.bg = Some(Picture::record(w, h, |cr| Self::paint_night(cr, env, &geo)));
        // tombstones and pumpkins: along the fence, at the sides and along the front, so the middle
        // stays free. (x as fraction of the width, depth 0 = at the fence .. 1 = at the front, kind)
        let front = h - env.strip_h - 7.0 * s;
        let gh = front - gt;
        let spots: [(f64, f64, Kind); 21] = [
            (0.215, 0.03, Kind::Tomb(0)),
            (0.262, 0.11, Kind::Pumpkin(1)),
            (0.305, 0.07, Kind::Tomb(1)),
            (0.385, 0.04, Kind::Tomb(4)),
            (0.560, 0.05, Kind::Tomb(2)),
            (0.602, 0.12, Kind::Pumpkin(0)),
            (0.640, 0.08, Kind::Tomb(3)),
            (0.735, 0.03, Kind::Tomb(1)),
            (0.780, 0.11, Kind::Pumpkin(2)),
            (0.815, 0.06, Kind::Tomb(0)),
            (0.040, 0.30, Kind::Tomb(2)),
            (0.080, 0.60, Kind::Tomb(3)),
            (0.028, 0.72, Kind::Pumpkin(0)),
            (0.966, 0.27, Kind::Tomb(1)),
            (0.938, 0.56, Kind::Tomb(0)),
            (0.978, 0.70, Kind::Pumpkin(1)),
            (0.050, 1.0, Kind::Pumpkin(2)),
            (0.160, 1.0, Kind::Pumpkin(0)),
            (0.665, 1.0, Kind::Pumpkin(1)),
            (0.855, 1.0, Kind::Pumpkin(0)),
            (0.950, 1.0, Kind::Pumpkin(2)),
        ];
        self.props = spots
            .iter()
            .map(|&(fx, d, kind)| {
                let (x, y) = (fx * w, gt + (0.02 + d * 0.98) * gh);
                let k = lerp(0.60, 1.12, d) * s * if matches!(kind, Kind::Pumpkin(_)) && d >= 1.0 { 0.92 + 0.2 * (fx * 37.0).sin().abs() } else { 1.0 };
                let pic = Picture::record(160.0 * k, 170.0 * k, |cr| match kind {
                    Kind::Tomb(shape) => tombstone(cr, 80.0 * k, 150.0 * k, k, shape, 1.0),
                    Kind::Pumpkin(face) => pumpkin(cr, 80.0 * k, 150.0 * k, k, face, 1.0),
                });
                let glow = matches!(kind, Kind::Pumpkin(_)).then(|| {
                    Picture::record(220.0 * k, 220.0 * k, |cr| {
                        cr.glow(110.0 * k, 110.0 * k, 110.0 * k, (1.0, 0.58, 0.14), 0.30, 0.0);
                        cr.save();
                        cr.translate(110.0 * k, 140.0 * k);
                        cr.scale(1.0, 0.3);
                        cr.glow(0.0, 0.0, 105.0 * k, (1.0, 0.62, 0.18), 0.28, 0.0);
                        cr.restore();
                    })
                });
                Prop { x, y, k, pic, glow, haunted: kind == Kind::Tomb(3) && d < 0.2, ph: rng.uniform(0.0, TAU) }
            })
            .collect();
        self.props.sort_by(|a, b| a.y.total_cmp(&b.y));
        // twinkling stars in the open sky to the right of the moon
        self.stars = (0..26)
            .map(|_| (w * rng.uniform(0.42, 0.98), h * rng.uniform(0.02, 0.26), rng.uniform(1.3, 2.4) * s, rng.uniform(0.0, TAU)))
            .collect();
        self.wisps = (0..7)
            .map(|i| {
                let side = if i % 2 == 0 { rng.uniform(0.04, 0.30) } else { rng.uniform(0.70, 0.96) };
                (w * side, gt + rng.uniform(0.0, 0.55) * gh, rng.uniform(40.0, 90.0) * s, rng.uniform(0.0, TAU))
            })
            .collect();
        self.mist = (0..6)
            .map(|i| {
                let near = i >= 4;
                let y = if near { gt + gh * rng.uniform(0.75, 0.95) } else { gt + rng.uniform(-6.0, 60.0) * s };
                (y, rng.random(), rng.uniform(0.006, 0.013), if near { rng.uniform(0.35, 0.5) } else { rng.uniform(0.6, 1.0) }, near)
            })
            .collect();
        self.fog = Some(Picture::record(1300.0 * s, 150.0 * s, |cr| {
            cr.translate(650.0 * s, 75.0 * s);
            cr.scale(2.7, 0.30);
            cr.glow(0.0, 0.0, 240.0 * s, (0.72, 0.70, 0.98), 0.30, 0.0);
        }));
        self.wisp = Some(Picture::record(48.0 * s, 48.0 * s, |cr| {
            cr.glow(24.0 * s, 24.0 * s, 24.0 * s, (0.62, 1.0, 0.72), 0.55, 0.12);
        }));
        self.sparks.clear();
    }

    fn update_world(&mut self, env: &mut Env, _actors: &mut [Skeleton], dt: f64) {
        let g = 420.0 * env.s;
        for p in &mut self.sparks {
            p.age += dt;
            p.vy += g * dt;
            p.x += p.vx * dt;
            p.y += p.vy * dt;
        }
        self.sparks.retain(|p| p.age < p.life);
    }

    fn draw_world(&mut self, cr: &mut Canvas, env: &Env, actors: &[Skeleton], t: f64) {
        let (w, s) = (env.w, env.s);
        if let Some(bg) = &self.bg {
            cr.picture(bg, 0.0, 0.0, 1.0);
        }
        for &(x, y, size, ph) in &self.stars {
            let tw = 0.5 + 0.5 * (t * 1.6 + ph).sin();
            cr.set_source_rgba(0.95, 0.93, 1.0, 0.25 + 0.7 * tw);
            cr.circle(x, y, size * (0.7 + 0.4 * tw));
            cr.fill();
        }
        self.draw_bats(cr, s, t);
        // candle light flickering in the windows of the house
        let (hx, hy) = self.geo.house;
        for (k, (x, y, ww, wh)) in WINDOWS.into_iter().enumerate() {
            let flick = 0.5 + 0.5 * (t * 7.0 + k as f64 * 2.1).sin() * (t * 2.3 + k as f64).sin();
            cr.set_source_rgba(1.0, 0.92, 0.6, 0.30 * flick);
            cr.rectangle(hx + x * s, hy + y * s, ww * s, wh * s);
            cr.fill();
        }
        self.draw_spider(cr, s, t);
        // drifting banks of mist behind the skeletons
        let fog_at = |cr: &mut Canvas, m: &(f64, f64, f64, f64, bool)| {
            let x = ((t * m.2 + m.1) % 1.0) * (w + 1300.0 * s) - 650.0 * s;
            if let Some(fog) = &self.fog {
                cr.picture(fog, (x - 650.0 * s).round(), (m.0 - 75.0 * s).round(), m.3);
            }
        };
        for m in self.mist.iter().filter(|m| !m.4) {
            fog_at(cr, m);
        }
        // will-o'-the-wisps
        for &(cx, cy, reach, ph) in &self.wisps {
            let (x, y) = (cx + (t * 0.23 + ph).sin() * reach * 1.6, cy + (t * 0.31 + ph * 1.7).sin() * reach * 0.4 - reach * 0.3);
            let blink = smooth(0.5 + 0.9 * (t * 0.5 + ph * 3.0).sin());
            if let Some(wisp) = &self.wisp {
                cr.picture(wisp, (x - 24.0 * s).round(), (y - 24.0 * s).round(), 0.85 * blink);
            }
            cr.set_source_rgba(0.9, 1.0, 0.9, 0.9 * blink);
            cr.circle(x, y, 2.2 * s);
            cr.fill();
        }
        // skeletons between the tombstones and pumpkins, back to front
        let mut pi = 0;
        for a in sorted_actors(actors) {
            let foot = a.b.y + a.b.radius * 0.95;
            while pi < self.props.len() && self.props[pi].y < foot {
                self.draw_prop(cr, &self.props[pi], t);
                pi += 1;
            }
            a.draw(cr, env, self, t);
        }
        while pi < self.props.len() {
            self.draw_prop(cr, &self.props[pi], t);
            pi += 1;
        }
        for p in &self.sparks {
            let f = p.age / p.life;
            cr.set_rgba(p.c, (1.0 - f) * 0.95);
            cr.circle(p.x, p.y, (3.4 - 2.0 * f) * s);
            cr.fill();
        }
        for m in self.mist.iter().filter(|m| m.4) {
            fog_at(cr, m);
        }
    }
}
