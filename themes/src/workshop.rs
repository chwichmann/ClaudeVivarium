//! Workshop theme: elves in Santa's workshop on a winter night.
//!
//! working = walks briskly carrying a wrapped present; every tool call makes the elf hop and toss a
//!           small present onto the pile next to Santa's sack, with the tool name rising
//! idle    = strolls slowly, stops and sips cocoa from a steaming mug
//! waiting = comes to the front and rings a handbell, pulses
//! blocked = stands still with its arms down, flickers
//! rest    = one elf asleep against a big present, hat drooping, "z z z"
//! Hat and tunic are in the state colour; skin, stockings and shoes stay natural.
use vivarium_core::prelude::*;

const SKIN: Rgb = (0.98, 0.82, 0.68);
const CREAM: Rgb = (0.96, 0.93, 0.86);
const RED: Rgb = (0.80, 0.14, 0.15);
const GREEN: Rgb = (0.12, 0.46, 0.24);
const GOLD: Rgb = (1.00, 0.80, 0.30);
const LEATHER: Rgb = (0.24, 0.13, 0.08);
const WOOD: Rgb = (0.55, 0.36, 0.20);

/// Wrapping paper and ribbon colours.
const WRAPS: [(Rgb, Rgb); 6] = [
    (RED, GOLD),
    (GREEN, RED),
    (CREAM, RED),
    ((0.92, 0.66, 0.18), GREEN),
    ((0.17, 0.30, 0.62), CREAM),
    ((0.52, 0.20, 0.46), GOLD),
];

/// Colours of the fairy lights.
const BULBS: [Rgb; 5] = [(1.0, 0.92, 0.65), (1.0, 0.35, 0.30), (1.0, 0.75, 0.25), (0.45, 0.95, 0.50), (0.50, 0.70, 1.0)];

/// Presents that fit on the growing pile before the sleigh crew carries it off.
const PILE_MAX: usize = 15;

#[derive(Clone)]
pub struct Elf {
    b: Base,
    /// 1 -> 0 after a tool call: arms thrown up
    toss: f64,
    /// 0..1 mug at the mouth
    sip: f64,
    blink_at: f64,
    wrap: usize,
}

/// A present flying to the pile.
struct Toss {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    rot: f64,
    vr: f64,
    age: f64,
    life: f64,
    size: f64,
    wrap: usize,
}

/// Where the fixed things are (all in pixels).
#[derive(Clone, Copy, Default)]
struct Geo {
    /// line where the wall meets the floor
    fl: f64,
    /// fireplace centre
    fx: f64,
    /// window pane (x, y, w, h)
    win: (f64, f64, f64, f64),
    /// bottom centre of the present pile
    pile: (f64, f64),
}

pub struct Workshop {
    bg: Option<Picture>,
    /// window frame, drawn over the falling snow
    frame: Option<Picture>,
    geo: Geo,
    /// fairy lights on tree and garland: (x, y, colour, phase)
    lights: Vec<(f64, f64, usize, f64)>,
    /// snow flakes: (x 0..1, phase, speed, size)
    snow: Vec<(f64, f64, f64, f64)>,
    tosses: Vec<Toss>,
    /// wrap index of each present lying on the pile
    pile: Vec<usize>,
    /// seconds since the pile is full
    full_t: f64,
}

/// A wrapped present standing on (x, y) (bottom centre).
#[allow(clippy::too_many_arguments)]
fn present(cr: &mut Canvas, x: f64, y: f64, w: f64, h: f64, wrap: usize, a: f64) {
    let (col, rib) = WRAPS[wrap % WRAPS.len()];
    cr.set_rgba(col, a);
    cr.rectangle(x - w / 2.0, y - h, w, h);
    cr.fill();
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.20 * a); // shaded side
    cr.rectangle(x + w * 0.2, y - h, w * 0.3, h);
    cr.fill();
    cr.set_rgba(lighten(col, 0.14), a); // lid
    cr.rectangle(x - w * 0.54, y - h, w * 1.08, h * 0.24);
    cr.fill();
    cr.set_rgba(rib, a);
    cr.rectangle(x - w * 0.08, y - h, w * 0.16, h);
    cr.fill();
    cr.ellipse(x - w * 0.15, y - h * 1.07, w * 0.15, h * 0.11); // bow
    cr.ellipse(x + w * 0.15, y - h * 1.07, w * 0.15, h * 0.11);
    cr.fill();
}

/// Place and size of pile slot `i`: rows of 5, 4, 3, 2, 1. Returns (x, y of the bottom centre, w, h).
fn pile_slot(geo: &Geo, s: f64, i: usize) -> (f64, f64, f64, f64) {
    let (mut row, mut k, mut n) = (0usize, i.min(PILE_MAX - 1), 5usize);
    while k >= n {
        k -= n;
        n -= 1;
        row += 1;
    }
    let jig = ((i * 37 % 11) as f64 / 10.0 - 0.5) * 6.0 * s;
    let x = geo.pile.0 + (k as f64 - (n as f64 - 1.0) / 2.0) * 46.0 * s + jig;
    let y = geo.pile.1 - row as f64 * 36.0 * s;
    (x, y, (36.0 + (i * 53 % 7) as f64) * s, 34.0 * s)
}

/// (top, bottom, half width) of tier `k` of the Christmas tree, relative to its tip, at scale 1.
fn tree_tier(k: usize) -> (f64, f64, f64) {
    let f = k as f64;
    (f * 88.0, 150.0 + f * 88.0, 58.0 + f * 19.0)
}

/// Height of the garland below the ceiling at fraction `f` of a swag.
fn swag_y(f: f64, s: f64) -> f64 {
    (24.0 + 4.0 * 44.0 * f * (1.0 - f)) * s
}

impl ActorT<Workshop> for Elf {
    const BASE_RADIUS: f64 = 80.0;
    const WALKER: bool = true;

    fn new(base: Base, env: &Env, _world: &mut Workshop) -> Self {
        Elf { b: base, toss: 0.0, sip: 0.0, blink_at: env.rng.uniform(1.0, 4.0), wrap: env.rng.index(WRAPS.len()) }
    }

    fn base(&self) -> &Base {
        &self.b
    }

    fn base_mut(&mut self) -> &mut Base {
        &mut self.b
    }

    fn companion_reset(&mut self) {
        self.toss = 0.0;
        self.sip = 0.0;
    }

    fn mv(&mut self, env: &mut Env, _world: &mut Workshop, others: &[Other], dt: f64, t: f64, target: Option<(f64, f64)>) {
        self.b.walker_move(env, others, dt, t, target, [85.0, 22.0, 50.0, 0.0], [1.0, 0.5, 0.5, 0.1]);
        self.toss = (self.toss - dt * 1.8).max(0.0);
        // standing still while idle: lift the mug now and then
        let want = if self.b.state() == State::Idle { self.b.graze * (0.5 + 0.5 * (t * 0.9 + self.b.seed).sin()) } else { 0.0 };
        self.sip = lerp(self.sip, want, (dt * 3.0).min(1.0));
        if t > self.blink_at + 0.2 {
            self.blink_at = t + env.rng.uniform(2.0, 6.0);
        }
    }

    fn draw(&self, cr: &mut Canvas, env: &Env, _world: &Workshop, t: f64) {
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
        let hop = if b.hop > 0.0 { (PI * b.hop).sin() * r * 0.4 } else { 0.0 };
        // glow on the floor boards + shadow
        let gy = b.y + r * 0.95;
        let mut g = Gradient::radial(b.x, gy, 0.0, b.x, gy, r * 1.7);
        g.stop(0.0, c, if st != State::Idle { 0.5 } else { 0.3 } * a);
        g.stop(1.0, c, 0.0);
        cr.save();
        cr.translate(b.x, gy);
        cr.scale(1.0, 0.3);
        cr.translate(-b.x, -gy);
        cr.set_source(&g);
        cr.circle(b.x, gy, r * 1.7);
        cr.fill();
        cr.restore();
        let sh = 1.0 - 0.3 * hop / (r * 0.4).max(1.0);
        cr.ellipse(b.x, gy, r * 0.6 * sh, r * 0.11 * sh);
        cr.set_source_rgba(0.0, 0.0, 0.0, 0.35 * a);
        cr.fill();
        if st == State::Waiting {
            cr.pulse_rings(b.x, b.y - r * 0.2, r, c, a, t, s);
        }
        cr.save();
        cr.translate(b.x, b.y - hop);
        cr.scale(if b.face.abs() > 0.15 { b.face } else { 0.15 }, 1.0);
        self.figure(cr, t, r, c, a, st);
        cr.restore();
    }
}

impl Elf {
    /// The standing elf in local coordinates, facing right (feet at y = +0.95 R, hat tip at -1.25 R).
    fn figure(&self, cr: &mut Canvas, t: f64, r: f64, c: Rgb, a: f64, st: State) {
        let b = &self.b;
        let mv = b.moving;
        cr.set_line_cap(LineCap::Round);
        cr.set_line_join(LineJoin::Round);
        let bob = (b.step * 2.0).sin() * r * 0.025 * mv;
        let swing = b.step.sin() * 0.16 * mv;
        // hands (front, back) in units of R
        let (mut fh, mut bh, mut be) = match st {
            State::Working => ((0.44, 0.16), (0.40, 0.20), (0.12, 0.16)),
            State::Idle => ((lerp(0.38, 0.27, self.sip), lerp(0.14, -0.20, self.sip)), (-0.27 - swing, 0.30), (-0.24, 0.12)),
            State::Waiting => ((0.50, -0.62), (-0.20, 0.22), (-0.42, 0.08)),
            _ => ((0.25 + swing, 0.34), (-0.27 - swing, 0.34), (-0.24, 0.14)),
        };
        if self.toss > 0.0 {
            let k = (self.toss * PI).sin();
            fh = (lerp(fh.0, 0.42, k), lerp(fh.1, -0.72, k));
            bh = (lerp(bh.0, 0.30, k), lerp(bh.1, -0.68, k));
            be = (lerp(be.0, 0.05, k), lerp(be.1, -0.3, k));
        }
        // legs: striped stockings, curled-toe shoes
        for k in 0..2 {
            let ph = b.step + k as f64 * PI;
            let hx = r * if k == 0 { -0.11 } else { 0.11 };
            let fx = hx + ph.sin() * r * 0.26 * mv;
            let fy = r * 0.88 - ph.cos().max(0.0) * r * 0.10 * mv;
            stocking(cr, hx, r * 0.42 + bob, fx, fy, r, a);
            shoe(cr, fx, fy, r, a);
        }
        let sleeve = darken(c, 0.22);
        // back arm
        arm(cr, r, (-0.12, -0.06 + bob / r), be, bh, sleeve, a);
        tunic(cr, r, bob, c, a);
        if st == State::Working {
            present(cr, (fh.0 + 0.16) * r, (fh.1 + 0.16) * r + bob, r * 0.46, r * 0.36, self.wrap, a);
        }
        // head
        cr.save();
        cr.translate(r * 0.03, -r * 0.42 + bob);
        cr.rotate(match st {
            State::Blocked => 0.2,
            State::Waiting => (t * 2.0 + b.seed).sin() * 0.05,
            _ => -0.14 * self.sip,
        });
        let blink = self.blink_at < t && t < self.blink_at + 0.2;
        let wob = (b.step * 2.0).sin() * 0.03 * mv;
        head(cr, r, c, a, blink, if st == State::Blocked { -1.0 } else { 1.0 }, if st == State::Blocked { 0.25 } else { 0.0 }, wob);
        cr.restore();
        // front arm and what it holds
        let fe = if st == State::Waiting { (0.36, -0.2) } else { ((0.14 + fh.0) / 2.0 - 0.03, (fh.1 - 0.06) / 2.0 + 0.06) };
        match st {
            State::Idle => mug(cr, fh.0 * r + r * 0.08, fh.1 * r + bob, r, a, t + b.seed),
            State::Waiting => {
                cr.save();
                cr.translate(fh.0 * r, fh.1 * r + bob);
                cr.rotate((t * 11.0 + b.seed).sin() * 0.55);
                bell(cr, r, a);
                cr.restore();
                // sound arcs
                cr.set_rgba(GOLD, 0.55 * a * (0.5 + 0.5 * (t * 11.0 + b.seed).sin().abs()));
                cr.set_line_width(r * 0.03);
                for k in 0..2 {
                    let rr = r * (0.34 + 0.12 * k as f64);
                    for (a0, a1) in [(-0.5, 0.5), (PI - 0.5, PI + 0.5)] {
                        cr.new_sub_path();
                        cr.arc(fh.0 * r, (fh.1 + 0.2) * r, rr, a0, a1);
                    }
                }
                cr.stroke();
            }
            _ => {}
        }
        arm(cr, r, (0.14, -0.06 + bob / r), fe, fh, sleeve, a);
    }

    /// Resting: asleep sitting against a big present, hat drooping over the face.
    fn draw_asleep(&self, cr: &mut Canvas, s: f64, t: f64, r: f64, a: f64) {
        let b = &self.b;
        if a < 0.01 {
            return;
        }
        let c = b.color;
        let gy = b.y + r * 0.95;
        cr.ellipse(b.x, gy, r * 1.25, r * 0.16);
        cr.set_source_rgba(0.0, 0.0, 0.0, 0.35 * a);
        cr.fill();
        let side = if b.face >= 0.0 { 1.0 } else { -1.0 };
        let breath = (t * 1.3 + b.seed).sin();
        cr.save();
        cr.translate(b.x, b.y);
        cr.scale(side, 1.0);
        cr.set_line_cap(LineCap::Round);
        cr.set_line_join(LineJoin::Round);
        present(cr, -r * 0.62, r * 0.95, r * 1.0, r * 0.95, 0, a);
        // legs stretched out, toes up
        for (fx, fy) in [(0.78, 0.80), (0.70, 0.88)] {
            stocking(cr, r * 0.02, r * 0.80, fx * r, fy * r, r, a);
            cr.set_rgba(LEATHER, a);
            cr.ellipse((fx + 0.03) * r, (fy - 0.06) * r, r * 0.075, r * 0.15);
            cr.fill();
            cr.set_line_width(r * 0.055);
            cr.move_to((fx + 0.03) * r, (fy - 0.18) * r);
            cr.quad_to((fx + 0.04) * r, (fy - 0.32) * r, (fx - 0.08) * r, (fy - 0.28) * r);
            cr.stroke();
            cr.set_rgba(GOLD, a);
            cr.circle((fx - 0.08) * r, (fy - 0.28) * r, r * 0.04);
            cr.fill();
        }
        // tunic, leaning back against the box
        let lift = breath * r * 0.012;
        cr.move_to(-r * 0.40, r * 0.16 - lift);
        cr.line_to(-r * 0.04, r * 0.10 - lift);
        cr.line_to(r * 0.26, r * 0.80);
        cr.line_to(-r * 0.30, r * 0.90);
        cr.close_path();
        cr.set_rgba(c, a);
        cr.fill_preserve();
        cr.set_rgba(darken(c, 0.4), a);
        cr.set_line_width(r * 0.03);
        cr.stroke();
        cr.set_rgba(LEATHER, a);
        cr.set_line_width(r * 0.09);
        cr.set_line_cap(LineCap::Butt);
        cr.move_to(-r * 0.36, r * 0.56);
        cr.line_to(r * 0.13, r * 0.50);
        cr.stroke();
        cr.set_line_cap(LineCap::Round);
        cr.set_rgba(GOLD, a);
        cr.rectangle(-r * 0.06, r * 0.46, r * 0.11, r * 0.12);
        cr.fill();
        arm(cr, r, (-0.14, 0.24), (-0.04, 0.52), (0.20, 0.62), darken(c, 0.22), a);
        cr.save();
        cr.translate(-r * 0.14, -r * 0.10 - lift * 2.0);
        cr.rotate(0.32 + breath * 0.02);
        head(cr, r, c, a, true, 0.0, 1.0, 0.0);
        cr.restore();
        cr.restore();
        // z z z
        for k in 0..3 {
            let f = (t * 0.3 + k as f64 / 3.0) % 1.0;
            let lay = text::layout("z", (14.0 + 14.0 * f) * s, weight::BOLD, None);
            let zx = b.x + side * r * (0.5 + 0.6 * f) + (f * 6.0).sin() * 4.0 * s;
            let zy = b.y - r * 0.6 - f * r * 1.5;
            text::show(cr, &lay, zx, zy, (1.0, 1.0, 1.0), 0.8 * (PI * f).sin() * a, 0.4);
        }
    }
}

/// Cream stocking with red stripes from the hip to the foot.
fn stocking(cr: &mut Canvas, hx: f64, hy: f64, fx: f64, fy: f64, r: f64, a: f64) {
    cr.set_line_width(r * 0.13);
    cr.set_line_cap(LineCap::Round);
    cr.set_rgba(CREAM, a);
    cr.move_to(hx, hy);
    cr.line_to(fx, fy);
    cr.stroke();
    cr.set_line_cap(LineCap::Butt);
    cr.set_rgba(RED, a);
    for f in [0.30, 0.55, 0.80] {
        cr.move_to(lerp(hx, fx, f - 0.07), lerp(hy, fy, f - 0.07));
        cr.line_to(lerp(hx, fx, f + 0.07), lerp(hy, fy, f + 0.07));
    }
    cr.stroke();
    cr.set_line_cap(LineCap::Round);
}

/// Shoe with a curled toe and a little bell.
fn shoe(cr: &mut Canvas, fx: f64, fy: f64, r: f64, a: f64) {
    cr.set_rgba(LEATHER, a);
    cr.ellipse(fx + r * 0.07, fy + r * 0.03, r * 0.17, r * 0.075);
    cr.fill();
    cr.set_line_width(r * 0.055);
    cr.move_to(fx + r * 0.2, fy + r * 0.04);
    cr.quad_to(fx + r * 0.36, fy + r * 0.03, fx + r * 0.31, fy - r * 0.10);
    cr.stroke();
    cr.set_rgba(GOLD, a);
    cr.circle(fx + r * 0.31, fy - r * 0.10, r * 0.04);
    cr.fill();
}

/// Sleeve from shoulder over the elbow to the hand; all three points in units of R.
fn arm(cr: &mut Canvas, r: f64, sh: (f64, f64), el: (f64, f64), hand: (f64, f64), sleeve: Rgb, a: f64) {
    cr.set_line_cap(LineCap::Round);
    cr.set_line_join(LineJoin::Round);
    cr.set_rgba(sleeve, a);
    cr.set_line_width(r * 0.13);
    cr.move_to(sh.0 * r, sh.1 * r);
    cr.line_to(el.0 * r, el.1 * r);
    cr.line_to(hand.0 * r, hand.1 * r);
    cr.stroke();
    cr.set_rgba(SKIN, a);
    cr.circle(hand.0 * r, hand.1 * r, r * 0.075);
    cr.fill();
}

/// Tunic with a zigzag hem, belt, buckle and collar.
fn tunic(cr: &mut Canvas, r: f64, bob: f64, c: Rgb, a: f64) {
    cr.move_to(-r * 0.23, -r * 0.16 + bob);
    cr.line_to(r * 0.23, -r * 0.16 + bob);
    cr.line_to(r * 0.40, r * 0.52 + bob);
    for k in 1..=8 {
        let f = k as f64 / 8.0;
        cr.line_to(lerp(r * 0.40, -r * 0.40, f), r * if k % 2 == 1 { 0.43 } else { 0.52 } + bob);
    }
    cr.close_path();
    cr.set_rgba(c, a);
    cr.fill_preserve();
    cr.set_rgba(darken(c, 0.4), a);
    cr.set_line_width(r * 0.03);
    cr.stroke();
    // belt (as wide as the tunic at that height)
    let hw = |y: f64| 0.23 + (y + 0.16) / 0.68 * 0.17;
    cr.set_rgba(LEATHER, a);
    cr.move_to(-r * hw(0.14), r * 0.14 + bob);
    cr.line_to(r * hw(0.14), r * 0.14 + bob);
    cr.line_to(r * hw(0.25), r * 0.25 + bob);
    cr.line_to(-r * hw(0.25), r * 0.25 + bob);
    cr.close_path();
    cr.fill();
    cr.set_rgba(GOLD, a);
    cr.rectangle(r * 0.02, r * 0.125 + bob, r * 0.13, r * 0.14);
    cr.fill();
    // collar
    cr.set_rgba(CREAM, a);
    cr.move_to(-r * 0.22, -r * 0.17 + bob);
    for k in 0..4 {
        let x = -0.22 + k as f64 * 0.11;
        cr.line_to(r * (x + 0.055), -r * 0.06 + bob);
        cr.line_to(r * (x + 0.11), -r * 0.17 + bob);
    }
    cr.close_path();
    cr.fill();
}

/// Head with pointed ear and hat, around (0, 0), facing right. `mood`: 1 smile, 0 asleep, -1 unhappy;
/// `droop`: 0 = hat tip up and back, 1 = hanging over the face.
#[allow(clippy::too_many_arguments)]
fn head(cr: &mut Canvas, r: f64, c: Rgb, a: f64, eyes_shut: bool, mood: f64, droop: f64, wob: f64) {
    let hr = r * 0.29;
    // ear
    cr.set_rgba(darken(SKIN, 0.08), a);
    cr.move_to(-r * 0.20, -r * 0.10);
    cr.line_to(-r * 0.52, -r * 0.20);
    cr.line_to(-r * 0.24, r * 0.08);
    cr.close_path();
    cr.fill();
    cr.set_rgba(SKIN, a);
    cr.circle(0.0, 0.0, hr);
    cr.fill();
    // face
    cr.set_source_rgba(0.95, 0.45, 0.42, 0.45 * a);
    cr.circle(r * 0.16, r * 0.08, r * 0.065);
    cr.fill();
    cr.set_rgba(darken(SKIN, 0.14), a);
    cr.circle(r * 0.275, r * 0.02, r * 0.045); // nose
    cr.fill();
    cr.set_source_rgba(0.16, 0.09, 0.06, a);
    cr.set_line_width(r * 0.03);
    cr.set_line_cap(LineCap::Round);
    for ex in [0.06, 0.20] {
        if eyes_shut {
            cr.new_sub_path();
            cr.arc(r * ex, -r * 0.035, r * 0.04, 0.2, PI - 0.2);
        } else {
            cr.circle(r * ex, -r * 0.03, r * 0.038);
        }
    }
    if eyes_shut {
        cr.stroke();
    } else {
        cr.fill();
    }
    cr.new_sub_path();
    if mood > 0.5 {
        cr.arc(r * 0.13, r * 0.07, r * 0.085, 0.35, PI - 0.6);
    } else if mood < -0.5 {
        cr.arc(r * 0.14, r * 0.19, r * 0.07, PI + 0.5, TAU - 0.5);
    } else {
        cr.arc(r * 0.15, r * 0.13, r * 0.025, 0.0, TAU);
    }
    cr.stroke();
    // hat: brim from the back of the head (l) to the forehead (rr), tip flopping back or forward
    let (l, rr) = ((-0.30, -0.07), (0.31, -0.13));
    let tip = (lerp(-0.44 + wob, 0.47, droop), lerp(-0.80, 0.06, droop));
    let c1 = (lerp(-0.30, -0.16, droop), lerp(-0.60, -0.70, droop));
    let c2 = (lerp(0.14, 0.40, droop), lerp(-0.68, -0.40, droop));
    cr.move_to(l.0 * r, l.1 * r);
    cr.quad_to(c1.0 * r, c1.1 * r, tip.0 * r, tip.1 * r);
    cr.quad_to(c2.0 * r, c2.1 * r, rr.0 * r, rr.1 * r);
    cr.quad_to(r * 0.02, -r * 0.27, l.0 * r, l.1 * r);
    cr.close_path();
    cr.set_rgba(c, a);
    cr.fill_preserve();
    cr.set_rgba(darken(c, 0.4), a);
    cr.set_line_width(r * 0.03);
    cr.stroke();
    cr.set_rgba(CREAM, a); // fur trim and bobble
    cr.set_line_width(r * 0.09);
    cr.move_to(l.0 * r, l.1 * r);
    cr.quad_to(r * 0.02, -r * 0.27, rr.0 * r, rr.1 * r);
    cr.stroke();
    cr.circle(tip.0 * r, tip.1 * r, r * 0.09);
    cr.fill();
}

/// Mug of cocoa with a little steam, handle towards the back; (x, y) is the middle of the mug.
fn mug(cr: &mut Canvas, x: f64, y: f64, r: f64, a: f64, t: f64) {
    cr.set_rgba(CREAM, a);
    cr.set_line_width(r * 0.035);
    cr.circle(x + r * 0.10, y, r * 0.06);
    cr.stroke();
    cr.rounded_rect(x - r * 0.10, y - r * 0.10, r * 0.20, r * 0.21, r * 0.03);
    cr.fill();
    cr.set_rgba(RED, a);
    cr.rectangle(x - r * 0.10, y - r * 0.02, r * 0.20, r * 0.05);
    cr.fill();
    cr.set_source_rgba(0.35, 0.18, 0.08, a);
    cr.ellipse(x, y - r * 0.10, r * 0.085, r * 0.025);
    cr.fill();
    cr.set_line_width(r * 0.03);
    for k in 0..2 {
        let f = (t * 0.5 + k as f64 * 0.5) % 1.0;
        let sx = x + (k as f64 - 0.5) * r * 0.08;
        let sy = y - r * 0.14 - f * r * 0.3;
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.5 * (PI * f).sin() * a);
        cr.move_to(sx, sy);
        cr.quad_to(sx + r * 0.06, sy - r * 0.05, sx, sy - r * 0.10);
        cr.stroke();
    }
}

/// Handbell hanging from the hand at (0, 0).
fn bell(cr: &mut Canvas, r: f64, a: f64) {
    cr.set_rgba(WOOD, a);
    cr.set_line_width(r * 0.06);
    cr.move_to(0.0, -r * 0.04);
    cr.line_to(0.0, r * 0.10);
    cr.stroke();
    cr.set_rgba(GOLD, a);
    cr.move_to(-r * 0.05, r * 0.09);
    cr.curve_to(-r * 0.12, r * 0.12, -r * 0.10, r * 0.26, -r * 0.17, r * 0.31);
    cr.line_to(r * 0.17, r * 0.31);
    cr.curve_to(r * 0.10, r * 0.26, r * 0.12, r * 0.12, r * 0.05, r * 0.09);
    cr.close_path();
    cr.fill();
    cr.set_rgba(darken(GOLD, 0.3), a);
    cr.circle(0.0, r * 0.33, r * 0.045);
    cr.fill();
}

/// One tongue of fire standing on (x, y).
fn flame(cr: &mut Canvas, x: f64, y: f64, w: f64, h: f64, lean: f64) {
    cr.move_to(x - w, y);
    cr.curve_to(x - w * 1.15, y - h * 0.45, x - w * 0.2 + lean * 0.5, y - h * 0.6, x + lean, y - h);
    cr.curve_to(x + w * 0.3 + lean * 0.5, y - h * 0.55, x + w * 1.15, y - h * 0.4, x + w, y);
    cr.close_path();
}

// ---------------------------------------------------------------------- the static room

fn paint_wall_and_floor(cr: &mut Canvas, env: &Env, geo: &Geo) {
    let (w, h, s, fl) = (env.w, env.h, env.s, geo.fl);
    let rng = &env.rng;
    let mut g = Gradient::linear(0.0, 0.0, 0.0, fl);
    g.add_color_stop_rgb(0.0, 0.19, 0.11, 0.07);
    g.add_color_stop_rgb(1.0, 0.36, 0.22, 0.13);
    cr.set_source(&g);
    cr.rectangle(0.0, 0.0, w, fl);
    cr.fill();
    // vertical wall planks, each a little lighter or darker
    let pw = 66.0 * s;
    let mut x = -rng.uniform(0.0, pw);
    while x < w {
        let v = rng.uniform(-1.0, 1.0);
        if v > 0.0 {
            cr.set_source_rgba(1.0, 0.8, 0.55, 0.05 * v);
        } else {
            cr.set_source_rgba(0.0, 0.0, 0.0, -0.12 * v);
        }
        cr.rectangle(x, 0.0, pw, fl);
        cr.fill();
        cr.set_source_rgba(0.08, 0.04, 0.02, 0.45);
        cr.rectangle(x, 0.0, 2.0 * s, fl);
        cr.fill();
        if rng.chance(0.3) {
            cr.set_source_rgba(0.10, 0.05, 0.03, 0.35); // knot
            cr.ellipse(x + rng.uniform(0.25, 0.75) * pw, rng.uniform(0.1, 0.9) * fl, 5.0 * s, 8.0 * s);
            cr.fill();
        }
        x += pw;
    }
    // ceiling beam
    cr.set_source_rgb(0.13, 0.07, 0.04);
    cr.rectangle(0.0, 0.0, w, 22.0 * s);
    cr.fill();
    // floor boards running towards a vanishing point above the middle
    let mut g = Gradient::linear(0.0, fl, 0.0, h);
    g.add_color_stop_rgb(0.0, 0.47, 0.31, 0.18);
    g.add_color_stop_rgb(1.0, 0.27, 0.17, 0.10);
    cr.set_source(&g);
    cr.rectangle(0.0, fl, w, h - fl);
    cr.fill();
    let vy = fl - h * 0.30;
    let k = (fl - vy) / (h - vy);
    let step = 150.0 * s;
    let n = (w / step) as i64 + 2;
    cr.set_line_width(2.0 * s);
    for i in -n..=n {
        let xb = w / 2.0 + (i as f64 + 0.5) * step;
        let xt = w / 2.0 + (i as f64 + 0.5) * step * k;
        cr.set_source_rgba(0.10, 0.05, 0.03, 0.40);
        cr.move_to(xt, fl);
        cr.line_to(xb, h);
        cr.stroke();
        // board ends
        for _ in 0..2 {
            let f = rng.uniform(0.05, 0.95);
            let y = lerp(fl, h, f * f);
            let d = (y - vy) / (h - vy);
            cr.set_source_rgba(0.10, 0.05, 0.03, 0.30);
            cr.move_to(w / 2.0 + (i as f64 + 0.5) * step * d, y);
            cr.line_to(w / 2.0 + (i as f64 + 1.5) * step * d, y);
            cr.stroke();
        }
    }
    // skirting board
    cr.set_source_rgb(0.20, 0.11, 0.06);
    cr.rectangle(0.0, fl - 16.0 * s, w, 16.0 * s);
    cr.fill();
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.25);
    cr.rectangle(0.0, fl, w, 8.0 * s);
    cr.fill();
}

fn paint_fireplace(cr: &mut Canvas, env: &Env, geo: &Geo) {
    let (s, fl, fx) = (env.s, geo.fl, geo.fx);
    let rng = &env.rng;
    let (hw, top) = (160.0 * s, fl - 280.0 * s);
    // chimney breast up to the ceiling, then the surround; bricks row by row
    for (x0, x1, y0, y1, shade) in [(fx - 115.0 * s, fx + 115.0 * s, 0.0, top, 0.25), (fx - hw, fx + hw, top, fl, 0.0)] {
        cr.set_source_rgb(0.30, 0.20, 0.16);
        cr.rectangle(x0, y0, x1 - x0, y1 - y0);
        cr.fill();
        let (bh, bw) = (24.0 * s, 54.0 * s);
        let mut row = 0;
        let mut y = y1 - bh;
        while y > y0 - bh {
            let mut x = x0 - if row % 2 == 1 { bw / 2.0 } else { 0.0 };
            while x < x1 {
                let (xa, xb) = (x.max(x0) + 1.5 * s, (x + bw).min(x1) - 1.5 * s);
                let (ya, yb) = (y.max(y0) + 1.5 * s, y + bh - 1.5 * s);
                if xb > xa && yb > ya {
                    let v = rng.uniform(0.0, 1.0);
                    cr.set_rgb(darken((lerp(0.52, 0.66, v), lerp(0.20, 0.27, v), lerp(0.14, 0.18, v)), shade));
                    cr.rectangle(xa, ya, xb - xa, yb - ya);
                    cr.fill();
                }
                x += bw;
            }
            y -= bh;
            row += 1;
        }
    }
    // hearth stone
    cr.set_source_rgb(0.34, 0.30, 0.28);
    cr.rectangle(fx - hw - 22.0 * s, fl - 6.0 * s, 2.0 * (hw + 22.0 * s), 26.0 * s);
    cr.fill();
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.10);
    cr.rectangle(fx - hw - 22.0 * s, fl - 6.0 * s, 2.0 * (hw + 22.0 * s), 5.0 * s);
    cr.fill();
    // fire box with an arched top, logs
    let (ow, oh) = (88.0 * s, 170.0 * s);
    cr.move_to(fx - ow, fl - 6.0 * s);
    cr.line_to(fx - ow, fl - oh + 50.0 * s);
    cr.curve_to(fx - ow, fl - oh - 14.0 * s, fx + ow, fl - oh - 14.0 * s, fx + ow, fl - oh + 50.0 * s);
    cr.line_to(fx + ow, fl - 6.0 * s);
    cr.close_path();
    cr.set_source_rgb(0.07, 0.035, 0.03);
    cr.fill_preserve();
    cr.set_source_rgb(0.22, 0.15, 0.13);
    cr.set_line_width(10.0 * s);
    cr.stroke();
    cr.set_line_cap(LineCap::Round);
    cr.set_line_width(20.0 * s);
    for (x0, y0, x1, y1, col) in [(-58.0, -18.0, 44.0, -26.0, (0.30, 0.17, 0.09)), (-40.0, -30.0, 60.0, -16.0, (0.38, 0.22, 0.11))] {
        cr.set_rgb(col);
        cr.move_to(fx + x0 * s, fl + y0 * s);
        cr.line_to(fx + x1 * s, fl + y1 * s);
        cr.stroke();
    }
    cr.set_line_cap(LineCap::Butt);
    // mantel shelf
    cr.set_source_rgb(0.42, 0.25, 0.13);
    cr.rectangle(fx - hw - 24.0 * s, top - 22.0 * s, 2.0 * (hw + 24.0 * s), 24.0 * s);
    cr.fill();
    cr.set_source_rgba(1.0, 0.85, 0.6, 0.18);
    cr.rectangle(fx - hw - 24.0 * s, top - 22.0 * s, 2.0 * (hw + 24.0 * s), 5.0 * s);
    cr.fill();
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.3);
    cr.rectangle(fx - hw, top + 2.0 * s, 2.0 * hw, 8.0 * s);
    cr.fill();
    // stockings hanging from the mantel
    for (k, dx) in [-118.0, -40.0, 40.0, 118.0].into_iter().enumerate() {
        let (x, y) = (fx + dx * s, top + 2.0 * s);
        let col = if k % 2 == 0 { RED } else { GREEN };
        cr.set_rgb(col);
        cr.set_line_cap(LineCap::Round);
        cr.set_line_join(LineJoin::Round);
        cr.set_line_width(24.0 * s);
        cr.move_to(x, y + 14.0 * s);
        cr.line_to(x, y + 52.0 * s);
        cr.line_to(x + 16.0 * s, y + 56.0 * s);
        cr.stroke();
        cr.set_line_cap(LineCap::Butt);
        cr.set_rgb(CREAM);
        cr.rounded_rect(x - 15.0 * s, y, 30.0 * s, 16.0 * s, 5.0 * s);
        cr.fill();
        cr.circle(x + 22.0 * s, y + 56.0 * s, 5.0 * s);
        cr.fill();
    }
    // on the mantel: two candles and a little clock
    for dx in [-140.0, 140.0] {
        let x = fx + dx * s;
        cr.set_rgb(CREAM);
        cr.rectangle(x - 6.0 * s, top - 58.0 * s, 12.0 * s, 36.0 * s);
        cr.fill();
        cr.glow(x, top - 68.0 * s, 46.0 * s, (1.0, 0.8, 0.4), 0.45, 0.0);
        cr.set_source_rgb(1.0, 0.85, 0.45);
        cr.ellipse(x, top - 67.0 * s, 4.5 * s, 9.0 * s);
        cr.fill();
    }
    cr.set_rgb(WOOD);
    cr.rounded_rect(fx - 30.0 * s, top - 76.0 * s, 60.0 * s, 54.0 * s, 24.0 * s);
    cr.fill();
    cr.set_rgb(CREAM);
    cr.circle(fx, top - 50.0 * s, 19.0 * s);
    cr.fill();
    cr.set_source_rgb(0.15, 0.1, 0.08);
    cr.set_line_width(2.5 * s);
    cr.move_to(fx, top - 50.0 * s);
    cr.line_to(fx, top - 64.0 * s);
    cr.move_to(fx, top - 50.0 * s);
    cr.line_to(fx - 9.0 * s, top - 46.0 * s);
    cr.stroke();
    // wreath on the chimney breast
    let wy = top - 190.0 * s;
    cr.set_line_width(26.0 * s);
    cr.set_source_rgb(0.08, 0.30, 0.16);
    cr.circle(fx, wy, 52.0 * s);
    cr.stroke();
    cr.set_line_width(8.0 * s);
    cr.set_source_rgb(0.15, 0.44, 0.24);
    cr.circle(fx, wy, 56.0 * s);
    cr.stroke();
    for k in 0..7 {
        let ang = k as f64 * TAU / 7.0 + 0.4;
        cr.set_rgb(RED);
        cr.circle(fx + ang.cos() * 52.0 * s, wy + ang.sin() * 52.0 * s, 5.5 * s);
        cr.fill();
    }
    cr.set_rgb(RED);
    cr.ellipse(fx - 16.0 * s, wy + 54.0 * s, 17.0 * s, 10.0 * s);
    cr.ellipse(fx + 16.0 * s, wy + 54.0 * s, 17.0 * s, 10.0 * s);
    cr.fill();
    cr.set_rgb(GOLD);
    cr.circle(fx, wy + 54.0 * s, 6.0 * s);
    cr.fill();
}

/// The winter night behind the glass.
fn paint_window_view(cr: &mut Canvas, env: &Env, geo: &Geo) {
    let s = env.s;
    let rng = &env.rng;
    let (x, y, w, h) = geo.win;
    cr.save();
    cr.rectangle(x, y, w, h);
    cr.clip();
    let mut g = Gradient::linear(0.0, y, 0.0, y + h);
    g.add_color_stop_rgb(0.0, 0.04, 0.07, 0.20);
    g.add_color_stop_rgb(1.0, 0.16, 0.25, 0.48);
    cr.set_source(&g);
    cr.rectangle(x, y, w, h);
    cr.fill();
    for _ in 0..22 {
        cr.set_source_rgba(1.0, 1.0, 1.0, rng.uniform(0.4, 0.9));
        cr.circle(x + rng.uniform(0.0, w), y + rng.uniform(0.0, h * 0.55), rng.uniform(0.8, 1.8) * s);
        cr.fill();
    }
    let (mx, my) = (x + w * 0.74, y + h * 0.22);
    cr.glow(mx, my, 70.0 * s, (0.8, 0.85, 1.0), 0.35, 0.0);
    cr.set_source_rgb(0.96, 0.96, 0.88);
    cr.circle(mx, my, 20.0 * s);
    cr.fill();
    // snowy hills and firs
    for (base, amp, col) in [(0.66, 0.05, (0.42, 0.52, 0.76)), (0.78, 0.04, (0.72, 0.80, 0.94))] {
        cr.move_to(x, y + h);
        for k in 0..=12 {
            let f = k as f64 / 12.0;
            cr.line_to(x + f * w, y + h * (base + amp * (f * 5.0 + base * 9.0).sin()));
        }
        cr.line_to(x + w, y + h);
        cr.close_path();
        cr.set_rgb(col);
        cr.fill();
        if base < 0.7 {
            for (fxp, th) in [(0.14, 78.0), (0.30, 56.0), (0.52, 66.0), (0.90, 84.0)] {
                let (tx, ty) = (x + fxp * w, y + h * 0.74);
                cr.set_source_rgb(0.07, 0.16, 0.20);
                cr.move_to(tx, ty - th * s);
                cr.line_to(tx + th * 0.30 * s, ty);
                cr.line_to(tx - th * 0.30 * s, ty);
                cr.close_path();
                cr.fill();
                cr.set_source_rgba(0.9, 0.94, 1.0, 0.85);
                cr.move_to(tx, ty - th * s);
                cr.line_to(tx + th * 0.13 * s, ty - th * 0.58 * s);
                cr.line_to(tx - th * 0.13 * s, ty - th * 0.58 * s);
                cr.close_path();
                cr.fill();
            }
        }
    }
    // snow lying on the outer sill
    cr.set_source_rgb(0.93, 0.96, 1.0);
    cr.move_to(x, y + h);
    cr.curve_to(x + w * 0.2, y + h - 20.0 * s, x + w * 0.5, y + h - 4.0 * s, x + w * 0.62, y + h - 12.0 * s);
    cr.curve_to(x + w * 0.8, y + h - 22.0 * s, x + w * 0.95, y + h - 10.0 * s, x + w, y + h - 16.0 * s);
    cr.line_to(x + w, y + h);
    cr.close_path();
    cr.fill();
    cr.restore();
}

fn paint_window_frame(cr: &mut Canvas, s: f64, geo: &Geo) {
    let (x, y, w, h) = geo.win;
    cr.set_source_rgb(0.25, 0.14, 0.08);
    cr.set_line_width(22.0 * s);
    cr.rectangle(x - 6.0 * s, y - 6.0 * s, w + 12.0 * s, h + 12.0 * s);
    cr.stroke();
    cr.set_source_rgb(0.56, 0.36, 0.20);
    cr.set_line_width(13.0 * s);
    cr.rectangle(x - 5.0 * s, y - 5.0 * s, w + 10.0 * s, h + 10.0 * s);
    cr.stroke();
    cr.set_line_width(8.0 * s);
    cr.move_to(x + w / 2.0, y);
    cr.line_to(x + w / 2.0, y + h);
    cr.move_to(x, y + h * 0.5);
    cr.line_to(x + w, y + h * 0.5);
    cr.stroke();
    // sill
    cr.set_source_rgb(0.50, 0.31, 0.17);
    cr.rectangle(x - 30.0 * s, y + h + 8.0 * s, w + 60.0 * s, 18.0 * s);
    cr.fill();
    cr.set_source_rgba(1.0, 0.9, 0.7, 0.2);
    cr.rectangle(x - 30.0 * s, y + h + 8.0 * s, w + 60.0 * s, 4.0 * s);
    cr.fill();
}

/// Two toy shelves above a workbench, centred on `cx`.
fn paint_shelves_and_bench(cr: &mut Canvas, s: f64, fl: f64, cx: f64) {
    let hw = 150.0 * s;
    let board = |cr: &mut Canvas, y: f64| {
        cr.set_source_rgb(0.20, 0.11, 0.06);
        for dx in [-110.0, 110.0] {
            cr.move_to(cx + dx * s, y + 12.0 * s);
            cr.line_to(cx + dx * s, y + 40.0 * s);
            cr.line_to(cx + (dx + 22.0) * s, y + 12.0 * s);
            cr.close_path();
        }
        cr.fill();
        cr.set_source_rgba(0.0, 0.0, 0.0, 0.25);
        cr.rectangle(cx - hw + 6.0 * s, y + 12.0 * s, 2.0 * hw, 10.0 * s);
        cr.fill();
        cr.set_source_rgb(0.50, 0.31, 0.17);
        cr.rectangle(cx - hw, y, 2.0 * hw, 13.0 * s);
        cr.fill();
        cr.set_source_rgba(1.0, 0.9, 0.7, 0.22);
        cr.rectangle(cx - hw, y, 2.0 * hw, 3.5 * s);
        cr.fill();
    };
    let (y1, y2) = (fl - 345.0 * s, fl - 225.0 * s);
    board(cr, y1);
    board(cr, y2);
    // --- top shelf: teddy, ball, building blocks
    let (tx, ty) = (cx - 100.0 * s, y1);
    let fur = (0.66, 0.44, 0.24);
    cr.set_rgb(fur);
    cr.circle(tx - 15.0 * s, ty - 66.0 * s, 8.0 * s); // ears
    cr.circle(tx + 15.0 * s, ty - 66.0 * s, 8.0 * s);
    cr.circle(tx - 22.0 * s, ty - 8.0 * s, 10.0 * s); // feet
    cr.circle(tx + 22.0 * s, ty - 8.0 * s, 10.0 * s);
    cr.circle(tx - 25.0 * s, ty - 32.0 * s, 8.0 * s); // arms
    cr.circle(tx + 25.0 * s, ty - 32.0 * s, 8.0 * s);
    cr.ellipse(tx, ty - 24.0 * s, 23.0 * s, 24.0 * s);
    cr.circle(tx, ty - 54.0 * s, 18.0 * s);
    cr.fill();
    cr.set_rgb(lighten(fur, 0.35));
    cr.ellipse(tx, ty - 22.0 * s, 13.0 * s, 15.0 * s);
    cr.circle(tx, ty - 49.0 * s, 8.0 * s);
    cr.fill();
    cr.set_source_rgb(0.12, 0.07, 0.05);
    cr.circle(tx - 7.0 * s, ty - 58.0 * s, 2.4 * s);
    cr.circle(tx + 7.0 * s, ty - 58.0 * s, 2.4 * s);
    cr.circle(tx, ty - 51.0 * s, 2.8 * s);
    cr.fill();
    cr.set_rgb(RED);
    cr.ellipse(tx - 8.0 * s, ty - 38.0 * s, 8.0 * s, 4.5 * s); // bow tie
    cr.ellipse(tx + 8.0 * s, ty - 38.0 * s, 8.0 * s, 4.5 * s);
    cr.fill();
    let (bx, by, br) = (cx - 22.0 * s, y1 - 24.0 * s, 24.0 * s);
    cr.set_rgb(RED);
    cr.circle(bx, by, br);
    cr.fill();
    cr.save();
    cr.circle(bx, by, br);
    cr.clip();
    cr.set_rgb(GOLD);
    cr.rectangle(bx - br, by - 6.0 * s, 2.0 * br, 12.0 * s);
    cr.fill();
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.35);
    cr.circle(bx - 9.0 * s, by - 10.0 * s, 7.0 * s);
    cr.fill();
    cr.restore();
    for (dx, dy, col) in [(50.0, 0.0, RED), (84.0, 0.0, (0.20, 0.42, 0.75)), (118.0, 0.0, GOLD), (67.0, 32.0, GREEN), (101.0, 32.0, CREAM)] {
        let (x, y) = (cx + dx * s, y1 - (dy + 32.0) * s);
        cr.set_rgb(col);
        cr.rectangle(x - 16.0 * s, y, 32.0 * s, 32.0 * s);
        cr.fill();
        cr.set_source_rgba(0.0, 0.0, 0.0, 0.22);
        cr.set_line_width(3.0 * s);
        cr.rectangle(x - 11.0 * s, y + 5.0 * s, 22.0 * s, 22.0 * s);
        cr.stroke();
    }
    // --- lower shelf: toy train, drum, sailing boat
    let (ex, ey) = (cx - 78.0 * s, y2 - 9.0 * s);
    cr.set_rgb(GREEN);
    cr.rectangle(ex - 60.0 * s, ey - 26.0 * s, 44.0 * s, 26.0 * s); // wagon
    cr.fill();
    cr.set_rgb(RED);
    cr.rectangle(ex - 8.0 * s, ey - 28.0 * s, 56.0 * s, 28.0 * s); // boiler
    cr.rectangle(ex - 8.0 * s, ey - 52.0 * s, 26.0 * s, 26.0 * s); // cab
    cr.fill();
    cr.set_source_rgb(0.14, 0.10, 0.09);
    cr.rectangle(ex + 30.0 * s, ey - 46.0 * s, 10.0 * s, 18.0 * s); // funnel
    cr.rectangle(ex - 18.0 * s, ey - 10.0 * s, 12.0 * s, 4.0 * s); // coupling
    cr.fill();
    cr.set_rgb(GOLD);
    cr.rectangle(ex - 12.0 * s, ey - 56.0 * s, 34.0 * s, 6.0 * s);
    cr.rectangle(ex - 2.0 * s, ey - 46.0 * s, 14.0 * s, 12.0 * s);
    cr.fill();
    for dx in [-48.0, -28.0, 4.0, 34.0] {
        cr.set_source_rgb(0.14, 0.10, 0.09);
        cr.circle(ex + dx * s, ey, 9.0 * s);
        cr.fill();
        cr.set_rgb(GOLD);
        cr.circle(ex + dx * s, ey, 3.5 * s);
        cr.fill();
    }
    let (dx, dy) = (cx + 24.0 * s, y2);
    cr.set_rgb(RED);
    cr.rectangle(dx - 26.0 * s, dy - 40.0 * s, 52.0 * s, 40.0 * s);
    cr.fill();
    cr.set_rgb(GOLD);
    cr.rectangle(dx - 28.0 * s, dy - 42.0 * s, 56.0 * s, 8.0 * s);
    cr.rectangle(dx - 28.0 * s, dy - 8.0 * s, 56.0 * s, 8.0 * s);
    cr.fill();
    cr.set_rgb(CREAM);
    cr.set_line_width(2.5 * s);
    cr.move_to(dx - 26.0 * s, dy - 34.0 * s);
    for k in 1..=6 {
        cr.line_to(dx + (-26.0 + k as f64 * 52.0 / 6.0) * s, dy - if k % 2 == 1 { 8.0 } else { 34.0 } * s);
    }
    cr.stroke();
    let (sx, sy) = (cx + 100.0 * s, y2);
    cr.set_rgb(WOOD);
    cr.rectangle(sx - 2.0 * s, sy - 66.0 * s, 4.0 * s, 52.0 * s);
    cr.fill();
    cr.set_source_rgb(0.20, 0.42, 0.75);
    cr.move_to(sx - 36.0 * s, sy - 18.0 * s);
    cr.line_to(sx + 36.0 * s, sy - 18.0 * s);
    cr.line_to(sx + 24.0 * s, sy);
    cr.line_to(sx - 24.0 * s, sy);
    cr.close_path();
    cr.fill();
    cr.set_rgb(CREAM);
    cr.move_to(sx + 4.0 * s, sy - 66.0 * s);
    cr.line_to(sx + 32.0 * s, sy - 24.0 * s);
    cr.line_to(sx + 4.0 * s, sy - 24.0 * s);
    cr.close_path();
    cr.move_to(sx - 4.0 * s, sy - 56.0 * s);
    cr.line_to(sx - 24.0 * s, sy - 24.0 * s);
    cr.line_to(sx - 4.0 * s, sy - 24.0 * s);
    cr.close_path();
    cr.fill();
    // --- workbench with tools
    let (top, bw) = (fl - 100.0 * s, 165.0 * s);
    cr.set_source_rgb(0.30, 0.18, 0.10);
    for dx in [-140.0, 124.0] {
        cr.rectangle(cx + dx * s, top, 16.0 * s, fl - top + 26.0 * s);
    }
    cr.rectangle(cx - 140.0 * s, fl - 34.0 * s, 280.0 * s, 12.0 * s);
    cr.fill();
    cr.set_source_rgb(0.58, 0.39, 0.22);
    cr.rectangle(cx - bw, top - 16.0 * s, 2.0 * bw, 18.0 * s);
    cr.fill();
    cr.set_source_rgba(1.0, 0.9, 0.7, 0.25);
    cr.rectangle(cx - bw, top - 16.0 * s, 2.0 * bw, 4.0 * s);
    cr.fill();
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.3);
    cr.rectangle(cx - bw + 8.0 * s, top + 2.0 * s, 2.0 * bw - 16.0 * s, 8.0 * s);
    cr.fill();
    let ty = top - 16.0 * s;
    // paint pots with brushes
    for (dx, col) in [(-128.0, RED), (-94.0, (0.20, 0.42, 0.75))] {
        let x = cx + dx * s;
        cr.set_rgb(WOOD);
        cr.set_line_width(4.0 * s);
        cr.move_to(x + 4.0 * s, ty - 26.0 * s);
        cr.line_to(x + 16.0 * s, ty - 54.0 * s);
        cr.stroke();
        cr.set_source_rgb(0.62, 0.64, 0.68);
        cr.rectangle(x - 13.0 * s, ty - 30.0 * s, 26.0 * s, 30.0 * s);
        cr.fill();
        cr.set_rgb(col);
        cr.rectangle(x - 13.0 * s, ty - 30.0 * s, 26.0 * s, 8.0 * s);
        cr.rectangle(x - 6.0 * s, ty - 24.0 * s, 5.0 * s, 12.0 * s);
        cr.fill();
    }
    // wooden toy car being built
    let x = cx - 22.0 * s;
    cr.set_source_rgb(0.78, 0.60, 0.38);
    cr.rounded_rect(x - 36.0 * s, ty - 26.0 * s, 72.0 * s, 18.0 * s, 6.0 * s);
    cr.rounded_rect(x - 18.0 * s, ty - 42.0 * s, 34.0 * s, 20.0 * s, 7.0 * s);
    cr.fill();
    for dx in [-20.0, 20.0] {
        cr.set_rgb(RED);
        cr.circle(x + dx * s, ty - 9.0 * s, 9.0 * s);
        cr.fill();
        cr.set_rgb(CREAM);
        cr.circle(x + dx * s, ty - 9.0 * s, 3.0 * s);
        cr.fill();
    }
    // hammer and a spool of ribbon
    let x = cx + 62.0 * s;
    cr.set_rgb(WOOD);
    cr.set_line_width(6.0 * s);
    cr.move_to(x - 30.0 * s, ty - 4.0 * s);
    cr.line_to(x + 22.0 * s, ty - 14.0 * s);
    cr.stroke();
    cr.set_source_rgb(0.45, 0.47, 0.52);
    cr.rounded_rect(x + 14.0 * s, ty - 28.0 * s, 14.0 * s, 28.0 * s, 3.0 * s);
    cr.fill();
    let x = cx + 122.0 * s;
    cr.set_rgb(RED);
    cr.circle(x, ty - 17.0 * s, 17.0 * s);
    cr.fill();
    cr.set_rgb(CREAM);
    cr.circle(x, ty - 17.0 * s, 6.0 * s);
    cr.fill();
    cr.set_rgb(RED);
    cr.set_line_width(5.0 * s);
    cr.move_to(x + 16.0 * s, ty - 12.0 * s);
    cr.curve_to(x + 32.0 * s, ty - 2.0 * s, x + 22.0 * s, ty + 20.0 * s, x + 34.0 * s, ty + 34.0 * s);
    cr.stroke();
}

/// Christmas tree with its tip at (tx, tip): bucket, tiers, tinsel, baubles and star.
fn paint_tree(cr: &mut Canvas, env: &Env, tx: f64, tip: f64) {
    let s = env.s;
    let rng = &env.rng;
    let base = tip + tree_tier(4).1 * s;
    cr.set_source_rgb(0.26, 0.15, 0.08);
    cr.rectangle(tx - 13.0 * s, base - 12.0 * s, 26.0 * s, 44.0 * s);
    cr.fill();
    cr.set_rgb(RED);
    cr.move_to(tx - 44.0 * s, base + 22.0 * s);
    cr.line_to(tx + 44.0 * s, base + 22.0 * s);
    cr.line_to(tx + 34.0 * s, base + 68.0 * s);
    cr.line_to(tx - 34.0 * s, base + 68.0 * s);
    cr.close_path();
    cr.fill();
    cr.set_rgb(GOLD);
    cr.rectangle(tx - 46.0 * s, base + 20.0 * s, 92.0 * s, 9.0 * s);
    cr.fill();
    for k in (0..5).rev() {
        let (t0, t1, hw) = tree_tier(k);
        let (top, bot, hw) = (tip + t0 * s, tip + t1 * s, hw * s);
        let f = k as f64 / 4.0;
        cr.move_to(tx, top);
        cr.line_to(tx + hw, bot);
        for j in 1..=6 {
            // scalloped lower edge
            let x0 = tx + hw - (j as f64 - 0.5) * hw / 3.0;
            let x1 = tx + hw - j as f64 * hw / 3.0;
            cr.quad_to(x0, bot + 20.0 * s, x1, bot - if j == 6 { 0.0 } else { 6.0 * s });
        }
        cr.close_path();
        cr.set_source_rgb(lerp(0.10, 0.05, f), lerp(0.42, 0.27, f), lerp(0.22, 0.15, f));
        cr.fill();
        // light from the fire on the right flank
        cr.move_to(tx, top);
        cr.line_to(tx + hw, bot);
        cr.line_to(tx + hw * 0.45, bot);
        cr.close_path();
        cr.set_source_rgba(1.0, 0.8, 0.4, 0.07);
        cr.fill();
        // tinsel
        cr.set_rgba(GOLD, 0.75);
        cr.set_line_width(3.0 * s);
        for j in 0..=10 {
            let u = j as f64 / 5.0 - 1.0;
            cr.line_to(tx + u * hw * 0.78, bot - (44.0 - 22.0 * (1.0 - u * u)) * s);
        }
        cr.stroke();
        for _ in 0..(2 + k) {
            let u = rng.uniform(-0.7, 0.7);
            let v = rng.uniform(0.45, 0.8);
            let (x, y) = (tx + u * hw * v, lerp(top, bot, v) - 2.0 * s);
            let col = *rng.choice(&[RED, GOLD, (0.25, 0.50, 0.85), CREAM]);
            cr.set_rgb(col);
            cr.circle(x, y, 8.0 * s);
            cr.fill();
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.5);
            cr.circle(x - 2.5 * s, y - 2.5 * s, 2.5 * s);
            cr.fill();
        }
    }
    cr.glow(tx, tip - 8.0 * s, 60.0 * s, GOLD, 0.5, 0.0);
    cr.set_rgb(GOLD);
    for k in 0..10 {
        let ang = -PI / 2.0 + k as f64 * PI / 5.0;
        let rad = if k % 2 == 0 { 24.0 } else { 10.0 } * s;
        cr.line_to(tx + ang.cos() * rad, tip - 8.0 * s + ang.sin() * rad);
    }
    cr.close_path();
    cr.fill();
}

/// Santa's sack with toys peeking out, standing on (x, y).
fn paint_sack(cr: &mut Canvas, s: f64, x: f64, y: f64) {
    // what sticks out of the top
    present(cr, x - 24.0 * s, y - 176.0 * s, 44.0 * s, 40.0 * s, 1, 1.0);
    present(cr, x + 26.0 * s, y - 170.0 * s, 40.0 * s, 46.0 * s, 3, 1.0);
    cr.set_line_cap(LineCap::Round);
    cr.set_rgb(CREAM); // candy cane
    cr.set_line_width(10.0 * s);
    cr.move_to(x + 2.0 * s, y - 170.0 * s);
    cr.line_to(x + 2.0 * s, y - 236.0 * s);
    cr.quad_to(x + 2.0 * s, y - 258.0 * s, x - 18.0 * s, y - 252.0 * s);
    cr.stroke();
    cr.set_rgb(RED);
    cr.set_line_cap(LineCap::Butt);
    for k in 0..4 {
        let yy = y - (182.0 + k as f64 * 16.0) * s;
        cr.move_to(x + 2.0 * s, yy);
        cr.line_to(x + 2.0 * s, yy - 7.0 * s);
    }
    cr.stroke();
    // the sack
    cr.move_to(x - 62.0 * s, y - 168.0 * s);
    cr.curve_to(x - 120.0 * s, y - 110.0 * s, x - 112.0 * s, y - 6.0 * s, x - 70.0 * s, y);
    cr.line_to(x + 72.0 * s, y);
    cr.curve_to(x + 112.0 * s, y - 8.0 * s, x + 118.0 * s, y - 112.0 * s, x + 62.0 * s, y - 168.0 * s);
    cr.close_path();
    let mut g = Gradient::linear(x - 100.0 * s, 0.0, x + 100.0 * s, 0.0);
    g.add_color_stop_rgb(0.0, 0.80, 0.16, 0.15);
    g.add_color_stop_rgb(0.55, 0.66, 0.10, 0.12);
    g.add_color_stop_rgb(1.0, 0.40, 0.05, 0.08);
    cr.set_source(&g);
    cr.fill();
    cr.set_source_rgba(0.25, 0.02, 0.04, 0.5); // folds
    cr.set_line_width(4.0 * s);
    cr.set_line_cap(LineCap::Round);
    for (dx, bend) in [(-34.0, -18.0), (8.0, 6.0), (44.0, 20.0)] {
        cr.move_to(x + dx * s, y - 150.0 * s);
        cr.quad_to(x + (dx + bend) * s, y - 100.0 * s, x + (dx + bend * 0.6) * s, y - 44.0 * s);
    }
    cr.stroke();
    // rolled rim and a patch
    cr.set_rgb(CREAM);
    cr.rounded_rect(x - 72.0 * s, y - 184.0 * s, 144.0 * s, 26.0 * s, 13.0 * s);
    cr.fill();
    cr.set_rgb(GREEN);
    cr.rectangle(x - 66.0 * s, y - 84.0 * s, 34.0 * s, 30.0 * s);
    cr.fill();
    cr.set_line_cap(LineCap::Butt);
}

impl Workshop {
    fn layout(env: &Env) -> Geo {
        let (w, h, s) = (env.w, env.h, env.s);
        let fl = h * 0.56;
        Geo {
            fl,
            fx: w * 0.275,
            win: (w * 0.5 - 125.0 * s, fl - 475.0 * s, 250.0 * s, 250.0 * s),
            pile: (w - 300.0 * s, fl + 66.0 * s),
        }
    }

    /// Tip of the Christmas tree.
    fn tree_at(env: &Env, geo: &Geo) -> (f64, f64) {
        (150.0 * env.s, geo.fl - 30.0 * env.s - tree_tier(4).1 * env.s)
    }

    fn paint_room(&self, cr: &mut Canvas, env: &Env) {
        let (w, h, s) = (env.w, env.h, env.s);
        let geo = &self.geo;
        let fl = geo.fl;
        paint_wall_and_floor(cr, env, geo);
        paint_fireplace(cr, env, geo);
        paint_window_view(cr, env, geo);
        cr.glow(geo.win.0 + geo.win.2 / 2.0, geo.win.1 + geo.win.3 / 2.0, 330.0 * s, (0.45, 0.6, 1.0), 0.10, 0.3);
        paint_shelves_and_bench(cr, s, fl, w * 0.715);
        let (tx, tip) = Self::tree_at(env, geo);
        // presents under the tree
        present(cr, tx + 92.0 * s, fl + 44.0 * s, 62.0 * s, 46.0 * s, 4, 1.0);
        paint_tree(cr, env, tx, tip);
        present(cr, tx - 78.0 * s, fl + 52.0 * s, 54.0 * s, 58.0 * s, 2, 1.0);
        present(cr, tx + 52.0 * s, fl + 62.0 * s, 46.0 * s, 36.0 * s, 5, 1.0);
        // Santa's sack and the big presents the pile grows in front of
        let (px, py) = geo.pile;
        present(cr, px - 62.0 * s, py - 20.0 * s, 96.0 * s, 110.0 * s, 1, 1.0);
        present(cr, px + 40.0 * s, py - 22.0 * s, 110.0 * s, 78.0 * s, 4, 1.0);
        present(cr, px + 34.0 * s, py - 100.0 * s, 70.0 * s, 54.0 * s, 2, 1.0);
        paint_sack(cr, s, w - 108.0 * s, fl + 78.0 * s);
        // garland along the ceiling beam
        let n = (w / (230.0 * s)).round().max(2.0) as usize;
        let span = w / n as f64;
        cr.set_line_cap(LineCap::Round);
        for (lw, col, dy) in [(18.0, (0.07, 0.27, 0.15), 0.0), (7.0, (0.14, 0.42, 0.23), -3.0)] {
            cr.set_rgb(col);
            cr.set_line_width(lw * s);
            for i in 0..n {
                for j in 0..=16 {
                    let f = j as f64 / 16.0;
                    let jag = if j % 2 == 0 { 0.0 } else { 5.0 * s };
                    cr.line_to((i as f64 + f) * span, swag_y(f, s) + dy * s + jag);
                }
            }
            cr.stroke();
        }
        cr.set_line_cap(LineCap::Butt);
        for i in 1..n {
            let (x, y) = (i as f64 * span, swag_y(0.0, s));
            cr.set_rgb(RED);
            cr.ellipse(x - 14.0 * s, y + 2.0 * s, 15.0 * s, 9.0 * s);
            cr.ellipse(x + 14.0 * s, y + 2.0 * s, 15.0 * s, 9.0 * s);
            cr.move_to(x, y);
            cr.line_to(x - 12.0 * s, y + 34.0 * s);
            cr.line_to(x, y + 26.0 * s);
            cr.line_to(x + 12.0 * s, y + 34.0 * s);
            cr.close_path();
            cr.fill();
            cr.set_rgb(GOLD);
            cr.circle(x, y + 2.0 * s, 6.0 * s);
            cr.fill();
        }
        // warm light from the fire, dark corners
        cr.glow(geo.fx, fl - 60.0 * s, 620.0 * s, (1.0, 0.62, 0.25), 0.20, 0.0);
        let mut v = Gradient::radial(w / 2.0, h * 0.55, h * 0.42, w / 2.0, h * 0.55, hypot(w, h) * 0.62);
        v.add_color_stop_rgba(0.0, 0.03, 0.01, 0.0, 0.0);
        v.add_color_stop_rgba(1.0, 0.03, 0.01, 0.0, 0.55);
        cr.set_source(&v);
        cr.rectangle(0.0, 0.0, w, h);
        cr.fill();
    }

    fn draw_fire(&self, cr: &mut Canvas, s: f64, t: f64) {
        let (fx, fy) = (self.geo.fx, self.geo.fl - 22.0 * s);
        let flick = 0.5 + 0.25 * (t * 9.0).sin() + 0.25 * (t * 14.3 + 1.0).sin();
        cr.glow(fx, fy - 40.0 * s, 300.0 * s, (1.0, 0.55, 0.18), 0.20 + 0.06 * flick, 0.0);
        // glow on the floor in front of the hearth
        cr.save();
        cr.translate(fx, self.geo.fl + 70.0 * s);
        cr.scale(1.0, 0.32);
        cr.glow(0.0, 0.0, 420.0 * s, (1.0, 0.6, 0.22), 0.16 + 0.05 * flick, 0.0);
        cr.restore();
        for (k, (dx, w, h, col)) in [
            (-34.0, 22.0, 62.0, (0.95, 0.32, 0.08)),
            (32.0, 24.0, 70.0, (0.95, 0.32, 0.08)),
            (0.0, 38.0, 112.0, (1.0, 0.45, 0.10)),
            (-14.0, 22.0, 74.0, (1.0, 0.72, 0.18)),
            (16.0, 18.0, 58.0, (1.0, 0.72, 0.18)),
            (0.0, 13.0, 44.0, (1.0, 0.95, 0.60)),
        ]
        .into_iter()
        .enumerate()
        {
            let kf = k as f64;
            let hh = h * (0.82 + 0.18 * (t * (6.0 + kf) + kf * 1.7).sin() + 0.10 * (t * 13.7 + kf).sin());
            let lean = (t * (4.0 + kf * 0.7) + kf * 2.1).sin() * w * 0.45;
            flame(cr, fx + dx * s, fy, w * s, hh * s, lean * s);
            cr.set_rgba(col, 0.92);
            cr.fill();
        }
        // sparks
        for k in 0..5 {
            let kf = k as f64;
            let f = (t * (0.35 + 0.06 * kf) + kf * 0.37) % 1.0;
            let x = fx + ((kf * 2.4).sin() * 40.0 + (t * 2.0 + kf).sin() * 8.0) * s;
            cr.set_source_rgba(1.0, 0.8, 0.35, (1.0 - f) * 0.9);
            cr.circle(x, fy - (50.0 + f * 100.0) * s, 2.0 * s);
            cr.fill();
        }
    }
}

impl Theme for Workshop {
    type Actor = Elf;
    const NAME: &'static str = "workshop";
    const LABEL_BG: Rgb = (0.10, 0.05, 0.03);
    const EMPTY_COLOR: Rgb = (1.0, 0.93, 0.80);
    const WAITING_ROW: f64 = 0.76;

    fn new(_env: &Env) -> Self {
        Workshop {
            bg: None,
            frame: None,
            geo: Geo::default(),
            lights: Vec::new(),
            snow: Vec::new(),
            tosses: Vec::new(),
            pile: Vec::new(),
            full_t: 0.0,
        }
    }

    fn spawn_point(&self, env: &Env) -> (f64, f64) {
        (env.rng.uniform(env.w * 0.2, env.w * 0.8), env.rng.uniform(env.ground_top + 40.0, env.h * 0.85))
    }

    fn on_tool(&mut self, env: &mut Env, actor: &mut Elf, name: &str, index: usize) {
        let s = env.s;
        actor.toss = 1.0;
        actor.b.hop = 1.0;
        let b = &actor.b;
        // a small present flies in an arc to the next free place on the pile
        let slot = pile_slot(&self.geo, s, self.pile.len() + self.tosses.len());
        let (x0, y0) = (b.x + b.face * b.radius * 0.4, b.y - b.radius * 1.0);
        let (tx, ty) = (slot.0, slot.1 - slot.3 / 2.0);
        let life = 0.9 + hypot(tx - x0, ty - y0) / (1600.0 * s) * 0.5 + index as f64 * 0.12;
        self.tosses.push(Toss {
            x: x0,
            y: y0,
            vx: (tx - x0) / life,
            vy: (ty - y0) / life - 0.5 * 900.0 * s * life,
            rot: 0.0,
            vr: env.rng.uniform(3.0, 6.0) * if tx > x0 { 1.0 } else { -1.0 },
            age: 0.0,
            life,
            size: slot.2,
            wrap: env.rng.index(WRAPS.len()),
        });
        env.spawn_floater(b.x, b.y - b.radius * 1.4 - index as f64 * 26.0 * s, name, b.color, FloatStyle::Text, 40.0, 3.5);
    }

    fn setup_world(&mut self, env: &mut Env) {
        let (w, h, s) = (env.w, env.h, env.s);
        env.ground_top = h * 0.61;
        self.geo = Self::layout(env);
        let geo = self.geo;
        let env: &Env = env;
        let rng = &env.rng;
        self.bg = Some(Picture::record(w, h, |cr| self.paint_room(cr, env)));
        self.frame = Some(Picture::record(w, h, |cr| paint_window_frame(cr, s, &geo)));
        // fairy lights: on the tree along the tinsel, and along the garland
        self.lights.clear();
        let (tx, tip) = Self::tree_at(env, &geo);
        for k in 0..5 {
            let (_, t1, hw) = tree_tier(k);
            let n = 3 + k;
            for j in 0..n {
                let u = (j as f64 + 0.5) / n as f64 * 2.0 - 1.0;
                let y = tip + (t1 - 44.0 + 22.0 * (1.0 - u * u) + 9.0) * s;
                self.lights.push((tx + u * hw * 0.78 * s, y, rng.index(BULBS.len()), rng.uniform(0.0, TAU)));
            }
        }
        let n = (w / (230.0 * s)).round().max(2.0) as usize;
        let span = w / n as f64;
        for i in 0..n {
            for j in 0..5 {
                let f = 0.1 + j as f64 * 0.2;
                let x = (i as f64 + f) * span;
                self.lights.push((x, swag_y(f, s) + 12.0 * s, (i * 5 + j) % BULBS.len(), rng.uniform(0.0, TAU)));
            }
        }
        self.snow = (0..36).map(|_| (rng.random(), rng.random(), rng.uniform(0.05, 0.13), rng.uniform(1.4, 3.2))).collect();
        self.tosses.clear();
        if self.pile.is_empty() {
            self.pile = (0..6).map(|_| rng.index(WRAPS.len())).collect();
        }
    }

    fn update_world(&mut self, env: &mut Env, _actors: &mut [Elf], dt: f64) {
        let g = 900.0 * env.s;
        for p in &mut self.tosses {
            p.age += dt;
            p.vy += g * dt;
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            p.rot += p.vr * dt;
        }
        // landed: the present joins the pile
        for p in self.tosses.iter().filter(|p| p.age >= p.life) {
            if self.pile.len() < PILE_MAX {
                self.pile.push(p.wrap);
            }
        }
        self.tosses.retain(|p| p.age < p.life);
        // a full pile is carried off to the sleigh after a moment, and a new one begins
        if self.pile.len() >= PILE_MAX {
            self.full_t += dt;
            if self.full_t > 5.0 {
                self.pile.clear();
                self.full_t = 0.0;
            }
        }
    }

    fn draw_world(&mut self, cr: &mut Canvas, env: &Env, actors: &[Elf], t: f64) {
        let s = env.s;
        if let Some(bg) = &self.bg {
            cr.picture(bg, 0.0, 0.0, 1.0);
        }
        // snow falling outside, behind the window frame
        let (wx, wy, ww, wh) = self.geo.win;
        cr.save();
        cr.rectangle(wx, wy, ww, wh);
        cr.clip();
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.85);
        for &(x, ph, sp, size) in &self.snow {
            let f = (t * sp + ph) % 1.0;
            cr.circle(wx + x * ww + (t * 0.8 + ph * 20.0).sin() * 8.0 * s, wy + f * (wh + 10.0 * s) - 5.0 * s, size * s);
        }
        cr.fill();
        cr.restore();
        if let Some(frame) = &self.frame {
            cr.picture(frame, 0.0, 0.0, 1.0);
        }
        self.draw_fire(cr, s, t);
        // twinkling lights: a soft halo and the bulb
        for &(x, y, col, ph) in &self.lights {
            let tw = 0.55 + 0.45 * (t * 1.7 + ph).sin();
            let c = BULBS[col];
            cr.set_rgba(c, 0.10 + 0.16 * tw);
            cr.circle(x, y, 11.0 * s);
            cr.fill();
            cr.set_rgba(lighten(c, 0.35 * tw), 0.55 + 0.45 * tw);
            cr.circle(x, y, 4.2 * s);
            cr.fill();
        }
        // the growing pile (it fades away when it is carried off)
        let fade = clamp(5.0 - self.full_t, 0.0, 1.0);
        for (i, &wrap) in self.pile.iter().enumerate() {
            let (x, y, pw, ph) = pile_slot(&self.geo, s, i);
            present(cr, x, y, pw, ph, wrap, fade);
        }
        for a in sorted_actors(actors) {
            a.draw(cr, env, self, t);
        }
        for p in &self.tosses {
            cr.save();
            cr.translate(p.x, p.y);
            cr.rotate(p.rot * (1.0 - p.age / p.life).max(0.0).sqrt());
            present(cr, 0.0, p.size * 0.45, p.size, p.size * 0.9, p.wrap, 1.0);
            cr.restore();
        }
    }
}
