//! Shared building blocks for all themes.
//!
//! A theme is a type implementing `Theme` (the world) with an actor type implementing `ActorT`
//! (one actor per Claude Code session). `Stage` does the session bookkeeping, state colours, tool-call
//! detection, labels, the "needs your input" banner, the bottom status strip with clock and the wake
//! hint, so every theme reads the same way:
//!   working = teal, idle = blue, waiting ("needs you") = amber, blocked = red, background = violet.
//!
//! Theme hooks:  Theme::setup_world / update_world / draw_world / on_tool,  ActorT::mv / draw
use std::collections::HashSet;
use std::rc::Rc;

use crate::canvas::{Canvas, Gradient, LineCap, LineJoin, Picture};
use crate::sessions::{Look, Session, State};
use crate::sysmon::SysData;
use crate::text::{self, weight, TextLayout};
use crate::util::*;

pub const REST_ID: &str = "__rest__";

pub fn color_of(look: Look) -> Rgb {
    match look {
        Look::Working => (0.30, 0.96, 0.84),
        Look::Idle => (0.42, 0.60, 1.00),
        Look::Waiting => (1.00, 0.70, 0.22),
        Look::Blocked => (1.00, 0.30, 0.36),
        Look::Rest => (0.62, 0.62, 0.78),
        Look::Background => (0.76, 0.55, 1.00),
    }
}

pub const WORKING_COLOR: Rgb = (0.30, 0.96, 0.84);

pub fn label_of(look: Look) -> &'static str {
    match look {
        Look::Working => "working",
        Look::Idle => "idle",
        Look::Waiting => "needs you",
        Look::Blocked => "blocked",
        Look::Background => "background",
        Look::Rest => "rest",
    }
}

fn size_of(st: State) -> f64 {
    match st {
        State::Working => 1.0,
        State::Idle => 0.9,
        State::Waiting => 1.45,
        State::Blocked => 0.95,
        State::Rest => 0.9,
    }
}

fn order_of(st: State) -> u8 {
    match st {
        State::Waiting => 0,
        State::Blocked => 1,
        State::Working => 2,
        State::Idle => 3,
        State::Rest => 9,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FloatStyle {
    Text,
    Bubble,
    Firefly,
}

/// Rising tool-call label.
#[derive(Clone, Debug)]
pub struct Floater {
    pub x0: f64,
    pub x: f64,
    pub y: f64,
    pub label: String,
    pub c: Rgb,
    pub style: FloatStyle,
    pub vy: f64,
    pub life: f64,
    pub age: f64,
    pub ph: f64,
}

/// What every theme knows about the screen (the Python `Scene` attributes).
pub struct Env {
    pub w: f64,
    pub h: f64,
    /// scale factor: 1.0 on a 1600x1200 screen
    pub s: f64,
    pub strip_h: f64,
    pub t: f64,
    pub rng: Rng,
    pub show_text: bool,
    /// extra monitor: creatures only, no labels/banner/strip/panel
    pub decor: bool,
    /// for Walker themes: where the ground begins (set in setup_world)
    pub ground_top: f64,
    pub floaters: Vec<Floater>,
}

impl Env {
    /// Rising tool-call label; `vy` in px/s at scale 1 (Python default 40), `life` in seconds (default 4).
    pub fn spawn_floater(&mut self, x: f64, y: f64, label: &str, color: Rgb, style: FloatStyle, vy: f64, life: f64) {
        let ph = self.rng.uniform(0.0, TAU);
        self.floaters.push(Floater { x0: x, x, y, label: label.to_string(), c: color, style, vy: vy * self.s, life, age: 0.0, ph });
    }

    /// `spawn_floater` with the default speed and life time.
    pub fn floater(&mut self, x: f64, y: f64, label: &str, color: Rgb, style: FloatStyle) {
        self.spawn_floater(x, y, label, color, style, 40.0, 4.0);
    }
}

/// Position of another actor, for keeping distance.
#[derive(Clone, Copy, Debug)]
pub struct Other {
    pub index: u64,
    pub x: f64,
    pub y: f64,
    pub radius: f64,
    pub dying: bool,
}

/// The part of an actor every theme has (the Python `Actor` attributes, plus those of `Walker`).
#[derive(Clone, Debug)]
pub struct Base {
    pub info: Rc<Session>,
    /// unique number of this actor
    pub index: u64,
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub seed: f64,
    pub heading: f64,
    pub phase: f64,
    pub color: Rgb,
    pub size_mul: f64,
    pub alpha: f64,
    pub dying: bool,
    pub radius: f64,
    pub flicker: f64,
    pub placed: bool,
    seen: HashSet<String>,
    first: bool,
    label_choice: usize,
    label_off: Option<(f64, f64)>,
    label_t: f64,
    // --- Walker: an animal walking on a ground plane
    /// gait phase (advances with walking speed)
    pub step: f64,
    /// -1..1 smooth facing (right = +1)
    pub face: f64,
    /// 0..1 (head down)
    pub graze: f64,
    /// 0..1 little jump after a tool call
    pub hop: f64,
    pub moving: f64,
}

impl Base {
    pub fn state(&self) -> State {
        self.info.state
    }

    pub fn look(&self) -> Look {
        self.info.look()
    }

    /// Shared motion: wander inside a box, avoid the others, swim to target when waiting.
    #[allow(clippy::too_many_arguments)]
    pub fn wander(
        &mut self,
        env: &Env,
        others: &[Other],
        dt: f64,
        t: f64,
        target: Option<(f64, f64)>,
        speed: f64,
        turn: f64,
        bbox: (f64, f64, f64, f64),
        spacing: f64,
    ) {
        let s = env.s;
        let (left, top, right, bottom) = bbox;
        self.heading += ((t * 0.31 + self.seed).sin() * 0.9 + (t * 0.17 + self.seed * 2.3).sin() * 0.6) * dt * turn;
        let mut dx = self.heading.cos() * speed;
        let mut dy = self.heading.sin() * speed * 0.7;
        if let Some((tx, ty)) = target {
            let (ox, oy) = (tx - self.x, ty - self.y);
            let d = match hypot(ox, oy) {
                d if d == 0.0 => 1.0,
                d => d,
            };
            let sp = (speed * 2.0).min(d * 1.2);
            dx = ox / d * sp + (t * 0.8 + self.seed).cos() * s * 18.0;
            dy = oy / d * sp + (t * 1.1 + self.seed).sin() * s * 12.0;
        }
        let (mut px, mut py) = (0.0, 0.0);
        if self.x < left {
            px += (left - self.x) * 3.0;
        }
        if self.x > right {
            px -= (self.x - right) * 3.0;
        }
        if self.y < top {
            py += (top - self.y) * 3.0;
        }
        if self.y > bottom {
            py -= (self.y - bottom) * 3.0;
        }
        for o in others {
            if o.index == self.index || o.dying {
                continue;
            }
            let (ox, oy) = (self.x - o.x, self.y - o.y);
            let d = hypot(ox, oy);
            let md = (self.radius + o.radius) * spacing;
            if 0.01 < d && d < md {
                let f = (md - d) * 2.0;
                px += ox / d * f;
                py += oy / d * f;
            }
        }
        dx += px;
        dy += py;
        if (px != 0.0 || py != 0.0) && target.is_none() {
            self.heading = angle_lerp(self.heading, dy.atan2(dx), (dt * 1.5).min(1.0));
        }
        let k = (dt * 1.2).min(1.0);
        self.vx = lerp(self.vx, dx, k);
        self.vy = lerp(self.vy, dy, k);
        self.x += self.vx * dt;
        self.y += self.vy * dt;
    }

    /// Walker: further down = closer = bigger.
    pub fn depth_scale(&self, env: &Env) -> f64 {
        let (top, bottom) = (env.ground_top, env.h - env.strip_h);
        lerp(0.65, 1.15, clamp((self.y - top) / (bottom - top).max(1.0), 0.0, 1.0))
    }

    /// Walker motion: idle animals keep stopping to graze/rest, working ones walk briskly, waiting
    /// ones come to the front. `speeds`/`turns` are for (working, idle, waiting, blocked).
    #[allow(clippy::too_many_arguments)]
    pub fn walker_move(
        &mut self,
        env: &Env,
        others: &[Other],
        dt: f64,
        t: f64,
        target: Option<(f64, f64)>,
        speeds: [f64; 4],
        turns: [f64; 4],
    ) {
        let s = env.s;
        let st = self.state();
        let pick = |v: [f64; 4], other: f64| match st {
            State::Working => v[0],
            State::Idle => v[1],
            State::Waiting => v[2],
            State::Blocked => v[3],
            State::Rest => other,
        };
        let mut speed = if st == State::Rest { 0.0 } else { pick(speeds, 20.0) * s };
        if st == State::Idle {
            // walk a bit, stop and graze, walk again
            speed *= clamp((t * 0.21 + self.seed).sin() * 1.6, 0.0, 1.0);
        }
        let r = self.radius;
        let bbox = (90.0 * s + r, env.ground_top + r * 0.3, env.w - 90.0 * s - r, env.h - env.strip_h - r * 1.1);
        self.wander(env, others, dt, t, target, speed, pick(turns, 0.5), bbox, 2.2);
        self.vy *= 0.97; // depth changes slowly
        let sp = hypot(self.vx, self.vy);
        self.moving = clamp(sp / (25.0 * s), 0.0, 1.0);
        self.step += dt * sp / (r * 0.35).max(1.0);
        let arrived = target.is_some_and(|(tx, ty)| hypot(tx - self.x, ty - self.y) < 80.0 * s);
        if arrived {
            self.face = lerp(self.face, 1.0, (dt * 2.0).min(1.0)); // arrived: stand still, no flipping
        } else if self.vx.abs() > 12.0 * s {
            self.face = lerp(self.face, if self.vx > 0.0 { 1.0 } else { -1.0 }, (dt * 4.0).min(1.0));
        }
        let grazing = st == State::Idle && sp < 10.0 * s;
        self.graze = lerp(self.graze, if grazing { 1.0 } else { 0.0 }, (dt * 1.5).min(1.0));
        self.hop = (self.hop - dt * 2.5).max(0.0);
    }

    /// Default places of the sub-agent companions: orbiting the creature. Returns (x, y, angle).
    pub fn orbit_spots(&self, t: f64) -> Vec<(f64, f64, f64)> {
        let n = self.info.subagents.len();
        let r = self.radius;
        (0..n)
            .map(|i| {
                let ang = t * 0.6 + i as f64 * TAU / n as f64 + self.seed;
                (self.x + ang.cos() * r * 1.9, self.y + ang.sin() * r * 0.55 - r * 0.15, ang)
            })
            .collect()
    }

    /// Walker: on the ground the young ones trot along behind the parent, like ducklings.
    pub fn trail_spots(&self, t: f64) -> Vec<(f64, f64, f64)> {
        let n = self.info.subagents.len();
        let r = self.radius;
        let back = if self.face >= 0.0 { -1.0 } else { 1.0 };
        (0..n)
            .map(|i| {
                let fi = i as f64;
                let wob = (t * 3.0 + fi * 1.7 + self.seed).sin() * r * 0.08;
                (self.x + back * r * (1.25 + fi * 0.75) + wob, self.y + r * (0.25 + 0.18 * (i % 2) as f64), 0.0)
            })
            .collect()
    }
}

/// The world of a theme: background, decoration, and the rules its actors follow.
pub trait Theme: Sized + 'static {
    type Actor: ActorT<Self>;
    const NAME: &'static str;
    const LABEL_BG: Rgb = (0.01, 0.04, 0.08);
    const EMPTY_COLOR: Rgb = (0.7, 0.85, 1.0);
    /// where sessions that need you gather (fraction of the height)
    const WAITING_ROW: f64 = 0.42;

    fn new(env: &Env) -> Self;

    /// Called when the size is known or changes: build backgrounds, place decoration, set `env.ground_top`.
    fn setup_world(&mut self, _env: &mut Env) {}

    fn update_world(&mut self, _env: &mut Env, _actors: &mut [Self::Actor], _dt: f64) {}

    fn draw_world(&mut self, cr: &mut Canvas, env: &Env, actors: &[Self::Actor], t: f64) {
        for a in sorted_actors(actors) {
            a.draw(cr, env, self, t);
        }
    }

    /// A session made a tool call. `index` counts the calls noticed in the same moment (0, 1, 2).
    fn on_tool(&mut self, _env: &mut Env, _actor: &mut Self::Actor, _name: &str, _index: usize) {}

    fn spawn_point(&self, env: &Env) -> (f64, f64) {
        (env.rng.uniform(env.w * 0.2, env.w * 0.8), env.rng.uniform(env.h * 0.25, env.h * 0.6))
    }

    /// Where the resting creature lies: front and centre on ground themes.
    fn rest_point(&self, env: &Env) -> (f64, f64) {
        if env.ground_top != 0.0 {
            return (env.w * env.rng.uniform(0.3, 0.42), env.h - env.strip_h - 150.0 * env.s);
        }
        self.spawn_point(env)
    }

    /// Where each waiting actor should go (same order as `waiting`).
    fn waiting_targets(&self, env: &Env, waiting: &[&Self::Actor]) -> Vec<(f64, f64)> {
        let n = waiting.len() as f64;
        (0..waiting.len())
            .map(|i| (env.w / 2.0 + (i as f64 - (n - 1.0) / 2.0) * env.w * 0.28, env.h * Self::WAITING_ROW))
            .collect()
    }
}

/// Actors back to front; the ones that need you last (on top).
pub fn sorted_actors<T: Theme, A: ActorT<T>>(actors: &[A]) -> Vec<&A> {
    let mut v: Vec<&A> = actors.iter().collect();
    v.sort_by(|a, b| {
        let (a, b) = (a.base(), b.base());
        (a.state() == State::Waiting, a.y)
            .partial_cmp(&(b.state() == State::Waiting, b.y))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    v
}

/// One Claude Code session on screen.
pub trait ActorT<T: Theme>: Clone + 'static {
    const BASE_RADIUS: f64 = 50.0;
    /// true = walks on a ground plane: the radius follows the depth, companions trail behind
    const WALKER: bool = false;
    const COMPANION_SCALE: f64 = 0.42;

    fn new(base: Base, env: &Env, world: &mut T) -> Self;
    fn base(&self) -> &Base;
    fn base_mut(&mut self) -> &mut Base;

    fn mv(&mut self, _env: &mut Env, _world: &mut T, _others: &[Other], _dt: f64, _t: f64, _target: Option<(f64, f64)>) {}

    fn draw(&self, cr: &mut Canvas, env: &Env, world: &T, t: f64);

    /// (x, y, angle) of each sub-agent companion.
    fn companion_spots(&self, _env: &Env, t: f64) -> Vec<(f64, f64, f64)> {
        if Self::WALKER { self.base().trail_spots(t) } else { self.base().orbit_spots(t) }
    }

    /// Called on the temporary copy that is drawn as a companion: clear trails, tongues and the like.
    fn companion_reset(&mut self) {}

    /// Default: a shrunken copy of the creature itself in the working colour (baby jellyfish, lambs,
    /// moons, ...). Themes can override this.
    fn draw_companions(&self, cr: &mut Canvas, env: &Env, world: &T, t: f64) {
        let b = self.base();
        if b.alpha < 0.05 || b.info.subagents.is_empty() {
            return;
        }
        let radius = b.radius * Self::COMPANION_SCALE;
        let info = Rc::new(Session { id: b.info.id.clone(), state: State::Working, ..Session::default() });
        for (x, y, ang) in self.companion_spots(env, t) {
            let mut c = self.clone();
            {
                let cb = c.base_mut();
                cb.x = x;
                cb.y = y;
                cb.vx = -ang.sin() * 40.0 * env.s;
                cb.vy = 0.0;
                cb.radius = radius;
                cb.color = WORKING_COLOR;
                cb.info = info.clone();
                cb.hop = 0.0;
                cb.graze = 0.0;
                cb.flicker = 1.0;
                cb.face = if cb.vx >= 0.0 { 1.0 } else { -1.0 };
            }
            c.companion_reset();
            c.draw(cr, env, world, t);
        }
    }

    /// Possible top-left corners for the label box, best first.
    fn label_candidates(&self, pw: f64, ph: f64) -> Vec<(f64, f64)> {
        let b = self.base();
        let r = b.radius;
        let (right, left) = (b.x + r * 1.5, b.x - r * 1.5 - pw);
        let y = b.y - r * 0.9;
        vec![
            (right, y),
            (left, y),
            (right, y + ph * 0.8),
            (left, y + ph * 0.8),
            (right, y - ph * 0.8),
            (left, y - ph * 0.8),
            (b.x - pw / 2.0, b.y + r * 1.8),
        ]
    }
}

/// What the screensaver sees of a theme.
pub trait Scene {
    fn name(&self) -> &'static str;
    fn set_sessions(&mut self, sessions: &[Rc<Session>]);
    fn set_system(&mut self, data: Option<Rc<SysData>>);
    fn show_hint(&mut self, text: &str, seconds: f64);
    /// The hint text and how many seconds it still shows.
    fn hint_left(&self) -> (String, f64);
    fn update(&mut self, dt: f64);
    fn draw(&mut self, cr: &mut Canvas, w: f64, h: f64);
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SceneOpts {
    pub show_text: bool,
    pub decor: bool,
    pub seed: Option<u64>,
}

struct Label {
    lines: Vec<(Rc<TextLayout>, Rgb, f64)>,
    sizes: Vec<(f64, f64)>,
    w: f64,
    h: f64,
}

pub struct Stage<T: Theme> {
    pub env: Env,
    pub theme: T,
    pub actors: Vec<T::Actor>,
    sessions: Vec<Rc<Session>>,
    hint: String,
    hint_until: f64,
    system: Option<Rc<SysData>>,
    sys_cache: Option<((u64, usize, u64, u64, u64), Picture)>,
    next_index: u64,
}

impl<T: Theme> Stage<T> {
    pub fn new(opts: SceneOpts) -> Self {
        let env = Env {
            w: 0.0,
            h: 0.0,
            s: 1.0,
            strip_h: 64.0,
            t: 0.0,
            rng: Rng::new(opts.seed),
            show_text: opts.show_text,
            decor: opts.decor,
            ground_top: 0.0,
            floaters: Vec::new(),
        };
        let theme = T::new(&env);
        Stage {
            env,
            theme,
            actors: Vec::new(),
            sessions: Vec::new(),
            hint: String::new(),
            hint_until: 0.0,
            system: None,
            sys_cache: None,
            next_index: 1,
        }
    }

    pub fn boxed(opts: SceneOpts) -> Box<dyn Scene> {
        Box::new(Self::new(opts))
    }

    fn start_point(&self, info: &Session) -> (f64, f64) {
        if info.state == State::Rest { self.theme.rest_point(&self.env) } else { self.theme.spawn_point(&self.env) }
    }

    fn new_base(&mut self, info: Rc<Session>, x: f64, y: f64) -> Base {
        let rng = &self.env.rng;
        let st = info.state;
        let index = self.next_index;
        self.next_index += 1;
        Base {
            index,
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            seed: rng.uniform(0.0, 100.0),
            heading: rng.uniform(0.0, TAU),
            phase: rng.uniform(0.0, TAU),
            color: color_of(info.look()),
            size_mul: size_of(st),
            alpha: 0.0,
            dying: false,
            radius: <T::Actor as ActorT<T>>::BASE_RADIUS * self.env.s * size_of(st),
            flicker: 1.0,
            placed: self.env.w != 0.0,
            seen: HashSet::new(),
            first: true,
            label_choice: 0,
            label_off: None,
            label_t: 0.0,
            step: 0.0,
            face: 1.0,
            graze: 0.0,
            hop: 0.0,
            moving: 0.0,
            info,
        }
    }

    fn update_actor(&mut self, i: usize, others: &[Other], dt: f64, target: Option<(f64, f64)>) {
        let t = self.env.t;
        let s = self.env.s;
        let a = &mut self.actors[i];
        {
            let rnd = self.env.rng.random();
            let b = a.base_mut();
            let st = b.state();
            b.color = lerp3(b.color, color_of(b.look()), (dt * 2.0).min(1.0));
            let breathe = 1.0 + if st == State::Waiting { 0.06 * (t * 3.0).sin() } else { 0.0 };
            b.size_mul = lerp(b.size_mul, size_of(st) * breathe, (dt * 1.5).min(1.0));
            let goal = if b.dying {
                0.0
            } else if st == State::Rest {
                0.8
            } else {
                1.0
            };
            b.alpha = lerp(b.alpha, goal, (dt * 1.2).min(1.0));
            b.radius = <T::Actor as ActorT<T>>::BASE_RADIUS * s * b.size_mul;
            b.flicker = if st == State::Blocked { 0.55 + 0.45 * rnd } else { 1.0 };
        }
        a.mv(&mut self.env, &mut self.theme, others, dt, t, target);
        if <T::Actor as ActorT<T>>::WALKER {
            let k = a.base().depth_scale(&self.env);
            a.base_mut().radius *= k;
        }
        let mut fresh: Vec<String> = Vec::new();
        {
            let b = a.base_mut();
            let info = b.info.clone();
            for ev in &info.tool_events {
                if !b.seen.contains(&ev.0) {
                    b.seen.insert(ev.0.clone());
                    if !b.first {
                        fresh.push(ev.1.clone());
                    }
                }
            }
            b.first = false;
            if b.seen.len() > 400 {
                let now: HashSet<&String> = info.tool_events.iter().map(|e| &e.0).collect();
                b.seen.retain(|id| now.contains(id));
            }
        }
        let skip = fresh.len().saturating_sub(3);
        for (k, name) in fresh[skip..].iter().enumerate() {
            self.theme.on_tool(&mut self.env, &mut self.actors[i], name, k);
        }
    }

    // ------------------------------------------------------------------ shared overlays
    fn label_layout(&self, a: &T::Actor) -> Option<Label> {
        let b = a.base();
        let env = &self.env;
        if b.alpha < 0.05 || b.state() == State::Rest {
            return None;
        }
        if !(0.0 <= b.x && b.x <= env.w && 0.0 <= b.y && b.y <= env.h - env.strip_h) {
            return None; // off screen (flying themes wrap around the edges)
        }
        let s = env.s;
        let info = &b.info;
        let st = b.state();
        let vis = b.look();
        let c = color_of(vis);
        let maxw = Some(380.0 * s);
        let mut lines = vec![(text::layout(&info.name, 21.0 * s, weight::BOLD, maxw), (1.0, 1.0, 1.0), 0.95)];
        let status = format!("● {} · {}", label_of(vis), fmt_duration(now() - info.since));
        lines.push((text::layout(&status, 16.0 * s, weight::SEMIBOLD, maxw), c, 1.0));
        if env.show_text {
            let detail = if st == State::Waiting {
                if info.waiting_for.is_empty() {
                    "waiting for your input".to_string()
                } else {
                    format!("waiting for {}", info.waiting_for)
                }
            } else if vis == Look::Background && !info.bg_task.is_empty() {
                format!("running: {}", info.bg_task)
            } else if st == State::Working && info.tool.as_deref().is_some_and(|t| !t.is_empty()) {
                format!("{}  {}", info.tool.as_deref().unwrap_or(""), info.tool_detail).trim().to_string()
            } else if !info.last_text.is_empty() {
                format!("“{}”", info.last_text)
            } else {
                String::new()
            };
            if !detail.is_empty() {
                lines.push((text::layout(&detail, 14.0 * s, weight::NORMAL, maxw), (0.85, 0.92, 1.0), 0.8));
            }
        }
        let subs = &info.subagents;
        if !subs.is_empty() {
            let names: Vec<&str> =
                subs.iter().map(|a| if a.description.is_empty() { a.kind.as_str() } else { a.description.as_str() }).collect();
            let line = format!("{} sub-agent{}: {}", subs.len(), if subs.len() > 1 { "s" } else { "" }, names.join(", "));
            lines.push((text::layout(&line, 14.0 * s, weight::NORMAL, maxw), lighten(WORKING_COLOR, 0.3), 0.85));
        }
        if !info.cwd.is_empty() {
            lines.push((text::layout(&info.cwd, 13.0 * s, weight::NORMAL, maxw), (0.7, 0.8, 0.9), 0.55));
        }
        let sizes: Vec<(f64, f64)> = lines.iter().map(|l| l.0.get_pixel_size()).collect();
        let w = sizes.iter().map(|z| z.0).fold(0.0, f64::max);
        let h = sizes.iter().map(|z| z.1).sum::<f64>() + 3.0 * s * (lines.len() as f64 - 1.0);
        Some(Label { lines, sizes, w, h })
    }

    fn draw_label(&self, cr: &mut Canvas, b: &Base, lab: &Label, lx: f64, ly: f64) {
        let (s, a) = (self.env.s, b.alpha);
        let c = color_of(b.look());
        let pad = 12.0 * s;
        cr.set_rgba(T::LABEL_BG, 0.6 * a);
        cr.new_path();
        cr.rounded_rect(lx, ly, lab.w + pad * 2.0, lab.h + pad * 2.0, 10.0 * s);
        cr.fill();
        cr.set_rgba(c, 0.75 * a);
        cr.rectangle(lx, ly + 8.0 * s, 3.0 * s, lab.h + pad * 2.0 - 16.0 * s);
        cr.fill();
        let mut yy = ly + pad;
        for ((lay, rgb, la), sz) in lab.lines.iter().zip(&lab.sizes) {
            text::show(cr, lay, lx + pad + 4.0 * s, yy, *rgb, la * a, 0.0);
            yy += sz.1 + 3.0 * s;
        }
    }

    /// Place labels so they do not cover each other; sessions that need you get the best spot.
    /// Each actor keeps its last spot while it is free, so labels do not jump around.
    fn draw_labels(&mut self, cr: &mut Canvas) {
        let s = self.env.s;
        let (pad, gap) = (12.0 * s, 8.0 * s);
        let mut placed: Vec<(f64, f64, f64, f64)> = Vec::new();
        if self.system.is_some() {
            placed.push(self.sysmon_rect()); // keep labels off the monitor panel
        }
        let (lo_x, lo_y) = (20.0 * s, 20.0 * s);
        let mut order: Vec<usize> = (0..self.actors.len()).collect();
        order.sort_by(|&i, &j| {
            let (a, b) = (self.actors[i].base(), self.actors[j].base());
            (order_of(a.state()), &a.info.id).cmp(&(order_of(b.state()), &b.info.id))
        });
        let t = self.env.t;
        for i in order {
            let Some(lab) = self.label_layout(&self.actors[i]) else { continue };
            let (pw, ph) = (lab.w + pad * 2.0, lab.h + pad * 2.0);
            let (hi_x, hi_y) = (self.env.w - pw - 20.0 * s, self.env.h - self.env.strip_h - ph - 10.0 * s);
            let cands = self.actors[i].label_candidates(pw, ph);
            let prev = self.actors[i].base().label_choice.min(cands.len().saturating_sub(1));
            let mut choice = None;
            for k in std::iter::once(prev).chain((0..cands.len()).filter(|k| *k != prev)) {
                let (x, y) = (clamp(cands[k].0, lo_x, hi_x), clamp(cands[k].1, lo_y, hi_y));
                let free = placed
                    .iter()
                    .all(|&(bx, by, bw, bh)| x + pw + gap <= bx || bx + bw + gap <= x || y + ph + gap <= by || by + bh + gap <= y);
                if free {
                    choice = Some((k, x, y));
                    break;
                }
            }
            let choice = choice.unwrap_or((0, clamp(cands[0].0, lo_x, hi_x), clamp(cands[0].1, lo_y, hi_y)));
            placed.push((choice.1, choice.2, pw, ph));
            // Glide to a new spot instead of jumping. The offset to the creature is smoothed (not the
            // absolute position), so the label still follows the creature exactly while it moves.
            let (lx, ly);
            {
                let b = self.actors[i].base_mut();
                b.label_choice = choice.0;
                let (tx, ty) = (choice.1 - b.x, choice.2 - b.y);
                let dtl = t - b.label_t;
                b.label_t = t;
                let off = match b.label_off {
                    Some(last) if (0.0..=0.5).contains(&dtl) => {
                        let k = 1.0 - (-dtl * 5.0).exp(); // ~90 % of the way in half a second
                        (lerp(last.0, tx, k), lerp(last.1, ty, k))
                    }
                    _ => (tx, ty), // first frame or after a pause: no animation
                };
                b.label_off = Some(off);
                lx = clamp(b.x + off.0, lo_x, hi_x);
                ly = clamp(b.y + off.1, lo_y, hi_y);
            }
            self.draw_label(cr, self.actors[i].base(), &lab, lx, ly);
        }
    }

    fn sysmon_rect(&self) -> (f64, f64, f64, f64) {
        let (s, t) = (self.env.s, self.env.t);
        let ox = (t / 53.0).sin() * 8.0 * s; // slow drift against burn-in
        let oy = (t / 41.0).sin() * 5.0 * s;
        let (w, h) = (300.0 * s, 192.0 * s);
        (self.env.w - w - 28.0 * s + ox, 24.0 * s + oy, w, h)
    }

    /// Small task-manager panel: CPU % and memory GB with a 30-minute graph each.
    /// Recorded once per new sample and then only shown again every frame.
    fn draw_sysmon(&mut self, cr: &mut Canvas) {
        let Some(d) = self.system.clone() else { return };
        if d.cpu.is_empty() {
            return;
        }
        let (x, y, w, h) = self.sysmon_rect();
        let key = (
            d.time.to_bits(),
            d.cpu.len(),
            d.cpu.last().copied().unwrap_or(0.0).to_bits(),
            d.mem.last().copied().unwrap_or(0.0).to_bits(),
            self.env.s.to_bits(),
        );
        if self.sys_cache.as_ref().map(|c| c.0) != Some(key) {
            let s = self.env.s;
            let pic = Picture::record(w + 4.0, h + 4.0, |c| paint_sysmon(c, 2.0, 2.0, w, h, s, &d));
            self.sys_cache = Some((key, pic));
        }
        if let Some((_, pic)) = &self.sys_cache {
            cr.picture(pic, x.round() - 2.0, y.round() - 2.0, 1.0);
        }
    }

    fn draw_floaters(&self, cr: &mut Canvas) {
        let s = self.env.s;
        for f in &self.env.floaters {
            let fade = (f.age * 3.0).min(1.0) * ((f.life - f.age) / 1.2).min(1.0);
            let (x, y, c) = (f.x, f.y, f.c);
            let mut tx = x + 8.0 * s;
            match f.style {
                FloatStyle::Bubble => {
                    let r = 10.0 * s;
                    cr.set_source_rgba(0.7, 0.9, 1.0, 0.08 * fade);
                    cr.new_path();
                    cr.arc(x, y, r, 0.0, TAU);
                    cr.fill_preserve();
                    cr.set_source_rgba(0.75, 0.92, 1.0, 0.5 * fade);
                    cr.set_line_width((1.2 * s).max(1.0));
                    cr.stroke();
                    cr.set_source_rgba(1.0, 1.0, 1.0, 0.6 * fade);
                    cr.arc(x - r * 0.35, y - r * 0.35, r * 0.25, 0.0, TAU);
                    cr.fill();
                    tx = x + r + 6.0 * s;
                }
                FloatStyle::Firefly => {
                    let blink = 0.6 + 0.4 * (f.age * 6.0 + f.ph).sin();
                    let mut g = Gradient::radial(x, y, 0.0, x, y, 14.0 * s);
                    g.add_color_stop_rgba(0.0, 1.0, 0.95, 0.6, 0.9 * fade * blink);
                    g.add_color_stop_rgba(1.0, 1.0, 0.9, 0.4, 0.0);
                    cr.set_source(&g);
                    cr.new_path();
                    cr.arc(x, y, 14.0 * s, 0.0, TAU);
                    cr.fill();
                    tx = x + 14.0 * s;
                }
                FloatStyle::Text => {}
            }
            if f.label.is_empty() {
                continue; // plain bubble (air stone, sleepy crab)
            }
            let lay = text::layout(&f.label, 15.0 * s, weight::SEMIBOLD, None);
            let th = lay.get_pixel_size().1;
            text::show(cr, &lay, tx, y - th / 2.0, lighten(c, 0.3), 0.95 * fade, 0.7);
        }
    }

    fn draw_banner(&self, cr: &mut Canvas) {
        let waiting: Vec<&Base> =
            self.actors.iter().map(|a| a.base()).filter(|b| b.state() == State::Waiting && !b.dying).collect();
        if waiting.is_empty() {
            return;
        }
        let (s, t) = (self.env.s, self.env.t);
        let msg = if waiting.len() == 1 {
            let n = &waiting[0].info.name;
            format!("{} needs your input", if n.is_empty() { "A session" } else { n })
        } else {
            format!("{} sessions need your input", waiting.len())
        };
        let lay = text::layout(&msg, 34.0 * s, weight::BOLD, None);
        let (tw, th) = lay.get_pixel_size();
        let x = self.env.w / 2.0 - tw / 2.0 + (t * 0.11).sin() * 40.0 * s;
        let y = 60.0 * s + (t * 0.4).sin() * 8.0 * s;
        let (px, py) = (28.0 * s, 12.0 * s);
        cr.set_source_rgba(0.02, 0.02, 0.04, 0.55);
        cr.new_path();
        cr.rounded_rect(x - px, y - py, tw + 2.0 * px, th + 2.0 * py, (th + 2.0 * py) / 2.0);
        cr.fill();
        let c = color_of(Look::Waiting);
        cr.set_rgba(c, 0.35 + 0.25 * (t * 3.0).sin());
        cr.set_line_width(2.0 * s);
        cr.rounded_rect(x - px, y - py, tw + 2.0 * px, th + 2.0 * py, (th + 2.0 * py) / 2.0);
        cr.stroke();
        text::show(cr, &lay, x, y, c, 0.8 + 0.2 * (t * 3.0).sin(), 0.5);
    }

    fn draw_strip(&self, cr: &mut Canvas) {
        let (w, h, s, t) = (self.env.w, self.env.h, self.env.s, self.env.t);
        let sh = self.env.strip_h;
        let mut g = Gradient::linear(0.0, h - sh * 1.4, 0.0, h);
        g.add_color_stop_rgba(0.0, 0.0, 0.0, 0.0, 0.0);
        g.add_color_stop_rgba(0.35, 0.0, 0.01, 0.02, 0.55);
        g.add_color_stop_rgba(1.0, 0.0, 0.01, 0.02, 0.75);
        cr.set_source(&g);
        cr.new_path();
        cr.rectangle(0.0, h - sh * 1.4, w, sh * 1.4);
        cr.fill();
        let ox = (t / 61.0).sin() * 12.0 * s; // slow drift so nothing burns in
        let oy = (t / 47.0).sin() * 4.0 * s;
        let cy = h - sh / 2.0 + oy;

        let clock = text::layout(&strftime("%H:%M"), 26.0 * s, weight::BOLD, None);
        let date = text::layout(&strftime("%a %d %b"), 14.0 * s, weight::NORMAL, None);
        let (cw, chh) = clock.get_pixel_size();
        let (dw, dh) = date.get_pixel_size();
        let rx = w - 36.0 * s + ox;
        text::show(cr, &clock, rx - cw, cy - chh / 2.0, (0.9, 0.95, 1.0), 0.85, 0.0);
        text::show(cr, &date, rx - cw - dw - 14.0 * s, cy - dh / 2.0, (0.7, 0.8, 0.9), 0.6, 0.0);

        let mut x = 36.0 * s + ox;
        // Claude asterisk drawn as lines (no symbol font needed on any platform), then the name
        let ar = 7.0 * s;
        cr.set_rgba(CLAUDE_ORANGE, 0.85);
        cr.set_line_width(2.0 * s);
        cr.set_line_cap(LineCap::Round);
        cr.new_path();
        for k in 0..4 {
            let a = k as f64 * std::f64::consts::PI / 4.0;
            cr.move_to(x + ar + a.cos() * ar, cy + a.sin() * ar);
            cr.line_to(x + ar - a.cos() * ar, cy - a.sin() * ar);
        }
        cr.stroke();
        cr.set_line_cap(LineCap::Butt);
        x += 2.0 * ar + 7.0 * s;
        let brand = text::layout("Claude Code", 15.0 * s, weight::SEMIBOLD, None);
        let (bw, bh) = brand.get_pixel_size();
        text::show(cr, &brand, x, cy - bh / 2.0, CLAUDE_ORANGE, 0.85, 0.0);
        x += bw + 34.0 * s;
        let limit = rx - cw - dw - 50.0 * s;
        let mut infos: Vec<&Rc<Session>> = self.sessions.iter().collect();
        infos.sort_by(|a, b| (order_of(a.state), &a.name).cmp(&(order_of(b.state), &b.name)));
        let tnow = now();
        for info in infos {
            let st = info.look();
            let c = color_of(st);
            let name = text::layout(&info.name, 16.0 * s, weight::BOLD, Some(220.0 * s));
            let n_sub = info.subagents.len();
            let extra = if n_sub > 0 { format!(" · +{} agent{}", n_sub, if n_sub > 1 { "s" } else { "" }) } else { String::new() };
            let lab = text::layout(
                &format!("{} · {}{}", label_of(st), fmt_duration(tnow - info.since), extra),
                14.0 * s,
                weight::NORMAL,
                None,
            );
            let (nw, nh) = name.get_pixel_size();
            let (lw, lh) = lab.get_pixel_size();
            if x + 20.0 * s + nw + 8.0 * s + lw > limit {
                let more = text::layout("…", 16.0 * s, weight::NORMAL, None);
                text::show(cr, &more, x, cy - more.get_pixel_size().1 / 2.0, (1.0, 1.0, 1.0), 0.6, 0.0);
                break;
            }
            let pr = 6.0 * s * if st == Look::Waiting { 1.0 + 0.35 * (t * 5.0).sin() } else { 1.0 };
            cr.set_rgba(c, 0.95);
            cr.new_path();
            cr.arc(x + 6.0 * s, cy, pr, 0.0, TAU);
            cr.fill();
            x += 20.0 * s;
            text::show(cr, &name, x, cy - nh / 2.0, (1.0, 1.0, 1.0), 0.92, 0.0);
            x += nw + 8.0 * s;
            text::show(cr, &lab, x, cy - lh / 2.0, c, 0.9, 0.0);
            x += lw + 34.0 * s;
        }
    }

    fn draw_hint(&self, cr: &mut Canvas) {
        let t = self.env.t;
        if self.hint.is_empty() || t >= self.hint_until {
            return;
        }
        let s = self.env.s;
        let a = ((self.hint_until - t) / 1.0).min(1.0);
        let lay = text::layout(&self.hint, 22.0 * s, weight::SEMIBOLD, None);
        let (tw, th) = lay.get_pixel_size();
        let (pw, ph) = (tw + 48.0 * s, th + 24.0 * s);
        let (x, y) = (self.env.w / 2.0 - pw / 2.0, self.env.h * 0.78 - ph / 2.0);
        cr.set_source_rgba(0.01, 0.04, 0.08, 0.75 * a);
        cr.new_path();
        cr.rounded_rect(x, y, pw, ph, ph / 2.0);
        cr.fill();
        cr.set_source_rgba(0.6, 0.85, 1.0, 0.5 * a);
        cr.set_line_width(1.5 * s);
        cr.rounded_rect(x, y, pw, ph, ph / 2.0);
        cr.stroke();
        text::show(cr, &lay, x + 24.0 * s, y + 12.0 * s, (0.9, 0.96, 1.0), a, 0.0);
    }
}

fn paint_sysmon(cr: &mut Canvas, x: f64, y: f64, w: f64, h: f64, s: f64, d: &SysData) {
    cr.set_source_rgba(0.02, 0.03, 0.06, 0.62);
    cr.rounded_rect(x, y, w, h, 12.0 * s);
    cr.fill();
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.10);
    cr.set_line_width(1.0 * s);
    cr.rounded_rect(x, y, w, h, 12.0 * s);
    cr.stroke();
    let pad = 14.0 * s;
    let (gw, gh) = (w - 2.0 * pad, 46.0 * s);
    let n_max = (30.0 * 60.0 / d.interval.max(1.0)).floor();
    let gib = 1024f64.powi(3);
    let mem_total = if d.mem_total > 0.0 { d.mem_total } else { 1.0 };
    let rows: [(&str, String, &Vec<f64>, f64, Rgb); 2] = [
        ("CPU", format!("{:.0} %", d.cpu.last().copied().unwrap_or(0.0)), &d.cpu, 100.0, (0.55, 0.85, 1.0)),
        (
            "Memory",
            format!("{:.1} / {:.1} GB", d.mem.last().copied().unwrap_or(0.0) / gib, mem_total / gib),
            &d.mem,
            mem_total,
            (0.78, 0.66, 1.0),
        ),
    ];
    let mut yy = y + pad - 2.0 * s;
    for (name, value, series, vmax, col) in rows {
        let lab = text::layout(name, 13.0 * s, weight::SEMIBOLD, None);
        let val = text::layout(&value, 15.0 * s, weight::BOLD, None);
        let (_, lh) = lab.get_pixel_size();
        let (vw, vh) = val.get_pixel_size();
        text::show(cr, &lab, x + pad, yy + (vh - lh) / 2.0, (0.75, 0.8, 0.9), 0.8, 0.0);
        text::show(cr, &val, x + w - pad - vw, yy, col, 0.95, 0.0);
        let gy = yy + vh + 4.0 * s;
        sparkline(cr, x + pad, gy, gw, gh, series, vmax, n_max, col);
        yy = gy + gh + 10.0 * s;
    }
    let foot = text::layout("last 30 min", 11.0 * s, weight::NORMAL, None);
    text::show(cr, &foot, x + pad, yy - 6.0 * s, (0.7, 0.75, 0.85), 0.45, 0.0);
}

#[allow(clippy::too_many_arguments)]
fn sparkline(cr: &mut Canvas, x: f64, y: f64, w: f64, h: f64, series: &[f64], vmax: f64, n_max: f64, col: Rgb) {
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.05); // graph background + 50 % line
    cr.rectangle(x, y, w, h);
    cr.fill();
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.08);
    cr.set_line_width(1.0);
    cr.move_to(x, y + h / 2.0);
    cr.line_to(x + w, y + h / 2.0);
    cr.stroke();
    let n = series.len();
    if n < 2 || vmax <= 0.0 {
        return;
    }
    let step = w / (n_max - 1.0).max(1.0); // newest sample at the right edge; a short history fills from the right
    let first = n.saturating_sub(n_max as usize); // never draw left of the graph
    let pts: Vec<(f64, f64)> = series
        .iter()
        .enumerate()
        .skip(first)
        .map(|(i, v)| (x + w - (n - 1 - i) as f64 * step, y + h - clamp(v / vmax, 0.0, 1.0) * h))
        .collect();
    cr.move_to(pts[0].0, y + h);
    for p in &pts {
        cr.line_to(p.0, p.1);
    }
    cr.line_to(pts[pts.len() - 1].0, y + h);
    cr.close_path();
    cr.set_rgba(col, 0.18);
    cr.fill();
    cr.move_to(pts[0].0, pts[0].1);
    for p in &pts[1..] {
        cr.line_to(p.0, p.1);
    }
    cr.set_rgba(col, 0.9);
    cr.set_line_width(1.5);
    cr.set_line_join(LineJoin::Round);
    cr.stroke();
    cr.set_line_join(LineJoin::Miter);
}

impl<T: Theme> Scene for Stage<T> {
    fn name(&self) -> &'static str {
        T::NAME
    }

    fn set_sessions(&mut self, sessions: &[Rc<Session>]) {
        self.sessions = sessions.to_vec();
        // no session running -> one resting creature (no label, not in the status strip)
        let infos: Vec<Rc<Session>> = if sessions.is_empty() {
            vec![Rc::new(Session { id: REST_ID.into(), state: State::Rest, ..Session::default() })]
        } else {
            sessions.to_vec()
        };
        let mut live: HashSet<String> = HashSet::new();
        for info in infos {
            live.insert(info.id.clone());
            if let Some(a) = self.actors.iter_mut().find(|a| a.base().info.id == info.id) {
                let b = a.base_mut();
                b.info = info;
                b.dying = false;
            } else {
                let (x, y) = if self.env.w != 0.0 { self.start_point(&info) } else { (400.0, 300.0) };
                let base = self.new_base(info, x, y);
                let actor = <T::Actor as ActorT<T>>::new(base, &self.env, &mut self.theme);
                self.actors.push(actor);
            }
        }
        for a in &mut self.actors {
            if !live.contains(&a.base().info.id) {
                a.base_mut().dying = true;
            }
        }
    }

    fn set_system(&mut self, data: Option<Rc<SysData>>) {
        self.system = data;
    }

    fn show_hint(&mut self, text: &str, seconds: f64) {
        self.hint = text.to_string();
        self.hint_until = self.env.t + seconds;
    }

    fn hint_left(&self) -> (String, f64) {
        (self.hint.clone(), self.hint_until - self.env.t)
    }

    fn update(&mut self, dt: f64) {
        if self.env.w == 0.0 {
            return;
        }
        let dt = clamp(dt, 0.0, 0.1);
        self.env.t += dt;
        let mut waiting: Vec<&T::Actor> =
            self.actors.iter().filter(|a| a.base().state() == State::Waiting && !a.base().dying).collect();
        waiting.sort_by(|a, b| a.base().info.id.cmp(&b.base().info.id));
        let spots = self.theme.waiting_targets(&self.env, &waiting);
        let targets: Vec<(u64, (f64, f64))> = waiting.iter().map(|a| a.base().index).zip(spots).collect();
        for i in 0..self.actors.len() {
            let others: Vec<Other> = self
                .actors
                .iter()
                .map(|a| {
                    let b = a.base();
                    Other { index: b.index, x: b.x, y: b.y, radius: b.radius, dying: b.dying }
                })
                .collect();
            let idx = self.actors[i].base().index;
            let target = targets.iter().find(|(k, _)| *k == idx).map(|(_, p)| *p);
            self.update_actor(i, &others, dt, target);
        }
        self.actors.retain(|a| !(a.base().dying && a.base().alpha < 0.02));
        let s = self.env.s;
        for f in &mut self.env.floaters {
            f.age += dt;
            f.y -= f.vy * dt;
            f.x = f.x0 + (f.age * 2.0 + f.ph).sin() * 10.0 * s;
        }
        self.env.floaters.retain(|f| f.age < f.life);
        self.theme.update_world(&mut self.env, &mut self.actors, dt);
    }

    fn draw(&mut self, cr: &mut Canvas, w: f64, h: f64) {
        if (w, h) != (self.env.w, self.env.h) {
            self.env.w = w;
            self.env.h = h;
            self.env.s = clamp((h / 1200.0).min(w / 1600.0), 0.4, 3.0);
            self.env.strip_h = 64.0 * self.env.s;
            self.theme.setup_world(&mut self.env);
            for i in 0..self.actors.len() {
                if !self.actors[i].base().placed {
                    let info = self.actors[i].base().info.clone();
                    let (x, y) = self.start_point(&info);
                    let b = self.actors[i].base_mut();
                    b.x = x;
                    b.y = y;
                    b.placed = true;
                }
            }
        }
        let (t, s) = (self.env.t, self.env.s);
        self.theme.draw_world(cr, &self.env, &self.actors, t);
        for a in sorted_actors::<T, _>(&self.actors) {
            a.draw_companions(cr, &self.env, &self.theme, t);
        }
        self.draw_floaters(cr);
        if self.env.decor {
            return;
        }
        self.draw_labels(cr);
        self.draw_sysmon(cr);
        self.draw_banner(cr);
        if self.sessions.is_empty() {
            let lay = text::layout("No Claude Code sessions running", 28.0 * s, weight::LIGHT, None);
            let (tw, _) = lay.get_pixel_size();
            let x = w / 2.0 - tw / 2.0 + (t * 0.13).sin() * w * 0.25;
            let y = h * 0.45 + (t * 0.21).sin() * h * 0.18;
            text::show(cr, &lay, x, y, T::EMPTY_COLOR, 0.5, 0.0);
        }
        self.draw_strip(cr);
        self.draw_hint(cr);
    }
}
