//! Guppies theme: a planted freshwater tank.
//!
//! A working session becomes a male guppy - slim, colourful, with a big flowing tail fin. Idle sessions are
//! females - larger, plainer, small tail. A fish changes smoothly between the two when its state changes.
//! working = darts around, each tool call is a bubble with the tool name
//! idle    = cruises slowly       waiting = swims to the front glass, grows, pulses
use vivarium_core::prelude::*;

const FEMALE: Rgb = (0.66, 0.68, 0.62);
const TAIL_COLORS: [Rgb; 5] =
    [(1.0, 0.45, 0.20), (0.95, 0.25, 0.55), (0.30, 0.55, 1.0), (1.0, 0.80, 0.25), (0.55, 0.35, 1.0)];

fn speed_of(st: State) -> f64 {
    match st {
        State::Working => 110.0,
        State::Idle => 32.0,
        State::Waiting => 60.0,
        State::Blocked => 8.0,
        State::Rest => 10.0,
    }
}

fn turn_of(st: State) -> f64 {
    match st {
        State::Working => 1.6,
        State::Idle => 0.6,
        State::Waiting => 0.5,
        State::Blocked => 0.2,
        State::Rest => 0.3,
    }
}

#[derive(Clone)]
pub struct Guppy {
    b: Base,
    tail_phase: f64,
    male: f64,
    tail_color: Rgb,
    /// (x, y, radius, colour) of the body spots of a male
    spots: Vec<(f64, f64, f64, Rgb)>,
}

struct Plant {
    x: f64,
    h: f64,
    w: f64,
    ph: f64,
    layer: u8,
    c: Rgb,
}

/// Floating particle: x, y, depth (0.2..1), phase.
struct Mote {
    x: f64,
    y: f64,
    z: f64,
    ph: f64,
}

pub struct Guppies {
    bg: Option<Picture>,
    plants: Vec<Plant>,
    motes: Vec<Mote>,
    next_bubble: f64,
    surface: f64,
    gravel_top: f64,
}

impl ActorT<Guppies> for Guppy {
    const BASE_RADIUS: f64 = 48.0;

    fn new(base: Base, env: &Env, _world: &mut Guppies) -> Self {
        let rng = &env.rng;
        let tail_phase = rng.uniform(0.0, TAU);
        let male = if base.state() == State::Working { 1.0 } else { 0.0 };
        let tail_color = TAIL_COLORS[base.seed as usize % TAIL_COLORS.len()];
        let spots = (0..5)
            .map(|_| {
                (
                    rng.uniform(-0.5, 0.4),
                    rng.uniform(-0.15, 0.15),
                    rng.uniform(0.06, 0.12),
                    *rng.choice(&[(0.05, 0.05, 0.08), (1.0, 0.55, 0.2), (0.3, 0.6, 1.0)]),
                )
            })
            .collect();
        Guppy { b: base, tail_phase, male, tail_color, spots }
    }

    fn base(&self) -> &Base {
        &self.b
    }

    fn base_mut(&mut self) -> &mut Base {
        &mut self.b
    }

    fn companion_reset(&mut self) {
        self.male = 0.0;
    }

    fn mv(&mut self, env: &mut Env, world: &mut Guppies, others: &[Other], dt: f64, t: f64, target: Option<(f64, f64)>) {
        let (st, s) = (self.b.state(), env.s);
        self.male = lerp(self.male, if st == State::Working { 1.0 } else { 0.0 }, (dt * 0.8).min(1.0));
        let speed = speed_of(st) * s;
        let r = self.b.radius;
        self.b.wander(
            env,
            others,
            dt,
            t,
            target,
            speed,
            turn_of(st),
            (90.0 * s + r, world.surface + 50.0 * s + r, env.w - 90.0 * s - r, world.gravel_top - r * 1.2),
            2.4,
        );
        self.b.vy *= 0.97;
        if self.b.vx.abs() > 8.0 * s {
            self.b.face = lerp(self.b.face, if self.b.vx > 0.0 { 1.0 } else { -1.0 }, (dt * 5.0).min(1.0));
        }
        let sp = hypot(self.b.vx, self.b.vy);
        self.tail_phase += dt * (4.0 + sp / (6.0 * s));
    }

    fn draw(&self, cr: &mut Canvas, env: &Env, _world: &Guppies, t: f64) {
        let s = env.s;
        let b = &self.b;
        let a = b.alpha * b.flicker;
        if a < 0.01 {
            return;
        }
        let (r, c, st, m) = (b.radius, b.color, b.state(), self.male);
        if st == State::Waiting {
            cr.pulse_rings(b.x, b.y, r, c, a, t, s);
        }
        cr.glow(b.x, b.y, r * 2.2, c, if st != State::Idle { 0.30 } else { 0.16 } * a, 0.0);

        cr.save();
        cr.translate(b.x, b.y);
        cr.rotate(clamp(b.vy / (300.0 * s), -0.4, 0.4) * if b.face > 0.0 { 1.0 } else { -1.0 });
        cr.scale(b.face.abs().max(0.35).copysign(b.face), 1.0); // never a thin sliver while turning
        let body_h = r * lerp(0.48, 0.34, m); // females are rounder
        let body_l = r * lerp(1.0, 0.9, m);
        let wag = self.tail_phase.sin();
        // tail fin: small fan (female) -> big flowing veil (male)
        let tl = r * lerp(0.45, 1.25, m);
        let spread = r * lerp(0.22, 0.75, m);
        let bx = -body_l * 0.82;
        let tail_col = lerp3(lerp3(FEMALE, c, 0.4), lerp3(self.tail_color, c, 0.35), m);
        cr.move_to(bx + r * 0.05, -r * 0.06);
        cr.curve_to(
            bx - tl * 0.4,
            -spread * 0.5 + wag * r * 0.08,
            bx - tl,
            -spread + wag * r * 0.2,
            bx - tl * 1.05,
            wag * r * 0.25,
        );
        cr.curve_to(bx - tl, spread + wag * r * 0.2, bx - tl * 0.4, spread * 0.5 + wag * r * 0.08, bx + r * 0.05, r * 0.06);
        cr.close_path();
        let mut tg = Gradient::linear(bx, 0.0, bx - tl, 0.0);
        tg.stop(0.0, tail_col, 0.95 * a);
        tg.stop(1.0, tail_col, 0.55 * a);
        cr.set_source(&tg);
        cr.fill();
        if m > 0.3 {
            // fin rays and the dark spot pattern of a fancy male tail
            cr.set_line_width(1.0 * s);
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.25 * m * a);
            for k in -3..=3 {
                cr.move_to(bx, 0.0);
                cr.line_to(bx - tl * 0.95, k as f64 / 3.0 * spread * 0.9 + wag * r * 0.2);
            }
            cr.stroke();
            cr.set_source_rgba(0.05, 0.05, 0.1, 0.35 * m * a);
            for k in 0..4 {
                let k = k as f64;
                cr.circle(bx - tl * (0.45 + 0.12 * k), (k * 2.1 + b.seed).sin() * spread * 0.45 + wag * r * 0.1, r * 0.07);
                cr.fill();
            }
        }
        // dorsal fin (long on males)
        let dl = r * lerp(0.25, 0.6, m);
        cr.move_to(-body_l * 0.1, -body_h * 0.85);
        cr.curve_to(
            -body_l * 0.3,
            -body_h * 1.6 - m * r * 0.2,
            -body_l * 0.3 - dl,
            -body_h * 1.4,
            -body_l * 0.55 - dl * 0.4,
            -body_h * 0.55,
        );
        cr.close_path();
        cr.set_rgba(tail_col, 0.7 * a);
        cr.fill();
        // body
        cr.move_to(body_l, 0.0);
        cr.curve_to(body_l * 0.9, -body_h * 1.05, -body_l * 0.2, -body_h * 1.15, bx, -r * 0.08);
        cr.line_to(bx, r * 0.08);
        cr.curve_to(-body_l * 0.2, body_h * (1.25 - 0.25 * m), body_l * 0.85, body_h * 1.1, body_l, 0.0);
        cr.close_path();
        let body_col = lerp3(lerp3(FEMALE, c, 0.25), lighten(c, 0.15), m);
        let mut bg = Gradient::linear(0.0, -body_h, 0.0, body_h);
        let hi = lighten(body_col, 0.45);
        bg.add_color_stop_rgba(0.0, body_col.0 * 0.75, body_col.1 * 0.75, body_col.2 * 0.75, a);
        bg.stop(0.55, hi, a);
        bg.add_color_stop_rgba(1.0, 0.92, 0.92, 0.88, a);
        cr.set_source(&bg);
        cr.fill();
        if m > 0.2 {
            // colourful male spots
            for &(sx, sy, sr, sc) in &self.spots {
                cr.set_rgba(sc, 0.75 * m * a);
                cr.circle(sx * body_l, sy * body_h * 2.0, sr * r);
                cr.fill();
            }
        } else {
            // female "gravid spot"
            cr.set_source_rgba(0.15, 0.12, 0.1, 0.35 * (1.0 - m) * a);
            cr.circle(-body_l * 0.15, body_h * 0.45, r * 0.12);
            cr.fill();
        }
        // pectoral fin, eye
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.25 * a);
        cr.save();
        cr.translate(body_l * 0.35, body_h * 0.2);
        cr.rotate(0.6 + (t * 6.0 + b.seed).sin() * 0.3);
        cr.ellipse(0.0, 0.0, r * 0.18, r * 0.07);
        cr.restore();
        cr.fill();
        let ex = body_l * 0.68;
        cr.set_source_rgba(1.0, 1.0, 1.0, a);
        cr.circle(ex, -body_h * 0.18, r * 0.11);
        cr.fill();
        cr.set_source_rgba(0.0, 0.0, 0.0, a);
        cr.circle(ex + r * 0.02, -body_h * 0.18, r * 0.06);
        cr.fill();
        cr.restore();
    }
}

impl Guppies {
    fn draw_plants(&self, cr: &mut Canvas, env: &Env, t: f64, layer: u8) {
        let s = env.s;
        cr.set_line_cap(LineCap::Round);
        for p in &self.plants {
            if p.layer != layer {
                continue;
            }
            let base = self.gravel_top + 6.0 * s;
            cr.move_to(p.x, base);
            for k in 1..9 {
                let f = k as f64 / 8.0;
                let xx = p.x + (t * 0.6 + p.ph + f * 2.5).sin() * f * 26.0 * s;
                cr.line_to(xx, base - f * p.h);
            }
            cr.set_rgba(p.c, 0.85);
            cr.set_line_width(p.w);
            cr.stroke();
        }
    }
}

impl Theme for Guppies {
    type Actor = Guppy;
    const NAME: &'static str = "guppies";
    const LABEL_BG: Rgb = (0.02, 0.06, 0.05);
    const EMPTY_COLOR: Rgb = (0.8, 1.0, 0.9);
    const WAITING_ROW: f64 = 0.55;

    fn new(_env: &Env) -> Self {
        Guppies { bg: None, plants: Vec::new(), motes: Vec::new(), next_bubble: 0.0, surface: 0.0, gravel_top: 0.0 }
    }

    fn on_tool(&mut self, env: &mut Env, actor: &mut Guppy, name: &str, index: usize) {
        let b = &actor.b;
        let dx = b.radius * if b.face > 0.0 { 1.0 } else { -1.0 };
        env.spawn_floater(
            b.x + dx,
            b.y - b.radius * 0.4 - index as f64 * 26.0 * env.s,
            name,
            b.color,
            FloatStyle::Bubble,
            60.0,
            6.0,
        );
    }

    fn setup_world(&mut self, env: &mut Env) {
        let (w, h, s) = (env.w, env.h, env.s);
        let rng = &env.rng;
        self.surface = h * 0.06;
        self.gravel_top = h - env.strip_h - 90.0 * s;
        let (surface, gt) = (self.surface, self.gravel_top);
        self.bg = Some(Picture::record(w, h, |cr| {
            let mut g = Gradient::linear(0.0, 0.0, 0.0, h);
            g.add_color_stop_rgb(0.0, 0.14, 0.36, 0.36);
            g.add_color_stop_rgb(0.6, 0.06, 0.20, 0.20);
            g.add_color_stop_rgb(1.0, 0.03, 0.09, 0.08);
            cr.set_source(&g);
            cr.rectangle(0.0, 0.0, w, h);
            cr.fill();
            cr.set_source_rgba(0.75, 0.95, 0.95, 0.25); // water surface line
            cr.rectangle(0.0, surface - 2.0 * s, w, 3.0 * s);
            cr.fill();
            // driftwood
            cr.set_source_rgb(0.12, 0.08, 0.05);
            cr.set_line_cap(LineCap::Round);
            for _ in 0..2 {
                let x0 = rng.uniform(0.1, 0.8) * w;
                cr.set_line_width(rng.uniform(18.0, 30.0) * s);
                cr.move_to(x0, gt + 20.0 * s);
                cr.curve_to(x0 + 80.0 * s, gt - 120.0 * s, x0 + 220.0 * s, gt - 160.0 * s, x0 + 320.0 * s, gt - 260.0 * s);
                cr.stroke();
            }
            // gravel
            cr.rectangle(0.0, gt, w, h - gt);
            let mut gg = Gradient::linear(0.0, gt, 0.0, h);
            gg.add_color_stop_rgb(0.0, 0.30, 0.27, 0.22);
            gg.add_color_stop_rgb(1.0, 0.12, 0.10, 0.08);
            cr.set_source(&gg);
            cr.fill();
            for _ in 0..(1400.0 * w / 1920.0) as usize {
                let (px, py) = (rng.uniform(0.0, w), rng.uniform(gt, h));
                let col = *rng.choice(&[(0.55, 0.50, 0.42), (0.35, 0.30, 0.25), (0.65, 0.60, 0.52), (0.45, 0.35, 0.28)]);
                cr.set_rgb(col);
                let (rx, ry) = (rng.uniform(3.0, 7.0) * s, rng.uniform(2.0, 4.0) * s);
                cr.ellipse(px, py, rx, ry);
                cr.fill();
            }
        }));
        // plants: vallisneria ribbons (back layer darker, front layer brighter)
        self.plants.clear();
        for layer in [0u8, 1] {
            for _ in 0..(if layer == 0 { 40.0 } else { 18.0 } * w / 1920.0) as usize {
                let x = rng.uniform(0.0, w);
                if layer == 1 && 0.25 * w < x && x < 0.75 * w {
                    continue; // keep the middle open in front
                }
                self.plants.push(Plant {
                    x,
                    h: rng.uniform(0.35, if layer == 0 { 0.75 } else { 0.55 }) * (gt - surface),
                    w: rng.uniform(4.0, 8.0) * s,
                    ph: rng.uniform(0.0, TAU),
                    layer,
                    c: (
                        rng.uniform(0.15, 0.3),
                        rng.uniform(0.45, 0.65) * if layer == 0 { 0.65 } else { 1.0 },
                        rng.uniform(0.15, 0.25),
                    ),
                });
            }
        }
        self.motes = (0..(90.0 * w / 1920.0) as usize)
            .map(|_| Mote { x: rng.uniform(0.0, w), y: rng.uniform(surface, gt), z: rng.uniform(0.2, 1.0), ph: rng.uniform(0.0, TAU) })
            .collect();
    }

    fn update_world(&mut self, env: &mut Env, _actors: &mut [Guppy], dt: f64) {
        let s = env.s;
        for p in &mut self.motes {
            p.x += (env.t * 0.2 + p.ph).sin() * 4.0 * s * dt;
            p.y += p.z * 3.0 * s * dt;
            if p.y > self.gravel_top {
                p.y = self.surface;
            }
        }
        if env.t > self.next_bubble {
            // air stone on the left
            self.next_bubble = env.t + env.rng.uniform(0.18, 0.4);
            let x = env.w * 0.06 + env.rng.uniform(-6.0, 6.0) * s;
            let vy = env.rng.uniform(90.0, 140.0);
            env.spawn_floater(x, self.gravel_top, "", (1.0, 1.0, 1.0), FloatStyle::Bubble, vy, 12.0);
        }
    }

    fn draw_world(&mut self, cr: &mut Canvas, env: &Env, actors: &[Guppy], t: f64) {
        let s = env.s;
        if let Some(bg) = &self.bg {
            cr.picture(bg, 0.0, 0.0, 1.0);
        }
        for k in 0..6 {
            // light shimmer from the surface
            let k = k as f64;
            let x0 = env.w * (k + 0.5) / 6.0 + (t * 0.3 + k).sin() * 60.0 * s;
            let mut gr = Gradient::linear(0.0, self.surface, 0.0, self.gravel_top);
            gr.add_color_stop_rgba(0.0, 0.8, 1.0, 0.9, 0.07);
            gr.add_color_stop_rgba(1.0, 0.8, 1.0, 0.9, 0.0);
            cr.move_to(x0 - 40.0 * s, self.surface);
            cr.line_to(x0 + 40.0 * s, self.surface);
            cr.line_to(x0 + 110.0 * s, self.gravel_top);
            cr.line_to(x0 - 10.0 * s, self.gravel_top);
            cr.close_path();
            cr.set_source(&gr);
            cr.fill();
        }
        self.draw_plants(cr, env, t, 0);
        for p in &self.motes {
            cr.set_source_rgba(0.85, 1.0, 0.9, 0.08 + 0.2 * p.z);
            cr.circle(p.x, p.y, (0.6 + p.z) * s);
            cr.fill();
        }
        for a in sorted_actors(actors) {
            a.draw(cr, env, self, t);
        }
        self.draw_plants(cr, env, t, 1);
    }
}
