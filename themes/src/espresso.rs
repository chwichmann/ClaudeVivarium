//! Espresso theme: winged espresso pots (moka pots) glide diagonally across a starry night sky,
//! coffee beans tumble by. An original homage to the flying-objects screensavers of the 90s.
//!
//! working = flaps hard and flies fast; every tool call puffs a steam cloud with the tool name
//! idle    = glides calmly           waiting = leaves the formation, hovers in the middle, pulses
//! rest    = one pot with folded wings drifts by asleep
use vivarium_core::prelude::*;

const DIR: (f64, f64) = (-1.0, 0.55); // everybody flies to the lower left, like a flock
const METAL: Rgb = (0.78, 0.79, 0.82);

fn speed_of(st: State) -> f64 {
    match st {
        State::Working => 125.0,
        State::Idle => 48.0,
        State::Waiting => 60.0,
        State::Blocked => 15.0,
        State::Rest => 22.0,
    }
}

fn flap_of(st: State) -> f64 {
    match st {
        State::Working => 11.0,
        State::Idle => 4.0,
        State::Waiting => 6.0,
        State::Blocked => 0.5,
        State::Rest => 0.0,
    }
}

#[derive(Clone)]
pub struct MokaPot {
    b: Base,
    flap: f64,
    /// small personal offset so they do not fly in lockstep
    lane: f64,
    puff_acc: f64,
}

struct Bean {
    x: f64,
    y: f64,
    /// depth: far beans are small, dark and slow
    z: f64,
    rot: f64,
    spin: f64,
}

struct Puff {
    x: f64,
    y: f64,
    r: f64,
    age: f64,
    life: f64,
    vx: f64,
    vy: f64,
}

pub struct Espresso {
    bg: Option<Picture>,
    beans: Vec<Bean>,
    /// x, y, z (brightness/size), phase
    stars: Vec<[f64; 4]>,
    puffs: Vec<Puff>,
}

impl ActorT<Espresso> for MokaPot {
    const BASE_RADIUS: f64 = 62.0;

    fn new(base: Base, env: &Env, _world: &mut Espresso) -> Self {
        let flap = env.rng.uniform(0.0, TAU);
        let lane = env.rng.uniform(-1.0, 1.0);
        MokaPot { b: base, flap, lane, puff_acc: 0.0 }
    }

    fn base(&self) -> &Base {
        &self.b
    }

    fn base_mut(&mut self) -> &mut Base {
        &mut self.b
    }

    fn mv(&mut self, env: &mut Env, world: &mut Espresso, others: &[Other], dt: f64, t: f64, target: Option<(f64, f64)>) {
        let (st, s) = (self.b.state(), env.s);
        let b = &mut self.b;
        self.flap += dt * flap_of(st);
        if let Some((mut tx, mut ty)) = target {
            // hover in the middle
            tx += (t * 0.7 + b.seed).sin() * 30.0 * s;
            ty += (t * 1.3 + b.seed).sin() * 15.0 * s;
            let k = (dt * 1.2).min(1.0);
            b.vx = lerp(b.vx, (tx - b.x) * 1.5, k);
            b.vy = lerp(b.vy, (ty - b.y) * 1.5, k);
        } else {
            let sp = speed_of(st) * s * (1.0 + 0.08 * self.lane);
            let k = (dt * 0.8).min(1.0);
            b.vx = lerp(b.vx, DIR.0 * sp, k);
            b.vy = lerp(b.vy, DIR.1 * sp + (t * 0.4 + b.seed).sin() * 12.0 * s, k);
            for o in others {
                // keep apart, sideways to the flight direction
                if o.index != b.index && !o.dying {
                    let (dx, dy) = (b.x - o.x, b.y - o.y);
                    let d = hypot(dx, dy);
                    let md = (b.radius + o.radius) * 2.4;
                    if 0.01 < d && d < md {
                        b.vx += dx / d * (md - d) * 1.5 * dt * 10.0;
                        b.vy += dy / d * (md - d) * 1.5 * dt * 10.0;
                    }
                }
            }
        }
        let bob = self.flap.sin() * if flap_of(st) != 0.0 { 6.0 * s } else { 0.0 };
        b.x += b.vx * dt;
        b.y += (b.vy + bob) * dt;
        // wrap around the edges (only while flying with the flock)
        let m = b.radius * 3.0;
        let bottom = env.h - env.strip_h;
        if target.is_none() && (b.x < -m || b.y > bottom + m) {
            if env.rng.random() < 0.55 {
                // come back on the right edge ...
                b.x = env.w + m * 0.9;
                b.y = env.rng.uniform(-m, bottom * 0.7);
            } else {
                // ... or on the top edge
                b.x = env.rng.uniform(env.w * 0.3, env.w + m);
                b.y = -m * 0.9;
            }
        }
        if st == State::Working && !b.dying {
            self.puff_acc += dt;
            if self.puff_acc > 0.25 {
                self.puff_acc = 0.0;
                world.puff(env, b.x - b.radius * 0.95, b.y - b.radius * 0.25, true);
            }
        }
    }

    fn draw(&self, cr: &mut Canvas, env: &Env, _world: &Espresso, t: f64) {
        let s = env.s;
        let b = &self.b;
        let a = b.alpha * b.flicker;
        if a < 0.01 {
            return;
        }
        let (r, c, st) = (b.radius, b.color, b.state());
        if st == State::Waiting {
            cr.pulse_rings(b.x, b.y, r, c, a, t, s);
        }
        let calm = st == State::Idle || st == State::Rest;
        cr.glow(b.x, b.y, r * 2.3, c, if calm { 0.16 } else { 0.32 } * a, 0.0);
        cr.save();
        cr.translate(b.x, b.y);
        cr.rotate(clamp(b.vy / (900.0 * s), -0.15, 0.15));
        let folded = st == State::Rest;
        let flap = self.flap.sin();
        wing(cr, r, flap, a, true, folded);
        pot(cr, r, c, a);
        wing(cr, r, flap, a, false, folded);
        cr.restore();
        if folded {
            // z z z
            for k in 0..3 {
                let f = (t * 0.3 + k as f64 / 3.0).rem_euclid(1.0);
                let lay = text::layout("z", (14.0 + 12.0 * f) * s, weight::BOLD, None);
                text::show(
                    cr,
                    &lay,
                    b.x + r * (0.6 + 0.5 * f),
                    b.y - r * (1.0 + 1.2 * f),
                    (1.0, 1.0, 1.0),
                    0.75 * (PI * f).sin() * a,
                    0.0,
                );
            }
        }
    }
}

/// Side view: spout left, handle right; two eight-sided chambers with a waist.
fn pot(cr: &mut Canvas, r: f64, c: Rgb, a: f64) {
    let metal = lerp3(METAL, c, 0.38);
    let (hi, lo) = (lighten(metal, 0.5), (metal.0 * 0.45, metal.1 * 0.45, metal.2 * 0.5));

    let chamber = |cr: &mut Canvas, y_top: f64, y_bot: f64, w_top: f64, w_bot: f64| {
        cr.move_to(-w_top, y_top);
        cr.line_to(w_top, y_top);
        cr.line_to(w_bot, y_bot);
        cr.line_to(-w_bot, y_bot);
        cr.close_path();
        let mut g = Gradient::linear(-w_bot, 0.0, w_bot, 0.0);
        g.stop(0.0, lo, a);
        g.stop(0.35, hi, a);
        g.stop(0.6, metal, a);
        g.stop(1.0, lo, a);
        cr.set_source(&g);
        cr.fill();
    };
    // handle (black, right side)
    cr.set_source_rgba(0.08, 0.08, 0.09, a);
    cr.set_line_width(r * 0.13);
    cr.set_line_cap(LineCap::Round);
    cr.move_to(r * 0.38, -r * 0.62);
    cr.curve_to(r * 0.95, -r * 0.62, r * 0.95, -r * 0.05, r * 0.42, -r * 0.02);
    cr.stroke();
    // spout (left)
    cr.move_to(-r * 0.36, -r * 0.62);
    cr.line_to(-r * 0.78, -r * 0.78);
    cr.line_to(-r * 0.72, -r * 0.55);
    cr.line_to(-r * 0.4, -r * 0.42);
    cr.close_path();
    cr.set_rgba(lo, a);
    cr.fill();
    chamber(cr, r * 0.08, r * 0.78, r * 0.38, r * 0.52); // lower (water) chamber
    chamber(cr, -r * 0.72, r * 0.02, r * 0.42, r * 0.36); // upper (coffee) chamber
    cr.set_source_rgba(lo.0 * 0.7, lo.1 * 0.7, lo.2 * 0.7, a); // waist
    cr.rectangle(-r * 0.4, 0.0, r * 0.8, r * 0.1);
    cr.fill();
    cr.set_rgba(lo, a); // lid + knob
    cr.save();
    cr.translate(0.0, -r * 0.74);
    cr.scale(r * 0.44, r * 0.08);
    cr.new_sub_path();
    cr.arc(0.0, 0.0, 1.0, PI, TAU);
    cr.restore();
    cr.fill();
    cr.set_source_rgba(0.08, 0.08, 0.09, a);
    cr.circle(0.0, -r * 0.86, r * 0.08);
    cr.fill();
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.35 * a); // metal shine
    cr.set_line_width(r * 0.05);
    cr.move_to(-r * 0.18, -r * 0.62);
    cr.line_to(-r * 0.15, -r * 0.08);
    cr.move_to(-r * 0.2, r * 0.2);
    cr.line_to(-r * 0.24, r * 0.66);
    cr.stroke();
}

/// A feathered wing hinged at the top of the pot; the back wing is darker and mirrored in phase.
fn wing(cr: &mut Canvas, r: f64, flap: f64, a: f64, back: bool, folded: bool) {
    cr.save();
    cr.translate(r * 0.05, -r * 0.45);
    let ang = if folded {
        if back { 0.25 } else { 0.35 }
    } else {
        -0.15 + flap * 0.95 * if back { 0.85 } else { 1.0 }
    };
    cr.rotate(-ang - if back { 0.2 } else { 0.0 });
    let shade = if back { 0.7 } else { 1.0 };
    for k in 0..4 {
        // four feathers fanning out
        let fa = -0.25 + k as f64 * 0.17;
        let ln = r * (1.25 - k as f64 * 0.16) * if folded { 0.75 } else { 1.0 };
        cr.save();
        cr.rotate(fa);
        cr.move_to(0.0, 0.0);
        cr.curve_to(ln * 0.3, -r * 0.28, ln * 0.8, -r * 0.3, ln, -r * 0.1);
        cr.curve_to(ln * 0.75, r * 0.05, ln * 0.3, r * 0.06, 0.0, 0.0);
        cr.restore();
        cr.set_source_rgba(0.95 * shade, 0.95 * shade, 0.98 * shade, a);
        cr.fill_preserve();
        cr.set_source_rgba(0.55 * shade, 0.58 * shade, 0.65 * shade, 0.8 * a);
        cr.set_line_width((r * 0.025).max(1.0));
        cr.stroke();
    }
    cr.restore();
}

impl Espresso {
    fn puff(&mut self, env: &Env, x: f64, y: f64, small: bool) {
        let s = env.s;
        self.puffs.push(Puff {
            x,
            y,
            r: if small { 6.0 } else { 12.0 } * s,
            age: 0.0,
            life: if small { 1.2 } else { 2.5 },
            vx: -20.0 * s,
            vy: -25.0 * s,
        });
    }

    fn bean(&self, cr: &mut Canvas, env: &Env, b: &Bean) {
        let s = env.s;
        let z = b.z;
        let l = (10.0 + 16.0 * z) * s;
        cr.save();
        cr.translate(b.x, b.y);
        cr.rotate(b.rot);
        cr.ellipse(0.0, 0.0, l, l * 0.68);
        let k = 0.35 + 0.65 * z;
        let mut g = Gradient::radial(-l * 0.3, -l * 0.25, l * 0.1, 0.0, 0.0, l);
        g.add_color_stop_rgb(0.0, 0.55 * k, 0.33 * k, 0.18 * k);
        g.add_color_stop_rgb(1.0, 0.25 * k, 0.13 * k, 0.06 * k);
        cr.set_source(&g);
        cr.fill();
        cr.set_source_rgb(0.12 * k, 0.06 * k, 0.03 * k); // the crease
        cr.set_line_width((l * 0.12).max(1.0));
        cr.move_to(-l * 0.75, 0.0);
        cr.curve_to(-l * 0.3, -l * 0.2, l * 0.3, l * 0.2, l * 0.75, 0.0);
        cr.stroke();
        cr.restore();
    }
}

impl Theme for Espresso {
    type Actor = MokaPot;
    const NAME: &'static str = "espresso";
    const LABEL_BG: Rgb = (0.02, 0.02, 0.06);
    const EMPTY_COLOR: Rgb = (0.95, 0.85, 0.7);
    const WAITING_ROW: f64 = 0.45;

    fn new(_env: &Env) -> Self {
        Espresso { bg: None, beans: Vec::new(), stars: Vec::new(), puffs: Vec::new() }
    }

    fn spawn_point(&self, env: &Env) -> (f64, f64) {
        (env.rng.uniform(env.w * 0.15, env.w * 0.95), env.rng.uniform(env.h * 0.05, env.h * 0.6))
    }

    fn on_tool(&mut self, env: &mut Env, actor: &mut MokaPot, name: &str, index: usize) {
        let b = &actor.b;
        let (x, y) = (b.x - b.radius * 1.0, b.y - b.radius * (0.9 + 0.35 * index as f64));
        self.puff(env, x, y, false);
        env.spawn_floater(x - 10.0 * env.s, y - 14.0 * env.s, name, b.color, FloatStyle::Text, 30.0, 3.5);
    }

    fn setup_world(&mut self, env: &mut Env) {
        let (w, h, s) = (env.w, env.h, env.s);
        let rng = &env.rng;
        self.bg = Some(Picture::record(w, h, |cr| {
            let mut g = Gradient::linear(0.0, 0.0, w * 0.3, h);
            g.add_color_stop_rgb(0.0, 0.04, 0.03, 0.10);
            g.add_color_stop_rgb(1.0, 0.005, 0.005, 0.02);
            cr.set_source(&g);
            cr.paint();
            let (mx, my, mr) = (w * 0.82, h * 0.18, 46.0 * s); // crescent moon
            let mut mg = Gradient::radial(mx, my, mr, mx, my, mr * 5.0);
            mg.add_color_stop_rgba(0.0, 0.9, 0.9, 1.0, 0.18);
            mg.add_color_stop_rgba(1.0, 0.9, 0.9, 1.0, 0.0);
            cr.set_source(&mg);
            cr.paint();
            cr.save(); // crescent: the moon disc minus an offset disc, so the glow shows through the cut
            cr.circle(mx, my, mr);
            cr.clip();
            cr.rectangle(mx - mr, my - mr, 2.0 * mr, 2.0 * mr);
            cr.new_sub_path();
            cr.arc(mx + mr * 0.45, my - mr * 0.15, mr * 0.92, 0.0, TAU);
            cr.set_fill_rule(FillRule::EvenOdd);
            cr.set_source_rgb(0.95, 0.93, 0.85);
            cr.fill();
            cr.restore();
        }));
        self.stars = (0..(220.0 * w * h / (1920.0 * 1200.0)) as usize)
            .map(|_| [rng.uniform(0.0, w), rng.uniform(0.0, h), rng.uniform(0.2, 1.0), rng.uniform(0.0, TAU)])
            .collect();
        self.beans = (0..(26.0 * w / 1920.0) as usize)
            .map(|_| {
                let z = rng.uniform(0.3, 1.0); // depth: far beans are small, dark and slow
                Bean { x: rng.uniform(0.0, w), y: rng.uniform(0.0, h), z, rot: rng.uniform(0.0, TAU), spin: rng.uniform(-1.5, 1.5) }
            })
            .collect();
        self.beans.sort_by(|a, b| a.z.total_cmp(&b.z));
    }

    fn update_world(&mut self, env: &mut Env, _actors: &mut [MokaPot], dt: f64) {
        let (s, w, h) = (env.s, env.w, env.h);
        for b in &mut self.beans {
            let sp = 70.0 * s * b.z;
            b.x += DIR.0 * sp * dt;
            b.y += DIR.1 * sp * dt;
            b.rot += b.spin * dt;
            if b.x < -40.0 * s || b.y > h + 40.0 * s {
                if env.rng.random() < 0.55 {
                    b.x = w + 40.0 * s;
                    b.y = env.rng.uniform(-40.0 * s, h * 0.7);
                } else {
                    b.x = env.rng.uniform(w * 0.3, w + 40.0 * s);
                    b.y = -40.0 * s;
                }
            }
        }
        for p in &mut self.puffs {
            p.age += dt;
            p.x += p.vx * dt;
            p.y += p.vy * dt;
        }
        self.puffs.retain(|p| p.age < p.life);
    }

    fn draw_world(&mut self, cr: &mut Canvas, env: &Env, actors: &[MokaPot], t: f64) {
        let s = env.s;
        if let Some(bg) = &self.bg {
            cr.picture(bg, 0.0, 0.0, 1.0);
        }
        for &[x, y, z, ph] in &self.stars {
            let tw = 0.5 + 0.5 * (t * (0.8 + z * 2.0) + ph * 5.0).sin();
            cr.set_source_rgba(0.9, 0.9, 1.0, (0.15 + 0.6 * z) * (0.4 + 0.6 * tw));
            cr.circle(x, y, (0.5 + 1.1 * z) * s);
            cr.fill();
        }
        for b in self.beans.iter().filter(|b| b.z < 0.7) {
            self.bean(cr, env, b); // far beans
        }
        for p in &self.puffs {
            let f = p.age / p.life;
            let r = p.r * (1.0 + 1.5 * f);
            cr.set_source_rgba(0.95, 0.95, 1.0, 0.45 * (1.0 - f));
            for (dx, dy, k) in [(0.0, 0.0, 1.0), (r * 0.7, -r * 0.2, 0.75), (-r * 0.6, -r * 0.3, 0.7)] {
                cr.new_sub_path();
                cr.arc(p.x + dx, p.y + dy, r * k, 0.0, TAU);
            }
            cr.fill();
        }
        for a in sorted_actors(actors) {
            a.draw(cr, env, self, t);
        }
        for b in self.beans.iter().filter(|b| b.z >= 0.7) {
            self.bean(cr, env, b); // near beans
        }
    }
}
