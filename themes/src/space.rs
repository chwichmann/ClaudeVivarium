//! Space theme: sessions are planets orbiting a Claude-orange star.
//!
//! working = orbits fast with a comet tail, tool calls shoot away as labelled meteors
//! idle    = crawls along its orbit       waiting = leaves its orbit, comes to the front, pulses
use vivarium_core::prelude::*;

fn omega(st: State) -> f64 {
    match st {
        State::Working => 0.30,
        State::Idle => 0.035,
        State::Waiting => 0.02,
        State::Blocked => 0.0,
        State::Rest => 0.05,
    }
}

const FLAT: f64 = 0.40; // orbits are ellipses: we look at the system from slightly above

#[derive(Clone)]
pub struct Planet {
    b: Base,
    theta: f64,
    orbit_r: f64,
    orbit_target: f64,
    trail: Vec<(f64, f64)>,
    has_ring: bool,
    ring_tilt: f64,
    depth: f64,
}

struct Meteor {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    age: f64,
    life: f64,
    label: String,
    c: Rgb,
}

struct Shooting {
    x: f64,
    y: f64,
    age: f64,
    vx: f64,
    vy: f64,
}

/// A drifting, twinkling star: x, y, depth (0.2..1), phase.
struct Star {
    x: f64,
    y: f64,
    z: f64,
    ph: f64,
}

pub struct Space {
    stars: Vec<Star>,
    meteors: Vec<Meteor>,
    shooting: Option<Shooting>,
    next_shooting: f64,
    bg: Option<Picture>,
    cx: f64,
    cy: f64,
    orbit_base: f64,
}

impl ActorT<Space> for Planet {
    const BASE_RADIUS: f64 = 30.0;

    fn new(base: Base, env: &Env, _world: &mut Space) -> Self {
        let theta = env.rng.uniform(0.0, TAU);
        let has_ring = env.rng.random() < 0.5;
        let ring_tilt = env.rng.uniform(-0.45, 0.45);
        Planet { b: base, theta, orbit_r: 0.0, orbit_target: 0.0, trail: Vec::new(), has_ring, ring_tilt, depth: 1.0 }
    }

    fn base(&self) -> &Base {
        &self.b
    }

    fn base_mut(&mut self) -> &mut Base {
        &mut self.b
    }

    fn companion_reset(&mut self) {
        self.trail.clear();
    }

    fn mv(&mut self, _env: &mut Env, world: &mut Space, _others: &[Other], dt: f64, _t: f64, target: Option<(f64, f64)>) {
        let sc = world;
        let st = self.b.state();
        if self.orbit_r == 0.0 {
            self.orbit_r = self.orbit_target;
        }
        self.orbit_r = lerp(self.orbit_r, self.orbit_target, (dt * 0.8).min(1.0));
        let rx = self.orbit_r.max(1.0);
        self.theta += omega(st) * (sc.orbit_base / rx).sqrt() * dt;
        let ox = sc.cx + self.theta.cos() * rx;
        let oy = sc.cy + self.theta.sin() * rx * FLAT;
        let (gx, gy) = target.unwrap_or((ox, oy));
        let k = (dt * 1.0).min(1.0);
        let (nx, ny) = (lerp(self.b.x, gx, k), lerp(self.b.y, gy, k));
        if dt > 0.0 {
            self.b.vx = (nx - self.b.x) / dt;
            self.b.vy = (ny - self.b.y) / dt;
        }
        self.b.x = nx;
        self.b.y = ny;
        self.depth = if target.is_some() { 1.0 } else { self.theta.sin() };
        if st == State::Working && !self.b.dying {
            self.trail.push((self.b.x, self.b.y));
            if self.trail.len() > 45 {
                self.trail.remove(0);
            }
        } else if !self.trail.is_empty() {
            self.trail.remove(0);
        }
    }

    fn draw(&self, cr: &mut Canvas, env: &Env, world: &Space, t: f64) {
        let (sc, s) = (world, env.s);
        let b = &self.b;
        let a = b.alpha * b.flicker;
        if a < 0.01 {
            return;
        }
        let (x, y, r, c, st) = (b.x, b.y, b.radius, b.color, b.state());
        // comet tail
        let n = self.trail.len();
        cr.set_line_cap(LineCap::Round);
        for i in 1..n {
            let f = i as f64 / n as f64;
            cr.set_rgba(c, 0.45 * f * a);
            cr.set_line_width(r * 0.9 * f);
            cr.move_to(self.trail[i - 1].0, self.trail[i - 1].1);
            cr.line_to(self.trail[i].0, self.trail[i].1);
            cr.stroke();
        }
        // glow
        cr.glow(x, y, r * 2.6, c, if st != State::Idle { 0.4 } else { 0.22 } * a, 0.5 / 2.6);
        if st == State::Waiting {
            cr.pulse_rings(x, y, r, c, a, t, s);
        }
        let rc = lighten(c, 0.35);
        if self.has_ring {
            self.ring(cr, s, x, y, r, rc, a, true);
        }
        // sphere lit from the star
        let (mut lx, mut ly) = (sc.cx - x, sc.cy - y);
        let d = hypot(lx, ly);
        let d = if d == 0.0 { 1.0 } else { d };
        lx /= d;
        ly /= d;
        let hi = lighten(c, 0.65);
        let mut sg = Gradient::radial(x + lx * r * 0.45, y + ly * r * 0.45, r * 0.1, x, y, r * 1.05);
        sg.stop(0.0, hi, a);
        sg.stop(0.45, c, a);
        sg.add_color_stop_rgba(1.0, c.0 * 0.18, c.1 * 0.18, c.2 * 0.22, a);
        cr.set_source(&sg);
        cr.circle(x, y, r);
        cr.fill();
        // surface bands
        cr.save();
        cr.circle(x, y, r);
        cr.clip();
        for k in 0..3 {
            let kf = k as f64;
            let yy = y + (kf - 1.0) * r * 0.45 + (t * 0.5 + kf + b.seed).sin() * r * 0.05;
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.06 * a);
            cr.rectangle(x - r, yy - r * 0.07, 2.0 * r, r * 0.14);
            cr.fill();
        }
        cr.restore();
        if self.has_ring {
            self.ring(cr, s, x, y, r, rc, a, false);
        }
    }
}

impl Planet {
    #[allow(clippy::too_many_arguments)]
    fn ring(&self, cr: &mut Canvas, s: f64, x: f64, y: f64, r: f64, c: Rgb, a: f64, back: bool) {
        cr.save();
        cr.translate(x, y);
        cr.rotate(self.ring_tilt);
        cr.scale(r * 1.9, r * 0.45);
        cr.new_sub_path();
        if back {
            cr.arc(0.0, 0.0, 1.0, PI, TAU);
        } else {
            cr.arc(0.0, 0.0, 1.0, 0.0, PI);
        }
        cr.restore();
        cr.set_rgba(c, 0.55 * a);
        cr.set_line_width(3.0 * s);
        cr.stroke();
    }
}

impl Space {
    fn draw_star(&self, cr: &mut Canvas, env: &Env, actors: &[Planet], t: f64) {
        let (s, x, y) = (env.s, self.cx, self.cy);
        let busy = actors.iter().any(|a| a.b.state() == State::Working);
        let r = 46.0 * s * (1.0 + 0.04 * (t * if busy { 2.5 } else { 0.8 }).sin());
        let c = CLAUDE_ORANGE;
        let mut g = Gradient::radial(x, y, r * 0.5, x, y, r * 5.0);
        g.stop(0.0, c, if busy { 0.45 } else { 0.3 });
        g.stop(1.0, c, 0.0);
        cr.set_source(&g);
        cr.circle(x, y, r * 5.0);
        cr.fill();
        // rotating asterisk rays
        cr.save();
        cr.translate(x, y);
        cr.rotate(t * if busy { 0.12 } else { 0.04 });
        for k in 0..8 {
            let kf = k as f64;
            let ln = r * (2.4 + 0.5 * (t * 0.9 + kf * 1.7).sin());
            cr.save();
            cr.rotate(kf * TAU / 8.0);
            let mut rg = Gradient::linear(0.0, 0.0, ln, 0.0);
            rg.add_color_stop_rgba(0.0, 1.0, 0.8, 0.6, 0.6);
            rg.stop(1.0, c, 0.0);
            cr.set_source(&rg);
            cr.move_to(0.0, -r * 0.22);
            cr.line_to(ln, 0.0);
            cr.line_to(0.0, r * 0.22);
            cr.close_path();
            cr.fill();
            cr.restore();
        }
        cr.restore();
        let mut cg = Gradient::radial(x - r * 0.25, y - r * 0.25, r * 0.1, x, y, r);
        cg.add_color_stop_rgba(0.0, 1.0, 0.95, 0.85, 1.0);
        cg.add_color_stop_rgba(0.6, 1.0, 0.68, 0.45, 1.0);
        cg.stop(1.0, c, 1.0);
        cr.set_source(&cg);
        cr.circle(x, y, r);
        cr.fill();
    }
}

impl Theme for Space {
    type Actor = Planet;
    const NAME: &'static str = "space";
    const LABEL_BG: Rgb = (0.02, 0.01, 0.06);
    const EMPTY_COLOR: Rgb = (0.95, 0.75, 0.6);

    fn new(_env: &Env) -> Self {
        Space {
            stars: Vec::new(),
            meteors: Vec::new(),
            shooting: None,
            next_shooting: 8.0,
            bg: None,
            cx: 0.0,
            cy: 0.0,
            orbit_base: 300.0,
        }
    }

    fn spawn_point(&self, env: &Env) -> (f64, f64) {
        // new planets are born from the star
        (if self.cx != 0.0 { self.cx } else { env.w / 2.0 }, if self.cy != 0.0 { self.cy } else { env.h / 2.0 })
    }

    fn waiting_targets(&self, env: &Env, waiting: &[&Planet]) -> Vec<(f64, f64)> {
        let n = waiting.len() as f64;
        (0..waiting.len()).map(|i| (self.cx + (i as f64 - (n - 1.0) / 2.0) * env.w * 0.24, self.cy + env.h * 0.24)).collect()
    }

    fn on_tool(&mut self, env: &mut Env, actor: &mut Planet, name: &str, index: usize) {
        let b = &actor.b;
        let (dx, dy) = (b.x - self.cx, b.y - self.cy);
        let ang = dy.atan2(dx) + env.rng.uniform(-0.5, 0.5);
        let sp = env.s * env.rng.uniform(170.0, 240.0);
        self.meteors.push(Meteor {
            x: b.x,
            y: b.y,
            vx: ang.cos() * sp,
            vy: ang.sin() * sp,
            age: -(index as f64) * 0.25,
            life: 3.2,
            label: name.to_string(),
            c: b.color,
        });
    }

    fn setup_world(&mut self, env: &mut Env) {
        let (w, h, s) = (env.w, env.h, env.s);
        let rng = &env.rng;
        self.cx = w / 2.0;
        self.cy = h * 0.45;
        self.orbit_base = w.min(h * 1.6) * 0.16;
        self.bg = Some(Picture::record(w, h, |cr| {
            let mut g = Gradient::linear(0.0, 0.0, 0.0, h);
            g.add_color_stop_rgb(0.0, 0.01, 0.01, 0.04);
            g.add_color_stop_rgb(1.0, 0.03, 0.01, 0.06);
            cr.set_source(&g);
            cr.rectangle(0.0, 0.0, w, h);
            cr.fill();
            for _ in 0..6 {
                // nebulae
                let (nx, ny) = (rng.uniform(0.0, w), rng.uniform(0.0, h));
                let r = rng.uniform(0.2, 0.45) * w;
                let col = *rng.choice(&[(0.45, 0.15, 0.6), (0.1, 0.4, 0.55), (0.6, 0.2, 0.35), (0.2, 0.2, 0.6)]);
                let mut ng = Gradient::radial(nx, ny, 0.0, nx, ny, r);
                ng.stop(0.0, col, rng.uniform(0.07, 0.14));
                ng.stop(1.0, col, 0.0);
                cr.set_source(&ng);
                cr.rectangle(0.0, 0.0, w, h);
                cr.fill();
            }
            for _ in 0..(500.0 * w * h / (1920.0 * 1200.0)) as usize {
                // faint static stars
                cr.set_source_rgba(0.8, 0.85, 1.0, rng.uniform(0.05, 0.35));
                cr.circle(rng.uniform(0.0, w), rng.uniform(0.0, h), rng.uniform(0.3, 0.9) * s);
                cr.fill();
            }
        }));
        let n = (220.0 * w * h / (1920.0 * 1200.0)) as usize + 20;
        self.stars = (0..n)
            .map(|_| Star { x: rng.uniform(0.0, w), y: rng.uniform(0.0, h), z: rng.uniform(0.2, 1.0), ph: rng.uniform(0.0, TAU) })
            .collect();
    }

    fn update_world(&mut self, env: &mut Env, actors: &mut [Planet], dt: f64) {
        let (s, w, h) = (env.s, env.w, env.h);
        let rng = &env.rng;
        // assign orbits: oldest session innermost
        let mut live: Vec<usize> = (0..actors.len()).filter(|&i| !actors[i].b.dying).collect();
        live.sort_by(|&i, &j| actors[i].b.info.started.total_cmp(&actors[j].b.info.started));
        let n = live.len();
        let gap = if n > 1 { (w * 0.085).min((w * 0.47 - self.orbit_base) / (n as f64 - 1.0).max(1.0)) } else { 0.0 };
        for (i, &ai) in live.iter().enumerate() {
            actors[ai].orbit_target = self.orbit_base + i as f64 * gap;
        }
        for st in &mut self.stars {
            st.x -= st.z * 3.0 * s * dt;
            if st.x < -2.0 {
                st.x = w + 2.0;
                st.y = rng.uniform(0.0, h);
            }
        }
        for m in &mut self.meteors {
            m.age += dt;
            if m.age > 0.0 {
                m.x += m.vx * dt;
                m.y += m.vy * dt;
            }
        }
        self.meteors.retain(|m| m.age < m.life);
        if env.t > self.next_shooting {
            self.next_shooting = env.t + rng.uniform(7.0, 16.0);
            let y0 = rng.uniform(0.0, h * 0.35);
            self.shooting = Some(Shooting {
                x: rng.uniform(w * 0.2, w),
                y: y0,
                age: 0.0,
                vx: -s * rng.uniform(700.0, 1000.0),
                vy: s * rng.uniform(150.0, 300.0),
            });
        }
        if let Some(sh) = &mut self.shooting {
            sh.age += dt;
            sh.x += sh.vx * dt;
            sh.y += sh.vy * dt;
            if sh.age > 1.2 {
                self.shooting = None;
            }
        }
    }

    fn draw_world(&mut self, cr: &mut Canvas, env: &Env, actors: &[Planet], t: f64) {
        let s = env.s;
        if let Some(bg) = &self.bg {
            cr.picture(bg, 0.0, 0.0, 1.0);
        }
        for st in &self.stars {
            let tw = 0.5 + 0.5 * (t * (1.0 + st.z * 2.0) + st.ph * 7.0).sin();
            cr.set_source_rgba(0.85, 0.9, 1.0, (0.2 + 0.6 * st.z) * (0.35 + 0.65 * tw));
            cr.circle(st.x, st.y, s * (0.5 + 1.3 * st.z));
            cr.fill();
        }
        if let Some(sh) = &self.shooting {
            let f = 1.0 - sh.age / 1.2;
            let mut g = Gradient::linear(sh.x, sh.y, sh.x - sh.vx * 0.15, sh.y - sh.vy * 0.15);
            g.add_color_stop_rgba(0.0, 1.0, 1.0, 1.0, 0.8 * f);
            g.add_color_stop_rgba(1.0, 1.0, 1.0, 1.0, 0.0);
            cr.set_source(&g);
            cr.set_line_width(2.0 * s);
            cr.move_to(sh.x, sh.y);
            cr.line_to(sh.x - sh.vx * 0.15, sh.y - sh.vy * 0.15);
            cr.stroke();
        }
        // orbits
        for a in actors {
            if a.orbit_r <= 0.0 {
                continue;
            }
            let working = a.b.state() == State::Working;
            let c = if working { a.b.color } else { (0.6, 0.65, 0.85) };
            cr.ellipse(self.cx, self.cy, a.orbit_r, a.orbit_r * 0.40);
            cr.set_rgba(c, if working { 0.16 } else { 0.08 } * a.b.alpha);
            cr.set_line_width(1.2 * s);
            cr.stroke();
        }
        let sorted = sorted_actors(actors);
        for a in &sorted {
            if a.depth < 0.0 {
                a.draw(cr, env, self, t);
            }
        }
        self.draw_star(cr, env, actors, t);
        for a in &sorted {
            if a.depth >= 0.0 {
                a.draw(cr, env, self, t);
            }
        }
        for m in &self.meteors {
            if m.age <= 0.0 {
                continue;
            }
            let f = ((m.life - m.age) / 1.0).min(1.0) * (m.age * 4.0).min(1.0);
            let c = lighten(m.c, 0.3);
            let (tx, ty) = (m.x - m.vx * 0.3, m.y - m.vy * 0.3);
            let mut g = Gradient::linear(m.x, m.y, tx, ty);
            g.stop(0.0, c, 0.9 * f);
            g.stop(1.0, c, 0.0);
            cr.set_source(&g);
            cr.set_line_width(3.0 * s);
            cr.set_line_cap(LineCap::Round);
            cr.move_to(m.x, m.y);
            cr.line_to(tx, ty);
            cr.stroke();
            cr.set_source_rgba(1.0, 1.0, 1.0, f);
            cr.circle(m.x, m.y, 3.0 * s);
            cr.fill();
            let lay = text::layout(&m.label, 15.0 * s, weight::SEMIBOLD, None);
            text::show(cr, &lay, m.x + 8.0 * s, m.y - 22.0 * s, c, 0.95 * f, 0.6);
        }
    }
}
