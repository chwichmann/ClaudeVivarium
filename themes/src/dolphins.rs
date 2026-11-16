//! Dolphin School theme: a pod of dolphins in the open sea, seen half above and half below the water.
//!
//! working    = swims fast and playfully in arcs and loops and now and then leaps out of the water with
//!              a splash; a tool call is a barrel roll through a ring of bubbles, the tool name rises
//! background = like working, a little calmer
//! idle       = cruises slowly near the surface, nods, blows a few bubbles, sometimes rolls on its back
//! waiting    = comes to the front, stands on its tail, bobs and chatters with its beak open, pulses
//! blocked    = hangs still with its head down, slowly sinking and correcting, flickers
//! rest       = one dolphin asleep just below the surface, eye closed, "z z z" in small bubbles
//! The state colour tints back and fins (the belly stays pale) and glows softly around the dolphin.
//! The dolphins keep together as a pod that travels through the water. Sub-agents are calves that
//! swim in formation behind their parent. Ships pass on the surface, a submarine, a turtle, a manta
//! and small schools of fish pass below - all of them calmer and paler than the dolphins.
use vivarium_core::prelude::*;

const NAVY: Rgb = (0.04, 0.10, 0.22);
const BELLY: Rgb = (0.94, 0.97, 0.98);
const SHALLOW: Rgb = (0.20, 0.70, 0.78);
const WATER: Rgb = (0.06, 0.36, 0.58);
/// Leaps: gravity and launch speed in px/s at scale 1, launch angle above the horizon.
const GRAVITY: f64 = 760.0;
const LEAP_SPEED: f64 = 450.0;
const LEAP_ANGLE: f64 = 1.05;
/// Seconds a barrel roll takes.
const TRICK: f64 = 0.9;
/// Samples along the spine.
const N: usize = 40;

/// Body outline: (position along the spine 0 = tail .. 1 = beak tip, thickness of the back, of the belly),
/// in units of the radius.
const PROFILE: [(f64, f64, f64); 14] = [
    (0.00, 0.045, 0.045),
    (0.08, 0.065, 0.065),
    (0.20, 0.130, 0.125),
    (0.34, 0.225, 0.220),
    (0.48, 0.295, 0.295),
    (0.60, 0.325, 0.315),
    (0.72, 0.320, 0.295),
    (0.80, 0.305, 0.255),
    (0.86, 0.275, 0.200),
    (0.895, 0.215, 0.155),
    (0.918, 0.120, 0.125),
    (0.94, 0.072, 0.105),
    (0.97, 0.060, 0.085),
    (1.00, 0.040, 0.050),
];

fn profile(u: f64) -> (f64, f64) {
    let u = clamp(u, 0.0, 1.0);
    for k in 1..PROFILE.len() {
        let (a, b) = (PROFILE[k - 1], PROFILE[k]);
        if u <= b.0 {
            let f = (u - a.0) / (b.0 - a.0);
            return (lerp(a.1, b.1, f), lerp(a.2, b.2, f));
        }
    }
    (PROFILE[13].1, PROFILE[13].2)
}

fn smooth(x: f64) -> f64 {
    let x = clamp(x, 0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

fn wrap(a: f64) -> f64 {
    (a + PI).rem_euclid(TAU) - PI
}

fn hash(n: f64) -> f64 {
    ((n * 12.9898).sin() * 43758.5453).rem_euclid(1.0)
}

fn look_index(look: Look) -> usize {
    match look {
        Look::Working => 0,
        Look::Idle => 1,
        Look::Waiting => 2,
        Look::Blocked => 3,
        Look::Background => 4,
        Look::Rest => 5,
    }
}

const LOOKS: [Look; 6] = [Look::Working, Look::Idle, Look::Waiting, Look::Blocked, Look::Background, Look::Rest];

fn poly(cr: &mut Canvas, pts: &[(f64, f64)]) {
    cr.move_to(pts[0].0, pts[0].1);
    for p in &pts[1..] {
        cr.line_to(p.0, p.1);
    }
    cr.close_path();
}

/// Closed smooth outline through the middle of each edge of `pts`.
fn smooth_loop(cr: &mut Canvas, pts: &[(f64, f64)]) {
    let n = pts.len();
    let mid = |a: (f64, f64), b: (f64, f64)| ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
    let m = mid(pts[n - 1], pts[0]);
    cr.move_to(m.0, m.1);
    for i in 0..n {
        let m = mid(pts[i], pts[(i + 1) % n]);
        cr.quad_to(pts[i].0, pts[i].1, m.0, m.1);
    }
    cr.close_path();
}

// ---------------------------------------------------------------------------------- the dolphin

#[derive(Clone, Copy, PartialEq)]
enum Eye {
    Open,
    Shut,
    Sad,
}

/// How one dolphin is drawn.
struct Pose {
    r: f64,
    /// direction of the body
    ang: f64,
    /// 1 = belly down, -1 = mirrored (swimming left, or on its back)
    flip: f64,
    /// phase and size of the tail beat
    tail: f64,
    amp: f64,
    /// -1..1 body bent into a turn
    bend: f64,
    c: Rgb,
    a: f64,
    eye: Eye,
    /// 0..1 beak open
    jaw: f64,
    calf: bool,
}

/// The bent centre line of the body in units of the radius: nose towards +x, belly towards +y.
struct Spine {
    p: [(f64, f64); N],
    n: [(f64, f64); N],
}

impl Spine {
    fn new(tail: f64, amp: f64, bend: f64) -> Spine {
        let mut p = [(0.0, 0.0); N];
        for (i, q) in p.iter_mut().enumerate() {
            let u = i as f64 / (N - 1) as f64;
            let w = 1.0 - u;
            // the beat grows towards the tail, the head nods a little against it
            let beat = amp * (w.powf(1.6) * (tail - 2.3 * w).sin() - 0.10 * tail.sin() * u);
            let k = (u - 0.62) / 0.62;
            *q = (-1.15 + 2.3 * u, beat + bend * k * k * 0.20);
        }
        let mut n = [(0.0, 1.0); N];
        for i in 0..N {
            let (a, b) = (p[i.saturating_sub(1)], p[(i + 1).min(N - 1)]);
            let (tx, ty) = (b.0 - a.0, b.1 - a.1);
            let d = hypot(tx, ty).max(1e-6);
            n[i] = (-ty / d, tx / d);
        }
        Spine { p, n }
    }

    /// Point at `u` along the spine, `off` to the belly side (negative = towards the back).
    fn at(&self, u: f64, off: f64) -> (f64, f64) {
        let f = clamp(u, 0.0, 1.0) * (N - 1) as f64;
        let i = (f.floor() as usize).min(N - 2);
        let k = f - i as f64;
        let (p, q) = (self.p[i], self.p[i + 1]);
        let (n, m) = (self.n[i], self.n[i + 1]);
        (lerp(p.0, q.0, k) + lerp(n.0, m.0, k) * off, lerp(p.1, q.1, k) + lerp(n.1, m.1, k) * off)
    }

    /// Direction the spine points at `u`.
    fn dir(&self, u: f64) -> f64 {
        let i = ((clamp(u, 0.0, 1.0) * (N - 1) as f64) as usize).min(N - 1);
        (-self.n[i].0).atan2(self.n[i].1)
    }
}

/// Strip along the body between two offsets from the spine.
fn band(cr: &mut Canvas, sp: &Spine, u0: f64, u1: f64, outer: impl Fn(f64) -> f64, inner: impl Fn(f64) -> f64) {
    const M: usize = 22;
    for i in 0..=M {
        let u = lerp(u0, u1, i as f64 / M as f64);
        let (x, y) = sp.at(u, outer(u));
        if i == 0 {
            cr.move_to(x, y);
        } else {
            cr.line_to(x, y);
        }
    }
    for i in (0..=M).rev() {
        let u = lerp(u0, u1, i as f64 / M as f64);
        let (x, y) = sp.at(u, inner(u));
        cr.line_to(x, y);
    }
    cr.close_path();
}

/// A dolphin at (x, y): counter-shaded, back and fins in the state colour.
fn dolphin(cr: &mut Canvas, x: f64, y: f64, p: &Pose) {
    let a = p.a;
    let c = p.c;
    let back = lerp3(c, NAVY, 0.14);
    let cape = lerp3(c, NAVY, 0.42);
    let far = lerp3(c, NAVY, 0.62);
    let flank = lerp3(back, BELLY, 0.5);
    let sp = Spine::new(p.tail, p.amp, p.bend);
    let top = |u: f64| profile(u).0;
    let bot = |u: f64| profile(u).1;

    cr.save();
    cr.translate(x, y);
    cr.rotate(p.ang);
    let flip = if p.flip.abs() < 0.24 { 0.24f64.copysign(p.flip) } else { p.flip };
    cr.scale(p.r, p.r * flip);
    cr.set_line_cap(LineCap::Round);
    cr.set_line_join(LineJoin::Round);

    // flipper of the far side
    let paddle = (p.tail * 0.5).sin() * 0.03;
    let (f0, f1) = (sp.at(0.80, bot(0.80) * 0.8), sp.at(0.73, bot(0.73) * 0.9));
    let tip = sp.at(0.68, bot(0.68) + 0.20 - paddle);
    let (c0, c1) = (sp.at(0.79, bot(0.79) + 0.17), sp.at(0.71, bot(0.71) + 0.04));
    cr.move_to(f0.0, f0.1);
    cr.quad_to(c0.0, c0.1, tip.0, tip.1);
    cr.quad_to(c1.0, c1.1, f1.0, f1.1);
    cr.close_path();
    cr.set_rgba(far, a);
    cr.fill();

    // tail flukes: they lie flat, so from the side they are a narrow swept crescent that rocks with the beat
    let t0 = sp.at(0.0, 0.0);
    cr.save();
    cr.translate(t0.0, t0.1);
    cr.rotate(sp.dir(0.0) + p.amp * 3.2 * (p.tail - 2.3).cos());
    cr.move_to(0.09, -0.04);
    cr.quad_to(-0.10, -0.06, -0.33, -0.17);
    cr.quad_to(-0.22, -0.05, -0.18, 0.0);
    cr.line_to(0.09, 0.0);
    cr.close_path();
    cr.set_rgba(far, a);
    cr.fill();
    cr.move_to(0.09, 0.045);
    cr.quad_to(-0.10, 0.07, -0.38, 0.21);
    cr.quad_to(-0.24, 0.06, -0.18, 0.0);
    cr.line_to(0.09, -0.01);
    cr.close_path();
    cr.set_rgba(cape, a);
    cr.fill();
    cr.restore();

    // dorsal fin, curved back
    let (d0, d1) = (sp.at(0.65, -top(0.65) * 0.9), sp.at(0.46, -top(0.46) * 0.9));
    let dt = sp.at(0.425, -top(0.425) - 0.33);
    let (e0, e1) = (sp.at(0.56, -top(0.56) - 0.31), sp.at(0.49, -top(0.49) - 0.08));
    cr.move_to(d0.0, d0.1);
    cr.quad_to(e0.0, e0.1, dt.0, dt.1);
    cr.quad_to(e1.0, e1.1, d1.0, d1.1);
    cr.close_path();
    cr.set_rgba(cape, a);
    cr.fill();

    // body
    let mut pts = [(0.0, 0.0); 2 * N];
    for i in 0..N {
        let u = i as f64 / (N - 1) as f64;
        let (t, b) = profile(u);
        pts[i] = sp.at(u, -t);
        pts[2 * N - 1 - i] = sp.at(u, b);
    }
    smooth_loop(cr, &pts);
    cr.set_rgba(back, a);
    cr.fill();
    // darker cape along the back
    let ends = |u: f64, u0: f64, u1: f64, k: f64| smooth(((u - u0) / k).min((u1 - u) / k));
    band(cr, &sp, 0.03, 0.91, |u| -top(u) * 0.985, |u| -top(u) * (1.0 - 0.42 * ends(u, 0.03, 0.91, 0.16)));
    cr.set_rgba(cape, 0.85 * a);
    cr.fill();
    // lighter flank, pale belly and lower jaw
    let line = |u: f64| lerp(0.30, 0.14, smooth((u - 0.5) / 0.4));
    band(cr, &sp, 0.08, 0.87, |u| bot(u) * 0.98, |u| bot(u) * (1.0 - (1.22 - line(u)) * ends(u, 0.08, 0.87, 0.12)));
    cr.set_rgba(flank, a);
    cr.fill();
    band(cr, &sp, 0.06, 0.992, |u| bot(u) * 0.985, |u| bot(u) * (1.0 - (1.0 - line(u)) * ends(u, 0.06, 0.992, 0.035).min(ends(u, 0.06, 2.0, 0.2))));
    cr.set_rgba(BELLY, a);
    cr.fill();
    // rim light along the back
    for i in 0..=20 {
        let u = lerp(0.17, 0.87, i as f64 / 20.0);
        let (px, py) = sp.at(u, -top(u) + 0.012);
        if i == 0 {
            cr.move_to(px, py);
        } else {
            cr.line_to(px, py);
        }
    }
    cr.set_rgba(lighten(c, 0.6), 0.6 * a);
    cr.set_line_width(0.018);
    cr.stroke();

    // beak: open (chattering) or a closed line with a hint of a smile
    if p.jaw > 0.03 {
        let (h, up, lo) = (sp.at(0.895, 0.025), sp.at(1.0, 0.0), sp.at(0.985, 0.04 + 0.13 * p.jaw));
        poly(cr, &[h, up, lo]);
        cr.set_source_rgba(0.42, 0.12, 0.18, a);
        cr.fill();
        cr.move_to(h.0, h.1);
        cr.line_to(lo.0, lo.1);
        cr.set_rgba(BELLY, a);
        cr.set_line_width(0.05);
        cr.stroke();
    } else {
        let (m0, m1, m2, m3) = (sp.at(0.997, 0.012), sp.at(0.94, 0.034), sp.at(0.895, 0.022), sp.at(0.872, -0.004));
        cr.move_to(m0.0, m0.1);
        cr.quad_to(m1.0, m1.1, m2.0, m2.1);
        cr.line_to(m3.0, m3.1);
        cr.set_rgba(NAVY, 0.75 * a);
        cr.set_line_width(0.014);
        cr.stroke();
    }
    // eye
    let (ex, ey) = sp.at(0.868, -0.045);
    let er = if p.calf { 0.046 } else { 0.036 };
    if p.eye == Eye::Shut {
        cr.move_to(ex - er, ey - er * 0.2);
        cr.quad_to(ex, ey + er * 0.9, ex + er, ey - er * 0.2);
        cr.set_rgba(NAVY, 0.9 * a);
        cr.set_line_width(0.016);
        cr.stroke();
    } else {
        cr.set_source_rgba(0.02, 0.04, 0.09, a);
        cr.circle(ex, ey, er);
        cr.fill();
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.9 * a);
        cr.circle(ex + er * 0.3, ey - er * 0.35, er * 0.34);
        cr.fill();
        if p.eye == Eye::Sad {
            // heavy lid
            cr.move_to(ex - er * 1.2, ey - er * 0.75);
            cr.line_to(ex + er * 1.2, ey - er * 0.25);
            cr.set_rgba(cape, a);
            cr.set_line_width(er * 0.9);
            cr.stroke();
        }
    }

    // flipper of the near side
    let (f0, f1) = (sp.at(0.775, bot(0.775) * 0.72), sp.at(0.695, bot(0.695) * 0.86));
    let tip = sp.at(0.60, bot(0.60) + 0.27 + paddle);
    let (c0, c1) = (sp.at(0.745, bot(0.745) + 0.27), sp.at(0.655, bot(0.655) + 0.04));
    cr.move_to(f0.0, f0.1);
    cr.quad_to(c0.0, c0.1, tip.0, tip.1);
    cr.quad_to(c1.0, c1.1, f1.0, f1.1);
    cr.close_path();
    cr.set_rgba(cape, a);
    cr.fill();
    cr.restore();
    cr.set_line_cap(LineCap::Butt);
    cr.set_line_join(LineJoin::Miter);
}

/// Push away from the dolphins that are too close; grows to 1 as they come to touch.
fn crowd(b: &Base, others: &[Other]) -> (f64, f64) {
    let (mut px, mut py) = (0.0, 0.0);
    for o in others {
        if o.index == b.index || o.dying {
            continue;
        }
        let (ex, ey) = (b.x - o.x, b.y - o.y);
        let d = hypot(ex, ey);
        let md = (b.radius + o.radius) * 1.6;
        if 0.01 < d && d < md {
            px += ex / d * (md - d) / md;
            py += ey / d * (md - d) / md;
        }
    }
    (px, py)
}

#[derive(Clone, Copy, PartialEq)]
enum Leap {
    None,
    /// racing up to the surface
    Rising,
    /// flying through the air
    Air,
}

/// A calf (sub-agent): where it is and which way it points.
#[derive(Clone, Copy)]
struct Calf {
    x: f64,
    y: f64,
    ang: f64,
}

#[derive(Clone)]
pub struct Dolphin {
    b: Base,
    /// direction the body is drawn in
    ang: f64,
    /// 1 = swimming to the right, -1 = to the left
    side: f64,
    /// -1..1 belly down or up on screen (rolls over smoothly)
    roll: f64,
    tail: f64,
    amp: f64,
    speed: f64,
    /// smoothed turn rate, bends the body
    turn: f64,
    /// 0..1 standing on its tail (waiting)
    upright: f64,
    leap: Leap,
    leap_dir: f64,
    leap_t: f64,
    next_leap: f64,
    /// 1 -> 0 while it does a barrel roll
    trick: f64,
    next_trick: f64,
    /// > 0: swimming on its back
    back_t: f64,
    next_back: f64,
    next_blow: f64,
    calves: Vec<Calf>,
    /// the other dolphins (x, y, radius) and the screen (width, height, scale, strip), for placing the label
    near: Vec<(f64, f64, f64)>,
    screen: (f64, f64, f64, f64),
}

impl Dolphin {
    /// Screen position of a point along the body axis (1 = beak tip), `up` towards the back.
    fn along(&self, f: f64, up: f64) -> (f64, f64) {
        let (r, (sa, ca)) = (self.b.radius, self.ang.sin_cos());
        let k = if self.roll < 0.0 { -1.0 } else { 1.0 };
        (self.b.x + ca * r * f + sa * r * up * k, self.b.y + sa * r * f - ca * r * up * k)
    }

    /// Free swimming with the pod; working dolphins also leap.
    fn swim(&mut self, env: &mut Env, world: &mut Dolphins, others: &[Other], dt: f64, t: f64) {
        let s = env.s;
        let (surf, bed, w) = (world.surf, world.bed, env.w);
        let working = self.b.state() == State::Working;
        let calm = self.b.look() == Look::Background;
        let b = &mut self.b;
        let r = b.radius;
        let h0 = b.heading;
        if self.leap == Leap::Rising {
            self.leap_t += dt;
            let want = (-LEAP_ANGLE.sin()).atan2(self.leap_dir * LEAP_ANGLE.cos());
            b.heading += clamp(wrap(want - b.heading) * 5.0, -4.5, 4.5) * dt;
            self.speed = lerp(self.speed, LEAP_SPEED * s, (dt * 3.5).min(1.0));
            if b.y < surf + r * 0.15 && b.heading.sin() < 0.0 {
                // out of the water: from here on it only falls
                let el = clamp((-b.heading.sin()).atan2(b.heading.cos().abs()), 0.85, 1.25);
                let v = LEAP_SPEED * s;
                b.vx = self.leap_dir * el.cos() * v;
                b.vy = -el.sin() * v;
                self.leap = Leap::Air;
                world.splash(&env.rng, s, b.x, r);
                return;
            }
            if self.leap_t > 4.0 {
                self.leap = Leap::None;
                self.next_leap = t + 5.0;
            }
        } else {
            let (cruise, rate, noise) = match (working, calm) {
                (true, false) => (150.0, 2.4, 1.1),
                (true, true) => (105.0, 1.8, 0.8),
                _ => (34.0, 0.9, 0.45),
            };
            let sd = b.seed;
            b.heading += ((t * 0.9 + sd).sin() * 0.9 + (t * 0.53 + sd * 2.3).sin() * 0.7 + (t * 0.21 + sd * 1.7).sin() * 0.5) * noise * dt;
            let (mut dx, mut dy) = (b.heading.cos(), b.heading.sin());
            // keep with the pod: the further away, the stronger the pull back to it
            let (px, mut py) = world.pod;
            if !working {
                py = surf + (bed - surf) * (0.09 + 0.09 * hash(sd)); // idle dolphins drift near the surface
            }
            let (ox, oy) = (px - b.x, py - b.y);
            let d = hypot(ox, oy).max(1.0);
            let r0 = w * if working { 0.19 } else { 0.15 };
            let k = clamp((d - r0) / r0, 0.0, 1.6) * 1.3;
            dx += ox / d * k;
            dy += oy / d * k;
            // travel the way the pod travels
            dx += world.pod_dir.0 * 0.4;
            dy += world.pod_dir.1 * 0.4;
            // but keep a distance from each other
            let (cx, cy) = crowd(b, others);
            dx += cx * 5.0;
            dy += cy * 5.0;
            // dolphins travel level and only arc up and down
            dy *= 0.4;
            if !working {
                dy += clamp((py - b.y) / (140.0 * s), -1.0, 1.0) * 0.7;
            }
            // stay in the water
            let m = r * 1.3;
            let (left, top, right, bottom) = (50.0 * s + r, surf + r * 0.5, w - 50.0 * s - r, bed - r * 1.3);
            if b.x < left + m {
                dx += ((left + m - b.x) / m).min(2.5) * 1.5;
            }
            if b.x > right - m {
                dx -= ((b.x - right + m) / m).min(2.5) * 1.5;
            }
            if b.y < top + m {
                dy += ((top + m - b.y) / m).min(2.5) * 1.5;
            }
            if b.y > bottom - m {
                dy -= ((b.y - bottom + m) / m).min(2.5) * 1.5;
            }
            let edge = b.y > bottom - m || b.y < top + m * 0.5;
            let rate = if edge { rate * 1.8 } else { rate };
            b.heading += clamp(wrap(dy.atan2(dx) - b.heading) * 2.5, -rate, rate) * dt;
            let pace = 1.0 + 0.25 * (t * 0.6 + sd).sin();
            self.speed = lerp(self.speed, cruise * s * pace * (1.0 + 0.6 * self.trick), (dt * 1.6).min(1.0));

            if working && t > self.next_leap {
                // a leap needs room: where it leaves the water and where it comes down again
                let depth = (b.y - surf).max(0.0);
                let range = LEAP_SPEED * s * LEAP_ANGLE.cos() * 2.0 * LEAP_SPEED * LEAP_ANGLE.sin() / GRAVITY;
                let first = if b.heading.cos() >= 0.0 { 1.0 } else { -1.0 };
                self.next_leap = t + 2.0;
                for dir in [first, -first] {
                    let out = b.x + dir * depth / LEAP_ANGLE.tan();
                    let down = out + dir * range;
                    let free = |x: f64| x > w * 0.06 && x < w - 400.0 * s;
                    if free(out) && free(down) {
                        self.leap = Leap::Rising;
                        self.leap_dir = dir;
                        self.leap_t = 0.0;
                        break;
                    }
                }
            }
        }
        b.heading = wrap(b.heading);
        if dt > 0.0 {
            self.turn = lerp(self.turn, wrap(b.heading - h0) / dt, (dt * 4.0).min(1.0));
        }
        b.vx = b.heading.cos() * self.speed;
        b.vy = b.heading.sin() * self.speed;
        b.x += b.vx * dt;
        b.y += b.vy * dt;
        if self.leap == Leap::None && b.y < surf + r * 0.34 {
            // the back may break the surface, no more
            b.y = surf + r * 0.34;
            if b.heading.sin() < 0.0 {
                b.heading = (b.heading.sin() * (1.0 - (dt * 6.0).min(1.0))).atan2(b.heading.cos());
            }
        }
        b.y = b.y.min(bed - r);
    }
}

impl ActorT<Dolphins> for Dolphin {
    const BASE_RADIUS: f64 = 70.0;
    const COMPANION_SCALE: f64 = 0.42;

    fn new(base: Base, env: &Env, _world: &mut Dolphins) -> Self {
        let rng = &env.rng;
        let rest = base.state() == State::Rest;
        let side = if rest || base.heading.cos() >= 0.0 { 1.0 } else { -1.0 };
        Dolphin {
            ang: if rest { 0.0 } else { base.heading },
            side,
            roll: side,
            tail: rng.uniform(0.0, TAU),
            amp: 0.05,
            speed: 0.0,
            turn: 0.0,
            upright: 0.0,
            leap: Leap::None,
            leap_dir: 1.0,
            leap_t: 0.0,
            next_leap: env.t + rng.uniform(5.0, 16.0),
            trick: 0.0,
            next_trick: 0.0,
            back_t: 0.0,
            next_back: env.t + rng.uniform(8.0, 30.0),
            next_blow: env.t + rng.uniform(1.0, 6.0),
            calves: Vec::new(),
            near: Vec::new(),
            screen: (env.w, env.h, env.s, env.strip_h),
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
        self.leap = Leap::None;
        self.trick = 0.0;
        self.back_t = 0.0;
        self.upright = 0.0;
        self.calves.clear();
    }

    fn mv(&mut self, env: &mut Env, world: &mut Dolphins, others: &[Other], dt: f64, t: f64, target: Option<(f64, f64)>) {
        let s = env.s;
        let st = self.b.state();
        let r = self.b.radius;
        let (surf, bed) = (world.surf, world.bed);
        self.screen = (env.w, env.h, s, env.strip_h);
        self.near.clear();
        self.near.extend(others.iter().filter(|o| o.index != self.b.index && !o.dying).map(|o| (o.x, o.y, o.radius)));
        self.trick = (self.trick - dt / TRICK).max(0.0);
        self.back_t = (self.back_t - dt).max(0.0);
        let mut upright = 0.0;
        let want;
        if self.leap == Leap::Air {
            // through the air in an arc, nose first
            let b = &mut self.b;
            b.vy += GRAVITY * s * dt;
            b.x += b.vx * dt;
            b.y += b.vy * dt;
            b.heading = b.vy.atan2(b.vx);
            self.speed = hypot(b.vx, b.vy);
            if b.y > surf && b.vy > 0.0 {
                world.splash(&env.rng, s, b.x, r);
                self.leap = Leap::None;
                self.speed *= 0.7;
                let calm = b.look() == Look::Background;
                self.next_leap = t + env.rng.uniform(14.0, 30.0) * if calm { 1.6 } else { 1.0 };
            }
            self.turn = 0.0;
            want = self.b.heading;
        } else {
            match st {
                State::Rest => {
                    // asleep just below the surface, rising and sinking with its breath
                    let b = &mut self.b;
                    let ty = surf + r * 1.8 + (t * 0.8).sin() * r * 0.07;
                    b.y = lerp(b.y, ty, (dt * 1.5).min(1.0));
                    b.vx = 0.0;
                    b.vy = 0.0;
                    self.speed = 0.0;
                    self.turn = 0.0;
                    self.leap = Leap::None;
                    want = if self.side > 0.0 { -0.05 } else { PI + 0.05 };
                }
                State::Blocked => {
                    // hangs head down: sinks slowly, then corrects with a lazy flick
                    let b = &mut self.b;
                    let up = (t * 0.5 + b.seed).sin() > 0.75;
                    let (cx, cy) = crowd(b, others); // drifts out of the way of the others
                    b.vx = lerp(b.vx, cx * 90.0 * s, (dt * 1.5).min(1.0));
                    b.vy = lerp(b.vy, if up { -26.0 * s } else { 7.0 * s } + cy * 90.0 * s, (dt * 1.5).min(1.0));
                    b.x = clamp(b.x + b.vx * dt, r * 1.5, env.w - r * 1.5);
                    b.y = clamp(b.y + b.vy * dt, surf + r * 1.3, bed - r * 1.3);
                    self.speed = if up { 40.0 * s } else { 0.0 };
                    self.turn = 0.0;
                    self.leap = Leap::None;
                    let tilt = 0.5 + (t * 0.4 + b.seed).sin() * 0.05;
                    want = if self.side > 0.0 { tilt } else { PI - tilt };
                }
                State::Waiting => {
                    // to its place at the front, then up on the tail
                    let b = &mut self.b;
                    let (tx, ty) = target.unwrap_or((b.x, b.y));
                    let (ox, oy) = (tx - b.x, ty - b.y);
                    let d = hypot(ox, oy);
                    let sp = (190.0 * s).min(d * 1.4);
                    let (gx, gy) = if d > 1.0 { (ox / d * sp, oy / d * sp) } else { (0.0, 0.0) };
                    let k = (dt * 2.5).min(1.0);
                    b.vx = lerp(b.vx, gx, k);
                    b.vy = lerp(b.vy, gy, k);
                    b.x += b.vx * dt;
                    b.y += b.vy * dt;
                    self.speed = hypot(b.vx, b.vy);
                    self.turn = 0.0;
                    self.leap = Leap::None;
                    if d < 130.0 * s {
                        upright = 1.0;
                        let lean = 0.2 + (t * 1.4 + b.seed).sin() * 0.1;
                        want = -PI / 2.0 + lean * self.side;
                    } else {
                        if self.speed > 5.0 * s {
                            b.heading = b.vy.atan2(b.vx);
                        }
                        want = b.heading;
                    }
                }
                _ => {
                    self.swim(env, world, others, dt, t);
                    want = self.b.heading;
                }
            }
        }
        let air = self.leap == Leap::Air;
        self.upright = lerp(self.upright, upright, (dt * 3.0).min(1.0));
        self.ang = angle_lerp(self.ang, want, (dt * if air { 30.0 } else { 7.0 }).min(1.0));
        if upright < 0.5 {
            let cx = self.ang.cos();
            if cx.abs() > 0.25 {
                self.side = cx.signum();
            }
        }
        // idle: now and then a while on its back, and a few bubbles
        if st == State::Idle && !air {
            if t > self.next_back {
                self.back_t = 3.5;
                self.next_back = t + env.rng.uniform(18.0, 40.0);
            }
            if t > self.next_blow {
                self.next_blow = t + env.rng.uniform(3.0, 8.0);
                let (hx, hy) = self.along(0.75, 0.25);
                for _ in 0..3 {
                    world.bubble(&env.rng, hx + env.rng.uniform(-6.0, 6.0) * s, hy + env.rng.uniform(-8.0, 4.0) * s, s, 1.0);
                }
            }
        } else if st != State::Idle {
            self.back_t = 0.0;
        }
        let belly = if self.back_t > 0.0 { -1.0 } else { 1.0 };
        self.roll = lerp(self.roll, self.side * belly, (dt * 5.0).min(1.0));
        let f = clamp(self.speed / (170.0 * s), 0.0, 1.3);
        let (beat, amp) = match st {
            State::Rest => (0.9, 0.02),
            State::Blocked => (0.8 + 6.0 * f, 0.025 + 0.2 * f),
            _ if air => (5.0, 0.03),
            _ => (3.0 + 7.5 * f + 5.0 * self.upright, 0.04 + 0.11 * f.min(1.0) + 0.05 * self.upright),
        };
        self.tail += dt * beat;
        self.amp = lerp(self.amp, amp, (dt * 3.0).min(1.0));

        // calves keep their place behind the parent, under water even while it leaps
        let n = self.b.info.subagents.len();
        self.calves.truncate(n);
        while self.calves.len() < n {
            self.calves.push(Calf { x: self.b.x, y: self.b.y, ang: self.ang });
        }
        let base = if air { (self.ang.sin() * 0.2).atan2(self.ang.cos()) } else { self.ang };
        let (sa, ca) = base.sin_cos();
        for (i, c) in self.calves.iter_mut().enumerate() {
            let back = r * (0.55 + 0.75 * (i / 2 + 1) as f64);
            let off = r * 0.55 * if i % 2 == 0 { self.side } else { -self.side };
            let tx = self.b.x - ca * back - sa * off;
            let ty = clamp(self.b.y - sa * back + ca * off, surf + r * 0.4, bed - r * 0.3);
            let k = (dt * 3.5).min(1.0);
            let (ox, oy) = (c.x, c.y);
            c.x = lerp(c.x, tx, k);
            c.y = lerp(c.y, ty, k);
            let (mx, my) = (c.x - ox, c.y - oy);
            let dir = if hypot(mx, my) > 25.0 * s * dt { my.atan2(mx) } else { base };
            c.ang = angle_lerp(c.ang, dir, (dt * 5.0).min(1.0));
        }
    }

    /// Beside, above or below the dolphin - but never where the screen edge would push the label back
    /// over it, and not on top of another dolphin of the pod while there is a free place.
    fn label_candidates(&self, pw: f64, ph: f64) -> Vec<(f64, f64)> {
        let b = &self.b;
        let r = b.radius;
        let (w, h, s, strip) = self.screen;
        let (right, left) = (b.x + r * 1.45, b.x - r * 1.45 - pw);
        let y = b.y - r * 0.9;
        let mut v = vec![
            (right, y),
            (left, y),
            (right, y + ph * 0.8),
            (left, y + ph * 0.8),
            (right, y - ph * 0.8),
            (left, y - ph * 0.8),
            (b.x - pw / 2.0, b.y + r * 1.0),
            (b.x - pw / 2.0, b.y - r * 1.0 - ph),
        ];
        let (sa, ca) = self.ang.sin_cos();
        let good = |p: (f64, f64)| {
            let (x, y) = (clamp(p.0, 20.0 * s, w - pw - 20.0 * s), clamp(p.1, 20.0 * s, h - strip - ph - 10.0 * s));
            let hit = |cx: f64, cy: f64, rad: f64| hypot(cx - clamp(cx, x, x + pw), cy - clamp(cy, y, y + ph)) < rad;
            let own = [-0.8, 0.0, 0.8].iter().any(|k| hit(b.x + ca * r * k, b.y + sa * r * k, r * 0.4));
            !own && !self.near.iter().any(|&(ox, oy, or)| hit(ox, oy, or * 0.85))
        };
        if let Some(&first) = v.iter().find(|p| good(**p)) {
            for p in v.iter_mut() {
                if !good(*p) {
                    *p = first;
                }
            }
        }
        v
    }

    fn companion_spots(&self, _env: &Env, _t: f64) -> Vec<(f64, f64, f64)> {
        self.calves.iter().map(|c| (c.x, c.y, c.ang)).collect()
    }

    /// Calves: small dolphins in the working colour that trail in formation and face the way they swim.
    fn draw_companions(&self, cr: &mut Canvas, _env: &Env, _world: &Dolphins, _t: f64) {
        let b = &self.b;
        if b.alpha < 0.05 {
            return;
        }
        for (i, c) in self.calves.iter().enumerate() {
            let pose = Pose {
                r: b.radius * Self::COMPANION_SCALE,
                ang: c.ang,
                flip: clamp(c.ang.cos() * 5.0, -1.0, 1.0),
                tail: self.tail * 1.25 + i as f64 * 1.9,
                amp: self.amp.max(0.07),
                bend: 0.0,
                c: WORKING_COLOR,
                a: b.alpha,
                eye: Eye::Open,
                jaw: 0.0,
                calf: true,
            };
            dolphin(cr, c.x, c.y, &pose);
        }
    }

    fn draw(&self, cr: &mut Canvas, env: &Env, world: &Dolphins, t: f64) {
        let s = env.s;
        let b = &self.b;
        let st = b.state();
        // the sleeper settles at 80 % opacity, which would let fins show through the body
        let solid = if st == State::Rest { (b.alpha / 0.8).min(1.0) } else { b.alpha };
        if solid < 0.01 {
            return;
        }
        let a = solid * (0.85 + 0.15 * b.flicker);
        let (r, c) = (b.radius, b.color);
        let y = b.y + (t * 2.2 + b.seed).sin() * r * 0.07 * self.upright;
        if st == State::Waiting {
            cr.pulse_rings(b.x, y, r, c, a, t, s);
        }
        let glow = match b.look() {
            Look::Waiting => 1.0,
            Look::Working | Look::Background => 0.75,
            Look::Blocked => 0.9 * b.flicker,
            Look::Idle => 0.55,
            Look::Rest => 0.35,
        };
        if let Some(pic) = world.glows.get(look_index(b.look())) {
            cr.picture(pic, (b.x - world.glow_r).round(), (y - world.glow_r).round(), glow * solid);
        }
        let mut ang = self.ang;
        if st == State::Idle {
            ang += (t * 1.1 + b.seed).sin() * 0.07 * self.side; // nodding
        }
        let spin = if self.trick > 0.0 { (TAU * (1.0 - self.trick)).cos() } else { 1.0 };
        let chatter = if (t * 1.3 + b.seed).sin() > -0.3 { 0.5 + 0.5 * (t * 15.0).sin() } else { 0.0 };
        let pose = Pose {
            r,
            ang,
            flip: self.roll * spin,
            tail: self.tail,
            amp: self.amp,
            bend: clamp(self.turn * 0.32 * self.roll.signum(), -1.0, 1.0),
            c,
            a,
            eye: match st {
                State::Rest => Eye::Shut,
                State::Blocked => Eye::Sad,
                _ if self.back_t > 0.0 || (t * 0.31 + b.seed).fract() < 0.03 => Eye::Shut,
                _ => Eye::Open,
            },
            jaw: chatter * self.upright,
            calf: false,
        };
        dolphin(cr, b.x, y, &pose);

        if self.upright > 0.5 {
            // chattering: little sound arcs off the beak
            let (bx, by) = (b.x + ang.cos() * r * 1.3, y + ang.sin() * r * 1.3);
            cr.set_line_cap(LineCap::Round);
            cr.set_line_width(2.5 * s);
            for k in 0..3 {
                let f = (t * 1.6 + k as f64 / 3.0).fract();
                cr.new_sub_path();
                cr.arc(bx, by, r * (0.12 + 0.5 * f), ang - 0.55, ang + 0.55);
                cr.set_rgba(lighten(c, 0.4), (1.0 - f) * 0.8 * a * chatter.max(0.25));
                cr.stroke();
            }
            cr.set_line_cap(LineCap::Butt);
        }
        if st == State::Rest {
            // z z z in bubbles rising from the blowhole
            let (hx, hy) = self.along(0.72, 0.3);
            for k in 0..3 {
                let f = (t * 0.22 + k as f64 / 3.0).fract();
                let fade = (PI * f).sin() * solid;
                let (zx, zy) = (hx + (f * 7.0 + k as f64).sin() * 6.0 * s + f * 30.0 * s, lerp(hy, world.surf + 14.0 * s, f));
                let br = (8.0 + 8.0 * f) * s;
                cr.set_source_rgba(0.8, 0.95, 1.0, 0.12 * fade);
                cr.circle(zx, zy, br);
                cr.fill_preserve();
                cr.set_source_rgba(0.85, 0.97, 1.0, 0.6 * fade);
                cr.set_line_width((1.2 * s).max(1.0));
                cr.stroke();
                let lay = text::layout("z", (10.0 + 9.0 * f) * s, weight::BOLD, None);
                let (tw, th) = lay.get_pixel_size();
                text::show(cr, &lay, zx - tw / 2.0, zy - th / 2.0, (1.0, 1.0, 1.0), 0.85 * fade, 0.0);
            }
        }
    }
}

// ---------------------------------------------------------------------------------- the sea

struct Weed {
    x: f64,
    h: f64,
    w: f64,
    ph: f64,
    c: Rgb,
}

/// Plankton: x, y, depth (0.2..1), phase.
struct Mote {
    x: f64,
    y: f64,
    z: f64,
    ph: f64,
}

struct Bubble {
    x: f64,
    y: f64,
    r: f64,
    vy: f64,
    ph: f64,
}

/// A drop of a splash, flying above the water.
struct Drop {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    r: f64,
}

/// The bubble ring of a trick: a circle seen from the side, the dolphin swims through it.
struct Ring {
    x: f64,
    y: f64,
    ang: f64,
    r: f64,
    age: f64,
    c: Rgb,
}

struct Ship {
    kind: usize,
    x: f64,
    dir: f64,
}

/// Half length at the waterline and speed (px at scale 1) of sailing boat, fishing boat, freighter, steamer.
const SHIPS: [(f64, f64); 4] = [(64.0, 30.0), (74.0, 25.0), (172.0, 21.0), (100.0, 27.0)];

struct Sub {
    x: f64,
    y: f64,
    dir: f64,
    next_bubble: f64,
}

/// A turtle (kind 0) or a manta (kind 1) gliding through.
struct Glider {
    kind: usize,
    x: f64,
    y: f64,
    dir: f64,
}

struct School {
    x: f64,
    y: f64,
    h: f64,
    face: f64,
    ph: f64,
    c: Rgb,
    /// offset x, y (px at scale 1), size, phase
    fish: Vec<(f64, f64, f64, f64)>,
}

struct Jelly {
    x: f64,
    y: f64,
    ph: f64,
    r: f64,
}

pub struct Dolphins {
    bg: Option<Picture>,
    shaft: Option<Picture>,
    /// soft glow in each state colour (index = `look_index`)
    glows: Vec<Picture>,
    glow_r: f64,
    surf: f64,
    bed: f64,
    /// the point the pod gathers around, and the direction it travels in
    pod: (f64, f64),
    pod_dir: (f64, f64),
    weeds: Vec<Weed>,
    motes: Vec<Mote>,
    bubbles: Vec<Bubble>,
    drops: Vec<Drop>,
    /// foam where something broke the surface: x, age, size
    foam: Vec<(f64, f64, f64)>,
    rings: Vec<Ring>,
    ship: Option<Ship>,
    next_ship: f64,
    ships: usize,
    sub: Option<Sub>,
    next_sub: f64,
    glider: Option<Glider>,
    next_glider: f64,
    gliders: usize,
    schools: Vec<School>,
    jellies: Vec<Jelly>,
    /// crab on the seabed: x, direction, phase of its legs
    crab: (f64, f64, f64),
    vents: [f64; 2],
    next_bubble: f64,
}

fn bed_y(bed: f64, x: f64, w: f64, s: f64) -> f64 {
    let u = x / w * TAU;
    bed + ((u * 1.5 + 1.0).sin() * 9.0 + (u * 4.0).sin() * 3.5) * s
}

fn wave_y(surf: f64, x: f64, t: f64, s: f64) -> f64 {
    surf + ((x / (41.0 * s) + t * 1.1).sin() * 3.6 + (x / (15.5 * s) - t * 0.8).sin() * 1.6) * s
}

fn cloud(cr: &mut Canvas, x: f64, y: f64, k: f64) {
    cr.set_source_rgba(1.0, 0.99, 0.97, 0.92);
    for &(dx, dy, rx, ry) in &[(-48.0, 4.0, 34.0, 15.0), (-14.0, -8.0, 36.0, 24.0), (26.0, -3.0, 32.0, 19.0), (58.0, 6.0, 26.0, 12.0), (4.0, 9.0, 66.0, 12.0)] {
        cr.ellipse(x + dx * k, y + dy * k, rx * k, ry * k);
    }
    cr.fill();
    cr.set_source_rgba(1.0, 0.86, 0.70, 0.35); // warm underside
    cr.ellipse(x + 4.0 * k, y + 14.0 * k, 60.0 * k, 6.0 * k);
    cr.fill();
}

fn rock(cr: &mut Canvas, x: f64, y: f64, w: f64, h: f64, c: Rgb) {
    cr.move_to(x - w, y + h * 0.3);
    cr.curve_to(x - w * 1.05, y - h * 0.7, x - w * 0.35, y - h * 1.1, x + w * 0.1, y - h);
    cr.curve_to(x + w * 0.6, y - h * 0.9, x + w * 1.1, y - h * 0.4, x + w, y + h * 0.3);
    cr.close_path();
    cr.set_rgb(c);
    cr.fill();
    cr.move_to(x - w * 0.8, y - h * 0.35);
    cr.curve_to(x - w * 0.6, y - h * 0.9, x - w * 0.2, y - h * 1.0, x + w * 0.15, y - h * 0.92);
    cr.curve_to(x - w * 0.2, y - h * 0.8, x - w * 0.5, y - h * 0.6, x - w * 0.8, y - h * 0.35);
    cr.set_rgba(lighten(c, 0.35), 0.5);
    cr.fill();
}

/// Branching coral: every arm forks twice.
fn coral(cr: &mut Canvas, x: f64, y: f64, ang: f64, len: f64, depth: u32, rng: &Rng) {
    let (x1, y1) = (x + ang.cos() * len, y + ang.sin() * len);
    cr.set_line_width(len * 0.2 + 1.0);
    cr.move_to(x, y);
    cr.line_to(x1, y1);
    cr.stroke();
    if depth > 0 {
        coral(cr, x1, y1, ang - rng.uniform(0.3, 0.7), len * 0.72, depth - 1, rng);
        coral(cr, x1, y1, ang + rng.uniform(0.3, 0.7), len * 0.72, depth - 1, rng);
    }
}

/// Sea fan: a flat fan of thin ribs.
fn sea_fan(cr: &mut Canvas, x: f64, y: f64, r: f64, c: Rgb) {
    cr.move_to(x, y);
    cr.arc(x, y, r, -PI * 0.92, -PI * 0.08);
    cr.close_path();
    cr.set_rgba(c, 0.45);
    cr.fill();
    cr.set_rgba(c, 0.9);
    cr.set_line_width(r * 0.035 + 0.6);
    for k in 0..9 {
        let a = -PI * (0.90 - 0.10 * k as f64);
        cr.move_to(x, y);
        cr.quad_to(x + a.cos() * r * 0.5, y + a.sin() * r * 0.6, x + (a + 0.08).cos() * r, y + (a + 0.08).sin() * r);
    }
    cr.stroke();
}

fn paint_background(cr: &mut Canvas, env: &Env, surf: f64, bed: f64) {
    let (w, h, s) = (env.w, env.h, env.s);
    let rng = &env.rng;
    let hor = surf - 15.0 * s;
    // --- sky with a warm sun
    let mut g = Gradient::linear(0.0, 0.0, 0.0, hor);
    g.add_color_stop_rgb(0.0, 0.30, 0.56, 0.88);
    g.add_color_stop_rgb(0.6, 0.62, 0.80, 0.93);
    g.add_color_stop_rgb(1.0, 1.0, 0.90, 0.74);
    cr.set_source(&g);
    cr.rectangle(0.0, 0.0, w, surf + 8.0 * s);
    cr.fill();
    let (sx, sy) = (w * 0.20, surf * 0.44);
    cr.glow(sx, sy, 230.0 * s, (1.0, 0.93, 0.70), 0.60, 0.0);
    cr.glow(sx, sy, 80.0 * s, (1.0, 0.97, 0.82), 0.9, 0.3);
    cr.set_source_rgb(1.0, 0.98, 0.88);
    cr.circle(sx, sy, 27.0 * s);
    cr.fill();
    for &(fx, fy, k) in &[(0.07, 0.36, 0.8), (0.40, 0.30, 1.15), (0.56, 0.58, 0.7), (0.74, 0.34, 1.0), (0.93, 0.50, 0.85)] {
        cloud(cr, w * fx, surf * fy, k * s);
    }
    // --- far coast and an island with a lighthouse on the horizon
    cr.set_source_rgb(0.62, 0.74, 0.83);
    cr.move_to(w * 0.46, hor);
    for &(fx, dy) in &[(0.50, 9.0), (0.55, 16.0), (0.60, 12.0), (0.66, 24.0), (0.71, 20.0), (0.76, 30.0), (0.82, 18.0), (0.90, 22.0), (0.96, 10.0), (1.0, 14.0)] {
        cr.line_to(w * fx, hor - dy * s);
    }
    cr.line_to(w, hor);
    cr.close_path();
    cr.fill();
    let ix = w * 0.60;
    cr.set_source_rgb(0.34, 0.54, 0.46);
    cr.move_to(ix - 150.0 * s, hor);
    cr.curve_to(ix - 100.0 * s, hor - 34.0 * s, ix - 40.0 * s, hor - 50.0 * s, ix + 10.0 * s, hor - 30.0 * s);
    cr.curve_to(ix + 50.0 * s, hor - 44.0 * s, ix + 110.0 * s, hor - 18.0 * s, ix + 160.0 * s, hor);
    cr.close_path();
    cr.fill();
    cr.set_source_rgb(0.93, 0.87, 0.70); // beach
    cr.ellipse(ix + 10.0 * s, hor, 150.0 * s, 3.5 * s);
    cr.fill();
    cr.set_source_rgb(0.97, 0.96, 0.92); // lighthouse
    poly(cr, &[(ix + 96.0 * s, hor - 14.0 * s), (ix + 104.0 * s, hor - 14.0 * s), (ix + 102.5 * s, hor - 46.0 * s), (ix + 97.5 * s, hor - 46.0 * s)]);
    cr.fill();
    cr.set_source_rgb(0.74, 0.30, 0.26);
    cr.rectangle(ix + 96.5 * s, hor - 52.0 * s, 7.0 * s, 6.0 * s);
    cr.rectangle(ix + 97.0 * s, hor - 34.0 * s, 6.0 * s, 6.0 * s);
    cr.fill();
    // --- the far sea between the horizon and the cut through the water
    let mut g = Gradient::linear(0.0, hor, 0.0, surf + 8.0 * s);
    g.add_color_stop_rgb(0.0, 0.16, 0.46, 0.68);
    g.stop(1.0, SHALLOW, 1.0);
    cr.set_source(&g);
    cr.rectangle(0.0, hor, w, surf + 8.0 * s - hor);
    cr.fill();
    cr.set_source_rgba(1.0, 0.97, 0.85, 0.55); // glitter below the sun
    for k in 0..7 {
        let f = k as f64 / 6.0;
        cr.ellipse(sx + (rng.random() - 0.5) * 60.0 * s, hor + 2.0 * s + f * 11.0 * s, (10.0 + 22.0 * f) * s * rng.uniform(0.5, 1.0), 1.0 * s);
    }
    cr.fill();

    // --- under water: bright turquoise below the surface, deep blue at the bottom
    let top = surf + 7.0 * s;
    let mut g = Gradient::linear(0.0, top, 0.0, h);
    g.stop(0.0, SHALLOW, 1.0);
    g.add_color_stop_rgb(0.14, 0.11, 0.55, 0.72);
    g.add_color_stop_rgb(0.50, 0.04, 0.31, 0.55);
    g.add_color_stop_rgb(1.0, 0.015, 0.10, 0.27);
    cr.set_source(&g);
    cr.rectangle(0.0, top, w, h - top);
    cr.fill();
    cr.save();
    cr.rectangle(0.0, top, w, h - top);
    cr.clip();
    cr.glow(sx + w * 0.08, top, h * 0.75, (0.62, 0.97, 0.93), 0.30, 0.0); // light falling in below the sun
    cr.restore();
    // far rocks fading into the blue, left and right
    cr.set_source_rgba(0.03, 0.20, 0.40, 0.55);
    cr.move_to(0.0, surf + (bed - surf) * 0.50);
    cr.curve_to(w * 0.05, surf + (bed - surf) * 0.52, w * 0.07, bed - (bed - surf) * 0.25, w * 0.13, bed - (bed - surf) * 0.16);
    cr.curve_to(w * 0.18, bed - (bed - surf) * 0.10, w * 0.20, bed, w * 0.26, bed + 20.0 * s);
    cr.line_to(0.0, bed + 20.0 * s);
    cr.close_path();
    cr.move_to(w, surf + (bed - surf) * 0.62);
    cr.curve_to(w * 0.96, surf + (bed - surf) * 0.66, w * 0.93, bed - (bed - surf) * 0.20, w * 0.87, bed - (bed - surf) * 0.12);
    cr.curve_to(w * 0.83, bed - (bed - surf) * 0.06, w * 0.80, bed, w * 0.74, bed + 20.0 * s);
    cr.line_to(w, bed + 20.0 * s);
    cr.close_path();
    cr.fill();
    cr.set_source_rgba(0.03, 0.22, 0.42, 0.45);
    cr.move_to(w * 0.30, bed + 20.0 * s);
    cr.curve_to(w * 0.40, bed - 50.0 * s, w * 0.50, bed - 20.0 * s, w * 0.58, bed - 44.0 * s);
    cr.curve_to(w * 0.64, bed - 30.0 * s, w * 0.68, bed, w * 0.72, bed + 20.0 * s);
    cr.close_path();
    cr.fill();

    // --- seabed: sand, rocks, coral, a chest and an anchor
    let floor = |cr: &mut Canvas, lift: f64| {
        cr.move_to(0.0, h);
        let mut x = 0.0;
        while x <= w + 24.0 * s {
            cr.line_to(x, bed_y(bed, x + lift * 7.0, w, s) - lift);
            x += 24.0 * s;
        }
        cr.line_to(w, h);
        cr.close_path();
    };
    floor(cr, 14.0 * s);
    cr.set_source_rgb(0.10, 0.30, 0.44);
    cr.fill();
    // things standing behind the front dune
    let dark = (0.10, 0.22, 0.32);
    for &(fx, rw, rh) in &[(0.035, 70.0, 86.0), (0.115, 52.0, 48.0), (0.90, 62.0, 70.0), (0.975, 60.0, 100.0), (0.80, 36.0, 30.0)] {
        let x = w * fx;
        rock(cr, x, bed_y(bed, x, w, s) + 12.0 * s, rw * s, rh * s, lerp3(dark, (0.16, 0.34, 0.44), rng.random()));
    }
    cr.set_line_cap(LineCap::Round);
    for &(fx, len, c) in &[
        (0.075, 34.0, (0.74, 0.44, 0.50)),
        (0.165, 26.0, (0.80, 0.56, 0.40)),
        (0.215, 18.0, (0.62, 0.50, 0.70)),
        (0.845, 30.0, (0.78, 0.50, 0.44)),
        (0.935, 24.0, (0.70, 0.46, 0.58)),
        (0.44, 12.0, (0.72, 0.48, 0.50)),
    ] {
        let x = w * fx;
        cr.set_rgb(c);
        for k in -1..=1 {
            coral(cr, x + k as f64 * len * 0.35 * s, bed_y(bed, x, w, s) + 12.0 * s, -PI / 2.0 + k as f64 * 0.45, len * s, 3, rng);
        }
    }
    cr.set_line_cap(LineCap::Butt);
    for &(fx, r, c) in &[(0.135, 54.0, (0.70, 0.42, 0.52)), (0.875, 46.0, (0.80, 0.58, 0.42)), (0.255, 30.0, (0.56, 0.48, 0.70))] {
        let x = w * fx;
        sea_fan(cr, x, bed_y(bed, x, w, s) + 12.0 * s, r * s, c);
    }
    // tube sponges
    for &(fx, c) in &[(0.185, (0.78, 0.68, 0.40)), (0.795, (0.52, 0.62, 0.70))] {
        let x = w * fx;
        let y = bed_y(bed, x, w, s) + 12.0 * s;
        for &(dx, th) in &[(-14.0, 40.0), (0.0, 58.0), (13.0, 34.0)] {
            cr.set_rgb(c);
            cr.rounded_rect(x + (dx - 6.0) * s, y - th * s, 12.0 * s, th * s, 6.0 * s);
            cr.fill();
            cr.set_rgb(darken(c, 0.45));
            cr.ellipse(x + dx * s, y - (th - 3.0) * s, 4.0 * s, 2.0 * s);
            cr.fill();
        }
    }
    // anchor, half buried
    let ax = w * 0.665;
    let ay = bed_y(bed, ax, w, s) + 14.0 * s;
    cr.save();
    cr.translate(ax, ay);
    cr.rotate(0.42);
    cr.scale(s, s);
    cr.set_source_rgb(0.15, 0.21, 0.27);
    cr.set_line_width(6.0);
    cr.set_line_cap(LineCap::Round);
    cr.move_to(0.0, 6.0);
    cr.line_to(0.0, -62.0);
    cr.move_to(-16.0, -50.0);
    cr.line_to(16.0, -50.0);
    cr.new_sub_path();
    cr.arc(0.0, -18.0, 26.0, 0.15, PI - 0.15);
    cr.stroke();
    cr.set_line_width(3.5);
    cr.new_sub_path();
    cr.arc(0.0, -69.0, 7.0, 0.0, TAU);
    cr.stroke();
    cr.set_line_cap(LineCap::Butt);
    cr.restore();
    // sunken chest
    let cx = w * 0.335;
    let cy = bed_y(bed, cx, w, s) + 22.0 * s;
    cr.save();
    cr.translate(cx, cy);
    cr.rotate(-0.10);
    cr.scale(s, s);
    cr.set_source_rgb(0.33, 0.23, 0.17);
    cr.rounded_rect(-30.0, -34.0, 60.0, 36.0, 4.0);
    cr.fill();
    cr.set_source_rgb(0.42, 0.29, 0.20);
    cr.move_to(-30.0, -30.0);
    cr.curve_to(-30.0, -52.0, 30.0, -52.0, 30.0, -30.0);
    cr.close_path();
    cr.fill();
    cr.set_source_rgb(0.66, 0.58, 0.34);
    for x in [-20.0, 16.0] {
        cr.rectangle(x, -46.0, 5.0, 46.0);
    }
    cr.rectangle(-30.0, -31.0, 60.0, 3.0);
    cr.rectangle(-4.0, -30.0, 8.0, 9.0);
    cr.fill();
    cr.restore();

    // the front dune
    floor(cr, 0.0);
    let mut g = Gradient::linear(0.0, bed - 10.0 * s, 0.0, h);
    g.add_color_stop_rgb(0.0, 0.50, 0.60, 0.56);
    g.add_color_stop_rgb(0.35, 0.30, 0.42, 0.46);
    g.add_color_stop_rgb(1.0, 0.10, 0.18, 0.26);
    cr.set_source(&g);
    cr.fill();
    cr.set_source_rgba(0.80, 0.90, 0.82, 0.16); // sand ripples
    for _ in 0..(46.0 * w / (1600.0 * s)) as usize {
        let x = rng.uniform(0.0, w);
        let y = bed_y(bed, x, w, s) + rng.uniform(8.0, 70.0) * s;
        cr.ellipse(x, y, rng.uniform(16.0, 44.0) * s, 1.3 * s);
    }
    cr.fill();
    for _ in 0..(70.0 * w / (1600.0 * s)) as usize {
        let x = rng.uniform(0.0, w);
        let y = bed_y(bed, x, w, s) + rng.uniform(6.0, 80.0) * s;
        cr.set_rgba(*rng.choice(&[(0.20, 0.30, 0.38), (0.58, 0.66, 0.62), (0.36, 0.46, 0.50)]), 0.7);
        cr.ellipse(x, y, rng.uniform(2.0, 5.0) * s, rng.uniform(1.5, 3.0) * s);
        cr.fill();
    }
    for &(fx, rw, rh) in &[(0.29, 26.0, 16.0), (0.385, 18.0, 11.0), (0.715, 30.0, 18.0), (0.555, 16.0, 9.0)] {
        let x = w * fx;
        rock(cr, x, bed_y(bed, x, w, s) + 26.0 * s, rw * s, rh * s, (0.16, 0.28, 0.36));
    }
    // a starfish and a few coins by the chest
    let (qx, qy) = (w * 0.715 - 8.0 * s, bed_y(bed, w * 0.715, w, s) + 10.0 * s);
    for k in 0..10 {
        let a = k as f64 * TAU / 10.0 - PI / 2.0;
        let rr = if k % 2 == 0 { 9.0 } else { 3.8 } * s;
        if k == 0 {
            cr.move_to(qx + a.cos() * rr, qy + a.sin() * rr * 0.7);
        } else {
            cr.line_to(qx + a.cos() * rr, qy + a.sin() * rr * 0.7);
        }
    }
    cr.close_path();
    cr.set_source_rgb(0.80, 0.52, 0.40);
    cr.fill();
    cr.set_source_rgb(0.78, 0.68, 0.36);
    for &(dx, dy) in &[(44.0, -2.0), (54.0, 4.0), (38.0, 7.0)] {
        cr.ellipse(cx + dx * s, cy + dy * s, 4.5 * s, 2.2 * s);
    }
    cr.fill();
}

impl Dolphins {
    fn bubble(&mut self, rng: &Rng, x: f64, y: f64, s: f64, size: f64) {
        if self.bubbles.len() < 170 {
            self.bubbles.push(Bubble { x, y, r: rng.uniform(1.6, 4.2) * s * size, vy: rng.uniform(45.0, 90.0) * s, ph: rng.uniform(0.0, TAU) });
        }
    }

    /// Something big broke the surface at x: drops fly, foam spreads, bubbles are dragged under.
    fn splash(&mut self, rng: &Rng, s: f64, x: f64, size: f64) {
        for _ in 0..18 {
            let a = rng.uniform(-PI * 0.93, -PI * 0.07);
            let v = rng.uniform(110.0, 330.0) * s;
            self.drops.push(Drop { x: x + rng.uniform(-0.4, 0.4) * size, y: self.surf, vx: a.cos() * v * 0.6, vy: a.sin() * v, r: rng.uniform(1.6, 4.4) * s });
        }
        self.foam.push((x, 0.0, size));
        for _ in 0..10 {
            self.bubble(rng, x + rng.uniform(-0.6, 0.6) * size, self.surf + rng.uniform(12.0, 80.0) * s, s, 1.2);
        }
    }

    fn wave_path(&self, cr: &mut Canvas, env: &Env, t: f64, dy: f64) {
        let step = 14.0 * env.s;
        let mut x = -step;
        cr.move_to(x, wave_y(self.surf, x, t, env.s) + dy);
        while x < env.w + step {
            x += step;
            cr.line_to(x, wave_y(self.surf, x, t, env.s) + dy);
        }
    }

    fn draw_ship(&self, cr: &mut Canvas, env: &Env, sh: &Ship, t: f64) {
        let s = env.s;
        let under = |c: Rgb| lerp3(c, SHALLOW, 0.5);
        let half = SHIPS[sh.kind].0;
        cr.save();
        cr.translate(sh.x, self.surf + (t * 0.9 + sh.x * 0.01).sin() * 1.5 * s);
        cr.rotate((t * 0.7 + sh.x * 0.01).cos() * 0.012);
        cr.scale(sh.dir * s, s);
        match sh.kind {
            0 => {
                // sailing boat
                cr.set_rgb(under((0.20, 0.30, 0.45)));
                cr.move_to(-62.0, 0.0);
                cr.line_to(64.0, 0.0);
                cr.quad_to(40.0, 15.0, 10.0, 16.0);
                cr.line_to(-44.0, 14.0);
                cr.quad_to(-58.0, 10.0, -62.0, 0.0);
                poly(cr, &[(-6.0, 14.0), (14.0, 14.0), (8.0, 34.0), (-2.0, 34.0)]);
                poly(cr, &[(-56.0, 8.0), (-48.0, 8.0), (-50.0, 26.0), (-58.0, 26.0)]);
                cr.fill();
                cr.set_source_rgb(0.96, 0.95, 0.92);
                poly(cr, &[(-68.0, -15.0), (84.0, -19.0), (64.0, 0.0), (-62.0, 0.0)]);
                cr.fill();
                cr.set_source_rgb(0.26, 0.42, 0.60);
                poly(cr, &[(-67.0, -11.0), (80.0, -14.5), (78.0, -11.5), (-66.0, -8.0)]);
                cr.fill();
                cr.set_source_rgb(0.86, 0.83, 0.76);
                cr.rounded_rect(-30.0, -26.0, 44.0, 10.0, 3.0);
                cr.fill();
                cr.set_source_rgb(0.42, 0.33, 0.26);
                cr.rectangle(0.5, -152.0, 3.0, 136.0);
                cr.fill();
                cr.set_source_rgb(0.99, 0.97, 0.91);
                cr.move_to(-2.0, -148.0);
                cr.line_to(-2.0, -31.0);
                cr.line_to(-66.0, -31.0);
                cr.quad_to(-42.0, -98.0, -2.0, -148.0);
                cr.fill();
                cr.set_source_rgb(0.90, 0.85, 0.74);
                cr.move_to(6.0, -140.0);
                cr.quad_to(52.0, -72.0, 80.0, -22.0);
                cr.line_to(8.0, -33.0);
                cr.close_path();
                cr.fill();
                cr.set_source_rgb(0.80, 0.42, 0.36);
                poly(cr, &[(-2.0, -52.0), (-2.0, -42.0), (-61.0, -42.0), (-57.0, -52.0)]);
                poly(cr, &[(2.0, -153.0), (2.0, -145.0), (-15.0, -149.0)]);
                cr.fill();
            }
            1 => {
                // fishing boat
                cr.set_rgb(under((0.50, 0.22, 0.18)));
                cr.move_to(-74.0, 0.0);
                cr.line_to(74.0, 0.0);
                cr.quad_to(60.0, 20.0, 30.0, 22.0);
                cr.line_to(-60.0, 20.0);
                cr.quad_to(-72.0, 12.0, -74.0, 0.0);
                cr.fill();
                cr.set_source_rgb(0.14, 0.33, 0.45);
                cr.move_to(-80.0, -22.0);
                cr.line_to(60.0, -22.0);
                cr.quad_to(80.0, -26.0, 92.0, -34.0);
                cr.line_to(74.0, 0.0);
                cr.line_to(-74.0, 0.0);
                cr.close_path();
                cr.fill();
                cr.set_source_rgb(0.92, 0.92, 0.88);
                poly(cr, &[(-80.0, -22.0), (60.0, -22.0), (80.0, -27.0), (79.0, -23.0), (60.0, -18.0), (-79.0, -18.0)]);
                cr.rectangle(-50.0, -52.0, 44.0, 30.0);
                cr.fill();
                cr.set_source_rgb(0.60, 0.30, 0.25);
                cr.rectangle(-54.0, -57.0, 52.0, 5.0);
                cr.fill();
                cr.set_source_rgb(0.30, 0.50, 0.64);
                cr.rectangle(-44.0, -46.0, 12.0, 10.0);
                cr.rectangle(-27.0, -46.0, 12.0, 10.0);
                cr.fill();
                cr.set_source_rgb(0.72, 0.58, 0.38);
                cr.rectangle(-76.0, -31.0, 20.0, 9.0);
                cr.rectangle(24.0, -30.0, 14.0, 8.0);
                cr.fill();
                cr.set_source_rgb(0.30, 0.26, 0.24);
                cr.set_line_width(3.0);
                cr.move_to(10.0, -22.0);
                cr.line_to(10.0, -94.0);
                cr.stroke();
                cr.set_line_width(1.6);
                cr.move_to(10.0, -84.0);
                cr.line_to(64.0, -52.0);
                cr.line_to(64.0, -24.0);
                cr.move_to(10.0, -94.0);
                cr.line_to(-48.0, -57.0);
                cr.stroke();
                cr.set_source_rgb(1.0, 0.92, 0.62);
                cr.circle(10.0, -96.0, 3.0);
                cr.fill();
            }
            2 => {
                // freighter with containers
                cr.set_rgb(under((0.55, 0.22, 0.18)));
                cr.move_to(-172.0, 0.0);
                cr.line_to(172.0, 0.0);
                cr.quad_to(160.0, 24.0, 140.0, 26.0);
                cr.line_to(-164.0, 26.0);
                cr.quad_to(-172.0, 14.0, -172.0, 0.0);
                cr.fill();
                cr.set_source_rgb(0.17, 0.20, 0.27);
                poly(cr, &[(-180.0, -30.0), (150.0, -30.0), (186.0, -40.0), (172.0, 0.0), (-172.0, 0.0)]);
                cr.fill();
                cr.set_source_rgb(0.60, 0.27, 0.23);
                poly(cr, &[(-173.5, -6.0), (174.5, -6.0), (172.0, 0.0), (-172.0, 0.0)]);
                cr.fill();
                let cols = [(0.64, 0.42, 0.36), (0.38, 0.54, 0.62), (0.72, 0.66, 0.48), (0.44, 0.58, 0.48)];
                for row in 0..3 {
                    for k in 0..8 {
                        if row == 2 && (k + 1) % 3 == 0 {
                            continue;
                        }
                        cr.set_rgb(cols[(k * 3 + row * 5 + k / 3) % 4]);
                        cr.rectangle(-104.0 + k as f64 * 30.0, -44.0 - row as f64 * 14.0, 28.0, 13.0);
                        cr.fill();
                    }
                }
                cr.set_source_rgb(0.91, 0.91, 0.88);
                cr.rectangle(-166.0, -80.0, 44.0, 50.0);
                cr.fill();
                cr.set_source_rgb(0.22, 0.34, 0.46);
                cr.rectangle(-162.0, -74.0, 36.0, 6.0);
                cr.rectangle(-162.0, -60.0, 36.0, 4.0);
                cr.fill();
                cr.set_source_rgb(0.62, 0.30, 0.26);
                cr.rectangle(-152.0, -98.0, 16.0, 18.0);
                cr.fill();
                cr.set_source_rgb(0.16, 0.17, 0.22);
                cr.rectangle(-152.0, -98.0, 16.0, 5.0);
                cr.rectangle(159.0, -64.0, 2.0, 28.0);
                cr.fill();
            }
            _ => {
                // little steamer
                for k in 0..6 {
                    let f = (t * 0.22 + k as f64 / 6.0).fract();
                    cr.set_source_rgba(0.88, 0.88, 0.92, 0.5 * (1.0 - f) * (f * 8.0).min(1.0));
                    cr.circle(-2.0 - f * 120.0, -104.0 - f * 44.0 + (f * 9.0 + k as f64).sin() * 4.0, 6.0 + 17.0 * f);
                    cr.fill();
                }
                cr.set_rgb(under((0.55, 0.20, 0.18)));
                cr.move_to(-100.0, 0.0);
                cr.line_to(100.0, 0.0);
                cr.quad_to(88.0, 20.0, 60.0, 22.0);
                cr.line_to(-90.0, 22.0);
                cr.quad_to(-100.0, 12.0, -100.0, 0.0);
                cr.fill();
                cr.set_source_rgb(0.14, 0.15, 0.21);
                poly(cr, &[(-108.0, -24.0), (96.0, -24.0), (116.0, -30.0), (100.0, 0.0), (-100.0, 0.0)]);
                cr.fill();
                cr.set_source_rgb(0.94, 0.92, 0.86);
                poly(cr, &[(-108.0, -24.0), (96.0, -24.0), (116.0, -30.0), (113.5, -26.0), (96.0, -20.0), (-107.0, -20.0)]);
                cr.rounded_rect(-72.0, -46.0, 132.0, 22.0, 3.0);
                cr.rounded_rect(-42.0, -62.0, 72.0, 18.0, 3.0);
                cr.fill();
                cr.set_source_rgb(0.28, 0.44, 0.58);
                for k in 0..8 {
                    cr.circle(-60.0 + k as f64 * 15.5, -35.0, 3.2);
                }
                cr.rectangle(-34.0, -57.0, 56.0, 5.0);
                cr.fill();
                cr.set_source_rgb(0.74, 0.38, 0.28);
                poly(cr, &[(-13.0, -62.0), (9.0, -62.0), (7.0, -100.0), (-11.0, -100.0)]);
                cr.fill();
                cr.set_source_rgb(0.14, 0.15, 0.21);
                poly(cr, &[(-11.6, -92.0), (7.6, -92.0), (7.0, -100.0), (-11.0, -100.0)]);
                cr.rectangle(69.0, -70.0, 2.0, 46.0);
                cr.rectangle(-87.0, -64.0, 2.0, 40.0);
                cr.fill();
            }
        }
        // bow wave and wake
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.7);
        cr.ellipse(half + 3.0, 0.5, 10.0, 2.6);
        cr.fill();
        for k in 0..7 {
            let f = (k as f64 + (t * 0.8).fract()) / 7.0;
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.6 * (1.0 - f));
            cr.ellipse(-half - 4.0 - f * 120.0, 1.5, 9.0 + 9.0 * f, 2.4 * (1.0 - f) + 0.6);
            cr.fill();
        }
        cr.restore();
    }

    fn draw_sub(&self, cr: &mut Canvas, s: f64, sub: &Sub, t: f64) {
        let hull = lerp3((1.0, 0.86, 0.32), WATER, 0.18);
        let shade = lerp3(hull, NAVY, 0.30);
        let y = sub.y + (t * 0.6).sin() * 6.0 * s;
        // sonar ping
        cr.set_line_width(2.0 * s);
        for k in 0..2 {
            let f = (t * 0.33 + k as f64 * 0.5).fract();
            let a0 = if sub.dir > 0.0 { 0.0 } else { PI };
            cr.set_source_rgba(0.80, 1.0, 0.92, 0.38 * (1.0 - f));
            cr.new_sub_path();
            cr.arc(sub.x + sub.dir * 120.0 * s, y, (26.0 + 170.0 * f) * s, a0 - 0.8, a0 + 0.8);
            cr.stroke();
        }
        cr.save();
        cr.translate(sub.x, y);
        cr.scale(sub.dir * s, s);
        // propeller and rudder fins
        cr.set_rgb(lerp3((0.55, 0.58, 0.60), WATER, 0.3));
        cr.ellipse(-131.0, 0.0, 5.0, 4.0 + 22.0 * (t * 9.0).sin().abs());
        cr.fill();
        cr.set_rgb(shade);
        poly(cr, &[(-84.0, -8.0), (-122.0, -34.0), (-128.0, -30.0), (-112.0, 0.0), (-128.0, 30.0), (-122.0, 34.0), (-84.0, 8.0)]);
        cr.rounded_rect(4.0, -62.0, 52.0, 36.0, 9.0);
        cr.fill();
        cr.set_line_width(4.0);
        cr.set_line_cap(LineCap::Round);
        cr.move_to(34.0, -60.0);
        cr.line_to(34.0, -86.0);
        cr.line_to(46.0, -86.0);
        cr.stroke();
        cr.set_line_cap(LineCap::Butt);
        cr.set_rgb(hull);
        cr.rounded_rect(-124.0, -33.0, 250.0, 66.0, 33.0);
        cr.fill();
        cr.set_rgba(shade, 0.55);
        cr.rounded_rect(-104.0, 12.0, 210.0, 9.0, 4.5);
        cr.fill();
        cr.set_rgba(lighten(hull, 0.5), 0.5);
        cr.rounded_rect(-96.0, -25.0, 190.0, 5.0, 2.5);
        cr.fill();
        for k in 0..3 {
            let px = -52.0 + k as f64 * 52.0;
            cr.set_rgb(shade);
            cr.circle(px, -2.0, 13.0);
            cr.fill();
            cr.set_source_rgb(0.70, 0.90, 0.95);
            cr.circle(px, -2.0, 9.0);
            cr.fill();
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.7);
            cr.circle(px - 3.0, -5.0, 2.6);
            cr.fill();
        }
        cr.set_source_rgba(1.0, 0.98, 0.80, 0.9); // head lamp
        cr.circle(116.0, 6.0, 5.0);
        cr.fill();
        cr.restore();
    }

    fn draw_glider(&self, cr: &mut Canvas, s: f64, g: &Glider, t: f64) {
        cr.save();
        cr.translate(g.x, g.y + (t * 0.5).sin() * 10.0 * s);
        cr.scale(g.dir * s, s);
        if g.kind == 0 {
            // sea turtle
            let shell = lerp3((0.42, 0.60, 0.40), WATER, 0.15);
            let skin = lerp3((0.66, 0.76, 0.52), WATER, 0.15);
            let flap = (t * 2.0).sin();
            cr.set_rgb(skin);
            for &(fx, fy, len, rot) in &[(22.0, 6.0, 46.0, 0.9 + flap * 0.55), (-30.0, 8.0, 22.0, 2.3 - flap * 0.2)] {
                cr.save();
                cr.translate(fx, fy);
                cr.rotate(rot);
                cr.move_to(0.0, -6.0);
                cr.quad_to(len * 0.6, -11.0, len, 0.0);
                cr.quad_to(len * 0.5, 7.0, 0.0, 6.0);
                cr.close_path();
                cr.restore();
            }
            cr.ellipse(50.0, -3.0, 14.0, 9.5);
            cr.fill();
            cr.set_rgb(shell);
            cr.ellipse(0.0, 0.0, 42.0, 24.0);
            cr.fill();
            cr.set_rgba(lighten(shell, 0.3), 0.6);
            for &(px, py, pr) in &[(-18.0, -5.0, 9.0), (2.0, -8.0, 10.0), (21.0, -4.0, 8.0), (-6.0, 8.0, 7.0), (14.0, 9.0, 6.0)] {
                cr.ellipse(px, py, pr, pr * 0.62);
            }
            cr.fill();
            cr.set_source_rgba(0.03, 0.06, 0.10, 0.9);
            cr.circle(55.0, -5.0, 2.2);
            cr.fill();
        } else {
            // manta
            let top = lerp3((0.20, 0.27, 0.40), WATER, 0.25);
            let fl = (t * 1.5).sin();
            cr.set_rgb(darken(top, 0.25));
            cr.move_to(52.0, 0.0);
            cr.quad_to(10.0, -20.0 - 30.0 * fl, -16.0, -22.0 - 44.0 * fl);
            cr.quad_to(-18.0, -8.0, -46.0, 0.0);
            cr.close_path();
            cr.fill();
            cr.set_line_width(2.0);
            cr.move_to(-40.0, 1.0);
            cr.quad_to(-80.0, 2.0 - 6.0 * fl, -118.0, 8.0 + 4.0 * fl);
            cr.stroke();
            cr.set_rgb(top);
            cr.move_to(54.0, 0.0);
            cr.quad_to(30.0, -10.0, -46.0, -3.0);
            cr.line_to(-46.0, 4.0);
            cr.quad_to(-24.0, 12.0 - 10.0 * fl, -26.0, 20.0 - 44.0 * fl);
            cr.quad_to(8.0, 22.0 - 24.0 * fl, 54.0, 0.0);
            cr.fill();
            poly(cr, &[(50.0, -3.0), (66.0, -7.0), (56.0, 0.0)]);
            poly(cr, &[(50.0, 2.0), (67.0, 3.0), (54.0, 5.0)]);
            cr.fill();
            cr.set_rgba(BELLY, 0.5);
            cr.ellipse(8.0, 3.0, 26.0, 3.0);
            cr.fill();
            cr.set_source_rgba(0.9, 0.95, 1.0, 0.8);
            cr.circle(44.0, -2.0, 1.8);
            cr.fill();
        }
        cr.restore();
    }

    fn draw_schools(&self, cr: &mut Canvas, s: f64, t: f64) {
        for sc in &self.schools {
            for (i, &(ox, oy, size, ph)) in sc.fish.iter().enumerate() {
                // the turn runs through the school like a wave
                let f = clamp((sc.face - (hash(i as f64 + sc.ph) - 0.5) * 1.1) * 4.0, -1.0, 1.0);
                let f = if f.abs() < 0.3 { 0.3f64.copysign(f) } else { f };
                let len = size * s;
                let x = sc.x + (ox + (t * 0.9 + ph).sin() * 7.0) * s;
                let y = sc.y + (oy + (t * 1.3 + ph * 1.7).sin() * 4.0) * s + sc.h.sin() * ox * sc.face * 0.25 * s;
                let wag = (t * 9.0 + ph).sin() * len * 0.10;
                cr.move_to(x + f * len * 0.5, y);
                cr.quad_to(x + f * len * 0.1, y - len * 0.25, x - f * len * 0.30, y - len * 0.03);
                cr.line_to(x - f * len * 0.56, y - len * 0.18 + wag);
                cr.line_to(x - f * len * 0.56, y + len * 0.18 + wag);
                cr.line_to(x - f * len * 0.30, y + len * 0.03);
                cr.quad_to(x + f * len * 0.1, y + len * 0.25, x + f * len * 0.5, y);
                cr.close_path();
            }
            cr.set_rgba(sc.c, 0.78);
            cr.fill();
        }
    }

    fn draw_jellies(&self, cr: &mut Canvas, s: f64, t: f64) {
        cr.set_line_cap(LineCap::Round);
        for j in &self.jellies {
            let pulse = (t * 1.3 + j.ph).sin();
            let (rx, ry) = (j.r * (1.0 - 0.10 * pulse), j.r * (1.0 + 0.14 * pulse));
            let (x, y) = (j.x + (t * 0.21 + j.ph).sin() * 30.0 * s, j.y + (t * 0.16 + j.ph * 2.0).sin() * 50.0 * s);
            cr.set_source_rgba(0.98, 0.84, 0.90, 0.34);
            cr.set_line_width(1.5 * s);
            for k in -2..=2 {
                let kx = x + k as f64 * rx * 0.36;
                cr.move_to(kx, y);
                cr.curve_to(
                    kx + (t * 1.3 + j.ph + k as f64).sin() * rx * 0.3,
                    y + j.r * 0.9,
                    kx - (t * 1.3 + j.ph + k as f64).sin() * rx * 0.3,
                    y + j.r * 1.7,
                    kx + (t * 1.0 + k as f64).sin() * rx * 0.2,
                    y + j.r * (2.3 + 0.2 * pulse),
                );
            }
            cr.stroke();
            cr.move_to(x - rx, y);
            cr.curve_to(x - rx, y - ry * 1.45, x + rx, y - ry * 1.45, x + rx, y);
            cr.quad_to(x, y + ry * 0.28, x - rx, y);
            cr.set_source_rgba(0.98, 0.82, 0.90, 0.42);
            cr.fill();
            cr.set_source_rgba(1.0, 0.95, 0.98, 0.35);
            cr.ellipse(x - rx * 0.2, y - ry * 0.62, rx * 0.5, ry * 0.3);
            cr.fill();
        }
        cr.set_line_cap(LineCap::Butt);
    }

    fn draw_crab(&self, cr: &mut Canvas, env: &Env, t: f64) {
        let s = env.s;
        let (x, _, step) = self.crab;
        let y = bed_y(self.bed, x, env.w, s) + 18.0 * s;
        let shell = (0.74, 0.40, 0.32);
        cr.set_rgb(darken(shell, 0.25));
        cr.set_line_width(2.2 * s);
        cr.set_line_cap(LineCap::Round);
        for side in [-1.0, 1.0] {
            for k in 0..3 {
                let kf = k as f64;
                let lift = (step * 9.0 + kf * 2.1 + side).sin() * 2.5 * s;
                cr.move_to(x + side * (6.0 + kf * 3.0) * s, y + 2.0 * s);
                cr.line_to(x + side * (16.0 + kf * 3.0) * s, y - 1.0 * s + lift * 0.3);
                cr.line_to(x + side * (20.0 + kf * 3.5) * s, y + 8.0 * s + lift);
            }
            cr.move_to(x + side * 9.0 * s, y - 3.0 * s);
            cr.line_to(x + side * 17.0 * s, y - (10.0 + (t * 1.4 + side).sin() * 2.0) * s);
        }
        cr.stroke();
        cr.set_line_cap(LineCap::Butt);
        cr.set_rgb(shell);
        cr.ellipse(x, y, 13.0 * s, 8.0 * s);
        for side in [-1.0, 1.0] {
            cr.circle(x + side * 18.5 * s, y - (12.0 + (t * 1.4 + side).sin() * 2.0) * s, 4.6 * s);
        }
        cr.fill();
        for side in [-1.0, 1.0] {
            cr.set_rgb(shell);
            cr.rectangle(x + side * 4.5 * s - 0.8 * s, y - 12.0 * s, 1.6 * s, 6.0 * s);
            cr.fill();
            cr.set_source_rgb(0.04, 0.05, 0.08);
            cr.circle(x + side * 4.5 * s, y - 12.5 * s, 2.1 * s);
            cr.fill();
        }
    }

    /// One half of each bubble ring: the far half goes behind the dolphins, the near half in front.
    fn draw_rings(&self, cr: &mut Canvas, s: f64, front: bool) {
        for g in &self.rings {
            let fade = (g.age * 6.0).min(1.0) * (1.0 - g.age / 2.6).max(0.0);
            let rr = g.r * (1.0 + 0.30 * g.age);
            let (sa, ca) = g.ang.sin_cos();
            let at = |th: f64| {
                let (lx, ly) = (th.cos() * rr * 0.30, th.sin() * rr);
                (g.x + ca * lx - sa * ly, g.y + sa * lx + ca * ly)
            };
            let a0 = if front { PI / 2.0 } else { -PI / 2.0 };
            for k in 0..=12 {
                let (px, py) = at(a0 + k as f64 / 12.0 * PI);
                if k == 0 {
                    cr.move_to(px, py);
                } else {
                    cr.line_to(px, py);
                }
            }
            cr.set_rgba(lighten(g.c, 0.6), 0.55 * fade);
            cr.set_line_width(if front { 3.0 } else { 2.0 } * s);
            cr.stroke();
            for k in 0..9 {
                let th = a0 + (k as f64 + 0.5) / 9.0 * PI;
                let (px, py) = at(th);
                let wob = (g.age * 5.0 + k as f64 * 1.3).sin() * 2.0 * s;
                cr.circle(px + wob, py - g.age * (3.0 + (k % 3) as f64 * 3.0) * s, (2.2 + (k % 3) as f64 * 1.1) * s);
            }
            cr.set_source_rgba(0.90, 1.0, 1.0, 0.16 * fade);
            cr.fill_preserve();
            cr.set_source_rgba(0.92, 1.0, 1.0, 0.75 * fade);
            cr.set_line_width((1.2 * s).max(1.0));
            cr.stroke();
        }
    }
}

impl Theme for Dolphins {
    type Actor = Dolphin;
    const NAME: &'static str = "dolphins";
    const LABEL_BG: Rgb = (0.01, 0.06, 0.13);
    const EMPTY_COLOR: Rgb = (0.78, 0.96, 0.98);
    const WAITING_ROW: f64 = 0.58;

    fn new(_env: &Env) -> Self {
        Dolphins {
            bg: None,
            shaft: None,
            glows: Vec::new(),
            glow_r: 0.0,
            surf: 0.0,
            bed: 0.0,
            pod: (0.0, 0.0),
            pod_dir: (1.0, 0.0),
            weeds: Vec::new(),
            motes: Vec::new(),
            bubbles: Vec::new(),
            drops: Vec::new(),
            foam: Vec::new(),
            rings: Vec::new(),
            ship: None,
            next_ship: 5.0,
            ships: 0,
            sub: None,
            next_sub: 30.0,
            glider: None,
            next_glider: 12.0,
            gliders: 0,
            schools: Vec::new(),
            jellies: Vec::new(),
            crab: (0.0, 1.0, 0.0),
            vents: [0.0; 2],
            next_bubble: 0.0,
        }
    }

    fn spawn_point(&self, env: &Env) -> (f64, f64) {
        let d = self.bed - self.surf;
        (env.rng.uniform(env.w * 0.2, env.w * 0.8), self.surf + d * env.rng.uniform(0.2, 0.7))
    }

    fn rest_point(&self, env: &Env) -> (f64, f64) {
        (env.w * env.rng.uniform(0.36, 0.5), self.surf + 115.0 * env.s)
    }

    /// Tool call: the name rises in a bubble; now and then the dolphin rolls through a bubble ring.
    fn on_tool(&mut self, env: &mut Env, actor: &mut Dolphin, name: &str, index: usize) {
        let s = env.s;
        let (hx, hy) = actor.along(1.2, 0.3);
        let y = hy.max(self.surf + 150.0 * s) - index as f64 * 26.0 * s;
        env.spawn_floater(hx, y, name, actor.b.color, FloatStyle::Bubble, 42.0, 4.5);
        let b = &actor.b;
        if index == 0 && actor.leap == Leap::None && b.state() == State::Working && env.t > actor.next_trick {
            actor.trick = 1.0;
            actor.next_trick = env.t + env.rng.uniform(6.0, 11.0);
            let (r, h) = (b.radius, b.heading);
            self.rings.push(Ring {
                x: b.x + h.cos() * r * 2.3,
                y: (b.y + h.sin() * r * 2.3).max(self.surf + r),
                ang: h,
                r: r * 0.62,
                age: 0.0,
                c: b.color,
            });
        }
    }

    fn setup_world(&mut self, env: &mut Env) {
        let (w, h, s) = (env.w, env.h, env.s);
        self.surf = (h * 0.20).round();
        self.bed = h - env.strip_h - 84.0 * s;
        let (surf, bed) = (self.surf, self.bed);
        let depth = bed - surf;
        self.bg = Some(Picture::record(w, h, |cr| paint_background(cr, env, surf, bed)));
        // one shaft of light, shown at several places with changing strength
        let (sw, sh) = (300.0 * s, depth * 0.86);
        self.shaft = Some(Picture::record(sw, sh, |cr| {
            for &(x0, x1, x2, x3, a) in &[(10.0, 86.0, 290.0, 130.0, 0.10), (30.0, 66.0, 250.0, 170.0, 0.10)] {
                let mut g = Gradient::linear(0.0, 0.0, 0.0, sh);
                g.add_color_stop_rgba(0.0, 0.78, 1.0, 0.95, a);
                g.add_color_stop_rgba(0.55, 0.78, 1.0, 0.95, a * 0.4);
                g.add_color_stop_rgba(1.0, 0.78, 1.0, 0.95, 0.0);
                poly(cr, &[(x0 * s, 0.0), (x1 * s, 0.0), (x2 * s, sh), (x3 * s, sh)]);
                cr.set_source(&g);
                cr.fill();
            }
        }));
        // glows are painted once per state colour and only copied each frame
        let gr = (Dolphin::BASE_RADIUS * 2.1 * s).round();
        self.glow_r = gr;
        self.glows = LOOKS.iter().map(|&look| Picture::record(gr * 2.0, gr * 2.0, |cr| cr.glow(gr, gr, gr, color_of(look), 0.42, 0.12))).collect();

        let rng = &env.rng;
        self.weeds = (0..(18.0 * w / (1600.0 * s)) as usize)
            .map(|i| {
                // mostly at the sides, the middle stays open
                let u = rng.random();
                let x = if i % 5 == 4 { rng.uniform(0.3, 0.7) } else if i % 2 == 0 { u * u * 0.28 } else { 1.0 - u * u * 0.28 } * w;
                let mid = (x / w - 0.5).abs() < 0.22;
                Weed {
                    x,
                    h: rng.uniform(50.0, if mid { 80.0 } else { 190.0 }) * s,
                    w: rng.uniform(4.0, 8.0) * s,
                    ph: rng.uniform(0.0, TAU),
                    c: (rng.uniform(0.10, 0.20), rng.uniform(0.40, 0.56), rng.uniform(0.30, 0.42)),
                }
            })
            .collect();
        self.motes = (0..(60.0 * w / (1600.0 * s)) as usize)
            .map(|_| Mote { x: rng.uniform(0.0, w), y: rng.uniform(surf + 20.0 * s, bed), z: rng.uniform(0.2, 1.0), ph: rng.uniform(0.0, TAU) })
            .collect();
        self.schools = [((0.80, 0.90, 0.96), 0.72, 0.62), ((0.90, 0.88, 0.66), 0.24, 0.78)]
            .iter()
            .map(|&(c, fx, fy)| School {
                x: w * fx,
                y: surf + depth * fy,
                h: rng.uniform(0.0, TAU),
                face: 1.0,
                ph: rng.uniform(0.0, 50.0),
                c: lerp3(c, WATER, 0.18),
                fish: (0..15)
                    .map(|_| {
                        let a = rng.uniform(0.0, TAU);
                        let d = rng.random().sqrt();
                        (a.cos() * d * 95.0, a.sin() * d * 40.0, rng.uniform(15.0, 22.0), rng.uniform(0.0, TAU))
                    })
                    .collect(),
            })
            .collect();
        self.jellies = [(0.045, 0.52, 20.0), (0.955, 0.42, 16.0)]
            .iter()
            .map(|&(fx, fy, r)| Jelly { x: w * fx, y: surf + depth * fy, ph: rng.uniform(0.0, TAU), r: r * s })
            .collect();
        self.crab = (w * 0.47, 1.0, 0.0);
        self.vents = [w * 0.335 + 44.0 * s, w * 0.80];
        self.pod = (w * 0.5, surf + depth * 0.4);
        self.bubbles.clear();
        self.drops.clear();
        self.foam.clear();
        self.rings.clear();
        self.ship = None;
        self.sub = None;
        self.glider = None;
    }

    fn update_world(&mut self, env: &mut Env, actors: &mut [Dolphin], dt: f64) {
        let (w, s, t) = (env.w, env.s, env.t);
        let (surf, bed) = (self.surf, self.bed);
        let depth = bed - surf;
        let rng = &env.rng;
        // the pod roams the open water on a long, slow figure
        let spot = |t: f64| {
            (
                w * (0.5 + 0.27 * (t * 0.11 + 1.0).sin() + 0.06 * (t * 0.37).sin()),
                surf + depth * (0.40 + 0.20 * (t * 0.17).sin() + 0.04 * (t * 0.41).cos()),
            )
        };
        let (p0, p1) = (spot(t), spot(t + 0.5));
        let d = hypot(p1.0 - p0.0, p1.1 - p0.1).max(1e-6);
        self.pod = p0;
        self.pod_dir = ((p1.0 - p0.0) / d, (p1.1 - p0.1) / d);

        for m in &mut self.motes {
            m.x += ((t * 0.2 + m.ph).sin() * 5.0 + 3.0 * m.z) * s * dt;
            m.y -= m.z * 2.5 * s * dt;
            if m.y < surf + 14.0 * s {
                m.y = bed;
            }
            if m.x > w {
                m.x = 0.0;
            }
        }
        if t > self.next_bubble {
            self.next_bubble = t + rng.uniform(0.25, 0.8);
            let x = self.vents[rng.index(2)] + rng.uniform(-5.0, 5.0) * s;
            let y = bed_y(bed, x, w, s) + 6.0 * s;
            self.bubble(&env.rng, x, y, s, 1.0);
        }
        for b in &mut self.bubbles {
            b.y -= b.vy * dt;
            b.x += (t * 3.0 + b.ph).sin() * 10.0 * s * dt;
        }
        self.bubbles.retain(|b| b.y > surf + 5.0 * s);
        for d in &mut self.drops {
            d.vy += GRAVITY * s * dt;
            d.x += d.vx * dt;
            d.y += d.vy * dt;
        }
        self.drops.retain(|d| d.y < surf + 3.0 * s);
        for f in &mut self.foam {
            f.1 += dt;
        }
        self.foam.retain(|f| f.1 < 1.3);
        for g in &mut self.rings {
            g.age += dt;
            g.y -= 10.0 * s * dt;
        }
        self.rings.retain(|g| g.age < 2.6);

        // a ship on the surface every minute or two
        if self.ship.is_none() && t > self.next_ship {
            let kind = self.ships % SHIPS.len();
            self.ships += 1;
            let dir = if rng.chance(0.5) { 1.0 } else { -1.0 };
            let off = (SHIPS[kind].0 + 30.0) * s;
            self.ship = Some(Ship { kind, x: if dir > 0.0 { -off } else { w + off }, dir });
        }
        if let Some(sh) = &mut self.ship {
            sh.x += sh.dir * SHIPS[sh.kind].1 * s * dt;
            let off = (SHIPS[sh.kind].0 + 170.0) * s;
            if sh.x < -off || sh.x > w + off {
                self.ship = None;
                self.next_ship = t + rng.uniform(55.0, 120.0);
            }
        }
        // the submarine
        if self.sub.is_none() && t > self.next_sub {
            let dir = if rng.chance(0.5) { 1.0 } else { -1.0 };
            self.sub = Some(Sub { x: if dir > 0.0 { -160.0 * s } else { w + 160.0 * s }, y: surf + depth * rng.uniform(0.60, 0.72), dir, next_bubble: 0.0 });
        }
        let mut trail = None;
        if let Some(sub) = &mut self.sub {
            sub.x += sub.dir * 44.0 * s * dt;
            if t > sub.next_bubble {
                sub.next_bubble = t + 0.13;
                trail = Some((sub.x - sub.dir * 136.0 * s, sub.y + (t * 0.6).sin() * 6.0 * s));
            }
            if sub.x < -360.0 * s || sub.x > w + 360.0 * s {
                self.sub = None;
                self.next_sub = t + rng.uniform(80.0, 150.0);
            }
        }
        if let Some((x, y)) = trail {
            self.bubble(&env.rng, x, y + rng.uniform(-14.0, 14.0) * s, s, 0.9);
        }
        // a turtle or a manta
        if self.glider.is_none() && t > self.next_glider {
            let dir = if rng.chance(0.5) { 1.0 } else { -1.0 };
            let kind = self.gliders % 2;
            self.gliders += 1;
            self.glider = Some(Glider { kind, x: if dir > 0.0 { -130.0 * s } else { w + 130.0 * s }, y: surf + depth * rng.uniform(0.30, 0.82), dir });
        }
        if let Some(g) = &mut self.glider {
            g.x += g.dir * if g.kind == 0 { 30.0 } else { 40.0 } * s * dt;
            if g.x < -140.0 * s || g.x > w + 140.0 * s {
                self.glider = None;
                self.next_glider = t + rng.uniform(40.0, 90.0);
            }
        }
        // the schools wander as one and give way to the dolphins
        for sc in &mut self.schools {
            sc.h += ((t * 0.23 + sc.ph).sin() * 0.5 + (t * 0.11 + sc.ph * 2.0).sin() * 0.4) * dt;
            let (mut dx, mut dy) = (sc.h.cos(), sc.h.sin() * 0.5);
            let (left, top, right, bottom) = (w * 0.10, surf + depth * 0.30, w * 0.90, bed - 70.0 * s);
            dx += clamp((left - sc.x) / (100.0 * s), 0.0, 2.0) - clamp((sc.x - right) / (100.0 * s), 0.0, 2.0);
            dy += clamp((top - sc.y) / (80.0 * s), 0.0, 2.0) - clamp((sc.y - bottom) / (80.0 * s), 0.0, 2.0);
            let mut hurry: f64 = 0.0;
            for a in actors.iter() {
                let (ex, ey) = (sc.x - a.b.x, sc.y - a.b.y);
                let d = hypot(ex, ey);
                let md = 230.0 * s;
                if 1.0 < d && d < md {
                    let f = (md - d) / md;
                    dx += ex / d * f * 2.2;
                    dy += ey / d * f * 2.2;
                    hurry = hurry.max(f);
                }
            }
            sc.h = wrap(sc.h + clamp(wrap(dy.atan2(dx) - sc.h) * 2.0, -1.4, 1.4) * dt);
            let sp = (38.0 + 60.0 * hurry) * s;
            sc.x += sc.h.cos() * sp * dt;
            sc.y += sc.h.sin() * sp * 0.6 * dt;
            let want = if sc.h.cos() >= 0.0 { 1.0 } else { -1.0 };
            sc.face = lerp(sc.face, want, (dt * 2.2).min(1.0));
        }
        // the crab scuttles sideways and stops to wave
        let (cx, cdir, cstep) = &mut self.crab;
        if (t * 0.13).sin() > -0.2 {
            *cx += *cdir * 16.0 * s * dt;
            *cstep += dt;
        }
        if *cx > w * 0.62 {
            *cdir = -1.0;
        } else if *cx < w * 0.40 {
            *cdir = 1.0;
        }
    }

    fn draw_world(&mut self, cr: &mut Canvas, env: &Env, actors: &[Dolphin], t: f64) {
        let (w, s) = (env.w, env.s);
        let (surf, bed) = (self.surf, self.bed);
        if let Some(bg) = &self.bg {
            cr.picture(bg, 0.0, 0.0, 1.0);
        }
        // the moving surface, a ship cutting through it
        self.wave_path(cr, env, t, 0.0);
        cr.line_to(w + 20.0 * s, surf + 8.0 * s);
        cr.line_to(-20.0 * s, surf + 8.0 * s);
        cr.close_path();
        cr.set_rgb(SHALLOW);
        cr.fill();
        if let Some(sh) = &self.ship {
            self.draw_ship(cr, env, sh, t);
        }
        self.wave_path(cr, env, t, 5.0 * s);
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.22);
        cr.set_line_width(5.0 * s);
        cr.stroke();
        self.wave_path(cr, env, t, 0.0);
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.85);
        cr.set_line_width(2.2 * s);
        cr.stroke();
        // shafts of light and glints under the surface
        if let Some(shaft) = &self.shaft {
            for k in 0..7 {
                let kf = k as f64;
                let x = w * (kf * 0.155 - 0.04) + (t * 0.07 + kf * 1.7).sin() * 60.0 * s;
                let a = 0.55 + 0.45 * (t * 0.45 + kf * 2.1).sin() * (t * 0.13 + kf).cos();
                cr.picture(shaft, x.round(), (surf + 6.0 * s).round(), a);
            }
        }
        for k in 0..18 {
            let kf = k as f64;
            let x = (kf * w / 18.0 + t * (10.0 + (k % 3) as f64 * 5.0) * s + (kf * 7.3).sin() * 40.0 * s).rem_euclid(w + 80.0 * s) - 40.0 * s;
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.07 + 0.13 * (0.5 + 0.5 * (t * 1.3 + kf * 2.4).sin()));
            cr.ellipse(x, surf + (16.0 + (k % 4) as f64 * 9.0) * s, (18.0 + (k % 5) as f64 * 6.0) * s, 1.5 * s);
            cr.fill();
        }
        // passers-by, all behind the dolphins
        self.draw_jellies(cr, s, t);
        if let Some(g) = &self.glider {
            self.draw_glider(cr, s, g, t);
        }
        self.draw_schools(cr, s, t);
        if let Some(sub) = &self.sub {
            self.draw_sub(cr, s, sub, t);
        }
        for m in &self.motes {
            cr.set_source_rgba(0.85, 1.0, 0.95, 0.06 + 0.2 * m.z);
            cr.circle(m.x, m.y, (0.7 + m.z) * s);
            cr.fill();
        }
        // seaweed swaying on the seabed
        cr.set_line_cap(LineCap::Round);
        for p in &self.weeds {
            let base = bed_y(bed, p.x, w, s) + 14.0 * s;
            cr.move_to(p.x, base);
            for k in 1..9 {
                let f = k as f64 / 8.0;
                cr.line_to(p.x + (t * 0.7 + p.ph + f * 2.6).sin() * f * 22.0 * s, base - f * p.h);
            }
            cr.set_rgba(p.c, 0.85);
            cr.set_line_width(p.w);
            cr.stroke();
        }
        cr.set_line_cap(LineCap::Butt);
        self.draw_crab(cr, env, t);
        self.draw_rings(cr, s, false);
        for b in &self.bubbles {
            cr.circle(b.x, b.y, b.r);
        }
        cr.set_source_rgba(0.85, 0.98, 1.0, 0.14);
        cr.fill_preserve();
        cr.set_source_rgba(0.90, 1.0, 1.0, 0.6);
        cr.set_line_width((1.1 * s).max(1.0));
        cr.stroke();

        for a in sorted_actors(actors) {
            a.draw(cr, env, self, t);
        }
        self.draw_rings(cr, s, true);
        // splashes
        for &(x, age, size) in &self.foam {
            let f = age / 1.3;
            let y = wave_y(surf, x, t, s);
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.75 * (1.0 - f));
            cr.ellipse(x, y, size * (0.35 + 1.1 * f), (7.0 * (1.0 - f) + 1.5) * s);
            cr.fill();
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.5 * (1.0 - f) * (1.0 - f));
            cr.ellipse(x, y - 10.0 * s * (1.0 - f), size * 0.3 * (1.0 - f) + 2.0 * s, 16.0 * s * (PI * f.min(0.5)).sin());
            cr.fill();
        }
        for d in &self.drops {
            cr.circle(d.x, d.y, d.r);
        }
        cr.set_source_rgba(0.95, 1.0, 1.0, 0.9);
        cr.fill();
    }
}
