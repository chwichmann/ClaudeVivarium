//! Aquarium theme: every session is a glowing jellyfish in the deep sea.
//!
//! working = swims fast, sparkle trail, tool calls rise as labelled bubbles
//! idle    = drifts slowly          waiting = swims to the centre, grows, sends out rings
use vivarium_core::prelude::*;

/// Mean of max(0, sin): keeps the contraction push from adding a net drift (Python: 0.318).
const MEAN_PUSH: f64 = std::f64::consts::FRAC_1_PI;

fn speed_of(st: State) -> f64 {
    match st {
        State::Working => 80.0,
        State::Idle => 20.0,
        State::Waiting => 45.0,
        State::Blocked => 10.0,
        State::Rest => 9.0,
    }
}

fn turn_of(st: State) -> f64 {
    match st {
        State::Working => 1.3,
        State::Idle => 0.45,
        State::Waiting => 0.6,
        State::Blocked => 0.2,
        State::Rest => 0.25,
    }
}

fn pulse_of(st: State) -> f64 {
    match st {
        State::Working => 5.2,
        State::Idle => 1.5,
        State::Waiting => 3.4,
        State::Blocked => 9.0,
        State::Rest => 0.8,
    }
}

#[derive(Clone)]
pub struct Jelly {
    b: Base,
    spark_acc: f64,
}

struct Bubble {
    x0: f64,
    x: f64,
    y: f64,
    ph: f64,
    age: f64,
    r: f64,
    vy: f64,
    life: f64,
    label: Option<String>,
    c: Rgb,
}

struct Sparkle {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    age: f64,
    life: f64,
    c: Rgb,
}

struct Ray {
    x: f64,
    w: f64,
    ph: f64,
    sp: f64,
    a: f64,
}

struct Weed {
    x: f64,
    h: f64,
    ph: f64,
    lw: f64,
    c: Rgb,
}

pub struct Aquarium {
    bubbles: Vec<Bubble>,
    sparkles: Vec<Sparkle>,
    /// x, y, depth (0.15..1), phase
    plankton: Vec<[f64; 4]>,
    rays: Vec<Ray>,
    weeds: Vec<Weed>,
    bg: Option<Picture>,
    next_ambient: f64,
}

impl ActorT<Aquarium> for Jelly {
    const BASE_RADIUS: f64 = 50.0;

    fn new(base: Base, _env: &Env, _world: &mut Aquarium) -> Self {
        Jelly { b: base, spark_acc: 0.0 }
    }

    fn base(&self) -> &Base {
        &self.b
    }

    fn base_mut(&mut self) -> &mut Base {
        &mut self.b
    }

    fn mv(&mut self, env: &mut Env, world: &mut Aquarium, others: &[Other], dt: f64, t: f64, target: Option<(f64, f64)>) {
        let (st, s) = (self.b.state(), env.s);
        self.b.phase += dt * pulse_of(st);
        let speed = speed_of(st) * s;
        let r = self.b.radius;
        let bbox = (110.0 * s, 140.0 * s, env.w - 110.0 * s, env.h - env.strip_h - r * 2.6);
        self.b.wander(env, others, dt, t, target, speed, turn_of(st), bbox, 2.8);
        self.b.y += -(self.b.phase.sin().max(0.0) - MEAN_PUSH) * speed * 0.4 * dt; // push on each contraction
        if st == State::Working && !self.b.dying {
            self.spark_acc += dt;
            while self.spark_acc > 0.05 {
                self.spark_acc -= 0.05;
                world.sparkles.push(Sparkle {
                    x: self.b.x + env.rng.uniform(-r * 0.7, r * 0.7),
                    y: self.b.y + r * env.rng.uniform(0.3, 1.9),
                    vx: -self.b.vx * 0.15 + env.rng.uniform(-8.0, 8.0) * s,
                    vy: env.rng.uniform(-12.0, 4.0) * s,
                    age: 0.0,
                    life: env.rng.uniform(1.0, 2.2),
                    c: self.b.color,
                });
            }
        }
    }

    fn draw(&self, cr: &mut Canvas, env: &Env, _world: &Aquarium, t: f64) {
        let s = env.s;
        let b = &self.b;
        let a = b.alpha * b.flicker;
        if a < 0.01 {
            return;
        }
        let (x, y, r, c) = (b.x, b.y, b.radius, b.color);
        let st = b.state();
        let p = b.phase.sin();
        let (bw, bh) = (r * (1.0 + 0.10 * p), r * (0.80 - 0.10 * p));

        let glow = match st {
            State::Working => 0.34,
            State::Waiting => 0.45,
            State::Blocked => 0.4,
            _ => 0.22,
        };
        cr.glow(x, y - bh * 0.3, r * 2.8, c, glow * a, 0.0);
        if st == State::Waiting {
            cr.pulse_rings(x, y - bh * 0.3, r, c, a, t, s);
        }

        cr.save();
        cr.translate(x, y);
        cr.rotate(clamp(b.vx / (400.0 * s), -0.35, 0.35));
        let drag = clamp(b.vx / (60.0 * s), -2.5, 2.5);
        cr.set_line_cap(LineCap::Round);
        let n = 9;
        let length = r * (1.9 - 0.25 * p);
        for i in 0..n {
            // tentacles
            let fi = i as f64;
            let fx = (fi / (n - 1) as f64 - 0.5) * bw * 1.5;
            cr.move_to(fx, r * 0.08);
            for k in 1..13 {
                let f = k as f64 / 12.0;
                let xx = fx * (1.0 - 0.25 * f) + (t * 2.2 + fi * 0.9 - f * 4.0 + b.seed).sin() * r * 0.14 * f
                    - drag * r * 0.18 * f * f;
                cr.line_to(xx, r * 0.08 + length * f);
            }
            cr.set_rgba(c, 0.40 * a);
            cr.set_line_width(1.6 * s);
            cr.stroke();
        }
        let lc = lighten(c, 0.3);
        for i in [-1.0, 1.0] {
            // oral arms
            cr.move_to(i * r * 0.15, 0.0);
            for k in 1..11 {
                let f = k as f64 / 10.0;
                let xx = i * r * 0.15 + (t * 1.5 + i + f * 3.0).sin() * r * 0.22 * f - drag * r * 0.12 * f * f;
                cr.line_to(xx, length * 0.75 * f);
            }
            cr.set_rgba(lc, 0.30 * a);
            cr.set_line_width(5.0 * s);
            cr.stroke();
        }

        cr.new_path(); // bell
        cr.save();
        cr.scale(bw, bh);
        cr.arc(0.0, 0.0, 1.0, PI, TAU);
        cr.restore();
        for j in 0..8 {
            let x0 = bw - 2.0 * bw * j as f64 / 8.0;
            let x1 = bw - 2.0 * bw * (j + 1) as f64 / 8.0;
            cr.curve_to(x0 - bw * 0.05, r * 0.16, x1 + bw * 0.05, r * 0.16, x1, 0.0);
        }
        cr.close_path();
        let hi = lighten(c, 0.6);
        let mut lg = Gradient::radial(0.0, -bh * 0.55, 0.0, 0.0, -bh * 0.3, bw * 1.15);
        lg.stop(0.0, hi, 0.85 * a);
        lg.stop(0.55, c, 0.50 * a);
        lg.stop(1.0, c, 0.20 * a);
        cr.set_source(&lg);
        cr.fill_preserve();
        let ec = lighten(c, 0.4);
        cr.set_rgba(ec, 0.8 * a);
        cr.set_line_width(1.5 * s);
        cr.stroke();

        let core = 0.45 + if st == State::Working { 0.35 * (t * 9.0 + b.seed).sin().max(0.0) } else { 0.1 * p };
        cr.set_source_rgba(1.0, 1.0, 1.0, core * a);
        cr.ellipse(0.0, -bh * 0.38, r * 0.30, r * 0.20);
        cr.fill();
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.22 * a);
        cr.set_line_width(3.0 * s);
        cr.new_sub_path();
        cr.arc(0.0, 0.0, bh * 0.75, PI * 1.15, PI * 1.45);
        cr.stroke();
        cr.restore();
    }
}

fn bed_y(env: &Env, x: f64, far: bool) -> f64 {
    let (w, h, s) = (env.w, env.h, env.s);
    if far {
        return h - s * (150.0 + 35.0 * (x / w * 4.0 + 3.0).sin() + 15.0 * (x / w * 11.0 + 1.0).sin());
    }
    h - s * (78.0 + 24.0 * (x / w * 6.0 + 0.7).sin() + 12.0 * (x / w * 17.0 + 2.1).sin())
}

impl Aquarium {
    fn spawn_bubble(&mut self, env: &Env, x: f64, y: f64, label: Option<&str>, color: Rgb) {
        let (s, rng) = (env.s, &env.rng);
        let label = label.filter(|l| !l.is_empty());
        let ph = rng.uniform(0.0, TAU);
        let r = s * if label.is_some() { rng.uniform(9.0, 13.0) } else { rng.uniform(2.0, 5.0) };
        let vy = s * if label.is_some() { rng.uniform(45.0, 65.0) } else { rng.uniform(25.0, 50.0) };
        self.bubbles.push(Bubble {
            x0: x,
            x,
            y,
            ph,
            age: 0.0,
            r,
            vy,
            life: if label.is_some() { 9.0 } else { 30.0 },
            label: label.map(str::to_string),
            c: color,
        });
    }

    // The Python version bakes background + rays into one surface that is only refreshed 10x per
    // second (to save CPU); here the rays are simply drawn every frame over the recorded background.
    fn draw_rays(&self, cr: &mut Canvas, env: &Env, t: f64) {
        let (w, h) = (env.w, env.h);
        for r in &self.rays {
            let sway = (t * r.sp + r.ph).sin();
            let x0 = r.x * w + sway * w * 0.04;
            let x1 = x0 + w * 0.18;
            let (tw, bw) = (r.w * w * 0.35, r.w * w);
            let a = r.a * (0.6 + 0.4 * (t * 0.27 + r.ph * 3.0).sin());
            let mut g = Gradient::linear(0.0, 0.0, 0.0, h * 0.85);
            g.add_color_stop_rgba(0.0, 0.6, 0.9, 1.0, a);
            g.add_color_stop_rgba(1.0, 0.6, 0.9, 1.0, 0.0);
            cr.move_to(x0 - tw / 2.0, 0.0);
            cr.line_to(x0 + tw / 2.0, 0.0);
            cr.line_to(x1 + bw / 2.0, h * 0.85);
            cr.line_to(x1 - bw / 2.0, h * 0.85);
            cr.close_path();
            cr.set_source(&g);
            cr.fill();
        }
    }

    fn draw_plankton(&self, cr: &mut Canvas, env: &Env, t: f64, far: bool) {
        let s = env.s;
        for &[x, y, z, ph] in &self.plankton {
            if (z < 0.6) != far {
                continue;
            }
            let tw = 0.5 + 0.5 * (t * (0.8 + z) + ph * 5.0).sin();
            cr.set_source_rgba(0.7, 0.9, 1.0, (0.08 + 0.3 * z) * (0.4 + 0.6 * tw));
            cr.circle(x, y, s * (0.6 + 1.6 * z));
            cr.fill();
        }
    }

    fn draw_weeds(&self, cr: &mut Canvas, env: &Env, t: f64) {
        let s = env.s;
        cr.set_line_cap(LineCap::Round);
        cr.set_line_join(LineJoin::Round);
        for wd in &self.weeds {
            let base = bed_y(env, wd.x, false) + 3.0 * s;
            cr.move_to(wd.x, base);
            for k in 1..10 {
                let f = k as f64 / 9.0;
                let xx = wd.x
                    + (t * 0.7 + wd.ph + f * 2.2).sin() * f * s * 22.0
                    + f * f * (t * 0.3 + wd.ph).sin() * s * 12.0;
                cr.line_to(xx, base - f * wd.h);
            }
            cr.set_rgba(wd.c, 0.55);
            cr.set_line_width(wd.lw);
            cr.stroke();
        }
    }

    fn draw_bubbles(&self, cr: &mut Canvas, env: &Env, labels: bool) {
        let s = env.s;
        for b in &self.bubbles {
            if b.label.is_some() != labels {
                continue;
            }
            let fade = (b.age * 3.0).min(1.0) * ((b.life - b.age) / 1.5).min(1.0);
            let (x, y, r) = (b.x, b.y, b.r);
            cr.set_source_rgba(0.7, 0.9, 1.0, 0.07 * fade);
            cr.circle(x, y, r);
            cr.fill_preserve();
            cr.set_source_rgba(0.75, 0.92, 1.0, 0.45 * fade);
            cr.set_line_width((1.2 * s).max(1.0));
            cr.stroke();
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.55 * fade);
            cr.circle(x - r * 0.35, y - r * 0.35, r * 0.25);
            cr.fill();
            if let Some(label) = &b.label {
                let lay = text::layout(label, 15.0 * s, weight::SEMIBOLD, None);
                let th = lay.get_pixel_size().1;
                text::show(cr, &lay, x + r + 6.0 * s, y - th / 2.0, lighten(b.c, 0.3), 0.9 * fade, 0.6);
            }
        }
    }
}

impl Theme for Aquarium {
    type Actor = Jelly;
    const NAME: &'static str = "aquarium";

    fn new(_env: &Env) -> Self {
        Aquarium {
            bubbles: Vec::new(),
            sparkles: Vec::new(),
            plankton: Vec::new(),
            rays: Vec::new(),
            weeds: Vec::new(),
            bg: None,
            next_ambient: 0.0,
        }
    }

    fn on_tool(&mut self, env: &mut Env, actor: &mut Jelly, name: &str, index: usize) {
        let b = &actor.b;
        let r = b.radius;
        let x = b.x + env.rng.uniform(-r * 0.5, r * 0.5);
        self.spawn_bubble(env, x, b.y - r * 0.2 + index as f64 * 26.0 * env.s, Some(name), b.color);
    }

    fn setup_world(&mut self, env: &mut Env) {
        let (w, h, s) = (env.w, env.h, env.s);
        let env = &*env;
        let rng = &env.rng;
        self.bg = Some(Picture::record(w, h, |cr| {
            let mut g = Gradient::linear(0.0, 0.0, 0.0, h);
            g.add_color_stop_rgb(0.0, 0.03, 0.18, 0.31);
            g.add_color_stop_rgb(0.5, 0.01, 0.08, 0.17);
            g.add_color_stop_rgb(1.0, 0.00, 0.02, 0.05);
            cr.set_source(&g);
            cr.rectangle(0.0, 0.0, w, h);
            cr.fill();
            let mut v = Gradient::radial(w / 2.0, h * 0.4, w.min(h) * 0.3, w / 2.0, h * 0.4, w.max(h) * 0.8);
            v.add_color_stop_rgba(0.0, 0.0, 0.0, 0.0, 0.0);
            v.add_color_stop_rgba(1.0, 0.0, 0.0, 0.0, 0.6);
            cr.set_source(&v);
            cr.rectangle(0.0, 0.0, w, h);
            cr.fill();
            for (far, col) in [(true, (0.02, 0.07, 0.12, 0.85)), (false, (0.005, 0.025, 0.045, 1.0))] {
                cr.move_to(0.0, h);
                let mut xi = 0.0;
                while xi < w + 9.0 {
                    cr.line_to(xi, bed_y(env, xi, far));
                    xi += 8.0;
                }
                cr.line_to(w, h);
                cr.close_path();
                cr.set_source_rgba(col.0, col.1, col.2, col.3);
                cr.fill();
            }
            cr.set_line_width(1.5 * s);
            cr.set_source_rgba(0.3, 0.8, 0.9, 0.07);
            let mut xi = 0.0;
            while xi < w + 9.0 {
                cr.line_to(xi, bed_y(env, xi, false));
                xi += 8.0;
            }
            cr.stroke();
        }));

        self.rays = (0..7)
            .map(|_| Ray {
                x: rng.uniform(-0.1, 1.0),
                w: rng.uniform(0.04, 0.12),
                ph: rng.uniform(0.0, TAU),
                sp: rng.uniform(0.05, 0.14),
                a: rng.uniform(0.03, 0.06),
            })
            .collect();
        let n = (160.0 * w * h / (1920.0 * 1200.0)) as usize + 20;
        self.plankton =
            (0..n).map(|_| [rng.uniform(0.0, w), rng.uniform(0.0, h), rng.uniform(0.15, 1.0), rng.uniform(0.0, TAU)]).collect();
        self.weeds = (0..(w / (70.0 * s)) as usize)
            .map(|_| Weed {
                x: rng.uniform(0.0, w),
                h: s * rng.uniform(50.0, 190.0),
                ph: rng.uniform(0.0, TAU),
                lw: s * rng.uniform(3.0, 7.0),
                c: (rng.uniform(0.02, 0.08), rng.uniform(0.25, 0.45), rng.uniform(0.3, 0.45)),
            })
            .collect();
    }

    fn update_world(&mut self, env: &mut Env, _actors: &mut [Jelly], dt: f64) {
        let (t, s) = (env.t, env.s);
        if t >= self.next_ambient {
            self.next_ambient = t + env.rng.uniform(0.4, 1.6);
            let x = env.rng.uniform(0.0, env.w);
            self.spawn_bubble(env, x, bed_y(env, x, false), None, (0.8, 0.9, 1.0));
        }
        for b in &mut self.bubbles {
            b.age += dt;
            b.y -= b.vy * dt;
            b.x = b.x0 + (b.age * 2.2 + b.ph).sin() * s * 10.0;
        }
        self.bubbles.retain(|b| b.y > -40.0 && b.age < b.life);
        for p in &mut self.sparkles {
            p.age += dt;
            p.x += p.vx * dt;
            p.y += p.vy * dt;
        }
        self.sparkles.retain(|p| p.age < p.life);
        for p in &mut self.plankton {
            p[0] += (t * 0.1 + p[3]).sin() * p[2] * 6.0 * s * dt;
            p[1] -= p[2] * 4.0 * s * dt;
            if p[1] < -5.0 {
                p[1] = env.h + 5.0;
                p[0] = env.rng.uniform(0.0, env.w);
            }
        }
    }

    fn draw_world(&mut self, cr: &mut Canvas, env: &Env, actors: &[Jelly], t: f64) {
        let s = env.s;
        if let Some(bg) = &self.bg {
            cr.picture(bg, 0.0, 0.0, 1.0);
        }
        self.draw_rays(cr, env, t);
        self.draw_plankton(cr, env, t, true);
        self.draw_weeds(cr, env, t);
        self.draw_bubbles(cr, env, false);
        for p in &self.sparkles {
            let f = p.age / p.life;
            cr.set_rgba(p.c, (1.0 - f) * 0.8);
            cr.circle(p.x, p.y, s * 2.2 * (1.0 - f) + 0.4);
            cr.fill();
        }
        for a in sorted_actors(actors) {
            a.draw(cr, env, self, t);
        }
        self.draw_bubbles(cr, env, true);
        self.draw_plankton(cr, env, t, false);
    }
}
