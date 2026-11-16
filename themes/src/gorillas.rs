//! Gorillas theme: a misty jungle with big trees, branches and lianas.
//!
//! working = knuckle-walks about and forages; now and then it climbs a tree and swings on a liana
//!           to the next one; every tool call makes it rear up and drum its chest, the tool name rises
//! idle    = sits and chews on a leafy stalk, scratches its head, sometimes shifts place (swings rarely)
//! waiting = comes to the front, stands upright and beats its chest slowly, looking at you, pulses
//! blocked = sits hunched with folded arms and flickers
//! background task = like working, in violet
//! rest    = one gorilla asleep on a nest of leaves, breathing slowly, "z z z"
//! Sub-agents are baby gorillas that trot behind; while the parent swings they scurry along below.
//! The fur stays natural; the state colour shows as a rim light, a glow on the moss and in the
//! silver saddle on the back.
use std::rc::Rc;

use vivarium_core::prelude::*;

const FUR: Rgb = (0.15, 0.15, 0.17);
const FUR_FAR: Rgb = (0.085, 0.085, 0.10);
const FUR_HI: Rgb = (0.27, 0.27, 0.30);
const SKIN: Rgb = (0.07, 0.065, 0.07);
const MUZZLE: Rgb = (0.23, 0.215, 0.21);
const SILVER: Rgb = (0.64, 0.66, 0.68);
const GREENS: [Rgb; 4] = [(0.10, 0.30, 0.13), (0.16, 0.40, 0.16), (0.25, 0.52, 0.20), (0.07, 0.22, 0.11)];
/// lengths in units of the radius: one arm segment, one leg segment, hand above the body centre when hanging
const ARM: f64 = 0.58;
const LEG: f64 = 0.40;
const REACH: f64 = 1.4;
/// depth factor of a gorilla on a liana
const AIR_K: f64 = 0.9;

type P = (f64, f64);

fn smooth(x: f64) -> f64 {
    let x = clamp(x, 0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

fn mixp(a: P, b: P, k: f64) -> P {
    (lerp(a.0, b.0, k), lerp(a.1, b.1, k))
}

/// Where the joints are, in units of the radius, facing right, feet at y = +0.95.
/// Index 0 is the far arm/leg (the left one when seen from the front), 1 the near one.
#[derive(Clone, Copy)]
struct Pose {
    hip: P,
    sh: P,
    head: P,
    /// half distance between the hips / the shoulders (small in profile, wide from the front)
    hw: f64,
    sw: f64,
    /// 1 = profile, 0 = looking at the viewer
    look: f64,
    /// 1 = seen from behind (no face, the whole silver back)
    back: f64,
    hands: [P; 2],
    feet: [P; 2],
    /// which way elbows and knees bend (-1..1)
    eb: [f64; 2],
    kb: [f64; 2],
    mouth: f64,
}

impl Pose {
    fn mix(&self, o: &Pose, k: f64) -> Pose {
        let two = |a: [P; 2], b: [P; 2]| [mixp(a[0], b[0], k), mixp(a[1], b[1], k)];
        let twof = |a: [f64; 2], b: [f64; 2]| [lerp(a[0], b[0], k), lerp(a[1], b[1], k)];
        Pose {
            hip: mixp(self.hip, o.hip, k),
            sh: mixp(self.sh, o.sh, k),
            head: mixp(self.head, o.head, k),
            hw: lerp(self.hw, o.hw, k),
            sw: lerp(self.sw, o.sw, k),
            look: lerp(self.look, o.look, k),
            back: lerp(self.back, o.back, k),
            hands: two(self.hands, o.hands),
            feet: two(self.feet, o.feet),
            eb: twof(self.eb, o.eb),
            kb: twof(self.kb, o.kb),
            mouth: lerp(self.mouth, o.mouth, k),
        }
    }

    /// Knuckle-walk: the arms are the front legs. `forage` 0..1: picks something up and eats it.
    fn walk(step: f64, mv: f64, forage: f64, t: f64) -> Pose {
        let bob = (step * 2.0).sin() * 0.025 * mv;
        let hand = |ph: f64| (0.42 + 0.26 * ph.sin() * mv, 0.93 - 0.12 * ph.cos().max(0.0) * mv);
        let foot = |ph: f64| (-0.48 + 0.22 * ph.sin() * mv, 0.95 - 0.09 * ph.cos().max(0.0) * mv);
        let mut hands = [hand(step + PI), hand(step)];
        if forage > 0.0 {
            let pick = mixp((0.82, 0.90), (0.76, 0.06), smooth(0.5 + 0.8 * (t * 2.6).sin()));
            hands[1] = mixp(hands[1], pick, forage);
        }
        Pose {
            hip: (-0.42, 0.29 + bob * 0.5),
            sh: (0.30, -0.14 + bob),
            head: (0.63, -0.15 + bob + 0.13 * forage),
            hw: 0.04,
            sw: 0.05,
            look: 1.0,
            back: 0.0,
            hands,
            feet: [foot(step), foot(step + PI)],
            eb: [0.5, 0.5],
            kb: [-1.0, -1.0],
            mouth: 0.0,
        }
    }

    /// Sitting, half turned to the viewer; the near hand lifts a stalk to the mouth or scratches the head.
    fn sit(t: f64, seed: f64, scratch: f64) -> Pose {
        let breath = (t * 1.2 + seed).sin() * 0.012;
        let bite = smooth(0.5 + 0.9 * (t * 0.6 + seed).sin());
        let mut near = mixp((0.44, 0.30), (0.36, -0.24), bite);
        near = mixp(near, (0.03 + 0.05 * (t * 13.0).sin(), -0.74), scratch);
        Pose {
            hip: (-0.18, 0.60),
            sh: (-0.02, -0.08 + breath),
            head: (0.13, -0.46 + breath),
            hw: 0.10,
            sw: 0.14,
            look: 0.6,
            back: 0.0,
            hands: [(0.34, 0.50), near],
            feet: [(0.30, 0.93), (0.44, 0.94)],
            eb: [0.8, 0.9],
            kb: [-1.0, -1.0],
            mouth: 0.25 * bite * (0.5 + 0.5 * (t * 7.0 + seed).sin()),
        }
    }

    /// Upright, seen from the front, the fists drumming on the chest. `roar` 0..1 opens the mouth.
    fn stand(ph: f64, roar: f64) -> Pose {
        let (b0, b1) = (0.5 + 0.5 * ph.sin(), 0.5 - 0.5 * ph.sin());
        let sway = ph.sin() * 0.015;
        Pose {
            hip: (0.0, 0.33),
            sh: (sway, -0.45),
            head: (0.02 + sway, -0.84 - 0.03 * roar),
            hw: 0.15,
            sw: 0.25,
            look: 0.15,
            back: 0.0,
            hands: [mixp((-0.10, -0.22), (-0.46, -0.44), b0), mixp((0.12, -0.22), (0.48, -0.44), b1)],
            feet: [(-0.27, 0.95), (0.27, 0.95)],
            eb: [1.0, -1.0],
            kb: [0.6, -0.6],
            mouth: roar,
        }
    }

    /// Blocked: sitting hunched, the head sunk between the shoulders, arms folded.
    fn hunch(t: f64, seed: f64) -> Pose {
        let breath = (t * 0.9 + seed).sin() * 0.01;
        Pose {
            hip: (0.0, 0.60),
            sh: (0.0, 0.0 + breath),
            head: (0.02, -0.27 + breath),
            hw: 0.20,
            sw: 0.24,
            look: 0.2,
            back: 0.0,
            hands: [(0.22, 0.27), (-0.20, 0.34)],
            feet: [(-0.36, 0.93), (0.36, 0.93)],
            eb: [1.0, -1.0],
            kb: [1.0, -1.0],
            mouth: 0.0,
        }
    }

    /// Climbing a trunk, seen from behind: hand over hand, the feet gripping the bark.
    fn climb(step: f64) -> Pose {
        let (c0, c1) = (0.5 + 0.5 * step.sin(), 0.5 - 0.5 * step.sin());
        let lean = step.sin() * 0.03;
        Pose {
            hip: (-lean, 0.36),
            sh: (lean, -0.30),
            head: (lean * 1.5, -0.64),
            hw: 0.15,
            sw: 0.24,
            look: 0.1,
            back: 1.0,
            hands: [(-0.36, -1.02 + 0.36 * c0), (0.36, -1.02 + 0.36 * c1)],
            feet: [(-0.36, 0.58 + 0.22 * c1), (0.36, 0.58 + 0.22 * c0)],
            eb: [-0.7, 0.7],
            kb: [1.0, -1.0],
            mouth: 0.0,
        }
    }

    /// Hanging on a liana; `u` runs from 0 to 1 through the swing.
    fn hang(u: f64) -> Pose {
        let v = (PI * u).sin();
        let f = 0.50 * u - 0.32 * v; // the legs trail at the bottom and fly forward at the end
        let far = mixp((0.03, -REACH + 0.2), (0.58, -0.84), smooth((u - 0.55) / 0.35));
        Pose {
            hip: (-0.05 - 0.07 * v, 0.30),
            sh: (0.0, -0.40),
            head: (0.27, -0.50),
            hw: 0.04,
            sw: 0.05,
            look: 0.8,
            back: 0.0,
            hands: [far, (0.0, -REACH)],
            feet: [(0.04 + f, 0.94), (0.24 + f, 0.84)],
            eb: [0.3, 0.3],
            kb: [-1.0, -1.0],
            mouth: 0.0,
        }
    }

    /// Dropping to the ground: arms thrown up, legs tucked.
    fn fall(t: f64) -> Pose {
        let w = (t * 16.0).sin() * 0.04;
        Pose {
            hip: (-0.04, 0.30),
            sh: (0.0, -0.32),
            head: (0.10, -0.64),
            hw: 0.06,
            sw: 0.10,
            look: 0.7,
            back: 0.0,
            hands: [(-0.42 + w, -0.92), (0.50 - w, -0.86)],
            feet: [(-0.14, 0.80), (0.24, 0.76)],
            eb: [-0.6, 0.6],
            kb: [-1.0, -1.0],
            mouth: 0.6,
        }
    }
}

/// What a gorilla is doing about the trees.
#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Ground,
    /// walking to the foot of the tree where the liana `vine` can be reached
    Approach { vine: usize, dir: f64, since: f64 },
    Climb { vine: usize, dir: f64, u: f64, dur: f64, x0: f64, y0: f64, k0: f64, pull: f64 },
    /// on the liana; `next` is the one it will grab afterwards
    Swing { vine: usize, next: Option<usize>, dir: f64, u: f64, pull: f64 },
    /// letting go of one liana and catching the next
    Reach { vine: usize, dir: f64, u: f64, x0: f64, y0: f64, tilt0: f64 },
    Drop { dir: f64, tt: f64, x0: f64, y0: f64, y1: f64, k0: f64, k1: f64 },
}

/// A baby gorilla (sub-agent) following its parent.
#[derive(Clone, Copy)]
struct Kid {
    x: f64,
    y: f64,
    face: f64,
    step: f64,
    moving: f64,
}

#[derive(Clone)]
pub struct Gorilla {
    b: Base,
    mode: Mode,
    /// number of the world layout the mode belongs to (a resize makes new trees)
    epoch: u64,
    next_swing: f64,
    pose: Pose,
    /// rotation of the whole body (hanging on a liana)
    tilt: f64,
    /// depth factor while off the ground
    air_k: f64,
    /// height of the ground below while off the ground (body centre)
    ground_y: f64,
    /// chest beat after a tool call (seconds left)
    beat: f64,
    forage: f64,
    /// 1..0 after landing
    land: f64,
    blink_at: f64,
    kids: Vec<Kid>,
}

struct Tree {
    x: f64,
    base: f64,
    w: f64,
}

/// A liana to swing on: hangs from (ax, ay); `len` reaches down to the body centre of a swinging gorilla.
struct Vine {
    ax: f64,
    ay: f64,
    len: f64,
    /// angle from the vertical at both ends of a swing
    th0: f64,
    ang: f64,
    vel: f64,
    busy: bool,
    ph: f64,
}

impl Vine {
    fn at(&self, th: f64, l: f64) -> P {
        (self.ax + l * th.sin(), self.ay + l * th.cos())
    }
}

/// A leaf kicked up by a landing gorilla.
struct Bit {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    rot: f64,
    age: f64,
    life: f64,
}

struct Fern {
    x: f64,
    y: f64,
    size: f64,
    ph: f64,
    col: usize,
}

pub struct Gorillas {
    bg: Option<Picture>,
    epoch: u64,
    trees: Vec<Tree>,
    vines: Vec<Vine>,
    bits: Vec<Bit>,
    ferns: Vec<Fern>,
    /// thin lianas swaying in the background: x, length, phase
    danglers: Vec<(f64, f64, f64)>,
    /// motes in the light: x, y (fractions), phase, speed
    motes: Vec<(f64, f64, f64, f64)>,
    /// butterflies: centre, radius, phase, colour
    flies: Vec<(f64, f64, f64, f64, Rgb)>,
    /// one bank of mist, painted once and shown at several places
    fog: Option<Picture>,
    /// mist banks: y, phase, speed, opacity
    mist: Vec<(f64, f64, f64, f64)>,
}

impl ActorT<Gorillas> for Gorilla {
    const BASE_RADIUS: f64 = 74.0;
    const WALKER: bool = true;

    fn new(base: Base, env: &Env, world: &mut Gorillas) -> Self {
        let first = match base.look() {
            Look::Idle => env.rng.uniform(25.0, 70.0),
            _ => env.rng.uniform(4.0, 22.0),
        };
        Gorilla {
            b: base,
            mode: Mode::Ground,
            epoch: world.epoch,
            next_swing: env.t + first,
            pose: Pose::walk(0.0, 0.0, 0.0, 0.0),
            tilt: 0.0,
            air_k: 1.0,
            ground_y: 0.0,
            beat: 0.0,
            forage: 0.0,
            land: 0.0,
            blink_at: env.rng.uniform(1.0, 4.0),
            kids: Vec::new(),
        }
    }

    fn base(&self) -> &Base {
        &self.b
    }

    fn base_mut(&mut self) -> &mut Base {
        &mut self.b
    }

    fn companion_reset(&mut self) {
        self.mode = Mode::Ground;
        self.tilt = 0.0;
        self.beat = 0.0;
        self.forage = 0.0;
        self.land = 0.0;
        self.kids.clear();
        self.pose = Pose::walk(self.b.step, self.b.moving, 0.0, 0.0);
    }

    fn companion_spots(&self, _env: &Env, _t: f64) -> Vec<(f64, f64, f64)> {
        self.kids.iter().map(|k| (k.x, k.y, if k.face >= 0.0 { -PI / 2.0 } else { PI / 2.0 })).collect()
    }

    /// Baby gorillas: each has its own place and gait, so they also look right while the parent
    /// is up in the trees (they follow on the ground below and sit down to watch).
    fn draw_companions(&self, cr: &mut Canvas, env: &Env, world: &Gorillas, t: f64) {
        let b = &self.b;
        if b.alpha < 0.05 || self.kids.is_empty() {
            return;
        }
        let info = Rc::new(Session { id: b.info.id.clone(), state: State::Working, ..Session::default() });
        let air = self.airborne();
        for (i, kid) in self.kids.iter().enumerate() {
            let mut c = self.clone();
            c.companion_reset();
            c.b.x = kid.x;
            c.b.y = kid.y;
            c.b.vx = 0.0;
            c.b.vy = 0.0;
            c.b.radius = b.radius * Self::COMPANION_SCALE;
            c.b.color = WORKING_COLOR;
            c.b.info = info.clone();
            c.b.hop = 0.0;
            c.b.graze = 0.0;
            c.b.flicker = 1.0;
            c.b.face = kid.face;
            c.b.seed = b.seed + i as f64 * 3.1;
            let walk = Pose::walk(kid.step, kid.moving, 0.0, t);
            c.pose = if air { Pose::sit(t, c.b.seed, 0.0).mix(&walk, smooth(kid.moving * 2.0)) } else { walk };
            c.draw(cr, env, world, t);
        }
    }

    fn mv(&mut self, env: &mut Env, world: &mut Gorillas, others: &[Other], dt: f64, t: f64, target: Option<(f64, f64)>) {
        let s = env.s;
        let st = self.b.state();
        let r0 = self.b.radius; // not yet scaled with the depth
        if self.epoch != world.epoch {
            // the screen changed: trees and lianas are new, so come back down
            self.epoch = world.epoch;
            if self.mode != Mode::Ground {
                self.mode = Mode::Ground;
                self.tilt = 0.0;
                self.b.x = clamp(self.b.x, 120.0 * s, env.w - 120.0 * s);
                self.b.y = env.ground_top + r0 * 0.3 + 40.0 * s;
                self.b.vx = 0.0;
                self.b.vy = 0.0;
            }
            self.kids.clear();
        }
        self.beat = (self.beat - dt).max(0.0);
        self.land = (self.land - dt * 2.0).max(0.0);
        let may_swing = matches!(st, State::Working | State::Idle) && !self.b.dying;
        let mut want: Pose;
        let mut quick = 12.0;
        match self.mode {
            Mode::Ground => {
                // forage: walk, stop and pick something up, walk on
                let f = if st == State::Working { clamp((t * 0.45 + self.b.seed).sin() * 3.0 - 1.6, 0.0, 1.0) } else { 0.0 };
                self.forage = lerp(self.forage, f, (dt * 4.0).min(1.0));
                self.b.walker_move(env, others, dt, t, target, [72.0, 20.0, 50.0, 0.0], [0.9, 0.5, 0.5, 0.1]);
                if self.forage > 0.5 || self.beat > 0.0 || self.land > 0.3 {
                    self.b.x -= self.b.vx * dt * 0.92; // (almost) stands still
                    self.b.y -= self.b.vy * dt * 0.92;
                    self.b.moving *= 0.08;
                }
                let walk = Pose::walk(self.b.step, self.b.moving, self.forage, t);
                want = match st {
                    State::Waiting => {
                        // at its place, or as near as the others let it come
                        let slow = hypot(self.b.vx, self.b.vy) < 30.0 * s;
                        let there = slow || target.is_none_or(|(tx, ty)| hypot(tx - self.b.x, ty - self.b.y) < 220.0 * s);
                        if there { Pose::stand(t * 5.0 + self.b.seed, 0.25) } else { walk }
                    }
                    State::Blocked => Pose::hunch(t, self.b.seed),
                    State::Idle => {
                        let scratch = clamp((t * 0.37 + self.b.seed * 3.0).sin() * 5.0 - 4.0, 0.0, 1.0);
                        walk.mix(&Pose::sit(t, self.b.seed, scratch), smooth(self.b.graze * 1.3))
                    }
                    _ => walk,
                };
                if self.beat > 0.0 {
                    want = Pose::stand(t * 17.0, 1.0); // a tool call: drum the chest
                    quick = 16.0;
                }
                self.ground_y = self.b.y;
                if may_swing && target.is_none() && self.beat <= 0.0 && t > self.next_swing {
                    self.next_swing = t + 3.0;
                    if let Some((vine, dir)) = world.pick_vine(env, self.b.x) {
                        world.vines[vine].busy = true;
                        self.mode = Mode::Approach { vine, dir, since: t };
                    }
                }
            }
            Mode::Approach { vine, dir, since } => {
                // walk to the foot of the tree on the side where the liana hangs
                let tree = &world.trees[if dir > 0.0 { vine } else { vine + 1 }];
                let (fx, fy) = (tree.x + dir * (tree.w * 0.5 + r0 * 0.2), env.ground_top + r0 * 0.3 + 14.0 * s);
                let (dx, dy) = (fx - self.b.x, fy - self.b.y);
                let d = hypot(dx, dy).max(0.001);
                let sp = (105.0 * s).min(d * 2.5 + 12.0 * s);
                let k = (dt * 3.0).min(1.0);
                self.b.vx = lerp(self.b.vx, dx / d * sp, k);
                self.b.vy = lerp(self.b.vy, dy / d * sp, k);
                self.b.x += self.b.vx * dt;
                self.b.y += self.b.vy * dt;
                let v = hypot(self.b.vx, self.b.vy);
                self.b.moving = clamp(v / (25.0 * s), 0.0, 1.0);
                self.b.step += dt * v / (r0 * 0.35).max(1.0);
                if self.b.vx.abs() > 12.0 * s {
                    self.b.face = lerp(self.b.face, self.b.vx.signum(), (dt * 4.0).min(1.0));
                }
                self.forage = 0.0;
                self.ground_y = self.b.y;
                want = Pose::walk(self.b.step, self.b.moving, 0.0, t);
                if !may_swing || target.is_some() || t - since > 14.0 {
                    self.mode = Mode::Ground;
                    self.next_swing = t + env.rng.uniform(8.0, 16.0);
                } else if d < 9.0 * s {
                    self.air_k = self.b.depth_scale(env);
                    let v = &world.vines[vine];
                    let c0 = v.at(-dir * v.th0, v.len);
                    let dur = clamp((self.b.y - c0.1) / (175.0 * s), 1.5, 3.4);
                    self.mode = Mode::Climb { vine, dir, u: 0.0, dur, x0: self.b.x, y0: self.b.y, k0: self.air_k, pull: v.ang };
                }
            }
            Mode::Climb { vine, dir, u, dur, x0, y0, k0, pull } => {
                let u = (u + dt / dur).min(1.0);
                // the lianas hang nearer to us than the trunks: grow a little on the way up
                self.air_k = lerp(k0, AIR_K.max(k0), smooth(u));
                let rd = r0 * self.air_k;
                let tree = &world.trees[if dir > 0.0 { vine } else { vine + 1 }];
                let v = &mut world.vines[vine];
                let ths = -dir * v.th0;
                let c0 = v.at(ths, v.len);
                let cx = tree.x + dir * tree.w * 0.32;
                self.b.x = if u < 0.75 { lerp(x0, cx, smooth(u / 0.2)) } else { lerp(cx, c0.0, smooth((u - 0.75) / 0.25)) };
                self.b.y = lerp(y0, c0.1, smooth(u));
                self.b.step += dt * 7.5 * (1.0 - smooth((u - 0.8) / 0.2));
                self.b.moving = 1.0;
                // the liana is pulled over to the trunk, then the gorilla turns round and takes it
                let pull = if u < 0.45 { v.ang } else { pull };
                if u >= 0.45 {
                    v.ang = lerp(pull, ths, smooth((u - 0.45) / 0.4));
                    v.vel = 0.0;
                }
                let turn = smooth((u - 0.78) / 0.2);
                self.tilt = -ths * turn;
                self.b.face = lerp(self.b.face, dir, (dt * 6.0).min(1.0));
                want = Pose::climb(self.b.step);
                if u > 0.72 {
                    let mut hang = Pose::hang(0.0);
                    let grip = v.at(v.ang, v.len - REACH * rd);
                    hang.hands[1] = self.to_local(grip, rd);
                    want = want.mix(&hang, smooth((u - 0.72) / 0.2));
                }
                self.mode = if u >= 1.0 {
                    let next = if st == State::Working && may_swing { world.next_vine(env, vine, dir, 0.5) } else { None };
                    let pull = next.map_or(0.0, |j| world.vines[j].ang);
                    Mode::Swing { vine, next, dir, u: 0.0, pull }
                } else {
                    Mode::Climb { vine, dir, u, dur, x0, y0, k0, pull }
                };
            }
            Mode::Swing { vine, next, dir, u, pull } => {
                // half a pendulum period: slow at both ends, fastest at the bottom
                let v = &mut world.vines[vine];
                let period = PI * (v.len / (1000.0 * s)).sqrt() * 1.15;
                let u = (u + dt / period).min(1.0);
                let th = -dir * v.th0 * (PI * u).cos();
                v.ang = th;
                v.vel = 0.0;
                let c = v.at(th, v.len);
                self.b.x = c.0;
                self.b.y = c.1;
                self.tilt = -th;
                self.b.face = lerp(self.b.face, dir, (dt * 12.0).min(1.0));
                self.b.moving = 0.0;
                let mut pull = pull;
                if let Some(j) = next {
                    // the next liana comes swinging towards the hand
                    let n = &mut world.vines[j];
                    if u < 0.3 {
                        pull = n.ang;
                    } else {
                        n.ang = lerp(pull, -dir * n.th0, smooth((u - 0.3) / 0.6));
                        n.vel = 0.0;
                    }
                }
                want = Pose::hang(if next.is_some() { u } else { u * 0.75 });
                quick = 18.0;
                self.mode = if u < 1.0 {
                    Mode::Swing { vine, next, dir, u, pull }
                } else if let Some(j) = next {
                    Mode::Reach { vine: j, dir, u: 0.0, x0: c.0, y0: c.1, tilt0: self.tilt }
                } else {
                    // let go at the top of the swing and drop to the foot of the tree
                    let y1 = env.ground_top + r0 * 0.3 + env.rng.uniform(16.0, 46.0) * s;
                    let (top, bottom) = (env.ground_top, env.h - env.strip_h);
                    let k1 = lerp(0.65, 1.15, clamp((y1 - top) / (bottom - top).max(1.0), 0.0, 1.0));
                    self.ground_y = y1;
                    Mode::Drop { dir, tt: 0.0, x0: c.0, y0: c.1, y1, k0: self.air_k, k1 }
                };
            }
            Mode::Reach { vine, dir, u, x0, y0, tilt0 } => {
                let rd = r0 * self.air_k;
                let u = (u + dt / 0.46).min(1.0);
                let v = &mut world.vines[vine];
                let ths = -dir * v.th0;
                v.ang = ths;
                v.vel = 0.0;
                let c0 = v.at(ths, v.len);
                let e = smooth(u);
                self.b.x = lerp(x0, c0.0, e);
                self.b.y = lerp(y0, c0.1, e) - (PI * u).sin() * rd * 0.28;
                self.tilt = lerp(tilt0, -ths, e);
                self.b.face = dir;
                let mut hang = Pose::hang(0.0);
                hang.hands[0] = (-0.30, -0.95);
                hang.hands[1] = mixp((0.62, -0.95), self.to_local(v.at(ths, v.len - REACH * rd), rd), smooth(u * 1.6));
                want = hang;
                quick = 22.0;
                self.mode = if u >= 1.0 {
                    let next = if st == State::Working && may_swing { world.next_vine(env, vine, dir, 0.3) } else { None };
                    let pull = next.map_or(0.0, |j| world.vines[j].ang);
                    Mode::Swing { vine, next, dir, u: 0.0, pull }
                } else {
                    Mode::Reach { vine, dir, u, x0, y0, tilt0 }
                };
            }
            Mode::Drop { dir, tt, x0, y0, y1, k0, k1 } => {
                let tt = tt + dt;
                let y = y0 + 0.5 * 1500.0 * s * tt * tt;
                let f = clamp((y - y0) / (y1 - y0).max(1.0), 0.0, 1.0);
                self.b.x = x0 + dir * 26.0 * s * smooth(tt * 2.0);
                self.b.y = y.min(y1);
                self.air_k = lerp(k0, k1, f);
                self.tilt = lerp(self.tilt, 0.0, (dt * 7.0).min(1.0));
                self.b.face = dir;
                want = Pose::fall(t);
                if y >= y1 {
                    // landed: crouch, a few leaves fly up
                    self.mode = Mode::Ground;
                    self.tilt = 0.0;
                    self.land = 1.0;
                    self.b.vx = 0.0;
                    self.b.vy = 0.0;
                    self.b.heading = if dir > 0.0 { 0.0 } else { PI };
                    self.next_swing = t + swing_pause(&env.rng, self.b.look());
                    let rd = r0 * k1;
                    for _ in 0..7 {
                        world.bits.push(Bit {
                            x: self.b.x + env.rng.uniform(-0.6, 0.6) * rd,
                            y: y1 + rd * 0.9,
                            vx: env.rng.uniform(-110.0, 110.0) * s,
                            vy: -env.rng.uniform(90.0, 230.0) * s,
                            rot: env.rng.uniform(0.0, TAU),
                            age: 0.0,
                            life: env.rng.uniform(0.6, 1.1),
                        });
                    }
                    want = Pose::walk(self.b.step, 0.0, 0.0, t);
                } else {
                    self.mode = Mode::Drop { dir, tt, x0, y0, y1, k0, k1 };
                }
            }
        }
        if self.mode == Mode::Ground && !may_swing {
            self.next_swing = self.next_swing.max(t + 6.0);
        }
        self.pose = self.pose.mix(&want, (dt * quick).min(1.0));
        if matches!(self.mode, Mode::Swing { .. }) {
            self.pose.hands[1] = (0.0, -REACH); // the hand stays exactly on the liana
        }
        if t > self.blink_at + 0.2 {
            self.blink_at = t + env.rng.uniform(2.0, 6.0);
        }
        // Off the ground the size must not follow the height: the stage multiplies the radius with
        // the depth factor of `y` after this, so prepare it to end up at the factor we want.
        let kd = self.b.depth_scale(env);
        let k = if self.airborne() { self.air_k } else { kd };
        self.b.radius *= k / kd;
        self.move_kids(env, r0 * k, dt);
    }

    fn draw(&self, cr: &mut Canvas, env: &Env, _world: &Gorillas, t: f64) {
        let s = env.s;
        let b = &self.b;
        let (r, c, st) = (b.radius, b.color, b.state());
        if st == State::Rest {
            self.draw_asleep(cr, s, t, r * 1.35, (b.alpha / 0.8).min(1.0));
            return;
        }
        // blocked: the glow flickers hard, the body only a little (so the limbs do not show through)
        let a = b.alpha * b.flicker;
        let body_a = b.alpha * lerp(1.0, b.flicker, 0.35);
        if b.alpha < 0.01 {
            return;
        }
        let air = self.airborne();
        let gy = if air { self.ground_y + r * 0.95 } else { b.y + r * 0.95 };
        let near = if air { clamp(1.0 - (gy - b.y - r) / (r * 7.0), 0.15, 1.0) } else { 1.0 };
        // glow on the moss + shadow
        let mut g = Gradient::radial(b.x, gy, 0.0, b.x, gy, r * 1.9);
        g.stop(0.0, c, if st != State::Idle { 0.55 } else { 0.36 } * a * near);
        g.stop(1.0, c, 0.0);
        cr.save();
        cr.translate(b.x, gy);
        cr.scale(1.0, 0.3);
        cr.translate(-b.x, -gy);
        cr.set_source(&g);
        cr.circle(b.x, gy, r * 1.9);
        cr.fill();
        cr.restore();
        cr.ellipse(b.x, gy, r * 0.85 * near, r * 0.13 * near);
        cr.set_source_rgba(0.0, 0.02, 0.0, 0.38 * a * near);
        cr.fill();
        cr.glow(b.x, b.y - r * 0.1, r * 1.7, c, if air { 0.30 } else { 0.16 } * a, 0.0);
        if st == State::Waiting {
            cr.pulse_rings(b.x, b.y - r * 0.2, r, c, a, t, s);
        }
        cr.save();
        cr.translate(b.x, b.y);
        cr.rotate(self.tilt);
        // turning round: quick in profile, and a figure seen from the front or behind never gets thin
        let thin = b.face.abs().powf(0.4).max(0.15).max(1.0 - self.pose.look * 1.2);
        cr.scale(thin.copysign(b.face), 1.0);
        if self.land > 0.0 {
            // crouch after landing
            let q = (self.land * PI).sin().max(0.0) * 0.5 + self.land * 0.5;
            cr.translate(0.0, r * 0.95);
            cr.scale(1.0 + 0.10 * q, 1.0 - 0.22 * q);
            cr.translate(0.0, -r * 0.95);
        }
        let look = Looks {
            c,
            blink: self.blink_at < t && t < self.blink_at + 0.2,
            angry: st == State::Blocked,
            stalk: if st == State::Idle && self.mode == Mode::Ground { smooth(b.graze * 1.3) } else { 0.0 },
            edge: (r * 0.05).min(3.6 * s),
            t,
        };
        figure(cr, &self.pose, r, 3.4 * s, Some(c), 0.9 * a, &look); // rim light
        figure(cr, &self.pose, r, 0.0, None, body_a, &look);
        cr.restore();
    }
}

/// How long until the next swing.
fn swing_pause(rng: &Rng, look: Look) -> f64 {
    match look {
        Look::Working => rng.uniform(10.0, 25.0),
        Look::Background => rng.uniform(12.0, 28.0),
        _ => rng.uniform(50.0, 120.0),
    }
}

impl Gorilla {
    fn airborne(&self) -> bool {
        !matches!(self.mode, Mode::Ground | Mode::Approach { .. })
    }

    /// The lianas this gorilla holds or is about to use.
    fn vines_in_use(&self) -> [Option<usize>; 2] {
        match self.mode {
            Mode::Approach { vine, .. } | Mode::Climb { vine, .. } | Mode::Reach { vine, .. } => [Some(vine), None],
            Mode::Swing { vine, next, .. } => [Some(vine), next],
            _ => [None, None],
        }
    }

    /// A point of the scene in the coordinates of the figure (units of the radius).
    fn to_local(&self, p: P, rd: f64) -> P {
        let (dx, dy) = (p.0 - self.b.x, p.1 - self.b.y);
        let (sn, cs) = self.tilt.sin_cos();
        let face = if self.b.face >= 0.0 { 1.0 } else { -1.0 };
        ((dx * cs + dy * sn) / rd * face, (-dx * sn + dy * cs) / rd)
    }

    /// The babies trot behind on the ground; under a swinging parent they run along below.
    fn move_kids(&mut self, env: &Env, rd: f64, dt: f64) {
        let n = self.b.info.subagents.len();
        self.kids.truncate(n);
        let air = self.airborne();
        let gy = if air { self.ground_y } else { self.b.y };
        let back = if self.b.face >= 0.0 { -1.0 } else { 1.0 };
        for i in 0..n {
            let fi = i as f64;
            let tx = if air {
                self.b.x + (fi - (n as f64 - 1.0) / 2.0) * rd * 0.9
            } else {
                self.b.x + back * rd * (1.25 + fi * 0.72)
            };
            let ty = gy + rd * (0.36 + 0.16 * (i % 2) as f64);
            if i >= self.kids.len() {
                self.kids.push(Kid { x: tx, y: ty, face: -back, step: fi * 2.0, moving: 0.0 });
                continue;
            }
            let kid = &mut self.kids[i];
            let k = 1.0 - (-dt * 3.2).exp();
            let (dx, dy) = ((tx - kid.x) * k, (ty - kid.y) * k);
            kid.x += dx;
            kid.y += dy;
            let v = hypot(dx, dy) / dt.max(1e-4);
            kid.moving = lerp(kid.moving, clamp(v / (22.0 * env.s), 0.0, 1.0), (dt * 6.0).min(1.0));
            kid.step += dt * v / (rd * 0.42 * 0.35).max(1.0);
            if v > 14.0 * env.s {
                kid.face = lerp(kid.face, dx.signum(), (dt * 6.0).min(1.0));
            } else if !air {
                kid.face = lerp(kid.face, -back, (dt * 3.0).min(1.0));
            }
        }
    }

    /// Resting: asleep on its side on a nest of leaves, breathing slowly.
    fn draw_asleep(&self, cr: &mut Canvas, s: f64, t: f64, r: f64, a: f64) {
        let b = &self.b;
        if a < 0.01 {
            return;
        }
        let c = b.color;
        let side = if b.face >= 0.0 { 1.0 } else { -1.0 };
        let breath = (t * 1.1 + b.seed).sin();
        let ny = b.y + r * 0.80;
        cr.ellipse(b.x, ny + r * 0.08, r * 1.7, r * 0.26);
        cr.set_source_rgba(0.0, 0.02, 0.0, 0.35 * a);
        cr.fill();
        cr.glow(b.x, b.y + r * 0.3, r * 1.9, c, 0.14 * a, 0.0);
        // the nest: a ring of big leaves, the back half first
        let nest = |cr: &mut Canvas, front: bool| {
            for i in 0..11 {
                let f = i as f64 / 10.0;
                let ang = if front { PI * (0.08 + 0.84 * f) } else { PI + PI * f };
                let (ca, sa) = ang.sin_cos();
                let (lx, ly) = (b.x + sa * r * 0.95, ny + ca * r * 0.16);
                let dir = (ca * 0.42).atan2(sa);
                leaf_path(cr, lx, ly, r * (0.62 + 0.12 * ((i * 7) % 3) as f64), r * 0.17, dir);
                cr.set_rgba(GREENS[(i + front as usize) % 3], a);
                cr.fill();
            }
        };
        nest(cr, false);
        cr.save();
        cr.translate(b.x, b.y);
        cr.scale(side, 1.0);
        cr.set_line_cap(LineCap::Round);
        cr.set_line_join(LineJoin::Round);
        let lift = breath * 0.022;
        let fur = lerp3(FUR, c, 0.06);
        for pass in 0..2 {
            let e = if pass == 0 { 3.2 * s } else { 0.0 };
            // rump, back, shoulder and head as one dark heap
            cr.circle(-r * 0.66, r * 0.36, r * 0.40 + e);
            cr.ellipse(-r * 0.08, r * (0.22 - lift), r * 0.80 + e, r * (0.50 + lift) + e);
            cr.circle(r * 0.40, r * (0.18 - lift), r * 0.45 + e);
            cr.circle(r * 0.93, r * 0.42, r * 0.27 + e);
            cr.ellipse(r * 0.90, r * 0.30, r * 0.20 + e, r * 0.21 + e);
            cr.set_rgba(if pass == 0 { c } else { fur }, if pass == 0 { 0.75 * a } else { a });
            cr.fill();
            // tucked leg and the arm the head rests on
            cr.set_line_width(r * 0.25 + e * 2.0);
            cr.move_to(-r * 0.62, r * 0.50);
            cr.line_to(-r * 0.20, r * 0.72);
            cr.line_to(r * 0.06, r * 0.74);
            cr.stroke();
            cr.set_rgba(if pass == 0 { c } else { lerp3(fur, FUR_HI, 0.4) }, if pass == 0 { 0.75 * a } else { a });
            cr.set_line_width(r * 0.23 + e * 2.0);
            cr.move_to(r * 0.46, r * 0.22);
            cr.line_to(r * 0.60, r * 0.72);
            cr.line_to(r * 1.04, r * 0.72);
            cr.stroke();
        }
        // silver back, rising and sinking with the breath
        cr.save();
        cr.translate(-r * 0.12, r * (-0.02 - lift * 1.6));
        cr.rotate(-0.06);
        cr.ellipse(0.0, 0.0, r * 0.52, r * 0.17);
        cr.restore();
        cr.set_rgba(lerp3(SILVER, c, 0.35), 0.8 * a);
        cr.fill();
        // sleeping face
        cr.ellipse(r * 0.98, r * 0.45, r * 0.16, r * 0.18);
        cr.set_rgba(SKIN, a);
        cr.fill();
        cr.ellipse(r * 1.02, r * 0.53, r * 0.11, r * 0.08);
        cr.set_rgba(MUZZLE, a);
        cr.fill();
        cr.set_rgba(lighten(MUZZLE, 0.25), a);
        cr.set_line_width(r * 0.022);
        for ex in [0.92, 1.05] {
            cr.new_sub_path();
            cr.arc(r * ex, r * 0.40, r * 0.04, 0.15 * PI, 0.85 * PI); // closed eyes
        }
        cr.stroke();
        cr.set_rgba(SKIN, a);
        cr.circle(r * 1.00, r * 0.51, r * 0.016);
        cr.circle(r * 1.05, r * 0.51, r * 0.016);
        cr.circle(r * 0.72, r * 0.38, r * 0.06); // ear
        cr.fill();
        cr.circle(r * 1.06, r * 0.72, r * 0.12); // fist
        cr.set_rgba(lighten(SKIN, 0.06), a);
        cr.fill();
        cr.restore();
        nest(cr, true);
        // z z z
        for k in 0..3 {
            let f = (t * 0.3 + k as f64 / 3.0) % 1.0;
            let lay = text::layout("z", (14.0 + 14.0 * f) * s, weight::BOLD, None);
            let zx = b.x + side * r * (1.0 + 0.5 * f) + (f * 6.0).sin() * 4.0 * s;
            let zy = b.y - r * 0.3 - f * r * 1.5;
            text::show(cr, &lay, zx, zy, (0.9, 1.0, 0.9), 0.8 * (PI * f).sin() * a, 0.4);
        }
    }
}

/// The things about the face and the fur that do not belong to the pose.
struct Looks {
    c: Rgb,
    blink: bool,
    angry: bool,
    /// 0..1: holds a leafy stalk in the near hand
    stalk: f64,
    /// width of the dark edge around the limbs in front (thinner than the rim light)
    edge: f64,
    t: f64,
}

/// A limb with one joint: from `a` to `b`, both segments `len` long, bending to the side `bend`.
fn limb(cr: &mut Canvas, r: f64, a: P, b: P, len: f64, bend: f64, width: f64) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let d = hypot(dx, dy).max(1e-6);
    let half = (d / 2.0).min(len);
    let off = (len * len - half * half).max(0.0).sqrt() * bend;
    let j = ((a.0 + b.0) / 2.0 - dy / d * off, (a.1 + b.1) / 2.0 + dx / d * off);
    cr.move_to(a.0 * r, a.1 * r);
    cr.line_to(j.0 * r, j.1 * r);
    cr.line_to(b.0 * r, b.1 * r);
    cr.set_line_width(width);
    cr.stroke();
}

/// The gorilla in local coordinates. With `rim` everything is drawn a little bigger in one colour
/// (the rim light); without, the real animal with its details.
fn figure(cr: &mut Canvas, p: &Pose, r: f64, extra: f64, rim: Option<Rgb>, a: f64, lk: &Looks) {
    if a < 0.01 {
        return;
    }
    let look = p.look;
    let front = look < 0.5 && p.back < 0.5;
    let behind = p.back >= 0.5; // hugging a trunk: all limbs are on the far side of the body
    let fur = lerp3(FUR, lk.c, 0.07);
    let col = |base: Rgb| rim.unwrap_or(base);
    let far_c = lerp3(fur, FUR_FAR, look);
    let near_c = lerp3(fur, FUR_HI, 0.30);
    let spread = 0.12 * (1.0 - look);
    let shs = [(p.sh.0 - p.sw - spread, p.sh.1 + 0.02), (p.sh.0 + p.sw + spread, p.sh.1 + 0.02)];
    let hips = [(p.hip.0 - p.hw - spread * 0.8, p.hip.1 + 0.04), (p.hip.0 + p.hw + spread * 0.8, p.hip.1 + 0.04)];
    let skin = lighten(SKIN, 0.07);
    cr.set_line_cap(LineCap::Round);
    cr.set_line_join(LineJoin::Round);
    let edge = lerp3(FUR_FAR, lk.c, 0.05);
    let arm = |cr: &mut Canvas, k: usize, c: Rgb| {
        if rim.is_none() && c == near_c {
            cr.set_rgba(edge, 0.8 * a); // a dark edge sets the limb off against the body
            limb(cr, r, shs[k], p.hands[k], ARM, p.eb[k], r * 0.26 + lk.edge);
        }
        cr.set_rgba(col(c), a);
        limb(cr, r, shs[k], p.hands[k], ARM, p.eb[k], r * 0.26 + extra * 2.0);
        cr.set_rgba(col(skin), a);
        cr.circle(p.hands[k].0 * r, p.hands[k].1 * r, r * 0.13 + extra);
        cr.fill();
    };
    let leg = |cr: &mut Canvas, k: usize, c: Rgb| {
        if rim.is_none() && c == near_c {
            cr.set_rgba(edge, 0.8 * a);
            limb(cr, r, hips[k], p.feet[k], LEG, p.kb[k], r * 0.28 + lk.edge);
        }
        cr.set_rgba(col(c), a);
        limb(cr, r, hips[k], p.feet[k], LEG, p.kb[k], r * 0.28 + extra * 2.0);
        cr.set_rgba(col(skin), a);
        cr.ellipse((p.feet[k].0 + 0.05 * look) * r, p.feet[k].1 * r, r * 0.15 + extra, r * 0.085 + extra);
        cr.fill();
    };
    if behind {
        for k in 0..2 {
            leg(cr, k, far_c);
            arm(cr, k, far_c);
        }
    } else if !front {
        leg(cr, 0, far_c);
        arm(cr, 0, far_c);
    }
    // torso: heavy shoulders, a smaller rump, the belly between
    let (ax, ay) = (p.sh.0 - p.hip.0, p.sh.1 - p.hip.1);
    let d = hypot(ax, ay).max(1e-6);
    let n = (ay / d, -ax / d); // towards the back
    let (srx, sry) = (0.34 + p.sw, 0.40);
    let (hrx, hry) = (0.30 + p.hw, 0.31);
    let e = extra / r;
    let es = hypot(srx * n.0, sry * n.1) * 0.97 + e;
    let eh = hypot(hrx * n.0, hry * n.1) * 0.97 + e;
    let mut quad = [
        (p.sh.0 + n.0 * es, p.sh.1 + n.1 * es),
        (p.sh.0 - n.0 * es, p.sh.1 - n.1 * es),
        (p.hip.0 - n.0 * eh, p.hip.1 - n.1 * eh),
        (p.hip.0 + n.0 * eh, p.hip.1 + n.1 * eh),
    ];
    // the same turning sense as the ellipses, or the overlap would become a hole
    let area: f64 = (0..4).map(|i| quad[i].0 * quad[(i + 1) % 4].1 - quad[(i + 1) % 4].0 * quad[i].1).sum();
    if area < 0.0 {
        quad.reverse();
    }
    cr.ellipse(p.sh.0 * r, p.sh.1 * r, srx * r + extra, sry * r + extra);
    cr.ellipse(p.hip.0 * r, p.hip.1 * r, hrx * r + extra, hry * r + extra);
    let belly = mixp(p.hip, p.sh, 0.45);
    cr.ellipse((belly.0 - n.0 * 0.05) * r, (belly.1 - n.1 * 0.05) * r, (srx + hrx) * 0.5 * r + extra, (sry + hry) * 0.5 * r + extra);
    cr.move_to(quad[0].0 * r, quad[0].1 * r);
    for q in &quad[1..] {
        cr.line_to(q.0 * r, q.1 * r);
    }
    cr.close_path();
    cr.set_rgba(col(fur), a);
    cr.fill();
    if rim.is_none() {
        // silver saddle on the back, tinted with the state colour
        let m = mixp(p.hip, p.sh, 0.40);
        cr.save();
        cr.translate((m.0 + n.0 * 0.15) * r, (m.1 + n.1 * 0.15) * r);
        cr.rotate(ay.atan2(ax));
        cr.ellipse(0.0, 0.0, r * d * 0.38, r * 0.145);
        cr.restore();
        cr.set_rgba(lerp3(SILVER, lk.c, 0.5), 0.85 * a * smooth(look * 1.5 - 0.3));
        cr.fill();
        if p.back > 0.05 {
            // seen from behind the whole saddle shows
            cr.ellipse(m.0 * r, (m.1 - 0.02) * r, (srx - 0.10) * r, r * d * 0.36);
            cr.set_rgba(lerp3(SILVER, lk.c, 0.5), 0.85 * a * p.back);
            cr.fill();
        }
        if front {
            // bare chest and a faint tint of the state colour on the shoulders
            let fa = a * (1.0 - look * 2.0).max(0.0) * (1.0 - p.back);
            cr.ellipse((p.sh.0 - 0.17) * r, (p.sh.1 + 0.16) * r, r * 0.19, r * 0.15);
            cr.ellipse((p.sh.0 + 0.17) * r, (p.sh.1 + 0.16) * r, r * 0.19, r * 0.15);
            cr.set_rgba(lerp3(FUR_FAR, lk.c, 0.10), 0.85 * fa);
            cr.fill();
            cr.ellipse(p.sh.0 * r, (p.sh.1 - 0.20) * r, (srx - 0.06) * r, r * 0.12);
            cr.set_rgba(lerp3(SILVER, lk.c, 0.55), 0.45 * fa);
            cr.fill();
        }
    }
    // head: skull with the high crest, the muzzle sticks out in profile
    let (hx, hy) = (p.head.0 * r, p.head.1 * r);
    cr.circle(hx, hy, r * 0.265 + extra);
    cr.ellipse(hx - r * 0.07 * look, hy - r * 0.15, r * 0.20 + extra, r * 0.21 + extra);
    cr.ellipse(hx + r * 0.14 * look, hy + r * 0.10, r * 0.15 + extra, r * 0.13 + extra);
    if rim.is_none() {
        cr.set_rgba(edge, 0.8 * a);
        cr.set_line_width(lk.edge);
        cr.stroke_preserve();
    }
    cr.set_rgba(col(lerp3(fur, FUR_HI, 0.12)), a);
    cr.fill();
    if rim.is_none() && p.back < 0.5 {
        face(cr, hx, hy, r, look, a, p.mouth, lk);
    }
    if behind {
        return;
    }
    if front {
        leg(cr, 0, near_c);
    }
    leg(cr, 1, near_c);
    if rim.is_none() && lk.stalk > 0.3 {
        // the leafy stalk it chews on
        let (sx, sy) = (p.hands[1].0 * r, p.hands[1].1 * r);
        let sa = a * smooth((lk.stalk - 0.3) / 0.4);
        cr.set_rgba((0.30, 0.50, 0.20), sa);
        cr.set_line_width(r * 0.045);
        cr.move_to(sx + r * 0.05, sy + r * 0.20);
        cr.quad_to(sx - r * 0.02, sy - r * 0.15, sx - r * 0.14, sy - r * 0.42);
        cr.stroke();
        for (k, f) in [0.35, 0.6, 0.85, 1.0].iter().enumerate() {
            let (lx, ly) = (sx - r * 0.14 * f, sy - r * 0.42 * f);
            let side = if k % 2 == 0 { 1.0 } else { -1.0 };
            leaf_path(cr, lx, ly, r * 0.24, r * 0.07, -PI / 2.0 + side * 0.9 + (lk.t * 5.0).sin() * 0.05);
        }
        cr.set_rgba((0.36, 0.62, 0.24), sa);
        cr.fill();
    }
    arm(cr, 1, near_c);
    if front {
        arm(cr, 0, near_c);
    }
}

/// Dark bare face with the brow ridge, small eyes, nostrils and mouth.
#[allow(clippy::too_many_arguments)]
fn face(cr: &mut Canvas, hx: f64, hy: f64, r: f64, look: f64, a: f64, mouth: f64, lk: &Looks) {
    let fx = hx + r * 0.09 * look;
    let ear = lighten(SKIN, 0.10);
    cr.circle(hx + r * lerp(-0.26, -0.10, look), hy + r * 0.02, r * 0.055);
    cr.circle(hx + r * lerp(0.26, -0.10, look), hy + r * 0.02, r * 0.055);
    cr.set_rgba(ear, a);
    cr.fill();
    cr.ellipse(fx, hy + r * 0.035, r * (0.175 - 0.035 * look), r * 0.185);
    cr.set_rgba(lighten(SKIN, 0.03), a);
    cr.fill();
    let mx = hx + r * 0.14 * look;
    cr.ellipse(mx, hy + r * 0.115, r * 0.118, r * 0.088);
    cr.set_rgba(MUZZLE, a);
    cr.fill();
    if mouth > 0.05 {
        cr.ellipse(mx, hy + r * 0.15, r * 0.07, r * 0.06 * mouth);
        cr.set_source_rgba(0.30, 0.05, 0.06, a);
        cr.fill();
    } else {
        cr.set_rgba(SKIN, 0.9 * a);
        cr.set_line_width(r * 0.018);
        cr.move_to(mx - r * 0.07, hy + r * 0.15);
        cr.quad_to(mx, hy + r * if lk.angry { 0.13 } else { 0.17 }, mx + r * 0.07, hy + r * 0.15);
        cr.stroke();
    }
    cr.set_rgba(SKIN, a);
    cr.circle(mx - r * 0.03, hy + r * 0.085, r * 0.017);
    cr.circle(mx + r * 0.03, hy + r * 0.085, r * 0.017);
    cr.fill();
    // eyes in the shadow of the brow
    let gap = r * 0.068 * (1.0 - 0.3 * look);
    let ey = hy - r * 0.015;
    if lk.blink {
        cr.set_rgba(lighten(MUZZLE, 0.1), a);
        cr.set_line_width(r * 0.018);
        for sx in [-1.0, 1.0] {
            cr.move_to(fx + sx * gap - r * 0.025, ey);
            cr.line_to(fx + sx * gap + r * 0.025, ey);
        }
        cr.stroke();
    } else {
        cr.set_source_rgba(0.80, 0.52, 0.20, a);
        cr.circle(fx - gap, ey, r * 0.03);
        cr.circle(fx + gap, ey, r * 0.03);
        cr.fill();
        cr.set_source_rgba(0.02, 0.02, 0.02, a);
        cr.circle(fx - gap + r * 0.006 * look, ey, r * 0.016);
        cr.circle(fx + gap + r * 0.006 * look, ey, r * 0.016);
        cr.fill();
    }
    let drop = if lk.angry { 0.035 } else { 0.0 };
    cr.set_rgba(lerp3(FUR, FUR_HI, 0.25), a);
    cr.set_line_width(r * 0.06);
    cr.move_to(fx - r * 0.14, hy - r * 0.065);
    cr.quad_to(fx, hy - r * (0.085 - drop * 1.6), fx + r * 0.14, hy - r * 0.065);
    cr.stroke();
}

/// A pointed leaf from (x, y) in direction `ang` (only the path).
fn leaf_path(cr: &mut Canvas, x: f64, y: f64, len: f64, wid: f64, ang: f64) {
    cr.save();
    cr.translate(x, y);
    cr.rotate(ang);
    cr.move_to(0.0, 0.0);
    cr.quad_to(len * 0.45, -wid, len, 0.0);
    cr.quad_to(len * 0.45, wid, 0.0, 0.0);
    cr.close_path();
    cr.restore();
}

/// A fern: a fan of fronds from one point (path and fill).
fn fern(cr: &mut Canvas, x: f64, y: f64, size: f64, sway: f64, col: Rgb, alpha: f64) {
    for i in 0..7 {
        let f = i as f64 / 6.0;
        let ang = -PI * (0.08 + 0.84 * f) + sway * (0.6 + (f - 0.5).abs());
        leaf_path(cr, x, y, size * (0.72 + 0.28 * (PI * f).sin()), size * 0.10, ang);
    }
    cr.set_rgba(col, alpha);
    cr.fill();
}

/// A tuft of leaves fanning out around a point, mostly upwards.
#[allow(clippy::too_many_arguments)]
fn leaf_tuft(cr: &mut Canvas, rng: &Rng, x: f64, y: f64, size: f64, n: usize, a0: f64, a1: f64) {
    for i in 0..n {
        let ang = rng.uniform(a0, a1);
        let len = size * rng.uniform(0.6, 1.0);
        leaf_path(cr, x + rng.uniform(-0.2, 0.2) * size, y + rng.uniform(-0.1, 0.1) * size, len, len * 0.2, ang);
        cr.set_rgb(GREENS[(i + rng.index(2)) % 4]);
        cr.fill();
    }
}

/// A plant with a few very big leaves and light midribs.
fn big_leaves(cr: &mut Canvas, rng: &Rng, x: f64, y: f64, size: f64, col: Rgb) {
    for i in 0..5 {
        let ang = -PI / 2.0 + (i as f64 - 2.0) * 0.42 + rng.uniform(-0.12, 0.12);
        let len = size * rng.uniform(0.7, 1.0);
        leaf_path(cr, x, y, len, len * 0.19, ang);
        cr.set_rgb(lerp3(col, GREENS[2], 0.25 * (i % 2) as f64));
        cr.fill();
        cr.move_to(x, y);
        cr.line_to(x + ang.cos() * len * 0.9, y + ang.sin() * len * 0.9);
        cr.set_rgba(lighten(col, 0.3), 0.5);
        cr.set_line_width(size * 0.014);
        cr.stroke();
    }
}

fn flower(cr: &mut Canvas, x: f64, y: f64, size: f64, col: Rgb) {
    for i in 0..5 {
        let ang = i as f64 * TAU / 5.0 - 0.3;
        cr.circle(x + ang.cos() * size, y + ang.sin() * size, size * 0.75);
    }
    cr.set_rgb(col);
    cr.fill();
    cr.circle(x, y, size * 0.55);
    cr.set_source_rgb(1.0, 0.86, 0.35);
    cr.fill();
}

/// A toucan sitting on a branch at (x, y), looking in direction `dir`.
fn toucan(cr: &mut Canvas, x: f64, y: f64, z: f64, dir: f64) {
    cr.save();
    cr.translate(x, y);
    cr.scale(dir, 1.0);
    cr.set_source_rgb(0.04, 0.04, 0.06);
    cr.move_to(-z * 0.5, -z * 0.6); // tail
    cr.line_to(-z * 1.1, z * 0.7);
    cr.line_to(-z * 0.7, z * 0.75);
    cr.line_to(-z * 0.1, -z * 0.3);
    cr.close_path();
    cr.fill();
    cr.save();
    cr.translate(0.0, -z * 0.95);
    cr.rotate(-0.5);
    cr.ellipse(0.0, 0.0, z * 0.62, z * 0.95);
    cr.restore();
    cr.circle(z * 0.42, -z * 1.75, z * 0.46);
    cr.fill();
    cr.ellipse(z * 0.50, -z * 1.35, z * 0.34, z * 0.48); // white bib
    cr.set_source_rgb(0.97, 0.95, 0.82);
    cr.fill();
    cr.move_to(z * 0.72, -z * 2.08); // the big bill
    cr.quad_to(z * 1.75, -z * 2.15, z * 2.0, -z * 1.42);
    cr.quad_to(z * 1.4, -z * 1.6, z * 0.78, -z * 1.48);
    cr.close_path();
    let mut g = Gradient::linear(z * 0.7, 0.0, z * 2.0, 0.0);
    g.add_color_stop_rgb(0.0, 1.0, 0.80, 0.20);
    g.add_color_stop_rgb(0.75, 0.98, 0.50, 0.12);
    g.add_color_stop_rgb(1.0, 0.75, 0.15, 0.10);
    cr.set_source(&g);
    cr.fill();
    cr.circle(z * 0.52, -z * 1.85, z * 0.13);
    cr.set_source_rgb(0.35, 0.75, 0.95);
    cr.fill();
    cr.circle(z * 0.53, -z * 1.85, z * 0.06);
    cr.set_source_rgb(0.0, 0.0, 0.0);
    cr.fill();
    cr.restore();
}

/// One layer of undergrowth: a bumpy silhouette with rounded crowns on it.
#[allow(clippy::too_many_arguments)]
fn undergrowth(cr: &mut Canvas, rng: &Rng, w: f64, bottom: f64, y: f64, amp: f64, freq: f64, col: Rgb) {
    let ph = rng.uniform(0.0, TAU);
    let top = |x: f64| {
        let u = x / w * freq + ph;
        y - amp * (0.5 * u.sin() + 0.3 * (u * 2.7 + 1.0).sin() + 0.2 * (u * 6.1).sin())
    };
    cr.move_to(0.0, bottom);
    let mut x = 0.0;
    while x < w + 12.0 {
        cr.line_to(x, top(x));
        x += 12.0;
    }
    cr.line_to(w, bottom);
    cr.close_path();
    cr.set_rgb(col);
    cr.fill();
    for _ in 0..(w / (amp * 0.9)) as usize {
        let cx = rng.uniform(0.0, w);
        cr.circle(cx, top(cx) + amp * rng.uniform(-0.2, 0.5), amp * rng.uniform(0.45, 1.0));
    }
    cr.fill();
}

/// A big tree: trunk with flared roots, moss and bark.
fn paint_trunk(cr: &mut Canvas, rng: &Rng, s: f64, tr: &Tree) {
    let (x, by, tw) = (tr.x, tr.base, tr.w);
    let (l, rg) = (x - tw / 2.0, x + tw / 2.0);
    cr.move_to(l + tw * 0.07, -10.0);
    cr.line_to(l, by - 150.0 * s);
    cr.curve_to(l, by - 50.0 * s, l - tw * 0.22, by - 14.0 * s, l - tw * 0.62, by + 6.0 * s);
    cr.quad_to(x - tw * 0.25, by - 6.0 * s, x - tw * 0.05, by + 12.0 * s);
    cr.quad_to(x + tw * 0.2, by - 6.0 * s, rg + tw * 0.62, by + 6.0 * s);
    cr.curve_to(rg + tw * 0.22, by - 14.0 * s, rg, by - 50.0 * s, rg, by - 150.0 * s);
    cr.line_to(rg - tw * 0.07, -10.0);
    cr.close_path();
    let mut g = Gradient::linear(l - tw * 0.2, 0.0, rg + tw * 0.2, 0.0);
    g.add_color_stop_rgb(0.0, 0.40, 0.34, 0.24);
    g.add_color_stop_rgb(0.4, 0.28, 0.235, 0.17);
    g.add_color_stop_rgb(1.0, 0.13, 0.11, 0.085);
    cr.set_source(&g);
    cr.fill();
    // bark
    cr.set_source_rgba(0.07, 0.05, 0.04, 0.30);
    cr.set_line_width(1.6 * s);
    for k in 0..6 {
        let bx = l + tw * (0.12 + 0.15 * k as f64) + rng.uniform(-3.0, 3.0) * s;
        let mut y = rng.uniform(0.0, 60.0) * s;
        while y < by - 30.0 * s {
            let len = rng.uniform(40.0, 130.0) * s;
            cr.move_to(bx, y);
            cr.quad_to(bx + rng.uniform(-5.0, 5.0) * s, y + len * 0.5, bx + rng.uniform(-2.0, 2.0) * s, y + len);
            y += len + rng.uniform(20.0, 70.0) * s;
        }
    }
    cr.stroke();
    // moss on the light side and at the foot
    for _ in 0..9 {
        let my = rng.uniform(0.15, 1.0) * by;
        cr.ellipse(l + tw * rng.uniform(0.1, 0.45), my, tw * rng.uniform(0.08, 0.2), rng.uniform(10.0, 30.0) * s);
        cr.set_source_rgba(0.24, 0.44, 0.18, rng.uniform(0.3, 0.6));
        cr.fill();
    }
    cr.ellipse(x, by + 4.0 * s, tw * 1.05, 9.0 * s);
    cr.set_source_rgba(0.16, 0.34, 0.14, 0.9);
    cr.fill();
}

/// A sturdy branch from (x0, y) at the trunk to its tip at x1; `y` is the underside at the tip.
fn paint_branch(cr: &mut Canvas, rng: &Rng, s: f64, x0: f64, x1: f64, y: f64, thick: f64) {
    let dir = (x1 - x0).signum();
    let mid = (x0 + x1) / 2.0;
    cr.move_to(x0 - dir * 6.0 * s, y - thick * 1.35);
    cr.quad_to(mid, y - thick * 0.95 - 9.0 * s, x1, y - thick * 0.45);
    cr.quad_to(x1 + dir * 12.0 * s, y - thick * 0.2, x1, y);
    cr.quad_to(mid, y + 1.0 * s, x0 - dir * 6.0 * s, y + thick * 0.75);
    cr.close_path();
    let mut g = Gradient::linear(0.0, y - thick * 1.4, 0.0, y + thick * 0.6);
    g.add_color_stop_rgb(0.0, 0.36, 0.30, 0.21);
    g.add_color_stop_rgb(1.0, 0.13, 0.11, 0.085);
    cr.set_source(&g);
    cr.fill();
    // moss and leaves on top
    cr.set_source_rgba(0.22, 0.42, 0.17, 0.75);
    cr.set_line_width(4.0 * s);
    cr.set_line_cap(LineCap::Round);
    cr.move_to(lerp(x0, x1, 0.15), y - thick * 1.18);
    cr.quad_to(mid, y - thick * 0.9 - 9.0 * s, lerp(x0, x1, 0.85), y - thick * 0.62);
    cr.stroke();
    for f in [0.3, 0.62] {
        leaf_tuft(cr, rng, lerp(x0, x1, f), y - thick * 1.0 - 4.0 * s, 34.0 * s, 5, -PI * 0.95, -PI * 0.05);
    }
    leaf_tuft(cr, rng, x1 + dir * 8.0 * s, y - thick * 0.4, 46.0 * s, 8, -PI * 0.5 - dir * 1.5, -PI * 0.5 + dir * 0.9);
}

impl Gorillas {
    /// A free liana at the tree next to `x`: (index, direction of the swing).
    fn pick_vine(&self, env: &Env, x: f64) -> Option<(usize, f64)> {
        let mut near = 0;
        for (i, tr) in self.trees.iter().enumerate() {
            if (tr.x - x).abs() < (self.trees[near].x - x).abs() {
                near = i;
            }
        }
        let mut cands: Vec<(usize, f64)> = Vec::new();
        if near < self.vines.len() && !self.vines[near].busy {
            cands.push((near, 1.0));
        }
        if near > 0 && !self.vines[near - 1].busy {
            cands.push((near - 1, -1.0));
        }
        if cands.is_empty() { None } else { Some(*env.rng.choice(&cands)) }
    }

    /// The liana after `vine` in direction `dir`, taken with probability `p` if it is free.
    fn next_vine(&mut self, env: &Env, vine: usize, dir: f64, p: f64) -> Option<usize> {
        let j = vine as i64 + dir as i64;
        if j < 0 || j as usize >= self.vines.len() || self.vines[j as usize].busy || !env.rng.chance(p) {
            return None;
        }
        self.vines[j as usize].busy = true;
        Some(j as usize)
    }

    /// Everything that never moves, painted once.
    fn paint_jungle(&self, cr: &mut Canvas, env: &Env, branches: &[(f64, f64, f64, f64)]) {
        let (w, h, s) = (env.w, env.h, env.s);
        let gt = env.ground_top;
        let rng = &env.rng;
        // luminous mist behind everything
        let mut g = Gradient::linear(0.0, 0.0, 0.0, gt);
        g.add_color_stop_rgb(0.0, 0.66, 0.82, 0.50);
        g.add_color_stop_rgb(0.35, 0.56, 0.76, 0.56);
        g.add_color_stop_rgb(1.0, 0.36, 0.58, 0.47);
        cr.set_source(&g);
        cr.rectangle(0.0, 0.0, w, h);
        cr.fill();
        let (gx, gy) = (w * 0.42, h * 0.12);
        let mut sg = Gradient::radial(gx, gy, 0.0, gx, gy, h * 0.6);
        sg.add_color_stop_rgba(0.0, 1.0, 0.98, 0.72, 0.55);
        sg.add_color_stop_rgba(1.0, 1.0, 0.98, 0.72, 0.0);
        cr.set_source(&sg);
        cr.rectangle(0.0, 0.0, w, gt);
        cr.fill();
        // three layers of far trees and undergrowth, each darker and nearer, with mist between
        let layers: [(f64, f64, f64, Rgb, Rgb, usize); 3] = [
            (0.40, 26.0, 14.0, (0.47, 0.67, 0.53), (0.50, 0.70, 0.56), 10),
            (0.49, 34.0, 26.0, (0.33, 0.54, 0.41), (0.37, 0.57, 0.45), 6),
            (0.565, 40.0, 40.0, (0.20, 0.40, 0.27), (0.24, 0.42, 0.30), 3),
        ];
        for (k, (fy, amp, tw, col, trunk, n)) in layers.iter().enumerate() {
            let y = h * fy;
            for _ in 0..(*n as f64 * w / (1600.0 * s)).ceil() as usize {
                let tx = rng.uniform(0.0, w);
                let wd = tw * rng.uniform(0.7, 1.3) * s;
                cr.move_to(tx - wd * 0.4, 0.0);
                cr.line_to(tx + wd * 0.4, 0.0);
                cr.line_to(tx + wd * 0.5 + rng.uniform(-20.0, 20.0) * s, y + 10.0 * s);
                cr.rel_line_to(-wd, 0.0);
                cr.close_path();
                cr.set_rgb(*trunk);
                cr.fill();
            }
            undergrowth(cr, rng, w, gt + 4.0 * s, y, amp * s, 9.0 + 5.0 * k as f64, *col);
            let mut mg = Gradient::linear(0.0, y - amp * s, 0.0, y + h * 0.10);
            mg.add_color_stop_rgba(0.0, 0.72, 0.88, 0.74, 0.0);
            mg.add_color_stop_rgba(1.0, 0.72, 0.88, 0.74, 0.34 - 0.08 * k as f64);
            cr.set_source(&mg);
            cr.rectangle(0.0, y - amp * s, w, h * 0.10 + amp * s);
            cr.fill();
            if k == 1 {
                // shafts of light falling through the canopy
                for i in 0..4 {
                    let x0 = w * (0.16 + 0.2 * i as f64 + rng.uniform(-0.05, 0.05));
                    let (wd, lean) = (rng.uniform(50.0, 130.0) * s, h * 0.2);
                    let mut lg = Gradient::linear(0.0, 0.0, 0.0, h * 0.72);
                    lg.add_color_stop_rgba(0.0, 1.0, 0.98, 0.78, 0.26);
                    lg.add_color_stop_rgba(1.0, 1.0, 0.98, 0.78, 0.0);
                    cr.set_source(&lg);
                    cr.move_to(x0, 0.0);
                    cr.line_to(x0 + wd, 0.0);
                    cr.line_to(x0 + wd * 1.7 - lean, h * 0.72);
                    cr.line_to(x0 - wd * 0.2 - lean, h * 0.72);
                    cr.close_path();
                    cr.fill();
                }
            }
        }
        // bushes where the floor begins
        undergrowth(cr, rng, w, gt + 30.0 * s, gt - 6.0 * s, 16.0 * s, 31.0, (0.13, 0.30, 0.18));
        // mossy floor
        let mut fg = Gradient::linear(0.0, gt, 0.0, h);
        fg.add_color_stop_rgb(0.0, 0.25, 0.41, 0.19);
        fg.add_color_stop_rgb(0.45, 0.17, 0.31, 0.14);
        fg.add_color_stop_rgb(1.0, 0.08, 0.17, 0.09);
        cr.move_to(0.0, h);
        let mut x = 0.0;
        while x < w + 16.0 {
            cr.line_to(x, gt + (4.0 * (x / w * 9.0).sin() + 3.0 * (x / w * 23.0 + 1.0).sin()) * s);
            x += 16.0;
        }
        cr.line_to(w, h);
        cr.close_path();
        cr.set_source(&fg);
        cr.fill();
        let floor_k = |y: f64| 0.5 + 0.8 * (y - gt) / (h - gt).max(1.0);
        for _ in 0..(260.0 * w / (1600.0 * s)) as usize {
            // patches of moss, lighter and darker
            let (px, py) = (rng.uniform(0.0, w), rng.uniform(gt + 6.0 * s, h));
            let k = floor_k(py);
            cr.ellipse(px, py, rng.uniform(14.0, 60.0) * s * k, rng.uniform(3.0, 9.0) * s * k);
            if rng.chance(0.5) {
                cr.set_source_rgba(0.36, 0.55, 0.22, rng.uniform(0.08, 0.2));
            } else {
                cr.set_source_rgba(0.04, 0.12, 0.06, rng.uniform(0.08, 0.2));
            }
            cr.fill();
        }
        for i in 0..(16.0 * w / (1600.0 * s)) as usize {
            // sun flecks on the floor
            let (px, py) = (w * rng.uniform(0.08, 0.8), rng.uniform(gt + 12.0 * s, h - 60.0 * s));
            let k = floor_k(py);
            cr.ellipse(px, py, rng.uniform(40.0, 110.0) * s * k, rng.uniform(7.0, 16.0) * s * k);
            cr.set_source_rgba(1.0, 0.96, 0.6, if i % 3 == 0 { 0.13 } else { 0.07 });
            cr.fill();
        }
        for _ in 0..(70.0 * w / (1600.0 * s)) as usize {
            // fallen leaves
            let (px, py) = (rng.uniform(0.0, w), rng.uniform(gt + 10.0 * s, h));
            leaf_path(cr, px, py, rng.uniform(9.0, 17.0) * s * floor_k(py), 3.0 * s, rng.uniform(-0.5, 0.5));
            cr.set_rgba(*rng.choice(&[(0.55, 0.40, 0.16), (0.42, 0.50, 0.18), (0.36, 0.24, 0.12)]), rng.uniform(0.35, 0.7));
            cr.fill();
        }
        for _ in 0..(5.0 * w / (1600.0 * s)) as usize {
            // stones with a cap of moss
            let (px, py) = (rng.uniform(0.03, 0.97) * w, rng.uniform(gt + 30.0 * s, h - 70.0 * s));
            let z = rng.uniform(14.0, 26.0) * s * floor_k(py);
            cr.save();
            cr.translate(px, py);
            cr.scale(z, z * 0.55);
            cr.new_sub_path();
            cr.arc(0.0, 0.0, 1.0, PI, TAU);
            cr.close_path();
            cr.restore();
            cr.set_source_rgb(0.30, 0.33, 0.30);
            cr.fill();
            cr.ellipse(px - z * 0.1, py - z * 0.42, z * 0.7, z * 0.16);
            cr.set_source_rgba(0.28, 0.48, 0.20, 0.9);
            cr.fill();
        }
        // back row of the canopy, the big trees, then the dark leaves right at the top
        for _ in 0..(22.0 * w / (1600.0 * s)) as usize {
            cr.circle(rng.uniform(0.0, w), rng.uniform(-10.0, 34.0) * s, rng.uniform(45.0, 80.0) * s);
            cr.set_rgb(*rng.choice(&[(0.17, 0.38, 0.19), (0.21, 0.44, 0.22), (0.14, 0.33, 0.18)]));
            cr.fill();
        }
        for tr in &self.trees {
            paint_trunk(cr, rng, s, tr);
        }
        // lianas hanging in loops between the trunks
        cr.set_line_cap(LineCap::Round);
        for pair in self.trees.windows(2) {
            for _ in 0..2 {
                let (x0, x1) = (pair[0].x + rng.uniform(-0.2, 0.6) * pair[0].w, pair[1].x + rng.uniform(-0.6, 0.2) * pair[1].w);
                let dip = h * rng.uniform(0.05, 0.11);
                cr.move_to(x0, -5.0);
                cr.curve_to(lerp(x0, x1, 0.25), dip * 1.3, lerp(x0, x1, 0.75), dip * 1.3, x1, -5.0);
                cr.set_source_rgba(0.10, 0.22, 0.10, 0.85);
                cr.set_line_width(rng.uniform(2.0, 3.6) * s);
                cr.stroke();
            }
        }
        for _ in 0..(26.0 * w / (1600.0 * s)) as usize {
            cr.circle(rng.uniform(0.0, w), rng.uniform(-34.0, 4.0) * s, rng.uniform(38.0, 70.0) * s);
            cr.set_rgb(*rng.choice(&[(0.05, 0.15, 0.08), (0.07, 0.20, 0.10), (0.04, 0.12, 0.07)]));
            cr.fill();
        }
        for _ in 0..(18.0 * w / (1600.0 * s)) as usize {
            leaf_tuft(cr, rng, rng.uniform(0.0, w), rng.uniform(26.0, 60.0) * s, 44.0 * s, 5, PI * 0.15, PI * 0.85);
        }
        // the branches the lianas hang from, and a few more
        for &(x0, x1, y, thick) in branches {
            paint_branch(cr, rng, s, x0, x1, y, thick);
        }
        for v in &self.vines {
            // the knot of the liana around its branch
            cr.set_source_rgb(0.12, 0.20, 0.08);
            cr.set_line_width(5.0 * s);
            cr.move_to(v.ax, v.ay + 2.0 * s);
            cr.line_to(v.ax, v.ay - 20.0 * s);
            cr.stroke();
        }
        if let Some(&(x0, x1, y, thick)) = branches.get(self.vines.len()) {
            let dir = (x1 - x0).signum();
            toucan(cr, lerp(x0, x1, 0.6), y - thick * 0.95, 13.0 * s, -dir);
        }
        // plants at the feet of the trees and along the back of the floor
        for tr in &self.trees {
            for side in [-1.0, 1.0] {
                let px = tr.x + side * tr.w * rng.uniform(0.9, 1.5);
                if rng.chance(0.5) {
                    big_leaves(cr, rng, px, tr.base + 8.0 * s, rng.uniform(60.0, 95.0) * s, GREENS[rng.index(2)]);
                } else {
                    fern(cr, px, tr.base + 8.0 * s, rng.uniform(46.0, 70.0) * s, rng.uniform(-0.1, 0.1), GREENS[1 + rng.index(2)], 1.0);
                }
            }
            if rng.chance(0.7) {
                // an orchid on the trunk
                let (ox, oy) = (tr.x - tr.w * 0.2, h * rng.uniform(0.30, 0.50));
                leaf_tuft(cr, rng, ox, oy, 20.0 * s, 4, -PI * 0.9, -PI * 0.1);
                flower(cr, ox + 3.0 * s, oy - 9.0 * s, 4.2 * s, (0.98, 0.70, 0.86));
            }
        }
        for _ in 0..(12.0 * w / (1600.0 * s)) as usize {
            let (px, py) = (rng.uniform(0.0, w), gt + rng.uniform(-4.0, 20.0) * s);
            fern(cr, px, py, rng.uniform(26.0, 46.0) * s, rng.uniform(-0.15, 0.15), GREENS[rng.index(3)], 1.0);
            if rng.chance(0.6) {
                let col = *rng.choice(&[(0.95, 0.30, 0.25), (1.0, 0.55, 0.20), (0.98, 0.62, 0.80), (0.96, 0.94, 0.85)]);
                flower(cr, px + rng.uniform(-30.0, 30.0) * s, py - rng.uniform(4.0, 22.0) * s, rng.uniform(3.0, 4.6) * s, col);
            }
        }
        // darker towards the corners
        let mut vg = Gradient::radial(w / 2.0, h * 0.5, h * 0.45, w / 2.0, h * 0.5, hypot(w, h) * 0.62);
        vg.add_color_stop_rgba(0.0, 0.0, 0.05, 0.02, 0.0);
        vg.add_color_stop_rgba(1.0, 0.0, 0.05, 0.02, 0.5);
        cr.set_source(&vg);
        cr.rectangle(0.0, 0.0, w, h);
        cr.fill();
    }

    /// A liana to swing on, from its branch down to the tuft at its end.
    fn draw_vine(&self, cr: &mut Canvas, s: f64, v: &Vine) {
        let end = v.at(v.ang, v.len);
        let bend = -v.vel * v.len * 0.07; // a free liana lags behind its own swing
        let (cx, cy) = ((v.ax + end.0) / 2.0 + v.ang.cos() * bend, (v.ay + end.1) / 2.0 - v.ang.sin() * bend);
        cr.set_line_cap(LineCap::Round);
        cr.move_to(v.ax, v.ay);
        cr.quad_to(cx, cy, end.0, end.1);
        cr.set_source_rgb(0.10, 0.19, 0.08);
        cr.set_line_width(6.0 * s);
        cr.stroke_preserve();
        cr.set_source_rgb(0.30, 0.44, 0.17);
        cr.set_line_width(2.4 * s);
        cr.stroke();
        for (i, f) in [0.22, 0.45, 0.68, 1.0].iter().enumerate() {
            let g = 1.0 - f;
            let (px, py) = (
                g * g * v.ax + 2.0 * g * f * cx + f * f * end.0,
                g * g * v.ay + 2.0 * g * f * cy + f * f * end.1,
            );
            let side = if i % 2 == 0 { 1.0 } else { -1.0 };
            let n = if *f == 1.0 { 3 } else { 1 };
            for k in 0..n {
                let ang = PI / 2.0 - v.ang + side * (0.9 - 0.9 * k as f64);
                leaf_path(cr, px, py, 20.0 * s, 5.0 * s, ang);
            }
        }
        cr.set_source_rgb(0.22, 0.46, 0.18);
        cr.fill();
    }
}

impl Theme for Gorillas {
    type Actor = Gorilla;
    const NAME: &'static str = "gorillas";
    const LABEL_BG: Rgb = (0.02, 0.08, 0.04);
    const EMPTY_COLOR: Rgb = (0.88, 0.96, 0.80);
    const WAITING_ROW: f64 = 0.76;

    fn new(_env: &Env) -> Self {
        Gorillas {
            bg: None,
            fog: None,
            epoch: 0,
            trees: Vec::new(),
            vines: Vec::new(),
            bits: Vec::new(),
            ferns: Vec::new(),
            danglers: Vec::new(),
            motes: Vec::new(),
            flies: Vec::new(),
            mist: Vec::new(),
        }
    }

    fn spawn_point(&self, env: &Env) -> (f64, f64) {
        (env.rng.uniform(env.w * 0.15, env.w * 0.85), env.rng.uniform(env.ground_top + 50.0 * env.s, env.h * 0.86))
    }

    fn on_tool(&mut self, env: &mut Env, actor: &mut Gorilla, name: &str, index: usize) {
        if actor.mode == Mode::Ground {
            actor.beat = 1.15;
        }
        let b = &actor.b;
        env.spawn_floater(b.x, b.y - b.radius * 1.25 - index as f64 * 26.0 * env.s, name, b.color, FloatStyle::Text, 40.0, 3.5);
    }

    fn setup_world(&mut self, env: &mut Env) {
        let (w, h, s) = (env.w, env.h, env.s);
        let gt = h * 0.64;
        env.ground_top = gt;
        self.epoch += 1; // the gorillas notice and let go of the old lianas
        let env: &Env = env;
        let rng = &env.rng;
        // the big trees, spread over the width
        let n = clamp((w / (430.0 * s)).round(), 3.0, 6.0) as usize;
        let span = w / n as f64;
        self.trees = (0..n)
            .map(|i| Tree {
                x: span * (i as f64 + 0.5) + rng.uniform(-0.07, 0.07) * span,
                base: gt + rng.uniform(16.0, 40.0) * s,
                w: rng.uniform(72.0, 96.0) * s,
            })
            .collect();
        // one liana between each pair of neighbours; a swing goes from trunk to trunk
        let rr = <Gorilla as ActorT<Gorillas>>::BASE_RADIUS * s * AIR_K;
        let mut branches: Vec<(f64, f64, f64, f64)> = Vec::new();
        let mut more: Vec<(f64, f64, f64, f64)> = Vec::new();
        self.vines.clear();
        for i in 0..n - 1 {
            let (a, b) = (&self.trees[i], &self.trees[i + 1]);
            let (left, right) = (a.x + a.w / 2.0 + rr * 0.1, b.x - b.w / 2.0 - rr * 0.1);
            let (ax, half) = ((left + right) / 2.0, (right - left) / 2.0);
            let ay = h * rng.uniform(0.115, 0.165);
            let room = gt - 16.0 * s - rr * 1.3 - ay; // the feet stay clear of the floor
            let len = (half / 0.60_f64.sin()).min(room).max(half * 1.06);
            self.vines.push(Vine { ax, ay, len, th0: (half / len).asin(), ang: 0.0, vel: 0.0, busy: false, ph: rng.uniform(0.0, TAU) });
            let from_left = rng.chance(0.5);
            let (x0, x1) = if from_left { (a.x + a.w * 0.4, ax + 30.0 * s) } else { (b.x - b.w * 0.4, ax - 30.0 * s) };
            branches.push((x0, x1, ay - 3.0 * s, 24.0 * s));
            // a shorter branch on the other tree, somewhere else
            let (t2, dir) = if from_left { (b, -1.0) } else { (a, 1.0) };
            more.push((t2.x + dir * t2.w * 0.4, t2.x + dir * rng.uniform(120.0, 170.0) * s, h * rng.uniform(0.25, 0.34), 17.0 * s));
        }
        branches.append(&mut more);
        let this: &Gorillas = self;
        let bg = Picture::record(w, h, |cr| this.paint_jungle(cr, env, &branches));
        self.bg = Some(bg);
        self.bits.clear();
        let floor = env.h - env.strip_h;
        self.ferns = (0..(11.0 * w / (1600.0 * s)) as usize)
            .map(|_| {
                let y = rng.uniform(gt + 50.0 * s, floor + 10.0 * s);
                let k = 0.6 + 0.8 * (y - gt) / (h - gt).max(1.0);
                Fern { x: rng.uniform(0.0, w), y, size: rng.uniform(34.0, 52.0) * s * k, ph: rng.uniform(0.0, TAU), col: rng.index(3) }
            })
            .collect();
        self.ferns.sort_by(|a, b| a.y.total_cmp(&b.y));
        self.danglers =
            (0..(7.0 * w / (1600.0 * s)) as usize).map(|_| (rng.uniform(0.0, w), h * rng.uniform(0.10, 0.30), rng.uniform(0.0, TAU))).collect();
        self.motes = (0..30).map(|_| (rng.random(), rng.random(), rng.uniform(0.0, TAU), rng.uniform(0.01, 0.03))).collect();
        let wings = [(1.0, 0.72, 0.20), (0.45, 0.80, 1.0), (1.0, 0.95, 0.80)];
        self.flies = (0..3)
            .map(|i| (w * rng.uniform(0.15, 0.85), h * rng.uniform(0.40, 0.62), rng.uniform(70.0, 150.0) * s, rng.uniform(0.0, TAU), wings[i]))
            .collect();
        self.mist = (0..4).map(|i| (h * (0.40 + 0.06 * i as f64), rng.random(), rng.uniform(0.006, 0.014), rng.uniform(0.6, 1.0))).collect();
        self.fog = Some(Picture::record(1248.0 * s, 202.0 * s, |cr| {
            cr.translate(624.0 * s, 101.0 * s);
            cr.scale(2.6, 0.42);
            cr.glow(0.0, 0.0, 240.0 * s, (0.82, 0.94, 0.84), 0.2, 0.0);
        }));
    }

    fn update_world(&mut self, env: &mut Env, actors: &mut [Gorilla], dt: f64) {
        let (s, t) = (env.s, env.t);
        // which lianas are in use right now (also frees those of gorillas that have gone)
        for v in &mut self.vines {
            v.busy = false;
        }
        for a in actors.iter().filter(|a| a.epoch == self.epoch) {
            for i in a.vines_in_use().into_iter().flatten() {
                if let Some(v) = self.vines.get_mut(i) {
                    v.busy = true;
                }
            }
        }
        // a free liana swings back and comes to rest
        let held: Vec<bool> = (0..self.vines.len())
            .map(|i| actors.iter().any(|a| a.epoch == self.epoch && !matches!(a.mode, Mode::Approach { .. }) && a.vines_in_use().contains(&Some(i))))
            .collect();
        for (v, held) in self.vines.iter_mut().zip(held) {
            if held {
                continue;
            }
            let acc = -(1000.0 * s / v.len) * 1.5 * v.ang.sin() - 0.9 * v.vel + 0.05 * (t * 0.8 + v.ph).sin();
            v.vel += acc * dt;
            v.ang += v.vel * dt;
        }
        let g = 520.0 * s;
        for p in &mut self.bits {
            p.age += dt;
            p.vy += g * dt;
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            p.rot += dt * 5.0;
        }
        self.bits.retain(|p| p.age < p.life);
    }

    fn draw_world(&mut self, cr: &mut Canvas, env: &Env, actors: &[Gorilla], t: f64) {
        let (w, h, s) = (env.w, env.h, env.s);
        let gt = env.ground_top;
        if let Some(bg) = &self.bg {
            cr.picture(bg, 0.0, 0.0, 1.0);
        }
        // thin lianas swaying far back
        cr.set_line_cap(LineCap::Round);
        cr.set_line_width(2.2 * s);
        for &(x, len, ph) in &self.danglers {
            let sway = (t * 0.7 + ph).sin() * 14.0 * s;
            cr.move_to(x, 0.0);
            cr.quad_to(x + sway * 0.2, len * 0.6, x + sway, len);
        }
        cr.set_source_rgba(0.10, 0.25, 0.12, 0.8);
        cr.stroke();
        // drifting banks of mist
        for &(y, ph, sp, size) in &self.mist {
            let x = ((t * sp + ph) % 1.0) * (w + 900.0 * s) - 450.0 * s;
            if let Some(fog) = &self.fog {
                cr.picture(fog, (x - 624.0 * s).round(), (y - 101.0 * s).round(), size);
            }
        }
        // motes in the light
        for &(x, y, ph, sp) in &self.motes {
            let f = (y + t * sp) % 1.0;
            let (mx, my) = (x * w + (t * 0.4 + ph).sin() * 22.0 * s, (1.0 - f) * gt * 1.05);
            cr.set_source_rgba(1.0, 0.98, 0.75, 0.55 * (PI * f).sin() * (0.6 + 0.4 * (t * 1.7 + ph).sin()));
            cr.circle(mx, my, 1.9 * s);
            cr.fill();
        }
        // one leaf sailing down
        let lf = (t * 0.07) % 1.0;
        let (lx, ly) = (w * (0.2 + 0.6 * ((t * 0.07).floor() * 0.37 % 1.0)) + (t * 1.3).sin() * 40.0 * s, lf * (gt + 60.0 * s));
        leaf_path(cr, lx, ly, 16.0 * s, 5.0 * s, (t * 1.3).cos() * 0.9);
        cr.set_source_rgba(0.72, 0.62, 0.20, 0.9 * (1.0 - lf).min(lf * 8.0).min(1.0));
        cr.fill();
        for v in &self.vines {
            self.draw_vine(cr, s, v);
        }
        // butterflies
        for &(cx, cy, rad, ph, col) in &self.flies {
            let ang = t * 0.35 + ph;
            let (bx, by) = (cx + ang.cos() * rad * 2.2 + (t * 1.9 + ph).sin() * 10.0 * s, cy + (ang * 1.7).sin() * rad * 0.6);
            let flap = (t * 13.0 + ph).sin().abs() * 0.85 + 0.15;
            cr.ellipse(bx - 4.5 * s * flap, by, 4.5 * s * flap, 5.5 * s);
            cr.ellipse(bx + 4.5 * s * flap, by, 4.5 * s * flap, 5.5 * s);
            cr.set_rgba(col, 0.95);
            cr.fill();
        }
        // gorillas and the ferns on the floor, back to front
        let mut fi = 0;
        let fern_at = |cr: &mut Canvas, f: &Fern| {
            fern(cr, f.x, f.y, f.size, (t * 0.9 + f.ph).sin() * 0.07, darken(GREENS[f.col], 0.15), 1.0);
        };
        for a in sorted_actors(actors) {
            let foot = if a.airborne() { 0.0 } else { a.b.y + a.b.radius };
            while fi < self.ferns.len() && self.ferns[fi].y < foot {
                fern_at(cr, &self.ferns[fi]);
                fi += 1;
            }
            a.draw(cr, env, self, t);
        }
        while fi < self.ferns.len() {
            fern_at(cr, &self.ferns[fi]);
            fi += 1;
        }
        for p in &self.bits {
            let f = p.age / p.life;
            leaf_path(cr, p.x, p.y, 13.0 * s, 4.0 * s, p.rot);
            cr.set_rgba(GREENS[2], (1.0 - f) * 0.95);
            cr.fill();
        }
        // big dark leaves in the front corners
        for (cx, dir) in [(-20.0 * s, 1.0), (w + 20.0 * s, -1.0)] {
            for i in 0..5 {
                let fi = i as f64;
                let ang = -PI / 2.0 + dir * (0.25 + 0.26 * fi) + (t * 0.6 + fi * 1.3 + dir).sin() * 0.025;
                let len = (250.0 - 22.0 * fi) * s;
                let (bx, by) = (cx + dir * fi * 16.0 * s, h - 6.0 * s);
                leaf_path(cr, bx, by, len, len * 0.17, ang);
                cr.set_rgb(if i % 2 == 0 { (0.035, 0.12, 0.065) } else { (0.05, 0.16, 0.08) });
                cr.fill();
                cr.move_to(bx, by);
                cr.line_to(bx + ang.cos() * len * 0.92, by + ang.sin() * len * 0.92);
                cr.set_source_rgba(0.16, 0.34, 0.16, 0.6);
                cr.set_line_width(2.0 * s);
                cr.stroke();
            }
        }
    }
}
