//! Seabed theme: every session is a crab scuttling sideways over the sand.
//!
//! working = scuttles quickly, claws snapping; each tool call snaps hard and releases a labelled bubble
//! idle    = sits, blinks, wanders now and then     waiting = comes to the front and waves both claws
//! The shell has the state colour.
use vivarium_core::prelude::*;

#[derive(Clone)]
pub struct Crab {
    b: Base,
    snap: f64,
    blink_at: f64,
    bubble_at: f64,
}

struct Caustic {
    x: f64,
    y: f64,
    r: f64,
    ph: f64,
    sp: f64,
}

struct Anemone {
    x: f64,
    y: f64,
    n: i64,
    len: f64,
    ph: f64,
    c: Rgb,
}

/// Marine snow: x, y, z (depth 0.2..1), phase.
struct Snow {
    x: f64,
    y: f64,
    z: f64,
    ph: f64,
}

pub struct Seabed {
    bg: Option<Picture>,
    caustics: Vec<Caustic>,
    snow: Vec<Snow>,
    anemones: Vec<Anemone>,
    next_bubble: f64,
}

impl ActorT<Seabed> for Crab {
    const BASE_RADIUS: f64 = 46.0;
    const WALKER: bool = true;

    fn new(base: Base, env: &Env, _world: &mut Seabed) -> Self {
        Crab { b: base, snap: 0.0, blink_at: env.rng.uniform(2.0, 6.0), bubble_at: env.rng.uniform(1.0, 4.0) }
    }

    fn base(&self) -> &Base {
        &self.b
    }

    fn base_mut(&mut self) -> &mut Base {
        &mut self.b
    }

    fn mv(&mut self, env: &mut Env, _world: &mut Seabed, others: &[Other], dt: f64, t: f64, target: Option<(f64, f64)>) {
        self.b.walker_move(env, others, dt, t, target, [85.0, 22.0, 50.0, 0.0], [1.0, 0.5, 0.5, 0.1]);
        self.b.vy *= 0.9; // crabs walk sideways
        self.snap = (self.snap - dt * 3.0).max(0.0);
        if t > self.blink_at + 0.25 {
            self.blink_at = t + env.rng.uniform(2.0, 7.0);
        }
        if self.b.state() == State::Rest && !self.b.dying && t > self.bubble_at {
            // a sleepy bubble now and then
            self.bubble_at = t + env.rng.uniform(3.0, 8.0);
            let (x, y, r) = (self.b.x, self.b.y, self.b.radius);
            env.spawn_floater(x + r * 0.3, y - r * 0.7, "", (1.0, 1.0, 1.0), FloatStyle::Bubble, 30.0, 9.0);
        }
    }

    fn draw(&self, cr: &mut Canvas, env: &Env, _world: &Seabed, t: f64) {
        let s = env.s;
        let b = &self.b;
        let a = b.alpha * b.flicker;
        if a < 0.01 {
            return;
        }
        let (r, c, st) = (b.radius, b.color, b.state());
        let (x, y) = (b.x, b.y + (b.step * 2.0).sin() * r * 0.03 * b.moving);
        let dark = (c.0 * 0.45, c.1 * 0.45, c.2 * 0.45);
        if st == State::Rest {
            self.draw_buried(cr, s, t, b.x, b.y, r, c, a, dark);
            return;
        }
        // glow + shadow on the sand
        let mut g = Gradient::radial(x, y + r * 0.75, 0.0, x, y + r * 0.75, r * 2.0);
        g.stop(0.0, c, if st != State::Idle { 0.4 } else { 0.22 } * a);
        g.stop(1.0, c, 0.0);
        cr.save();
        cr.translate(x, y + r * 0.75);
        cr.scale(1.0, 0.35);
        cr.translate(-x, -(y + r * 0.75));
        cr.set_source(&g);
        cr.new_sub_path();
        cr.arc(x, y + r * 0.75, r * 2.0, 0.0, TAU);
        cr.fill();
        cr.restore();
        cr.ellipse(x, y + r * 0.72, r * 1.3, r * 0.2);
        cr.set_source_rgba(0.0, 0.0, 0.0, 0.3 * a);
        cr.fill();
        if st == State::Waiting {
            cr.pulse_rings(x, y, r, c, a, t, s);
        }

        cr.set_line_cap(LineCap::Round);
        cr.set_line_join(LineJoin::Round);
        // legs: 4 per side, alternating gait
        cr.set_rgba(dark, a);
        cr.set_line_width(r * 0.11);
        for side in [-1.0, 1.0] {
            for i in 0..4 {
                let fi = i as f64;
                let gait = (b.step + fi * PI / 2.0 + if side > 0.0 { 0.0 } else { PI }).sin() * b.moving;
                let by = y + r * (-0.05 + fi * 0.16);
                cr.move_to(x + side * r * 0.8, by);
                cr.line_to(x + side * r * (1.3 + 0.05 * fi), by - r * 0.28 - gait * r * 0.12);
                cr.line_to(x + side * r * (1.55 + 0.08 * fi), by + r * 0.55 + gait * r * 0.08);
            }
        }
        cr.stroke();
        // claws
        let wave = st == State::Waiting;
        for side in [-1.0, 1.0] {
            let lift = if wave { 0.55 + 0.25 * (t * 4.0 + side).sin() } else { 0.0 };
            let (ex, ey) = (side * r * 1.1, r * (-0.65 - lift * 0.6));
            let (cx, cy) = (side * r * 1.3, r * (-1.05 - lift * 0.9));
            cr.move_to(x + side * r * 0.65, y - r * 0.2);
            cr.line_to(x + ex, y + ey);
            cr.line_to(x + cx, y + cy);
            cr.set_rgba(dark, a);
            cr.set_line_width(r * 0.16);
            cr.stroke();
            let mut opn = if st == State::Working {
                0.15 + 0.25 * (t * 9.0 + side).sin().max(0.0)
            } else if wave {
                0.35 + 0.2 * (t * 6.0 + side).sin()
            } else {
                0.12 + 0.06 * (t * 0.8 + b.seed + side).sin()
            };
            opn += self.snap * 0.5;
            let base_ang = -PI / 2.0 + side * 0.25;
            pincer(cr, x + cx, y + cy, r, base_ang, opn, side, c, a);
        }
        // carapace
        cr.ellipse(x, y, r, r * 0.62);
        let hi = lighten(c, 0.55);
        let mut sg = Gradient::radial(x - r * 0.25, y - r * 0.35, r * 0.1, x, y, r * 1.1);
        sg.stop(0.0, hi, a);
        sg.stop(0.6, c, a);
        sg.stop(1.0, dark, a);
        cr.set_source(&sg);
        cr.fill();
        cr.set_rgba(dark, 0.5 * a); // shell pattern
        for (sx, sy, sr) in [(-0.45, 0.05, 0.08), (0.45, 0.05, 0.08), (-0.2, 0.3, 0.06), (0.2, 0.3, 0.06), (0.0, -0.05, 0.1)] {
            cr.circle(x + sx * r, y + sy * r, sr * r);
        }
        cr.fill();
        // eyes on stalks
        let blink = t < self.blink_at + 0.15 && t > self.blink_at;
        for side in [-1.0, 1.0] {
            let (ex, ey) = (x + side * r * 0.3, y - r * 0.95 + (t * 2.0 + side).sin() * r * 0.03);
            cr.move_to(x + side * r * 0.22, y - r * 0.45);
            cr.line_to(ex, ey);
            cr.set_rgba(dark, a);
            cr.set_line_width(r * 0.08);
            cr.stroke();
            cr.ellipse(ex, ey, r * 0.13, r * 0.13 * if blink { 0.15 } else { 1.0 });
            cr.set_source_rgba(1.0, 1.0, 1.0, a);
            cr.fill();
            if !blink {
                let look = clamp(b.vx / (60.0 * s), -1.0, 1.0) * r * 0.04;
                cr.circle(ex + look, ey + r * 0.02, r * 0.065);
                cr.set_source_rgba(0.0, 0.0, 0.0, a);
                cr.fill();
            }
        }
    }
}

impl Crab {
    /// Resting crab: dug into the sand, only the top of the shell and the eyes show.
    #[allow(clippy::too_many_arguments)]
    fn draw_buried(&self, cr: &mut Canvas, s: f64, t: f64, x: f64, y: f64, r: f64, c: Rgb, a: f64, dark: Rgb) {
        let blink = self.blink_at < t && t < self.blink_at + 0.25;
        let look = (t * 0.4 + self.b.seed).sin() * r * 0.04; // slowly looks left and right
        for side in [-1.0, 1.0] {
            let (ex, ey) = (x + side * r * 0.3, y - r * 0.55);
            cr.move_to(x + side * r * 0.22, y - r * 0.05);
            cr.line_to(ex, ey);
            cr.set_rgba(dark, a);
            cr.set_line_width(r * 0.08);
            cr.stroke();
            cr.ellipse(ex, ey, r * 0.13, r * 0.13 * if blink { 0.15 } else { 1.0 });
            cr.set_source_rgba(1.0, 1.0, 1.0, a);
            cr.fill();
            if !blink {
                cr.circle(ex + look, ey + r * 0.02, r * 0.065);
                cr.set_source_rgba(0.0, 0.0, 0.0, a);
                cr.fill();
            }
        }
        cr.save(); // top of the shell
        cr.translate(x, y + r * 0.2);
        cr.scale(r, r * 0.55);
        cr.new_sub_path();
        cr.arc(0.0, 0.0, 1.0, PI, TAU);
        cr.restore();
        cr.close_path();
        let mut sg = Gradient::linear(x, y - r * 0.35, x, y + r * 0.2);
        let hi = lighten(c, 0.4);
        sg.stop(0.0, hi, a);
        sg.stop(1.0, dark, a);
        cr.set_source(&sg);
        cr.fill();
        // sand mound over the rest of the crab
        cr.ellipse(x, y + r * 0.32, r * 1.6, r * 0.38);
        let mut mg = Gradient::linear(x, y, x, y + r * 0.7);
        mg.add_color_stop_rgba(0.0, 0.42, 0.38, 0.29, a);
        mg.add_color_stop_rgba(1.0, 0.27, 0.24, 0.18, a);
        cr.set_source(&mg);
        cr.fill();
        cr.set_source_rgba(0.6, 0.55, 0.42, 0.5 * a); // a few grains
        for k in 0..9 {
            let fk = k as f64;
            let gx = x + (fk * 2.3 + self.b.seed).sin() * r * 1.2;
            let gy = y + r * (0.18 + 0.2 * (fk * 1.7).cos().abs());
            cr.circle(gx, gy, 1.4 * s);
            cr.fill();
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn pincer(cr: &mut Canvas, px: f64, py: f64, r: f64, ang: f64, opn: f64, side: f64, c: Rgb, a: f64) {
    let hi = lighten(c, 0.35);
    for (length, width, da) in [(r * 0.42, r * 0.17, -opn), (r * 0.32, r * 0.11, opn)] {
        cr.save();
        cr.translate(px, py);
        cr.rotate(ang + da * side);
        cr.scale(length, width);
        cr.new_sub_path();
        cr.arc(0.8, 0.0, 1.0, 0.0, TAU);
        cr.restore();
        let mut g = Gradient::linear(px, py - r * 0.4, px, py + r * 0.2);
        g.stop(0.0, hi, a);
        g.stop(1.0, (c.0 * 0.7, c.1 * 0.7, c.2 * 0.7), a);
        cr.set_source(&g);
        cr.fill();
    }
}

impl Theme for Seabed {
    type Actor = Crab;
    const NAME: &'static str = "seabed";
    const LABEL_BG: Rgb = (0.02, 0.05, 0.07);
    const EMPTY_COLOR: Rgb = (0.8, 0.9, 1.0);
    const WAITING_ROW: f64 = 0.78;

    fn new(_env: &Env) -> Self {
        Seabed { bg: None, caustics: Vec::new(), snow: Vec::new(), anemones: Vec::new(), next_bubble: 0.0 }
    }

    fn spawn_point(&self, env: &Env) -> (f64, f64) {
        (env.rng.uniform(env.w * 0.15, env.w * 0.85), env.rng.uniform(env.ground_top + 40.0, env.h * 0.85))
    }

    fn on_tool(&mut self, env: &mut Env, actor: &mut Crab, name: &str, index: usize) {
        actor.snap = 1.0;
        let b = &actor.b;
        env.spawn_floater(
            b.x + b.radius * 0.8,
            b.y - b.radius * 1.2 - index as f64 * 26.0 * env.s,
            name,
            b.color,
            FloatStyle::Bubble,
            55.0,
            5.0,
        );
    }

    fn setup_world(&mut self, env: &mut Env) {
        let (w, h, s) = (env.w, env.h, env.s);
        let gt = h * 0.55;
        env.ground_top = gt;
        let rng = &env.rng;
        let mut anemones: Vec<Anemone> = Vec::new(); // the rocks of this size only (Python kept the old ones on a resize)
        self.bg = Some(Picture::record(w, h, |cr| {
            let mut g = Gradient::linear(0.0, 0.0, 0.0, gt);
            g.add_color_stop_rgb(0.0, 0.04, 0.24, 0.34);
            g.add_color_stop_rgb(1.0, 0.03, 0.13, 0.20);
            cr.set_source(&g);
            cr.rectangle(0.0, 0.0, w, h);
            cr.fill();

            let sand_y = |x: f64| gt - h * 0.03 - h * 0.015 * (x / w * 5.0 + 1.0).sin() - h * 0.01 * (x / w * 13.0).sin();
            cr.move_to(0.0, h);
            let mut xi = 0.0;
            while xi < w + 9.0 {
                cr.line_to(xi, sand_y(xi));
                xi += 8.0;
            }
            cr.line_to(w, h);
            cr.close_path();
            let mut sg = Gradient::linear(0.0, gt - h * 0.05, 0.0, h);
            sg.add_color_stop_rgb(0.0, 0.26, 0.27, 0.24);
            sg.add_color_stop_rgb(0.4, 0.33, 0.30, 0.23);
            sg.add_color_stop_rgb(1.0, 0.20, 0.17, 0.12);
            cr.set_source(&sg);
            cr.fill();
            cr.set_line_width(1.5 * s); // sand ripples
            for _ in 0..(90.0 * w / 1920.0) as usize {
                let (rx, ry) = (rng.uniform(0.0, w), rng.uniform(gt, h));
                let ln = rng.uniform(40.0, 120.0) * s;
                cr.move_to(rx, ry);
                cr.curve_to(rx + ln * 0.3, ry - 4.0 * s, rx + ln * 0.7, ry + 4.0 * s, rx + ln, ry);
                cr.set_source_rgba(1.0, 0.95, 0.8, rng.uniform(0.04, 0.09));
                cr.stroke();
            }
            for _ in 0..(9.0 * w / 1920.0) as usize {
                // rocks
                let (rx, ry) = (rng.uniform(0.0, w), rng.uniform(gt - h * 0.02, gt + h * 0.1));
                let rr = rng.uniform(30.0, 80.0) * s;
                cr.save();
                cr.translate(rx, ry);
                cr.scale(rr, rr * rng.uniform(0.45, 0.65));
                cr.new_sub_path();
                cr.arc(0.0, 0.0, 1.0, PI, TAU);
                cr.restore();
                cr.close_path();
                let mut rg = Gradient::linear(rx, ry - rr * 0.6, rx, ry);
                rg.add_color_stop_rgb(0.0, 0.22, 0.26, 0.28);
                rg.add_color_stop_rgb(1.0, 0.08, 0.10, 0.11);
                cr.set_source(&rg);
                cr.fill();
                anemones.push(Anemone {
                    x: rx + rng.uniform(-rr * 0.4, rr * 0.4),
                    y: ry - rr * 0.45,
                    n: rng.randint(7, 12),
                    len: rng.uniform(18.0, 34.0) * s,
                    ph: rng.uniform(0.0, TAU),
                    c: *rng.choice(&[(1.0, 0.45, 0.55), (1.0, 0.6, 0.3), (0.7, 0.5, 1.0), (0.4, 0.9, 0.7)]),
                });
            }
            for _ in 0..(30.0 * w / 1920.0) as usize {
                // shells and pebbles
                let (px, py) = (rng.uniform(0.0, w), rng.uniform(gt, h));
                cr.set_source_rgba(0.85, 0.8, 0.7, rng.uniform(0.25, 0.5));
                let (prx, pry) = (rng.uniform(3.0, 7.0) * s, rng.uniform(2.0, 4.0) * s);
                cr.ellipse(px, py, prx, pry);
                cr.fill();
            }
        }));
        self.anemones = anemones;
        self.caustics = (0..(26.0 * w / 1920.0) as usize)
            .map(|_| Caustic {
                x: rng.uniform(0.0, w),
                y: rng.uniform(gt - 20.0 * s, h),
                r: rng.uniform(40.0, 110.0) * s,
                ph: rng.uniform(0.0, TAU),
                sp: rng.uniform(0.2, 0.5),
            })
            .collect();
        self.snow = (0..(110.0 * w / 1920.0) as usize)
            .map(|_| Snow { x: rng.uniform(0.0, w), y: rng.uniform(0.0, h), z: rng.uniform(0.2, 1.0), ph: rng.uniform(0.0, TAU) })
            .collect();
    }

    fn update_world(&mut self, env: &mut Env, _actors: &mut [Crab], dt: f64) {
        let (s, w, h) = (env.s, env.w, env.h);
        for p in &mut self.snow {
            p.x += (env.t * 0.2 + p.ph).sin() * 5.0 * s * dt;
            p.y += p.z * 6.0 * s * dt;
            if p.y > h {
                p.y = -5.0;
                p.x = env.rng.uniform(0.0, w);
            }
        }
        if env.t > self.next_bubble {
            self.next_bubble = env.t + env.rng.uniform(1.5, 4.0);
            if !self.anemones.is_empty() {
                let an = env.rng.choice(&self.anemones);
                env.spawn_floater(an.x, an.y - an.len, "", (1.0, 1.0, 1.0), FloatStyle::Bubble, 40.0, 9.0);
            }
        }
    }

    fn draw_world(&mut self, cr: &mut Canvas, env: &Env, actors: &[Crab], t: f64) {
        let s = env.s;
        if let Some(bg) = &self.bg {
            cr.picture(bg, 0.0, 0.0, 1.0);
        }
        for k in 0..5 {
            // light rays from above
            let fk = k as f64;
            let x0 = env.w * (0.1 + fk * 0.2) + (t * 0.1 + fk).sin() * env.w * 0.04;
            let mut gr = Gradient::linear(0.0, 0.0, 0.0, env.ground_top);
            gr.add_color_stop_rgba(0.0, 0.6, 0.9, 1.0, 0.06);
            gr.add_color_stop_rgba(1.0, 0.6, 0.9, 1.0, 0.0);
            cr.move_to(x0 - 30.0 * s, 0.0);
            cr.line_to(x0 + 30.0 * s, 0.0);
            cr.line_to(x0 + 160.0 * s, env.ground_top);
            cr.line_to(x0 + 20.0 * s, env.ground_top);
            cr.close_path();
            cr.set_source(&gr);
            cr.fill();
        }
        for cst in &self.caustics {
            // dancing light on the sand
            let cx = cst.x + (t * cst.sp + cst.ph).sin() * 30.0 * s;
            let cy = cst.y + (t * cst.sp * 0.8 + cst.ph).cos() * 10.0 * s;
            let r = cst.r * (0.8 + 0.2 * (t * 0.7 + cst.ph).sin());
            let mut g = Gradient::radial(cx, cy, r * 0.2, cx, cy, r);
            g.add_color_stop_rgba(0.0, 0.8, 1.0, 0.95, 0.07);
            g.add_color_stop_rgba(1.0, 0.8, 1.0, 0.95, 0.0);
            cr.save();
            cr.translate(cx, cy);
            cr.scale(1.0, 0.35);
            cr.translate(-cx, -cy);
            cr.set_source(&g);
            cr.new_sub_path();
            cr.arc(cx, cy, r, 0.0, TAU);
            cr.fill();
            cr.restore();
        }
        cr.set_line_cap(LineCap::Round);
        for an in &self.anemones {
            for i in 0..an.n {
                let fi = i as f64;
                let f = fi / (an.n - 1) as f64 - 0.5;
                let sway = (t * 1.4 + an.ph + fi * 0.5).sin() * 8.0 * s;
                cr.move_to(an.x + f * 16.0 * s, an.y);
                cr.curve_to(
                    an.x + f * 30.0 * s,
                    an.y - an.len * 0.5,
                    an.x + f * 40.0 * s + sway,
                    an.y - an.len * 0.8,
                    an.x + f * 46.0 * s + sway,
                    an.y - an.len,
                );
                cr.set_rgba(an.c, 0.75);
                cr.set_line_width(4.0 * s);
                cr.stroke();
            }
        }
        for p in &self.snow {
            cr.set_source_rgba(0.85, 0.95, 1.0, 0.1 + 0.3 * p.z);
            cr.circle(p.x, p.y, (0.6 + 1.2 * p.z) * s);
            cr.fill();
        }
        for a in sorted_actors(actors) {
            a.draw(cr, env, self, t);
        }
    }
}
