//! Forest theme: every session is a deer in a moonlit forest clearing.
//!
//! Deer are dark silhouettes with a rim light in the state colour.
//! working = walks briskly, fireflies follow it, every tool call sends up a labelled firefly
//! idle    = wanders, stops and grazes      waiting = comes to the front, head up, ears up
use vivarium_core::prelude::*;

const BODY: Rgb = (0.06, 0.05, 0.05);

#[derive(Clone)]
pub struct Deer {
    b: Base,
    antlers: bool,
    fly_acc: f64,
}

struct Firefly {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    age: f64,
    life: f64,
    ph: f64,
}

pub struct Forest {
    bg: Option<Picture>,
    fireflies: Vec<Firefly>,
    /// (x, y, length, phase)
    ferns: Vec<(f64, f64, f64, f64)>,
}

impl ActorT<Forest> for Deer {
    const BASE_RADIUS: f64 = 54.0;
    const WALKER: bool = true;

    fn new(base: Base, env: &Env, _world: &mut Forest) -> Self {
        Deer { b: base, antlers: env.rng.random() < 0.55, fly_acc: 0.0 }
    }

    fn base(&self) -> &Base {
        &self.b
    }

    fn base_mut(&mut self) -> &mut Base {
        &mut self.b
    }

    fn mv(&mut self, env: &mut Env, world: &mut Forest, others: &[Other], dt: f64, t: f64, target: Option<(f64, f64)>) {
        self.b.walker_move(env, others, dt, t, target, [65.0, 18.0, 40.0, 0.0], [1.0, 0.5, 0.5, 0.1]);
        if self.b.state() == State::Working && !self.b.dying {
            self.fly_acc += dt;
            if self.fly_acc > 0.6 {
                self.fly_acc = 0.0;
                let (r, s, rng) = (self.b.radius, env.s, &env.rng);
                world.fireflies.push(Firefly {
                    x: self.b.x + rng.uniform(-r, r),
                    y: self.b.y - rng.uniform(0.0, r * 1.5),
                    vx: rng.uniform(-15.0, 15.0) * s,
                    vy: -rng.uniform(5.0, 20.0) * s,
                    age: 0.0,
                    life: rng.uniform(3.0, 6.0),
                    ph: rng.uniform(0.0, TAU),
                });
            }
        }
    }

    fn draw(&self, cr: &mut Canvas, env: &Env, _world: &Forest, t: f64) {
        let s = env.s;
        let b = &self.b;
        let a = b.alpha * b.flicker;
        if a < 0.01 {
            return;
        }
        let (r, c, st) = (b.radius, b.color, b.state());
        cr.glow(b.x, b.y, r * 2.4, c, if st != State::Idle { 0.3 } else { 0.16 } * a, 0.0);
        if st == State::Waiting {
            cr.pulse_rings(b.x, b.y - r * 0.2, r, c, a, t, s);
        }
        cr.ellipse(b.x, b.y + r * 1.15, r * 1.1, r * 0.12);
        cr.set_source_rgba(0.0, 0.0, 0.0, 0.35 * a);
        cr.fill();
        cr.save();
        cr.translate(b.x, b.y + if st == State::Rest { r * 0.62 } else { 0.0 }); // lying deer sits lower
        cr.scale(if b.face.abs() > 0.15 { b.face } else { 0.15 }, 1.0);
        self.shape(cr, t, r, 3.0 * s, c, 0.85 * a);
        self.shape(cr, t, r, 0.0, BODY, a);
        cr.set_source_rgba(0.95, 0.92, 0.88, 0.85 * a); // white tail
        cr.save();
        cr.translate(-r * 0.95, -r * 0.12);
        cr.rotate(-0.6 + (t * 2.5 + b.seed).sin() * 0.15);
        cr.ellipse(0.0, 0.0, r * 0.12, r * 0.07);
        cr.restore();
        cr.fill();
        cr.restore();
    }
}

impl Deer {
    /// The whole deer in local coordinates; extra > 0 draws a wider copy used as the rim light.
    fn shape(&self, cr: &mut Canvas, t: f64, r: f64, extra: f64, col: Rgb, col_a: f64) {
        let b = &self.b;
        let st = b.state();
        cr.set_rgba(col, col_a);
        cr.set_line_cap(LineCap::Round);
        cr.set_line_join(LineJoin::Round);
        // legs
        let lying = st == State::Rest;
        cr.set_line_width(r * 0.09 + extra);
        if lying {
            // legs folded under the body
            for (x0, x1) in [(0.35, 0.8), (-0.55, -0.1)] {
                cr.move_to(x0 * r, r * 0.2);
                cr.line_to(x1 * r, r * 0.34);
            }
        } else {
            for (i, lx) in [-0.6, -0.42, 0.42, 0.6].into_iter().enumerate() {
                let sw = (b.step + if i == 0 || i == 3 { 0.0 } else { PI }).sin() * 0.4 * b.moving;
                let knee = (lx * r + sw.sin() * r * 0.25, r * 0.65);
                cr.move_to(lx * r, r * 0.1);
                cr.line_to(knee.0, knee.1);
                cr.line_to(knee.0 + (sw * 1.4).sin() * r * 0.2, r * 1.15);
            }
        }
        cr.stroke();
        // body: barrel with rounded chest and rump
        cr.ellipse(0.0, 0.0, r * 0.85, r * 0.34);
        cr.circle(r * 0.52, r * 0.0, r * 0.32);
        cr.circle(-r * 0.58, -r * 0.04, r * 0.30);
        if extra != 0.0 {
            cr.set_line_width(extra * 2.0);
            cr.stroke_preserve();
        }
        cr.fill();
        // neck + head
        let alert = if st == State::Waiting { 1.0 } else { 0.0 };
        let mut hx = r * lerp(0.98, 1.15, b.graze);
        let mut hy = r * lerp(-0.72 - 0.1 * alert, 0.85, b.graze);
        if lying {
            // resting, head up, slowly looking around
            hx = r * 0.9;
            hy = r * (-0.55 + 0.03 * (t * 0.5 + b.seed).sin());
        }
        cr.set_line_width(r * 0.26 + extra);
        cr.move_to(r * 0.58, -r * 0.05);
        cr.line_to(hx - r * 0.06, hy + r * 0.06);
        cr.stroke();
        let ang = lerp(0.35, 1.3, b.graze);
        cr.save();
        cr.translate(hx, hy);
        cr.rotate(ang);
        let (rx, ry) = (r * 0.30 + extra, r * 0.15 + extra);
        cr.ellipse(0.4 * rx, 0.0, rx, ry);
        cr.fill();
        let twitch = (t * 3.0 + b.seed).sin() * 0.15 + alert * 0.3;
        for side in [-1.0, 1.0] {
            // ears
            cr.save();
            cr.translate(-r * 0.05, -r * 0.12);
            cr.rotate(-1.9 + side * 0.35 - twitch * side);
            let (rx, ry) = (r * 0.22 + extra, r * 0.08 + extra);
            cr.ellipse(0.8 * rx, 0.0, rx, ry);
            cr.restore();
            cr.fill();
        }
        if self.antlers {
            cr.set_line_width(r * 0.05 + extra);
            for side in [-1.0, 1.0] {
                let (bx, by) = (-r * 0.02 + side * r * 0.05, -r * 0.15);
                cr.move_to(bx, by);
                let (tipx, tipy) = (bx - r * 0.35 + side * r * 0.1, by - r * 0.6);
                cr.curve_to(bx - r * 0.1, by - r * 0.3, tipx + r * 0.1, tipy + r * 0.2, tipx, tipy);
                cr.move_to(bx - r * 0.12, by - r * 0.3);
                cr.line_to(bx + r * 0.08, by - r * 0.48);
                cr.move_to(bx - r * 0.25, by - r * 0.45);
                cr.line_to(bx - r * 0.08, by - r * 0.65);
            }
            cr.stroke();
        }
        cr.restore();
    }
}

fn pine(cr: &mut Canvas, x: f64, base: f64, height: f64, width: f64) {
    cr.move_to(x, base - height);
    let tiers = 5;
    for k in 0..tiers {
        // jagged right side
        let f = (k + 1) as f64 / tiers as f64;
        let notch = if k < tiers - 1 { height * 0.03 } else { 0.0 };
        cr.line_to(x + width * f * 0.5, base - height * (1.0 - f) - height * 0.02);
        cr.line_to(x + width * f * 0.28, base - height * (1.0 - f) + notch);
    }
    cr.line_to(x + width * 0.06, base);
    cr.line_to(x - width * 0.06, base);
    for k in (0..tiers).rev() {
        let f = (k + 1) as f64 / tiers as f64;
        let notch = if k < tiers - 1 { height * 0.03 } else { 0.0 };
        cr.line_to(x - width * f * 0.28, base - height * (1.0 - f) + notch);
        cr.line_to(x - width * f * 0.5, base - height * (1.0 - f) - height * 0.02);
    }
    cr.close_path();
}

impl Theme for Forest {
    type Actor = Deer;
    const NAME: &'static str = "forest";
    const LABEL_BG: Rgb = (0.02, 0.03, 0.03);
    const EMPTY_COLOR: Rgb = (0.85, 0.95, 0.8);
    const WAITING_ROW: f64 = 0.76;

    fn new(_env: &Env) -> Self {
        Forest { bg: None, fireflies: Vec::new(), ferns: Vec::new() }
    }

    fn spawn_point(&self, env: &Env) -> (f64, f64) {
        (env.rng.uniform(env.w * 0.15, env.w * 0.85), env.rng.uniform(env.ground_top + 40.0, env.h * 0.85))
    }

    fn on_tool(&mut self, env: &mut Env, actor: &mut Deer, name: &str, index: usize) {
        let b = &actor.b;
        env.spawn_floater(b.x, b.y - b.radius * 1.6 - index as f64 * 26.0 * env.s, name, b.color, FloatStyle::Firefly, 30.0, 4.5);
    }

    fn setup_world(&mut self, env: &mut Env) {
        let (w, h, s) = (env.w, env.h, env.s);
        let gt = h * 0.60;
        env.ground_top = gt;
        let rng = &env.rng;
        self.bg = Some(Picture::record(w, h, |cr| {
            let mut g = Gradient::linear(0.0, 0.0, 0.0, gt);
            g.add_color_stop_rgb(0.0, 0.02, 0.03, 0.08);
            g.add_color_stop_rgb(1.0, 0.10, 0.14, 0.22);
            cr.set_source(&g);
            cr.rectangle(0.0, 0.0, w, h);
            cr.fill();
            for _ in 0..(260.0 * w / 1920.0) as usize {
                cr.set_source_rgba(0.9, 0.92, 1.0, rng.uniform(0.1, 0.6));
                let (x, y) = (rng.uniform(0.0, w), rng.uniform(0.0, gt * 0.7));
                cr.circle(x, y, rng.uniform(0.4, 1.2) * s);
                cr.fill();
            }
            let (mx, my, mr) = (w * 0.22, h * 0.15, 42.0 * s); // moon
            let mut mg = Gradient::radial(mx, my, mr, mx, my, mr * 6.0);
            mg.add_color_stop_rgba(0.0, 0.8, 0.85, 1.0, 0.25);
            mg.add_color_stop_rgba(1.0, 0.8, 0.85, 1.0, 0.0);
            cr.set_source(&mg);
            cr.rectangle(0.0, 0.0, w, h);
            cr.fill();
            cr.set_source_rgb(0.93, 0.93, 0.86);
            cr.circle(mx, my, mr);
            cr.fill();
            cr.set_source_rgba(0.75, 0.75, 0.7, 0.5);
            for (ox, oy, rr) in [(-0.3, -0.2, 0.18), (0.25, 0.15, 0.12), (0.05, 0.4, 0.09)] {
                cr.circle(mx + ox * mr, my + oy * mr, rr * mr);
            }
            cr.fill();
            // ground of the clearing
            let mut fg = Gradient::linear(0.0, gt - h * 0.05, 0.0, h);
            fg.add_color_stop_rgb(0.0, 0.09, 0.12, 0.10);
            fg.add_color_stop_rgb(1.0, 0.03, 0.05, 0.03);
            cr.set_source(&fg);
            cr.rectangle(0.0, gt - h * 0.05, w, h);
            cr.fill();
            // tree layers, far to near, with fog between them
            let layers = [(0.20, (0.10, 0.15, 0.20), 34.0), (0.28, (0.06, 0.10, 0.12), 26.0), (0.40, (0.03, 0.05, 0.06), 16.0)];
            for (li, (hh, col, n)) in layers.into_iter().enumerate() {
                let lf = li as f64;
                let base = gt - h * 0.035 + lf * h * 0.012;
                for _ in 0..(n * w / 1920.0) as usize {
                    let x = rng.uniform(-30.0, w + 30.0);
                    if li == 2 && w * 0.22 < x && x < w * 0.78 {
                        continue; // keep the clearing open
                    }
                    let ht = h * hh * rng.uniform(0.7, 1.25);
                    pine(cr, x, base, ht, ht * rng.uniform(0.38, 0.5));
                }
                cr.set_rgb(col);
                cr.fill();
                let mut fog = Gradient::linear(0.0, base - h * 0.12, 0.0, base + h * 0.02);
                fog.add_color_stop_rgba(0.0, 0.6, 0.7, 0.8, 0.0);
                fog.add_color_stop_rgba(1.0, 0.6, 0.7, 0.8, 0.10 - lf * 0.02);
                cr.set_source(&fog);
                cr.rectangle(0.0, base - h * 0.12, w, h * 0.14);
                cr.fill();
            }
            for _ in 0..(60.0 * w / 1920.0) as usize {
                // moss and stones in the clearing
                let (px, py) = (rng.uniform(0.0, w), rng.uniform(gt, h));
                cr.set_source_rgba(0.15, 0.22, 0.14, rng.uniform(0.3, 0.6));
                let (rx, ry) = (rng.uniform(8.0, 30.0) * s, rng.uniform(2.0, 6.0) * s);
                cr.ellipse(px, py, rx, ry);
                cr.fill();
            }
        }));
        self.fireflies = (0..(28.0 * w / 1920.0) as usize)
            .map(|_| Firefly {
                x: rng.uniform(0.0, w),
                y: rng.uniform(gt - h * 0.2, h * 0.9),
                vx: rng.uniform(-10.0, 10.0) * s,
                vy: rng.uniform(-6.0, 6.0) * s,
                age: 0.0,
                life: 1e9,
                ph: rng.uniform(0.0, TAU),
            })
            .collect();
        self.ferns = (0..(26.0 * w / 1920.0) as usize)
            .map(|_| (rng.uniform(0.0, w), h - rng.uniform(0.0, 30.0) * s, rng.uniform(40.0, 90.0) * s, rng.uniform(0.0, TAU)))
            .collect();
    }

    fn update_world(&mut self, env: &mut Env, _actors: &mut [Deer], dt: f64) {
        let (s, w, h, t) = (env.s, env.w, env.h, env.t);
        for ff in &mut self.fireflies {
            ff.age += dt;
            ff.vx += (t * 0.7 + ff.ph).sin() * 6.0 * s * dt;
            ff.vy += (t * 0.9 + ff.ph * 2.0).cos() * 6.0 * s * dt;
            ff.vx *= 0.99;
            ff.vy *= 0.99;
            ff.x = pymod(ff.x + ff.vx * dt, w);
            ff.y = (h * 0.95).min((h * 0.3).max(ff.y + ff.vy * dt));
        }
        self.fireflies.retain(|ff| ff.age < ff.life);
    }

    fn draw_world(&mut self, cr: &mut Canvas, env: &Env, actors: &[Deer], t: f64) {
        let s = env.s;
        if let Some(bg) = &self.bg {
            cr.picture(bg, 0.0, 0.0, 1.0);
        }
        for a in sorted_actors(actors) {
            a.draw(cr, env, self, t);
        }
        for ff in &self.fireflies {
            let fade = ff.age.min(1.0) * (ff.life - ff.age).min(1.0);
            let b = (t * 1.7 + ff.ph * 3.0).sin().max(0.0) * fade;
            if b < 0.05 {
                continue;
            }
            let mut g = Gradient::radial(ff.x, ff.y, 0.0, ff.x, ff.y, 10.0 * s);
            g.add_color_stop_rgba(0.0, 1.0, 0.95, 0.55, 0.9 * b);
            g.add_color_stop_rgba(1.0, 1.0, 0.9, 0.4, 0.0);
            cr.set_source(&g);
            cr.circle(ff.x, ff.y, 10.0 * s);
            cr.fill();
        }
        cr.set_line_cap(LineCap::Round);
        cr.set_source_rgba(0.02, 0.03, 0.02, 1.0);
        for &(x, y, ln, ph) in &self.ferns {
            // foreground ferns
            for k in -3..=3 {
                let ang = -PI / 2.0 + k as f64 * 0.28 + (t * 0.8 + ph).sin() * 0.06;
                cr.move_to(x, y);
                cr.curve_to(
                    x + ang.cos() * ln * 0.4,
                    y + ang.sin() * ln * 0.5,
                    x + ang.cos() * ln * 0.8,
                    y + ang.sin() * ln * 0.8,
                    x + (ang + 0.4 * if k >= 0 { 1.0 } else { -1.0 }).cos() * ln,
                    y + ang.sin() * ln * 0.75,
                );
            }
            cr.set_line_width(5.0 * s);
            cr.stroke();
        }
    }
}
