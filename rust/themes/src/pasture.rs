//! Pasture theme: every session is a sheep on an evening meadow - one session one sheep, many a flock.
//!
//! working = trots around, hops on every tool call (the tool name floats up)
//! idle    = wanders a bit, stops and grazes      waiting = comes to the front and bleats "Baa!"
//! State colour shows as the sheep's bell, a tint in the wool and a glow on the grass.
use vivarium_core::prelude::*;

const WOOL: Rgb = (0.96, 0.95, 0.91);
const SKIN: Rgb = (0.18, 0.16, 0.15);
const WOOL_BLOBS: [(f64, f64); 9] = [
    (-0.55, -0.05),
    (-0.3, -0.32),
    (0.05, -0.38),
    (0.38, -0.3),
    (0.6, -0.05),
    (0.42, 0.22),
    (0.05, 0.28),
    (-0.35, 0.22),
    (0.0, -0.02),
];

#[derive(Clone)]
pub struct Sheep {
    b: Base,
}

struct Cloud {
    x: f64,
    y: f64,
    sc: f64,
    sp: f64,
    blobs: Vec<(f64, f64, f64)>,
}

struct Birds {
    x: f64,
    y: f64,
    age: f64,
}

pub struct Pasture {
    bg: Option<Picture>,
    clouds: Vec<Cloud>,
    tufts: Vec<(f64, f64, f64)>,
    birds: Option<Birds>,
    next_birds: f64,
}

impl ActorT<Pasture> for Sheep {
    const BASE_RADIUS: f64 = 54.0;
    const WALKER: bool = true;

    fn new(base: Base, _env: &Env, _world: &mut Pasture) -> Self {
        Sheep { b: base }
    }

    fn base(&self) -> &Base {
        &self.b
    }

    fn base_mut(&mut self) -> &mut Base {
        &mut self.b
    }

    fn mv(&mut self, env: &mut Env, _world: &mut Pasture, others: &[Other], dt: f64, t: f64, target: Option<(f64, f64)>) {
        self.b.walker_move(env, others, dt, t, target, [70.0, 20.0, 45.0, 0.0], [1.0, 0.5, 0.5, 0.1]);
    }

    fn draw(&self, cr: &mut Canvas, env: &Env, _world: &Pasture, t: f64) {
        let s = env.s;
        let b = &self.b;
        let a = b.alpha * b.flicker;
        if a < 0.01 {
            return;
        }
        let (r, c, st) = (b.radius, b.color, b.state());
        let hop = if b.hop > 0.0 { (PI * b.hop).sin() * r * 0.35 } else { 0.0 };
        // glow on the grass + shadow
        let gy = b.y + r * 0.95;
        let mut g = Gradient::radial(b.x, gy, 0.0, b.x, gy, r * 1.6);
        g.stop(0.0, c, if st != State::Idle { 0.45 } else { 0.25 } * a);
        g.stop(1.0, c, 0.0);
        cr.set_source(&g);
        cr.ellipse(b.x, gy, r * 1.6, r * 1.6 * 0.3);
        cr.fill();
        cr.ellipse(b.x, gy, r * 0.95, r * 0.16);
        cr.set_source_rgba(0.0, 0.0, 0.0, 0.25 * a);
        cr.fill();
        if st == State::Waiting {
            cr.pulse_rings(b.x, b.y, r, c, a, t, s);
        }

        let lying = st == State::Rest; // asleep in the grass while no session is running
        cr.save();
        cr.translate(b.x, b.y - hop + if lying { r * 0.4 } else { 0.0 });
        cr.scale(if b.face.abs() > 0.15 { b.face } else { 0.15 }, 1.0);
        // legs (tucked away when lying)
        cr.set_line_cap(LineCap::Round);
        cr.set_line_width(r * 0.13);
        cr.set_rgba(SKIN, a);
        if !lying {
            for (i, lx) in [-0.5, -0.25, 0.3, 0.55].into_iter().enumerate() {
                let sw = (b.step + if i % 2 == 1 { 0.0 } else { PI }).sin() * 0.35 * b.moving;
                cr.move_to(lx * r, r * 0.35);
                cr.line_to(lx * r + sw.sin() * r * 0.55, r * 0.35 + sw.cos() * r * 0.6);
            }
            cr.stroke();
        }
        // wool: darker outline pass, then the fluffy fill
        let wool = lerp3(WOOL, c, 0.22);
        for (grow, col, al) in [(0.05, lerp3(wool, (0.4, 0.4, 0.4), 0.45), 1.0), (0.0, wool, 1.0)] {
            for (bx, by) in WOOL_BLOBS {
                let breathe = 1.0 + 0.03 * (t * 1.5 + bx * 3.0).sin();
                cr.circle(bx * r, by * r, r * (0.36 + grow) * breathe);
            }
            cr.set_rgba(col, al * a);
            cr.fill();
        }
        let mut sh = Gradient::radial(-r * 0.2, -r * 0.4, r * 0.1, 0.0, 0.0, r * 1.1);
        sh.add_color_stop_rgba(0.0, 1.0, 1.0, 1.0, 0.25 * a);
        sh.add_color_stop_rgba(1.0, 0.0, 0.0, 0.0, 0.18 * a);
        for (bx, by) in WOOL_BLOBS {
            cr.circle(bx * r, by * r, r * 0.36);
        }
        cr.set_source(&sh);
        cr.fill();
        // head: up when walking/alert, down in the grass when grazing
        let chew = (t * 9.0).sin() * 0.03 * b.graze;
        let mut hx = r * (0.82 + 0.15 * b.graze);
        let mut hy = r * (-0.25 + 0.95 * b.graze + chew);
        let mut ang = lerp(-0.25, 1.1, b.graze);
        if lying {
            // chin resting on the grass, breathing slowly
            hx = r * 0.9;
            hy = r * (0.22 + 0.02 * (t * 1.2).sin());
            ang = 0.25;
        }
        cr.save();
        cr.translate(hx, hy);
        cr.rotate(ang);
        cr.set_rgba(SKIN, a);
        cr.ellipse(0.35 * r * 0.30, 0.0, r * 0.30, r * 0.19);
        cr.fill();
        let ear = (t * 2.0 + b.seed).sin() * 0.15;
        for side in [-1.0, 1.0] {
            cr.save();
            cr.translate(-r * 0.05, side * r * 0.12);
            cr.rotate(side * (0.9 + ear));
            cr.ellipse(-0.6 * r * 0.18, 0.0, r * 0.18, r * 0.07);
            cr.restore();
            cr.fill();
        }
        cr.set_rgba(WOOL, a); // wool tuft on the head
        cr.circle(-r * 0.05, -r * 0.14, r * 0.15);
        cr.fill();
        if lying {
            // closed eye
            cr.set_source_rgba(0.85, 0.85, 0.85, 0.8 * a);
            cr.set_line_width(r * 0.025);
            cr.new_sub_path();
            cr.arc(r * 0.22, -r * 0.08, r * 0.05, 0.2, PI - 0.2);
            cr.stroke();
        } else {
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.9 * a);
            cr.circle(r * 0.22, -r * 0.06, r * 0.05);
            cr.fill();
        }
        cr.restore();
        // bell in the state colour
        let bc = lighten(c, 0.2);
        cr.set_rgba(bc, a);
        cr.circle(r * 0.62, r * 0.12, r * 0.10);
        cr.fill();
        cr.restore();

        if lying {
            // z z z
            let side = if b.face >= 0.0 { 1.0 } else { -1.0 };
            for k in 0..3 {
                let f = (t * 0.3 + k as f64 / 3.0) % 1.0;
                let lay = text::layout("z", (14.0 + 14.0 * f) * s, weight::BOLD, None);
                let zx = b.x + side * r * (0.9 + 0.6 * f) + (f * 6.0).sin() * 4.0 * s;
                let zy = b.y + r * 0.1 - f * r * 1.6;
                text::show(cr, &lay, zx, zy, (1.0, 1.0, 1.0), 0.8 * (PI * f).sin() * a, 0.4);
            }
        }

        if st == State::Waiting {
            // speech bubble
            let lay = text::layout("Baa!", 20.0 * s, weight::BOLD, None);
            let (tw, th) = lay.get_pixel_size();
            let (bx, by) = (b.x - tw / 2.0 - r * 0.2, b.y - r * 1.55 - th + (t * 4.0).sin() * 3.0 * s);
            let pad = 8.0 * s;
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.92 * a);
            cr.ellipse(bx + tw / 2.0, by + th / 2.0, (tw / 2.0 + pad) * 1.15, th / 2.0 + pad);
            cr.fill();
            cr.move_to(bx + tw / 2.0 - 6.0 * s, by + th + pad * 0.6);
            cr.line_to(bx + tw / 2.0 + 4.0 * s, by + th + pad * 0.6);
            cr.line_to(bx + tw / 2.0 + 8.0 * s, by + th + pad * 2.2);
            cr.close_path();
            cr.fill();
            text::show(cr, &lay, bx, by, (0.55, 0.32, 0.0), a, 0.0);
        }
    }
}

/// Height of a hill line at x.
#[allow(clippy::too_many_arguments)]
fn hill(w: f64, x: f64, base: f64, amp: f64, f1: f64, p1: f64, f2: f64, p2: f64) -> f64 {
    base - amp * (0.6 * (x / w * f1 + p1).sin() + 0.4 * (x / w * f2 + p2).sin())
}

impl Pasture {
    fn draw_tuft(&self, cr: &mut Canvas, env: &Env, x: f64, y: f64, ph: f64, t: f64) {
        let s = env.s;
        let k = 0.6 + 0.6 * (y - env.ground_top) / (env.h - env.ground_top).max(1.0);
        cr.set_line_width(2.0 * s * k);
        for i in [-1.0, 0.0, 1.0] {
            let sway = (t * 1.3 + ph + x * 0.01).sin() * 4.0 * s * k;
            cr.move_to(x + i * 4.0 * s * k, y);
            cr.curve_to(
                x + i * 5.0 * s * k,
                y - 8.0 * s * k,
                x + i * 7.0 * s * k + sway,
                y - 14.0 * s * k,
                x + i * 9.0 * s * k + sway,
                y - 20.0 * s * k,
            );
        }
        cr.set_source_rgba(0.16, 0.36, 0.12, 0.9);
        cr.stroke();
    }
}

impl Theme for Pasture {
    type Actor = Sheep;
    const NAME: &'static str = "pasture";
    const LABEL_BG: Rgb = (0.04, 0.07, 0.04);
    const EMPTY_COLOR: Rgb = (1.0, 0.95, 0.8);
    const WAITING_ROW: f64 = 0.80;

    fn new(_env: &Env) -> Self {
        Pasture { bg: None, clouds: Vec::new(), tufts: Vec::new(), birds: None, next_birds: 12.0 }
    }

    fn spawn_point(&self, env: &Env) -> (f64, f64) {
        (env.rng.uniform(env.w * 0.15, env.w * 0.85), env.rng.uniform(env.ground_top + 40.0, env.h * 0.85))
    }

    fn on_tool(&mut self, env: &mut Env, actor: &mut Sheep, name: &str, index: usize) {
        actor.b.hop = 1.0;
        let b = &actor.b;
        env.spawn_floater(b.x, b.y - b.radius * 1.1 - index as f64 * 26.0 * env.s, name, b.color, FloatStyle::Text, 35.0, 3.5);
    }

    fn setup_world(&mut self, env: &mut Env) {
        let (w, h, s) = (env.w, env.h, env.s);
        let gt = h * 0.58;
        env.ground_top = gt;
        let hz = h * 0.47;
        let rng = &env.rng;
        self.bg = Some(Picture::record(w, h, |cr| {
            let mut g = Gradient::linear(0.0, 0.0, 0.0, hz);
            g.add_color_stop_rgb(0.0, 0.16, 0.22, 0.42);
            g.add_color_stop_rgb(0.55, 0.52, 0.45, 0.58);
            g.add_color_stop_rgb(1.0, 0.95, 0.66, 0.45);
            cr.set_source(&g);
            cr.rectangle(0.0, 0.0, w, h);
            cr.fill();
            let (sx, sy) = (w * 0.76, hz - h * 0.04);
            let mut sg = Gradient::radial(sx, sy, 0.0, sx, sy, h * 0.35);
            sg.add_color_stop_rgba(0.0, 1.0, 0.92, 0.7, 0.9);
            sg.add_color_stop_rgba(0.12, 1.0, 0.85, 0.55, 0.6);
            sg.add_color_stop_rgba(1.0, 1.0, 0.7, 0.4, 0.0);
            cr.set_source(&sg);
            cr.paint();
            let layers = [
                (hz + h * 0.02, h * 0.05, (0.36, 0.42, 0.40), 3.0, 1.0, 9.0, 0.3),
                (hz + h * 0.06, h * 0.045, (0.26, 0.40, 0.26), 4.0, 2.0, 11.0, 1.2),
                (gt - h * 0.01, h * 0.03, (0.20, 0.38, 0.16), 5.0, 0.4, 13.0, 2.0),
            ];
            for (li, (base, amp, col, f1, p1, f2, p2)) in layers.into_iter().enumerate() {
                cr.move_to(0.0, h);
                let mut xi = 0.0;
                while xi < w + 9.0 {
                    cr.line_to(xi, hill(w, xi, base, amp, f1, p1, f2, p2));
                    xi += 8.0;
                }
                cr.line_to(w, h);
                cr.close_path();
                if li == 2 {
                    let mut mg = Gradient::linear(0.0, base - amp, 0.0, h);
                    mg.add_color_stop_rgb(0.0, 0.30, 0.50, 0.20);
                    mg.add_color_stop_rgb(1.0, 0.10, 0.25, 0.08);
                    cr.set_source(&mg);
                } else {
                    cr.set_rgb(col);
                }
                cr.fill();
                if li < 2 {
                    // trees on the far hills
                    for _ in 0..rng.randint(4, 8) {
                        let tx = rng.uniform(0.0, w);
                        let ty = hill(w, tx, base, amp, f1, p1, f2, p2);
                        let tr = rng.uniform(10.0, 22.0) * s * if li == 0 { 0.7 } else { 1.0 };
                        let dark = lerp3(col, (0.05, 0.12, 0.08), 0.5);
                        cr.set_rgb(dark);
                        cr.rectangle(tx - tr * 0.12, ty - tr * 1.2, tr * 0.24, tr * 1.2);
                        cr.fill();
                        cr.circle(tx, ty - tr * 1.5, tr);
                        cr.fill();
                    }
                }
            }
            // fence along the back of the meadow
            let fy = gt - h * 0.005;
            cr.set_source_rgba(0.30, 0.22, 0.15, 0.9);
            let mut xi = (-20.0 * s).trunc();
            let post_step = (110.0 * s).trunc().max(1.0);
            while xi < w + 40.0 {
                cr.rectangle(xi, fy - 46.0 * s, 7.0 * s, 50.0 * s);
                xi += post_step;
            }
            cr.fill();
            cr.set_line_width(4.0 * s);
            for ry in [fy - 38.0 * s, fy - 20.0 * s] {
                cr.move_to(0.0, ry);
                cr.line_to(w, ry);
            }
            cr.stroke();
            let colors: [Rgb; 4] = [(1.0, 1.0, 0.95), (1.0, 0.9, 0.3), (0.95, 0.6, 0.8), (0.75, 0.75, 1.0)];
            for _ in 0..(260.0 * w / 1920.0) as usize {
                // flowers
                let (fx, fy2) = (rng.uniform(0.0, w), rng.uniform(gt + 10.0 * s, h));
                let col = *rng.choice(&colors);
                cr.set_rgba(col, 0.8);
                cr.circle(fx, fy2, rng.uniform(1.5, 3.2) * s * (0.6 + 0.6 * (fy2 - gt) / (h - gt)));
                cr.fill();
            }
        }));
        self.clouds = (0..6)
            .map(|_| Cloud {
                x: rng.uniform(0.0, w),
                y: rng.uniform(h * 0.05, h * 0.3),
                sc: rng.uniform(0.6, 1.3),
                sp: rng.uniform(4.0, 10.0),
                blobs: (0..6).map(|_| (rng.uniform(-1.6, 1.6), rng.uniform(-0.4, 0.3), rng.uniform(0.5, 1.0))).collect(),
            })
            .collect();
        self.tufts = (0..(150.0 * w / 1920.0) as usize)
            .map(|_| (rng.uniform(0.0, w), rng.uniform(gt + 5.0 * s, h), rng.uniform(0.0, TAU)))
            .collect();
        self.tufts.sort_by(|a, b| a.1.total_cmp(&b.1));
    }

    fn update_world(&mut self, env: &mut Env, _actors: &mut [Sheep], dt: f64) {
        let (s, w) = (env.s, env.w);
        for c in &mut self.clouds {
            c.x += c.sp * s * dt;
            if c.x - 200.0 * s * c.sc > w {
                c.x = -200.0 * s * c.sc;
            }
        }
        if env.t > self.next_birds {
            self.next_birds = env.t + env.rng.uniform(20.0, 45.0);
            self.birds = Some(Birds { x: -50.0, y: env.rng.uniform(env.h * 0.08, env.h * 0.3), age: 0.0 });
        }
        if let Some(b) = &mut self.birds {
            b.x += 90.0 * s * dt;
            b.age += dt;
            if b.x > w + 100.0 {
                self.birds = None;
            }
        }
    }

    fn draw_world(&mut self, cr: &mut Canvas, env: &Env, actors: &[Sheep], t: f64) {
        let s = env.s;
        if let Some(bg) = &self.bg {
            cr.picture(bg, 0.0, 0.0, 1.0);
        }
        for c in &self.clouds {
            for &(bx, by, br) in &c.blobs {
                cr.circle(c.x + bx * 60.0 * s * c.sc, c.y + by * 60.0 * s * c.sc, br * 50.0 * s * c.sc);
            }
            cr.set_source_rgba(0.95, 0.80, 0.80, 0.85);
            cr.fill();
        }
        if let Some(b) = &self.birds {
            cr.set_line_width(2.0 * s);
            cr.set_source_rgba(0.15, 0.12, 0.2, 0.7);
            for (i, (ox, oy)) in [(0.0, 0.0), (-30.0, 14.0), (-34.0, -12.0), (-62.0, 6.0), (-66.0, -22.0)].into_iter().enumerate() {
                let (x, y) = (b.x + ox * s, b.y + oy * s);
                let flap = (b.age * 8.0 + i as f64).sin() * 5.0 * s;
                cr.move_to(x - 9.0 * s, y - flap);
                cr.line_to(x, y);
                cr.line_to(x + 9.0 * s, y - flap);
            }
            cr.stroke();
        }
        let mut ti = 0;
        for a in sorted_actors(actors) {
            // tufts and sheep sorted by depth
            while ti < self.tufts.len() && self.tufts[ti].1 < a.b.y + a.b.radius {
                let (x, y, ph) = self.tufts[ti];
                self.draw_tuft(cr, env, x, y, ph, t);
                ti += 1;
            }
            a.draw(cr, env, self, t);
        }
        while ti < self.tufts.len() {
            let (x, y, ph) = self.tufts[ti];
            self.draw_tuft(cr, env, x, y, ph, t);
            ti += 1;
        }
    }
}
