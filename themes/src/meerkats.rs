//! Meerkats theme: a savanna at sunset with burrows.
//!
//! working = scurries on all fours and digs; every tool call throws up sand with the tool name
//! idle    = sits half upright and basks, now and then wanders
//! waiting = stands upright as the sentinel at the front, turns its head, pulses
//! rest    = only the head peeks out of a burrow, blinking
//! The fur stays natural; the state colour shows as a rim light, a glow on the sand and a tint.
use vivarium_core::prelude::*;

const FUR: Rgb = (0.80, 0.64, 0.44);
const DARK: Rgb = (0.22, 0.15, 0.10);

#[derive(Clone)]
pub struct Meerkat {
    b: Base,
    upright: f64,
    dig: f64,
    head_turn: f64,
    spray_acc: f64,
    blink_at: f64,
}

struct Sand {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    age: f64,
    life: f64,
}

struct Bird {
    cx: f64,
    cy: f64,
    r: f64,
    ph: f64,
    sp: f64,
}

pub struct Meerkats {
    bg: Option<Picture>,
    sand: Vec<Sand>,
    tufts: Vec<(f64, f64, f64)>,
    birds: Vec<Bird>,
}

impl ActorT<Meerkats> for Meerkat {
    const BASE_RADIUS: f64 = 60.0;
    const WALKER: bool = true;

    fn new(base: Base, env: &Env, _world: &mut Meerkats) -> Self {
        Meerkat { b: base, upright: 0.0, dig: 0.0, head_turn: 0.0, spray_acc: 0.0, blink_at: env.rng.uniform(1.0, 4.0) }
    }

    fn base(&self) -> &Base {
        &self.b
    }

    fn base_mut(&mut self) -> &mut Base {
        &mut self.b
    }

    fn companion_reset(&mut self) {
        self.dig = 0.0;
        self.upright = 0.0;
    }

    fn mv(&mut self, env: &mut Env, world: &mut Meerkats, others: &[Other], dt: f64, t: f64, target: Option<(f64, f64)>) {
        let st = self.b.state();
        // scurry, stop and dig, scurry...
        self.dig = if st == State::Working { clamp((t * 0.5 + self.b.seed).sin() * 3.0 - 1.5, 0.0, 1.0) } else { 0.0 };
        self.b.walker_move(env, others, dt, t, target, [95.0, 25.0, 50.0, 0.0], [1.0, 0.5, 0.5, 0.1]);
        if self.dig > 0.5 {
            self.b.x -= self.b.vx * dt * 0.9; // (almost) stands still while digging
            self.b.y -= self.b.vy * dt * 0.9;
        }
        let mut want = match st {
            State::Waiting => 1.0,
            State::Idle => 0.62,
            State::Blocked => 0.4,
            _ => 0.0,
        };
        if st == State::Idle && self.b.moving > 0.3 {
            want = 0.0;
        }
        self.upright = lerp(self.upright, want, (dt * 2.5).min(1.0));
        let waiting = st == State::Waiting;
        self.head_turn = (t * if waiting { 1.6 } else { 0.5 } + self.b.seed).sin() * if waiting { 1.0 } else { 0.4 };
        if self.dig > 0.5 && !self.b.dying {
            // sand spray behind
            self.spray_acc += dt;
            while self.spray_acc > 0.03 {
                self.spray_acc -= 0.03;
                let side = if self.b.face >= 0.0 { -1.0 } else { 1.0 };
                world.sand.push(Sand {
                    x: self.b.x + side * self.b.radius * 0.5,
                    y: self.b.y + self.b.radius * 0.6,
                    vx: side * env.rng.uniform(60.0, 160.0) * env.s,
                    vy: -env.rng.uniform(60.0, 180.0) * env.s,
                    age: 0.0,
                    life: env.rng.uniform(0.6, 1.1),
                });
            }
        }
        if t > self.blink_at + 0.2 {
            self.blink_at = t + env.rng.uniform(2.0, 6.0);
        }
    }

    fn draw(&self, cr: &mut Canvas, env: &Env, _world: &Meerkats, t: f64) {
        let s = env.s;
        let b = &self.b;
        let a = b.alpha * b.flicker;
        if a < 0.01 {
            return;
        }
        let (r, c, st) = (b.radius, b.color, b.state());
        if st == State::Rest {
            self.draw_peeking(cr, t, r, a);
            return;
        }
        let gy = b.y + r * 0.95;
        let mut g = Gradient::radial(b.x, gy, 0.0, b.x, gy, r * 1.8);
        g.stop(0.0, c, if st != State::Idle { 0.45 } else { 0.25 } * a);
        g.stop(1.0, c, 0.0);
        cr.save();
        cr.translate(b.x, gy);
        cr.scale(1.0, 0.3);
        cr.translate(-b.x, -gy);
        cr.set_source(&g);
        cr.arc(b.x, gy, r * 1.8, 0.0, TAU);
        cr.fill();
        cr.restore();
        cr.ellipse(b.x, gy, r * 0.8, r * 0.14);
        cr.set_source_rgba(0.0, 0.0, 0.0, 0.3 * a);
        cr.fill();
        if st == State::Waiting {
            cr.pulse_rings(b.x, b.y - r * 0.3, r, c, a, t, s);
        }
        cr.save();
        cr.translate(b.x, b.y);
        cr.scale(if b.face.abs() > 0.15 { b.face } else { 0.15 }, 1.0);
        let fur = lerp3(FUR, c, 0.18);
        self.shape(cr, t, r, 3.0 * s, c, 0.85 * a, false, fur, a); // rim light
        self.shape(cr, t, r, 0.0, fur, a, true, fur, a);
        cr.restore();
    }
}

impl Meerkat {
    /// The meerkat in local coordinates (feet at y = +0.95 R). upright: 0 = on all fours, 1 = standing.
    #[allow(clippy::too_many_arguments)]
    fn shape(&self, cr: &mut Canvas, t: f64, r: f64, extra: f64, col: Rgb, col_a: f64, details: bool, fur: Rgb, a: f64) {
        let u = self.upright;
        cr.set_rgba(col, col_a);
        cr.set_line_cap(LineCap::Round);
        cr.set_line_join(LineJoin::Round);
        // body axis from hips to shoulders: horizontal when walking, vertical when standing
        let hip = (lerp(-r * 0.45, -r * 0.05, u), lerp(r * 0.45, r * 0.62, u));
        let ang = lerp(-0.12, -PI / 2.0 + 0.08, u);
        let blen = r * lerp(0.95, 1.05, u);
        let sh = (hip.0 + ang.cos() * blen, hip.1 + ang.sin() * blen);
        // tail
        cr.set_line_width(r * 0.11 + extra);
        cr.move_to(hip.0, hip.1);
        cr.curve_to(
            hip.0 - r * 0.4,
            hip.1 + r * 0.1,
            hip.0 - r * lerp(0.9, 0.55, u),
            hip.1 + r * lerp(0.2, 0.35, u),
            hip.0 - r * lerp(1.15, 0.75, u),
            hip.1 + r * lerp(0.05, 0.33, u),
        );
        cr.stroke();
        // legs
        cr.set_line_width(r * 0.11 + extra);
        let step = self.b.step;
        if u < 0.5 {
            for (i, base) in [hip, sh].iter().enumerate() {
                for k in 0..2 {
                    let mut sw = (step + if (i + k) % 2 == 1 { PI } else { 0.0 }).sin() * 0.5 * self.b.moving;
                    if self.dig > 0.5 && i == 1 {
                        sw = (t * 22.0 + k as f64 * PI).sin() * 0.6; // fast digging front paws
                    }
                    cr.move_to(base.0, base.1);
                    cr.line_to(base.0 + sw.sin() * r * 0.4, r * 0.95);
                }
            }
            cr.stroke();
        } else {
            // standing: bent hind legs, front paws hanging in front of the chest
            cr.move_to(hip.0 + r * 0.05, hip.1);
            cr.line_to(hip.0 + r * 0.3, r * 0.8);
            cr.line_to(hip.0 + r * 0.12, r * 0.95);
            cr.stroke();
            let mid = (lerp(hip.0, sh.0, 0.75) + r * 0.12, lerp(hip.1, sh.1, 0.75));
            cr.set_line_width(r * 0.08 + extra);
            cr.move_to(mid.0, mid.1);
            cr.line_to(mid.0 + r * 0.18, mid.1 + r * 0.22);
            cr.line_to(mid.0 + r * 0.12, mid.1 + r * 0.3);
            cr.stroke();
        }
        // body
        cr.set_line_width(r * 0.5 + extra * 2.0);
        cr.move_to(hip.0, hip.1);
        cr.line_to(sh.0, sh.1);
        cr.stroke();
        // head
        let hx = sh.0 + ang.cos() * r * 0.2 + r * lerp(0.15, 0.05, u);
        let hy = sh.1 + ang.sin() * r * 0.2 - r * lerp(0.05, 0.08, u);
        cr.save();
        cr.translate(hx, hy);
        cr.rotate(lerp(0.15, 0.0, u) + self.head_turn * 0.15);
        cr.ellipse(0.0, 0.0, r * 0.27 + extra, r * 0.21 + extra);
        cr.fill();
        cr.move_to(r * 0.1, -r * 0.14 - extra); // pointed snout
        cr.line_to(r * 0.48 + extra, r * 0.02);
        cr.line_to(r * 0.1, r * 0.16 + extra);
        cr.close_path();
        cr.fill();
        if !details {
            cr.restore();
            return;
        }
        // details only on the real pass
        cr.set_rgba(DARK, a);
        cr.circle(r * 0.46, r * 0.02, r * 0.05); // nose
        cr.fill();
        cr.ellipse(r * 0.1, -r * 0.03, r * 0.11, r * 0.08); // dark eye patch
        cr.fill();
        cr.circle(-r * 0.14, -r * 0.15, r * 0.07); // ear
        cr.fill();
        let blink = self.blink_at < t && t < self.blink_at + 0.2;
        if !blink {
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.85 * a);
            cr.circle(r * 0.13, -r * 0.04, r * 0.03);
            cr.fill();
        }
        cr.restore();
        // darker back stripes and tail tip
        cr.set_rgba(DARK, 0.25 * a);
        for k in 0..3 {
            let f = 0.25 + k as f64 * 0.22;
            let (px, py) = (lerp(hip.0, sh.0, f), lerp(hip.1, sh.1, f));
            cr.save();
            cr.translate(px, py);
            cr.rotate(ang + PI / 2.0);
            cr.scale(r * 0.04, r * 0.2);
            cr.new_sub_path();
            cr.arc(0.0, -1.0, 1.0, 0.0, TAU);
            cr.restore();
            cr.fill();
        }
        let tip = (hip.0 - r * lerp(1.15, 0.75, u), hip.1 + r * lerp(0.05, 0.33, u));
        cr.set_rgba(DARK, a);
        cr.circle(tip.0, tip.1, r * 0.07);
        cr.fill();
        let belly = lighten(fur, 0.35); // light chest
        cr.set_rgba(belly, 0.55 * a);
        cr.set_line_width(r * 0.18);
        cr.move_to(lerp(hip.0, sh.0, 0.2) + r * 0.12 * u, lerp(hip.1, sh.1, 0.2) + r * 0.1 * (1.0 - u));
        cr.line_to(lerp(hip.0, sh.0, 0.85) + r * 0.12 * u, lerp(hip.1, sh.1, 0.85) + r * 0.1 * (1.0 - u));
        cr.stroke();
    }

    /// Resting: only the head looks out of a burrow, blinking.
    fn draw_peeking(&self, cr: &mut Canvas, t: f64, r: f64, a: f64) {
        let (x, y) = (self.b.x, self.b.y + r * 0.6);
        cr.set_source_rgba(0.62, 0.46, 0.28, a); // mound
        cr.save();
        cr.translate(x, y + r * 0.15);
        cr.scale(r * 1.6, r * 0.45);
        cr.new_sub_path();
        cr.arc(0.0, 0.0, 1.0, PI, TAU);
        cr.restore();
        cr.fill();
        cr.set_source_rgba(0.08, 0.05, 0.03, a); // hole
        cr.ellipse(x, y, r * 0.6, r * 0.22);
        cr.fill();
        let bob = (t * 0.6 + self.b.seed).sin() * r * 0.05;
        let hy = y - r * 0.18 + bob;
        cr.save();
        cr.rectangle(x - r, hy - r, 2.0 * r, (y + r * 0.05) - (hy - r));
        cr.clip();
        cr.set_rgba(FUR, a);
        cr.ellipse(x, hy, r * 0.32, r * 0.36);
        cr.fill();
        cr.set_rgba(DARK, a);
        let blink = self.blink_at < t && t < self.blink_at + 0.25;
        for side in [-1.0, 1.0] {
            cr.ellipse(x + side * r * 0.13, hy - r * 0.06, r * 0.09, r * if blink { 0.02 } else { 0.08 });
            cr.fill();
            cr.circle(x + side * r * 0.28, hy - r * 0.26, r * 0.07);
            cr.fill();
        }
        cr.circle(x, hy + r * 0.12, r * 0.05);
        cr.fill();
        cr.restore();
    }
}

impl Meerkats {
    fn tuft(&self, cr: &mut Canvas, env: &Env, x: f64, y: f64, ph: f64, t: f64) {
        let s = env.s;
        let k = 0.6 + 0.6 * (y - env.ground_top) / (env.h - env.ground_top).max(1.0);
        cr.set_line_width(2.0 * s * k);
        for i in -2..=2 {
            let fi = i as f64;
            let sway = (t * 1.1 + ph).sin() * 4.0 * s * k;
            cr.move_to(x + fi * 3.0 * s * k, y);
            cr.line_to(x + fi * 8.0 * s * k + sway, y - (18.0 + 6.0 * fi.abs()) * s * k);
        }
        cr.set_source_rgba(0.55, 0.45, 0.20, 0.9);
        cr.stroke();
    }
}

impl Theme for Meerkats {
    type Actor = Meerkat;
    const NAME: &'static str = "meerkats";
    const LABEL_BG: Rgb = (0.10, 0.06, 0.03);
    const EMPTY_COLOR: Rgb = (1.0, 0.92, 0.75);
    const WAITING_ROW: f64 = 0.74;

    fn new(_env: &Env) -> Self {
        Meerkats { bg: None, sand: Vec::new(), tufts: Vec::new(), birds: Vec::new() }
    }

    fn spawn_point(&self, env: &Env) -> (f64, f64) {
        (env.rng.uniform(env.w * 0.15, env.w * 0.85), env.rng.uniform(env.ground_top + 40.0, env.h * 0.85))
    }

    fn on_tool(&mut self, env: &mut Env, actor: &mut Meerkat, name: &str, index: usize) {
        actor.dig = 1.0;
        let b = &actor.b;
        env.spawn_floater(b.x, b.y - b.radius * 1.1 - index as f64 * 26.0 * env.s, name, b.color, FloatStyle::Text, 40.0, 3.5);
    }

    fn setup_world(&mut self, env: &mut Env) {
        let (w, h, s) = (env.w, env.h, env.s);
        let gt = h * 0.6;
        env.ground_top = gt;
        let hz = h * 0.52;
        let rng = &env.rng;
        let strip_h = env.strip_h;
        self.bg = Some(Picture::record(w, h, |cr| {
            let mut g = Gradient::linear(0.0, 0.0, 0.0, hz);
            g.add_color_stop_rgb(0.0, 0.22, 0.24, 0.42);
            g.add_color_stop_rgb(0.55, 0.75, 0.48, 0.42);
            g.add_color_stop_rgb(1.0, 1.0, 0.70, 0.38);
            cr.set_source(&g);
            cr.rectangle(0.0, 0.0, w, h);
            cr.fill();
            let (sx, sy, sr) = (w * 0.3, hz - h * 0.02, h * 0.09); // big low sun
            let mut sg = Gradient::radial(sx, sy, sr, sx, sy, sr * 4.0);
            sg.add_color_stop_rgba(0.0, 1.0, 0.8, 0.5, 0.5);
            sg.add_color_stop_rgba(1.0, 1.0, 0.8, 0.5, 0.0);
            cr.set_source(&sg);
            cr.rectangle(0.0, 0.0, w, h);
            cr.fill();
            cr.set_source_rgb(1.0, 0.88, 0.62);
            cr.circle(sx, sy, sr);
            cr.fill();
            // distant hills and acacia trees
            cr.move_to(0.0, hz + 4.0 * s);
            let mut xi = 0.0;
            while xi < w + 9.0 {
                cr.line_to(xi, hz - (18.0 + 14.0 * (xi / w * 7.0 + 1.0).sin() + 8.0 * (xi / w * 19.0).sin()) * s);
                xi += 8.0;
            }
            cr.line_to(w, hz + 4.0 * s);
            cr.close_path();
            cr.set_source_rgb(0.45, 0.30, 0.30);
            cr.fill();
            for _ in 0..(5.0 * w / 1920.0) as usize {
                let tx = rng.uniform(0.05, 0.95) * w;
                let th = rng.uniform(60.0, 130.0) * s;
                cr.set_source_rgb(0.18, 0.11, 0.10);
                cr.set_line_width(5.0 * s);
                cr.move_to(tx, hz);
                cr.line_to(tx + th * 0.1, hz - th);
                cr.stroke();
                cr.ellipse(tx + th * 0.1, hz - th, th * 0.9, th * 0.18); // flat umbrella crown
                cr.fill();
            }
            // sand
            cr.move_to(0.0, hz);
            let mut xi = 0.0;
            while xi < w + 9.0 {
                cr.line_to(xi, hz + (6.0 + 5.0 * (xi / w * 5.0 + 2.0).sin()) * s);
                xi += 8.0;
            }
            cr.line_to(w, h);
            cr.line_to(0.0, h);
            cr.close_path();
            let mut ag = Gradient::linear(0.0, hz, 0.0, h);
            ag.add_color_stop_rgb(0.0, 0.78, 0.55, 0.34);
            ag.add_color_stop_rgb(1.0, 0.45, 0.30, 0.17);
            cr.set_source(&ag);
            cr.fill();
            for _ in 0..(500.0 * w / 1920.0) as usize {
                // pebbles
                cr.set_source_rgba(0.35, 0.24, 0.14, rng.uniform(0.2, 0.5));
                cr.circle(rng.uniform(0.0, w), rng.uniform(gt - 20.0 * s, h), rng.uniform(1.0, 3.0) * s);
                cr.fill();
            }
            for _ in 0..(4.0 * w / 1920.0) as usize {
                // burrows
                let (bx, by) = (rng.uniform(0.1, 0.9) * w, rng.uniform(gt + 20.0 * s, h - strip_h - 40.0 * s));
                cr.set_source_rgba(0.62, 0.44, 0.27, 1.0);
                cr.save();
                cr.translate(bx, by + 6.0 * s);
                cr.scale(70.0 * s, 22.0 * s);
                cr.new_sub_path();
                cr.arc(0.0, 0.0, 1.0, PI, TAU);
                cr.restore();
                cr.fill();
                cr.set_source_rgba(0.1, 0.06, 0.03, 1.0);
                cr.ellipse(bx, by, 26.0 * s, 9.0 * s);
                cr.fill();
            }
        }));
        self.tufts = (0..(70.0 * w / 1920.0) as usize)
            .map(|_| (rng.uniform(0.0, w), rng.uniform(gt, h), rng.uniform(0.0, TAU)))
            .collect();
        self.tufts.sort_by(|a, b| a.1.total_cmp(&b.1));
        self.birds = (0..2)
            .map(|_| Bird {
                cx: rng.uniform(0.2, 0.8) * w,
                cy: rng.uniform(0.12, 0.3) * h,
                r: rng.uniform(40.0, 90.0) * s,
                ph: rng.uniform(0.0, TAU),
                sp: rng.uniform(0.2, 0.4),
            })
            .collect();
    }

    fn update_world(&mut self, env: &mut Env, _actors: &mut [Meerkat], dt: f64) {
        let g = 400.0 * env.s;
        for p in &mut self.sand {
            p.age += dt;
            p.vy += g * dt;
            p.x += p.vx * dt;
            p.y += p.vy * dt;
        }
        self.sand.retain(|p| p.age < p.life);
    }

    fn draw_world(&mut self, cr: &mut Canvas, env: &Env, actors: &[Meerkat], t: f64) {
        let s = env.s;
        if let Some(bg) = &self.bg {
            cr.picture(bg, 0.0, 0.0, 1.0);
        }
        cr.set_source_rgba(0.12, 0.08, 0.08, 0.8); // vultures circling far away
        cr.set_line_width(2.0 * s);
        for b in &self.birds {
            let ang = t * b.sp + b.ph;
            let (bx, by) = (b.cx + ang.cos() * b.r * 2.0, b.cy + ang.sin() * b.r * 0.5);
            let flap = (t * 2.0 + b.ph).sin() * 3.0 * s;
            cr.move_to(bx - 12.0 * s, by - flap);
            cr.curve_to(bx - 6.0 * s, by - 6.0 * s, bx - 2.0 * s, by - 3.0 * s, bx, by);
            cr.curve_to(bx + 2.0 * s, by - 3.0 * s, bx + 6.0 * s, by - 6.0 * s, bx + 12.0 * s, by - flap);
            cr.stroke();
        }
        let mut ti = 0;
        for a in sorted_actors(actors) {
            while ti < self.tufts.len() && self.tufts[ti].1 < a.b.y + a.b.radius {
                let (x, y, ph) = self.tufts[ti];
                self.tuft(cr, env, x, y, ph, t);
                ti += 1;
            }
            a.draw(cr, env, self, t);
        }
        while ti < self.tufts.len() {
            let (x, y, ph) = self.tufts[ti];
            self.tuft(cr, env, x, y, ph, t);
            ti += 1;
        }
        for p in &self.sand {
            let f = p.age / p.life;
            cr.set_source_rgba(0.85, 0.65, 0.40, (1.0 - f) * 0.9);
            cr.circle(p.x, p.y, 2.2 * s);
            cr.fill();
        }
    }
}
