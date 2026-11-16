//! Synthwave theme: 80s neon sunset, scrolling grid floor, sessions are rotating wireframe cubes.
//!
//! working = spins fast, tool calls appear as glitchy neon text
//! idle    = slow spin, drifts          waiting = comes to the front, grows, pulses
use vivarium_core::prelude::*;

const NEON_PINK: Rgb = (1.0, 0.25, 0.75);
const NEON_CYAN: Rgb = (0.25, 0.95, 1.0);

fn spin_of(st: State) -> f64 {
    match st {
        State::Working => 1.8,
        State::Idle => 0.35,
        State::Waiting => 0.9,
        State::Blocked => 0.05,
        State::Rest => 0.15,
    }
}

fn speed_of(st: State) -> f64 {
    match st {
        State::Working => 70.0,
        State::Idle => 18.0,
        State::Waiting => 40.0,
        State::Blocked => 8.0,
        State::Rest => 8.0,
    }
}

fn turn_of(st: State) -> f64 {
    match st {
        State::Working => 1.1,
        State::Idle => 0.4,
        State::Waiting => 0.5,
        State::Blocked => 0.2,
        State::Rest => 0.25,
    }
}

/// The corners of the cube: x, y, z each -1 or 1 (x is the highest bit of the index).
fn cube_v(i: usize) -> (f64, f64, f64) {
    let sign = |bit: usize| if i & bit != 0 { 1.0 } else { -1.0 };
    (sign(4), sign(2), sign(1))
}

/// The 12 edges: pairs of corners that differ in exactly one coordinate.
fn cube_e() -> impl Iterator<Item = (usize, usize)> {
    (0..8usize).flat_map(|i| (i + 1..8).filter(move |j| (i ^ j).count_ones() == 1).map(move |j| (i, j)))
}

#[derive(Clone)]
pub struct Cube {
    b: Base,
    ax: f64,
    ay: f64,
}

struct Word {
    x: f64,
    y: f64,
    age: f64,
    life: f64,
    label: String,
    c: Rgb,
}

pub struct Synthwave {
    words: Vec<Word>,
    bg: Option<Picture>,
    scan: Option<Picture>,
    stars: Vec<(f64, f64, f64)>,
    horizon: f64,
}

impl ActorT<Synthwave> for Cube {
    const BASE_RADIUS: f64 = 44.0;

    fn new(base: Base, env: &Env, _world: &mut Synthwave) -> Self {
        Cube { b: base, ax: env.rng.uniform(0.0, TAU), ay: env.rng.uniform(0.0, TAU) }
    }

    fn base(&self) -> &Base {
        &self.b
    }

    fn base_mut(&mut self) -> &mut Base {
        &mut self.b
    }

    fn mv(&mut self, env: &mut Env, _world: &mut Synthwave, others: &[Other], dt: f64, t: f64, target: Option<(f64, f64)>) {
        let (st, s) = (self.b.state(), env.s);
        let spin = spin_of(st);
        self.ax += spin * 0.6 * dt;
        self.ay += spin * dt;
        let r = self.b.radius;
        let bbox = (110.0 * s, 150.0 * s, env.w - 110.0 * s, env.h - env.strip_h - r * 2.0);
        self.b.wander(env, others, dt, t, target, speed_of(st) * s, turn_of(st), bbox, 3.0);
    }

    fn draw(&self, cr: &mut Canvas, env: &Env, world: &Synthwave, t: f64) {
        let s = env.s;
        let b = &self.b;
        let a = b.alpha * b.flicker;
        if a < 0.01 {
            return;
        }
        let (r, c) = (b.radius, b.color);
        // floor shadow/reflection when hovering over the grid
        if b.y < env.h - env.strip_h {
            let fy = (b.y + r * 1.6).max(world.horizon + 10.0 * s);
            cr.ellipse(b.x, fy, r * 1.1, r * 0.18);
            cr.set_rgba(c, 0.12 * a);
            cr.fill();
        }
        cr.glow(b.x, b.y, r * 2.4, c, if b.state() != State::Idle { 0.35 } else { 0.18 } * a, 0.0);
        if b.state() == State::Waiting {
            cr.pulse_rings(b.x, b.y, r, c, a, t, s);
        }
        let pts = self.project(r);
        cr.set_line_cap(LineCap::Round);
        cr.set_line_join(LineJoin::Round);
        for (width, alpha, col) in [(8.0 * s, 0.18, c), (2.4 * s, 0.95, lighten(c, 0.35))] {
            for (i, j) in cube_e() {
                cr.move_to(pts[i].0, pts[i].1);
                cr.line_to(pts[j].0, pts[j].1);
            }
            cr.set_rgba(col, alpha * a);
            cr.set_line_width(width);
            cr.stroke();
        }
        for (px, py) in pts {
            // glowing vertices
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.8 * a);
            cr.circle(px, py, 2.4 * s);
            cr.fill();
        }
    }
}

impl Cube {
    fn project(&self, r: f64) -> [(f64, f64); 8] {
        let (ca, sa, cb, sb) = (self.ax.cos(), self.ax.sin(), self.ay.cos(), self.ay.sin());
        let mut pts = [(0.0, 0.0); 8];
        for (i, pt) in pts.iter_mut().enumerate() {
            let (x, y, z) = cube_v(i);
            let (y, z) = (y * ca - z * sa, y * sa + z * ca); // rotate around X
            let (x, z) = (x * cb + z * sb, -x * sb + z * cb); // rotate around Y
            let p = 3.2 / (3.2 + z);
            *pt = (self.b.x + x * r * 0.75 * p, self.b.y + y * r * 0.75 * p);
        }
        pts
    }
}

impl Synthwave {
    fn draw_sun(cr: &mut Canvas, w: f64, h: f64, hz: f64) {
        let r = h * 0.23;
        let (cx, cy) = (w / 2.0, hz - r * 0.35);
        cr.save();
        cr.rectangle(0.0, 0.0, w, hz); // nothing of the sun below the horizon
        cr.clip();
        let mut g = Gradient::radial(cx, cy, r * 0.8, cx, cy, r * 2.0);
        g.add_color_stop_rgba(0.0, 1.0, 0.3, 0.6, 0.35);
        g.add_color_stop_rgba(1.0, 1.0, 0.3, 0.6, 0.0);
        cr.set_source(&g);
        cr.circle(cx, cy, r * 2.0);
        cr.fill();
        cr.circle(cx, cy, r);
        cr.clip();
        cr.new_path();
        cr.rectangle(cx - r, cy - r, 2.0 * r, 2.0 * r);
        let top = cy - r * 0.15;
        for k in 0..7 {
            // the classic cut stripes, thicker towards the horizon
            let f = k as f64 / 7.0;
            let yy = top + f * (hz - top);
            let th = (hz - top) * (0.02 + 0.09 * f);
            cr.rectangle(cx - r, yy, 2.0 * r, th);
        }
        cr.set_fill_rule(FillRule::EvenOdd);
        let mut sg = Gradient::linear(0.0, cy - r, 0.0, cy + r);
        sg.add_color_stop_rgb(0.0, 1.0, 0.88, 0.35);
        sg.add_color_stop_rgb(0.55, 1.0, 0.45, 0.4);
        sg.add_color_stop_rgb(1.0, 1.0, 0.15, 0.6);
        cr.set_source(&sg);
        cr.fill();
        cr.restore();
        cr.set_fill_rule(FillRule::Winding);
    }

    fn draw_grid(&self, cr: &mut Canvas, env: &Env, t: f64) {
        let (w, h, s, hz) = (env.w, env.h, env.s, self.horizon);
        let vx = w / 2.0;
        // lines grouped by brightness so each group is a single stroke
        type Seg = ((f64, f64), (f64, f64));
        let mut groups: Vec<(f64, Vec<Seg>)> = vec![(1.0, Vec::new())];
        for i in -24..25 {
            // lines towards the vanishing point
            let fi = i as f64;
            groups[0].1.push(((vx + fi * w * 0.004, hz), (vx + fi * w * 0.075, h)));
        }
        let off = pymod(t * 0.9, 1.0); // floor moves towards the viewer
        for k in 0..22 {
            let z = k as f64 + 1.0 - off;
            let yy = hz + (h - hz) * clamp(1.0 / (z * 0.45 + 1.0), 0.0, 1.0);
            let f = (clamp(1.2 - z * 0.06, 0.15, 1.0) * 4.0).round_ties_even() / 4.0;
            let seg = ((0.0, yy), (w, yy));
            match groups.iter_mut().find(|g| g.0 == f) {
                Some(g) => g.1.push(seg),
                None => groups.push((f, vec![seg])),
            }
        }
        for (width, alpha) in [(5.0 * s, 0.12), (1.4 * s, 0.75)] {
            cr.set_line_width(width);
            for (f, segs) in &groups {
                for ((x0, y0), (x1, y1)) in segs {
                    cr.move_to(*x0, *y0);
                    cr.line_to(*x1, *y1);
                }
                cr.set_rgba(NEON_PINK, alpha * f);
                cr.stroke();
            }
        }
    }
}

impl Theme for Synthwave {
    type Actor = Cube;
    const NAME: &'static str = "synthwave";
    const LABEL_BG: Rgb = (0.06, 0.0, 0.10);
    const EMPTY_COLOR: Rgb = NEON_PINK;

    fn new(_env: &Env) -> Self {
        Synthwave { words: Vec::new(), bg: None, scan: None, stars: Vec::new(), horizon: 0.0 }
    }

    fn on_tool(&mut self, env: &mut Env, actor: &mut Cube, name: &str, index: usize) {
        let b = &actor.b;
        let r = b.radius;
        self.words.push(Word {
            x: b.x + env.rng.uniform(-r, r * 0.3),
            y: b.y - r * 1.2 - index as f64 * 30.0 * env.s,
            age: 0.0,
            life: 3.5,
            label: name.to_uppercase(),
            c: b.color,
        });
    }

    fn setup_world(&mut self, env: &mut Env) {
        let (w, h, s) = (env.w, env.h, env.s);
        let rng = &env.rng;
        let hz = h * 0.60;
        self.horizon = hz;
        self.bg = Some(Picture::record(w, h, |cr| {
            let mut g = Gradient::linear(0.0, 0.0, 0.0, hz);
            g.add_color_stop_rgb(0.0, 0.03, 0.0, 0.08);
            g.add_color_stop_rgb(0.6, 0.16, 0.02, 0.24);
            g.add_color_stop_rgb(1.0, 0.45, 0.06, 0.35);
            cr.set_source(&g);
            cr.rectangle(0.0, 0.0, w, hz);
            cr.fill();
            let mut fg = Gradient::linear(0.0, hz, 0.0, h);
            fg.add_color_stop_rgb(0.0, 0.10, 0.0, 0.14);
            fg.add_color_stop_rgb(1.0, 0.02, 0.0, 0.04);
            cr.set_source(&fg);
            cr.rectangle(0.0, hz, w, h - hz);
            cr.fill();
            for _ in 0..(220.0 * w / 1920.0) as usize {
                cr.set_source_rgba(1.0, 0.9, 1.0, rng.uniform(0.1, 0.6));
                cr.circle(rng.uniform(0.0, w), rng.uniform(0.0, hz * 0.75), rng.uniform(0.4, 1.2) * s);
                cr.fill();
            }
            // mountains (two layers)
            let mut mountains: Vec<(Vec<(f64, f64)>, Rgb)> = Vec::new();
            for (layer, (amp, col)) in [(0.16, (0.10, 0.02, 0.18)), (0.09, (0.05, 0.0, 0.10))].into_iter().enumerate() {
                let mut pts = Vec::new();
                let mut x = -20.0;
                while x < w + 40.0 {
                    let yy = if (x - w / 2.0).abs() < w * 0.16 && layer == 0 {
                        hz - rng.uniform(0.0, 0.02) * h // leave the sun visible
                    } else {
                        hz - rng.uniform(0.15, 1.0) * amp * h
                    };
                    pts.push((x, yy));
                    x += rng.uniform(40.0, 120.0) * s;
                }
                mountains.push((pts, col));
            }
            // sun, mountains and horizon glow never change -> drawn once into the background
            Self::draw_sun(cr, w, h, hz);
            for (pts, col) in &mountains {
                cr.move_to(-20.0, hz + 2.0);
                for p in pts {
                    cr.line_to(p.0, p.1);
                }
                cr.line_to(w + 40.0, hz + 2.0);
                cr.close_path();
                cr.set_rgb(*col);
                cr.fill_preserve();
                cr.set_rgba(NEON_CYAN, 0.25);
                cr.set_line_width(1.2 * s);
                cr.stroke();
            }
            let mut hg = Gradient::linear(0.0, hz - 6.0 * s, 0.0, hz + 6.0 * s);
            hg.add_color_stop_rgba(0.0, 1.0, 0.5, 0.9, 0.0);
            hg.add_color_stop_rgba(0.5, 1.0, 0.6, 0.95, 0.8);
            hg.add_color_stop_rgba(1.0, 1.0, 0.5, 0.9, 0.0);
            cr.set_source(&hg);
            cr.rectangle(0.0, hz - 6.0 * s, w, 12.0 * s);
            cr.fill();
        }));
        self.stars = (0..40).map(|_| (rng.uniform(0.0, w), rng.uniform(0.0, hz * 0.6), rng.uniform(0.0, TAU))).collect();
        // scanlines
        self.scan = Some(Picture::record(w, h, |sc| {
            sc.set_source_rgba(0.0, 0.0, 0.0, 0.10);
            let mut yy = 0.0;
            while yy < h {
                sc.rectangle(0.0, yy, w, 1.0);
                yy += 3.0;
            }
            sc.fill();
        }));
    }

    fn update_world(&mut self, env: &mut Env, _actors: &mut [Cube], dt: f64) {
        for wd in &mut self.words {
            wd.age += dt;
            wd.y -= 35.0 * env.s * dt;
        }
        self.words.retain(|wd| wd.age < wd.life);
    }

    fn draw_world(&mut self, cr: &mut Canvas, env: &Env, actors: &[Cube], t: f64) {
        let s = env.s;
        if let Some(bg) = &self.bg {
            cr.picture(bg, 0.0, 0.0, 1.0);
        }
        for &(x, y, ph) in &self.stars {
            cr.set_source_rgba(1.0, 0.9, 1.0, 0.3 + 0.6 * (0.5 + 0.5 * (t * 1.3 + ph * 5.0).sin()));
            cr.circle(x, y, 1.4 * s);
            cr.fill();
        }
        self.draw_grid(cr, env, t);
        for a in sorted_actors(actors) {
            a.draw(cr, env, self, t);
        }
        for wd in &self.words {
            let f = (wd.age * 4.0).min(1.0) * ((wd.life - wd.age) / 1.0).min(1.0);
            let lay = text::layout_family(&wd.label, 20.0 * s, weight::HEAVY, None, Family::Mono);
            let jx = if env.rng.random() < 0.08 { env.rng.uniform(-3.0, 3.0) * s } else { 0.0 }; // glitch
            text::show(cr, &lay, wd.x - 2.0 * s + jx, wd.y, NEON_CYAN, 0.55 * f, 0.0);
            text::show(cr, &lay, wd.x + 2.0 * s + jx, wd.y, NEON_PINK, 0.55 * f, 0.0);
            text::show(cr, &lay, wd.x, wd.y, lighten(wd.c, 0.6), 0.95 * f, 0.0);
        }
        if let Some(scan) = &self.scan {
            cr.picture(scan, 0.0, 0.0, 1.0);
        }
    }
}
