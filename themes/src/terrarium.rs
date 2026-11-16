//! Terrarium theme: every session is a chameleon on a branch - and it takes on the state colour.
//!
//! working = creeps along its branch with the typical rocking gait; every tool call it shoots its tongue
//!           at a fly, the tool name floats up from the catch
//! idle    = sits still, the eye swivels around      waiting = goes to the middle of its branch, grows, pulses
//! rest    = sleeps, pale, eyes closed
use std::rc::Rc;

use vivarium_core::prelude::*;

fn speed_of(st: State) -> f64 {
    match st {
        State::Working => 38.0,
        State::Idle => 6.0,
        State::Waiting => 30.0,
        State::Blocked => 0.0,
        State::Rest => 0.0,
    }
}

/// A branch as a smooth curve; y_at(x) gives the top surface, slope_at(x) the angle.
pub struct Branch {
    /// sampled (x, y) along the branch, sorted by x
    pts: Vec<(f64, f64)>,
    thick: f64,
    x0: f64,
    x1: f64,
}

impl Branch {
    fn new(pts: Vec<(f64, f64)>, thick: f64) -> Self {
        let (x0, x1) = (pts[0].0, pts[pts.len() - 1].0);
        Branch { pts, thick, x0, x1 }
    }

    fn y_at(&self, x: f64) -> f64 {
        let pts = &self.pts;
        let x = clamp(x, self.x0, self.x1);
        for p in pts.windows(2) {
            let ((ax, ay), (bx, by)) = (p[0], p[1]);
            if ax <= x && x <= bx {
                return lerp(ay, by, (x - ax) / (bx - ax).max(1e-6));
            }
        }
        pts[pts.len() - 1].1
    }

    fn slope_at(&self, x: f64) -> f64 {
        let d = 6.0;
        (self.y_at(x + d) - self.y_at(x - d)).atan2(2.0 * d)
    }
}

#[derive(Clone)]
pub struct Chameleon {
    b: Base,
    branch: Option<Rc<Branch>>,
    dir: f64,
    /// 0..1 animation of a tongue strike
    tongue: f64,
    tongue_target: (f64, f64),
    eye_angle: f64,
}

struct Fly {
    x: f64,
    y: f64,
    ph: f64,
    vx: f64,
    vy: f64,
}

struct Vine {
    x: f64,
    len: f64,
    ph: f64,
}

pub struct Terrarium {
    bg: Option<Picture>,
    branches: Vec<Rc<Branch>>,
    /// which chameleon (actor index) sits on which branch; emptied on a new layout
    assign: Vec<(u64, usize)>,
    flies: Vec<Fly>,
    vines: Vec<Vine>,
}

impl ActorT<Terrarium> for Chameleon {
    const BASE_RADIUS: f64 = 40.0;

    fn new(base: Base, _env: &Env, _world: &mut Terrarium) -> Self {
        Chameleon { b: base, branch: None, dir: 1.0, tongue: 0.0, tongue_target: (0.0, 0.0), eye_angle: 0.0 }
    }

    fn base(&self) -> &Base {
        &self.b
    }

    fn base_mut(&mut self) -> &mut Base {
        &mut self.b
    }

    fn companion_reset(&mut self) {
        self.tongue = 0.0;
    }

    fn mv(&mut self, env: &mut Env, world: &mut Terrarium, others: &[Other], dt: f64, t: f64, target: Option<(f64, f64)>) {
        let (st, s) = (self.b.state(), env.s);
        if world.branches.is_empty() {
            return;
        }
        let mut mine = world.branch_of(self.b.index);
        if self.branch.is_none() || mine.is_none() {
            // first time, or re-assign on the new layout
            let bi = world.branch_for(self.b.index, st, others);
            world.assign.retain(|e| e.0 != self.b.index);
            world.assign.push((self.b.index, bi));
            let br = world.branches[bi].clone();
            self.b.x = clamp(self.b.x, br.x0 + 80.0 * s, br.x1 - 80.0 * s);
            self.branch = Some(br);
            mine = Some(bi);
        }
        let Some(b) = self.branch.clone() else { return };
        let (lo, hi) = (b.x0 + 70.0 * s, b.x1 - 70.0 * s);
        let mut speed = speed_of(st) * s;
        if st == State::Working {
            // chameleon gait: rocking forward and back, mostly forward
            speed *= 0.6 + 0.8 * (t * 2.2 + self.b.seed).sin().max(0.0);
        } else if st == State::Idle {
            speed *= (t * 0.15 + self.b.seed).sin().max(0.0);
        }
        if target.is_some() {
            let mid = (b.x0 + b.x1) / 2.0;
            self.dir = if mid > self.b.x { 1.0 } else { -1.0 };
            speed = speed.min((mid - self.b.x).abs() * 1.5);
        }
        // keep a distance to other chameleons on the same branch
        for o in others {
            if o.index != self.b.index
                && world.branch_of(o.index) == mine
                && !o.dying
                && (o.x - self.b.x).abs() < (self.b.radius + o.radius) * 2.2
            {
                self.dir = if self.b.x > o.x { 1.0 } else { -1.0 };
            }
        }
        if self.b.x < lo {
            self.dir = 1.0;
        } else if self.b.x > hi {
            self.dir = -1.0;
        }
        self.b.vx = self.dir * speed;
        self.b.x += self.b.vx * dt;
        self.b.y = b.y_at(self.b.x) - self.b.radius * 0.5;
        self.b.step += dt * self.b.vx.abs() / (self.b.radius * 0.25).max(1.0);
        if self.b.vx.abs() > 1.0 {
            self.b.face = lerp(self.b.face, self.dir, (dt * 3.0).min(1.0));
        }
        self.tongue = (self.tongue - dt * 2.5).max(0.0);
        self.eye_angle = (t * 0.9 + self.b.seed).sin() * 1.2 + (t * 2.3 + self.b.seed * 2.0).sin() * 0.4;
    }

    fn companion_spots(&self, _env: &Env, _t: f64) -> Vec<(f64, f64, f64)> {
        let Some(branch) = &self.branch else { return Vec::new() };
        let n = self.b.info.subagents.len();
        let back = if self.b.face >= 0.0 { -1.0 } else { 1.0 };
        (0..n)
            .map(|i| {
                let x = self.b.x + back * self.b.radius * (1.4 + i as f64 * 0.85);
                (x, branch.y_at(x) - self.b.radius * Self::COMPANION_SCALE * 0.5, 0.0)
            })
            .collect()
    }

    fn draw(&self, cr: &mut Canvas, env: &Env, _world: &Terrarium, t: f64) {
        let s = env.s;
        let b = &self.b;
        let a = b.alpha * b.flicker;
        let Some(branch) = &self.branch else { return };
        if a < 0.01 {
            return;
        }
        let (r, c, st) = (b.radius, b.color, b.state());
        if st == State::Waiting {
            cr.pulse_rings(b.x, b.y, r, c, a, t, s);
        }
        cr.glow(b.x, b.y, r * 2.2, c, if st != State::Idle && st != State::Rest { 0.30 } else { 0.15 } * a, 0.0);
        let sleeping = st == State::Rest;
        let dark = (c.0 * 0.45, c.1 * 0.5, c.2 * 0.4);
        cr.save();
        cr.translate(b.x, b.y);
        cr.rotate(branch.slope_at(b.x));
        let fs = if b.face.abs() > 0.15 { b.face } else { 0.15 };
        cr.scale(fs, 1.0);
        // tongue (behind the head)
        if self.tongue > 0.0 {
            let f = (PI * self.tongue).sin(); // out and back
            let (mx, my) = (r * 0.98, -r * 0.22);
            let (tx, ty) = self.tongue_target;
            // target in local coordinates (undo translation/rotation/flip roughly)
            let ang = branch.slope_at(b.x);
            let (dx, dy) = (tx - b.x, ty - b.y);
            let lx = (dx * (-ang).cos() - dy * (-ang).sin()) / fs;
            let ly = dx * (-ang).sin() + dy * (-ang).cos();
            let (ex, ey) = (lerp(mx, lx, f), lerp(my, ly, f));
            cr.set_line_cap(LineCap::Round);
            cr.set_source_rgba(0.95, 0.45, 0.55, a);
            cr.set_line_width(r * 0.07);
            cr.move_to(mx, my);
            cr.line_to(ex, ey);
            cr.stroke();
            cr.circle(ex, ey, r * 0.1);
            cr.fill();
        }
        // curled tail (spiral, thinner towards the tip)
        cr.set_line_cap(LineCap::Round);
        let curl = if sleeping || st == State::Idle { 1.0 } else { 0.75 + 0.15 * (t * 0.7 + b.seed).sin() };
        let (mut px, mut py) = (-r * 0.85, r * 0.05);
        let mut ang = PI * 0.95;
        let width = r * 0.22;
        for k in 0..14 {
            let k = k as f64;
            let step = r * 0.16 * (1.0 - k / 18.0);
            let (nx, ny) = (px + ang.cos() * step, py + ang.sin() * step);
            cr.move_to(px, py);
            cr.line_to(nx, ny);
            cr.set_line_width(width * (1.0 - k / 16.0));
            cr.set_rgba(dark, a);
            cr.stroke();
            (px, py) = (nx, ny);
            ang -= 0.45 * curl;
        }
        // legs gripping the branch
        cr.set_line_width(r * 0.11);
        cr.set_rgba(dark, a);
        let walking = if b.vx.abs() > 1.0 { 1.0 } else { 0.0 };
        for (i, lx) in [-0.55, -0.35, 0.35, 0.55].into_iter().enumerate() {
            let sw = (b.step + if i == 0 || i == 3 { 0.0 } else { PI }).sin() * 0.18 * walking;
            cr.move_to(lx * r, r * 0.05);
            cr.line_to(lx * r + sw * r, r * 0.38);
            cr.line_to(lx * r + sw * r + r * 0.08, r * 0.5);
        }
        cr.stroke();
        // body: arched back, casque on the head
        cr.move_to(-r * 0.9, r * 0.05);
        cr.curve_to(-r * 0.75, -r * 0.75, r * 0.35, -r * 0.85, r * 0.62, -r * 0.55);
        cr.line_to(r * 0.7, -r * 0.78); // casque
        cr.curve_to(r * 0.85, -r * 0.62, r * 1.02, -r * 0.4, r * 1.05, -r * 0.22);
        cr.curve_to(r * 1.0, r * 0.0, r * 0.7, r * 0.22, r * 0.35, r * 0.25);
        cr.curve_to(-r * 0.1, r * 0.32, -r * 0.6, r * 0.3, -r * 0.9, r * 0.05);
        cr.close_path();
        let hi = lighten(c, 0.5);
        let mut bg = Gradient::linear(0.0, -r * 0.85, 0.0, r * 0.3);
        bg.stop(0.0, hi, a);
        bg.stop(0.6, c, a);
        bg.stop(1.0, dark, a);
        cr.set_source(&bg);
        cr.fill_preserve();
        cr.set_rgba(dark, 0.8 * a);
        cr.set_line_width(1.5 * s);
        cr.stroke();
        // skin pattern: dark bands and a light side stripe
        cr.set_rgba(dark, 0.35 * a);
        for k in 0..4 {
            let k = k as f64;
            let bx = -r * 0.6 + k * r * 0.32;
            cr.move_to(bx, -r * 0.72 + (k - 1.5).abs() * r * 0.07);
            cr.line_to(bx + r * 0.12, -r * 0.72 + (k - 1.5).abs() * r * 0.07);
            cr.line_to(bx + r * 0.06, r * 0.15);
            cr.close_path();
            cr.fill();
        }
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.35 * a);
        cr.set_line_width(r * 0.05);
        cr.move_to(-r * 0.6, -r * 0.05);
        cr.curve_to(-r * 0.2, r * 0.05, r * 0.3, r * 0.05, r * 0.7, -r * 0.05);
        cr.stroke();
        // turret eye
        let (ex, ey, er) = (r * 0.72, -r * 0.38, r * 0.17);
        cr.set_source_rgba(c.0 * 0.8, c.1 * 0.8, c.2 * 0.8, a);
        cr.circle(ex, ey, er);
        cr.fill();
        cr.set_rgba(dark, 0.6 * a);
        cr.set_line_width(1.2 * s);
        for k in [0.55, 0.8] {
            cr.circle(ex, ey, er * k);
            cr.stroke();
        }
        if sleeping {
            cr.set_source_rgba(0.0, 0.0, 0.0, 0.8 * a);
            cr.set_line_width(2.0 * s);
            cr.move_to(ex - er * 0.6, ey);
            cr.line_to(ex + er * 0.6, ey);
            cr.stroke();
        } else {
            let look = if self.tongue <= 0.0 { self.eye_angle } else { -0.4 };
            cr.set_source_rgba(0.0, 0.0, 0.0, a);
            cr.circle(ex + look.cos() * er * 0.5, ey + look.sin() * er * 0.5, er * 0.28);
            cr.fill();
        }
        // mouth line
        cr.set_source_rgba(dark.0 * 0.6, dark.1 * 0.6, dark.2 * 0.6, a);
        cr.set_line_width(1.5 * s);
        cr.move_to(r * 1.03, -r * 0.2);
        cr.curve_to(r * 0.85, -r * 0.12, r * 0.7, -r * 0.12, r * 0.55, -r * 0.18);
        cr.stroke();
        cr.restore();
    }
}

impl Chameleon {
    fn strike(&mut self, tx: f64, ty: f64) {
        self.tongue = 1.0;
        self.tongue_target = (tx, ty);
    }
}

impl Terrarium {
    fn branch_of(&self, index: u64) -> Option<usize> {
        self.assign.iter().find(|e| e.0 == index).map(|e| e.1)
    }

    /// each chameleon gets the branch with the fewest chameleons on it
    fn branch_for(&self, index: u64, state: State, others: &[Other]) -> usize {
        let n = self.branches.len();
        let mut counts = vec![0usize; n];
        for o in others {
            if o.index != index && !o.dying {
                if let Some(bi) = self.branch_of(o.index) {
                    counts[bi] += 1;
                }
            }
        }
        let waiting = state == State::Waiting;
        (0..n).min_by_key(|&i| (counts[i], if waiting { -(i as i64) } else { i as i64 })).unwrap_or(0)
    }
}

impl Theme for Terrarium {
    type Actor = Chameleon;
    const NAME: &'static str = "terrarium";
    const LABEL_BG: Rgb = (0.03, 0.06, 0.03);
    const EMPTY_COLOR: Rgb = (0.85, 1.0, 0.8);

    fn new(_env: &Env) -> Self {
        Terrarium { bg: None, branches: Vec::new(), assign: Vec::new(), flies: Vec::new(), vines: Vec::new() }
    }

    fn waiting_targets(&self, _env: &Env, waiting: &[&Chameleon]) -> Vec<(f64, f64)> {
        waiting.iter().map(|a| (a.b.x, a.b.y)).collect() // chameleons stay on their branch (see mv)
    }

    fn on_tool(&mut self, env: &mut Env, actor: &mut Chameleon, name: &str, index: usize) {
        if actor.branch.is_none() {
            return;
        }
        let side = if actor.b.face >= 0.0 { 1.0 } else { -1.0 };
        let tx = actor.b.x + side * actor.b.radius * (2.2 + 0.3 * index as f64);
        let ty = actor.b.y - actor.b.radius * (0.9 + 0.4 * env.rng.random());
        actor.strike(tx, ty);
        env.spawn_floater(tx, ty - 10.0 * env.s, name, actor.b.color, FloatStyle::Text, 35.0, 3.5);
    }

    fn setup_world(&mut self, env: &mut Env) {
        let (w, h, s) = (env.w, env.h, env.s);
        let rng = &env.rng;
        let strip_h = env.strip_h;
        let bottom = h - strip_h;
        let mut branches: Vec<Branch> = Vec::new();
        self.bg = Some(Picture::record(w, h, |cr| {
            let mut g = Gradient::linear(0.0, 0.0, 0.0, h);
            g.add_color_stop_rgb(0.0, 0.10, 0.17, 0.10);
            g.add_color_stop_rgb(1.0, 0.03, 0.06, 0.03);
            cr.set_source(&g);
            cr.rectangle(0.0, 0.0, w, h);
            cr.fill();
            let mut lg = Gradient::radial(w * 0.15, -h * 0.1, 0.0, w * 0.15, -h * 0.1, h * 0.9); // heat lamp
            lg.add_color_stop_rgba(0.0, 1.0, 0.85, 0.5, 0.35);
            lg.add_color_stop_rgba(1.0, 1.0, 0.85, 0.5, 0.0);
            cr.set_source(&lg);
            cr.rectangle(0.0, 0.0, w, h);
            cr.fill();
            for _ in 0..(26.0 * w / 1920.0) as usize {
                // big leaves in the background
                let (lx, ly) = (rng.uniform(0.0, w), rng.uniform(h * 0.05, bottom));
                let ln = rng.uniform(90.0, 200.0) * s;
                let ang = rng.uniform(-2.6, -0.5);
                cr.save();
                cr.translate(lx, ly);
                cr.rotate(ang);
                cr.move_to(0.0, 0.0);
                cr.curve_to(ln * 0.3, -ln * 0.28, ln * 0.75, -ln * 0.2, ln, 0.0);
                cr.curve_to(ln * 0.75, ln * 0.2, ln * 0.3, ln * 0.28, 0.0, 0.0);
                cr.restore();
                let shade = rng.uniform(0.6, 1.0);
                cr.set_source_rgba(0.08 * shade, 0.22 * shade, 0.09 * shade, 0.9);
                cr.fill();
            }
            cr.rectangle(0.0, bottom - 40.0 * s, w, 40.0 * s + strip_h); // substrate with moss
            let mut sg = Gradient::linear(0.0, bottom - 40.0 * s, 0.0, h);
            sg.add_color_stop_rgb(0.0, 0.16, 0.11, 0.06);
            sg.add_color_stop_rgb(1.0, 0.07, 0.05, 0.03);
            cr.set_source(&sg);
            cr.fill();
            for _ in 0..(60.0 * w / 1920.0) as usize {
                cr.set_source_rgba(0.2, 0.4, 0.15, rng.uniform(0.4, 0.8));
                let mx = rng.uniform(0.0, w);
                let my = bottom - 40.0 * s + rng.uniform(-6.0, 10.0) * s;
                cr.new_sub_path();
                cr.arc(mx, my, rng.uniform(8.0, 22.0) * s, PI, TAU);
                cr.fill();
            }
            // branches (3), drawn into the background
            let levels = [0.33, 0.55, 0.77];
            for (i, lv) in levels.into_iter().enumerate() {
                let y0 = h * lv;
                let (x0, x1) = if i % 2 == 0 { (-40.0, w * 0.78) } else { (w * 0.22, w + 40.0) };
                let amp = rng.uniform(20.0, 45.0) * s;
                let pts: Vec<(f64, f64)> = (0..61)
                    .map(|k| {
                        let f = k as f64 / 60.0;
                        let x = lerp(x0, x1, f);
                        let tilt = (f - 0.5) * 40.0 * s * if i % 2 == 1 { 1.0 } else { -1.0 };
                        (x, y0 + (f * PI * 1.3 + i as f64).sin() * amp + tilt)
                    })
                    .collect();
                let thick = rng.uniform(22.0, 30.0) * s;
                cr.set_line_cap(LineCap::Round);
                cr.set_line_join(LineJoin::Round);
                for (width, col) in
                    [(thick, (0.22, 0.14, 0.08)), (thick * 0.55, (0.32, 0.22, 0.13)), (thick * 0.15, (0.45, 0.33, 0.2))]
                {
                    cr.move_to(pts[0].0, pts[0].1);
                    for p in &pts[1..] {
                        cr.line_to(p.0, p.1 + (thick - width) * 0.25);
                    }
                    cr.set_rgb(col);
                    cr.set_line_width(width);
                    cr.stroke();
                }
                branches.push(Branch::new(pts, thick));
            }
            for _ in 0..(240.0 * w / 1920.0) as usize {
                // condensation drops on the glass
                cr.set_source_rgba(0.8, 1.0, 0.9, rng.uniform(0.05, 0.18));
                let (dx, dy) = (rng.uniform(0.0, w), rng.uniform(0.0, h));
                cr.circle(dx, dy, rng.uniform(1.0, 3.5) * s);
                cr.fill();
            }
        }));
        // the branch's top surface is what the chameleons walk on
        self.branches = branches
            .into_iter()
            .map(|mut b| {
                for p in &mut b.pts {
                    p.1 -= b.thick * 0.45;
                }
                Rc::new(b)
            })
            .collect();
        self.vines = (0..(9.0 * w / 1920.0) as usize)
            .map(|_| Vine { x: rng.uniform(0.0, w), len: rng.uniform(0.15, 0.4) * h, ph: rng.uniform(0.0, TAU) })
            .collect();
        self.flies = (0..5)
            .map(|_| Fly { x: rng.uniform(0.0, w), y: rng.uniform(h * 0.15, h * 0.8), ph: rng.uniform(0.0, TAU), vx: 0.0, vy: 0.0 })
            .collect();
        self.assign.clear(); // re-assign on the new layout
    }

    fn update_world(&mut self, env: &mut Env, actors: &mut [Chameleon], dt: f64) {
        let (s, t) = (env.s, env.t);
        for f in &mut self.flies {
            f.vx += (t * 3.1 + f.ph).sin() * 300.0 * s * dt;
            f.vy += (t * 2.7 + f.ph * 2.0).cos() * 300.0 * s * dt;
            f.vx *= 0.95;
            f.vy *= 0.95;
            f.x = pymod(f.x + f.vx * dt, env.w);
            f.y = clamp(f.y + f.vy * dt, env.h * 0.1, env.h * 0.85);
        }
        // forget the chameleons that are gone
        self.assign.retain(|e| actors.iter().any(|a| a.b.index == e.0));
    }

    fn draw_world(&mut self, cr: &mut Canvas, env: &Env, actors: &[Chameleon], t: f64) {
        let s = env.s;
        if let Some(bg) = &self.bg {
            cr.picture(bg, 0.0, 0.0, 1.0);
        }
        for f in &self.flies {
            cr.set_source_rgba(0.05, 0.05, 0.05, 0.8);
            cr.circle(f.x, f.y, 2.5 * s);
            cr.fill();
            let wing = (t * 40.0 + f.ph).sin().abs();
            cr.set_source_rgba(0.9, 0.95, 1.0, 0.35);
            cr.ellipse(f.x, f.y - 2.0 * s, 4.0 * s, (1.0 + 2.0 * wing) * s);
            cr.fill();
        }
        let mut order: Vec<&Chameleon> = actors.iter().collect();
        order.sort_by(|a, b| a.b.y.total_cmp(&b.b.y));
        for a in order {
            a.draw(cr, env, self, t);
        }
        cr.set_line_cap(LineCap::Round); // hanging vines in front
        for v in &self.vines {
            cr.move_to(v.x, 0.0);
            for k in 1..11 {
                let f = k as f64 / 10.0;
                cr.line_to(v.x + (t * 0.5 + v.ph + f * 2.0).sin() * f * 18.0 * s, f * v.len);
            }
            cr.set_source_rgba(0.12, 0.30, 0.10, 0.9);
            cr.set_line_width(4.0 * s);
            cr.stroke();
            for k in (2..10).step_by(2) {
                let f = k as f64 / 10.0;
                let lx = v.x + (t * 0.5 + v.ph + f * 2.0).sin() * f * 18.0 * s;
                cr.set_source_rgba(0.16, 0.42, 0.14, 0.9);
                cr.save();
                cr.translate(lx, f * v.len);
                cr.rotate(if k % 4 != 0 { 0.8 } else { -0.8 });
                cr.scale(14.0 * s, 6.0 * s);
                cr.new_sub_path();
                cr.arc(0.8, 0.0, 1.0, 0.0, TAU);
                cr.restore();
                cr.fill();
            }
        }
        cr.set_line_cap(LineCap::Butt);
    }
}
