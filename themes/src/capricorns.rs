//! Capricorns theme: Alpine ibex on steep rock faces above a mountain meadow, in clear morning light.
//!
//! working = walks about and grazes briefly; every now and then it goes to the foot of a cliff and
//!           leaps up from ledge to ledge, looks out over the valley and comes back down in bounds;
//!           every tool call makes it rear up, toss its horns and stamp down, the tool name rises
//! idle    = grazes with the head down or lies chewing with folded legs (also up on a ledge), ears
//!           flicking; climbs only rarely
//! waiting = comes to the front, stands proud facing you, stamps a front hoof and bleats, pulses
//! blocked = stands still with lowered head and horns, flickers
//! background task = like working, a little calmer, in violet
//! rest    = one ibex asleep with folded legs, the head on the ground, breathing slowly, "z z z"
//! Sub-agents are kids with tiny horn stubs that trot behind; while the parent is up on the cliff
//! they wait at the foot of it and hop about.
//! The fur stays natural; the state colour shows as a rim light, a glow under the animal and in
//! the big ridged horns.
use std::rc::Rc;

use vivarium_core::prelude::*;

const FUR: Rgb = (0.53, 0.40, 0.28);
const FUR_DARK: Rgb = (0.34, 0.25, 0.18);
const FUR_LIGHT: Rgb = (0.84, 0.76, 0.62);
const LEG_DARK: Rgb = (0.15, 0.11, 0.09);
const HORN: Rgb = (0.42, 0.35, 0.28);
const GRASS: [Rgb; 3] = [(0.30, 0.50, 0.20), (0.42, 0.60, 0.24), (0.24, 0.42, 0.18)];
/// length of one leg segment, in units of the radius
const LEG: f64 = 0.45;
/// depth factor of an ibex up on a ledge
const LEDGE_K: f64 = 0.74;

type P = (f64, f64);

fn smooth(x: f64) -> f64 {
    let x = clamp(x, 0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

fn mixp(a: P, b: P, k: f64) -> P {
    (lerp(a.0, b.0, k), lerp(a.1, b.1, k))
}

/// Where the joints are, in units of the radius, facing right, hooves at y = +1.
#[derive(Clone, Copy)]
struct Pose {
    hip: P,
    sh: P,
    head: P,
    /// direction of the muzzle: 0 = level, positive = nose down (the horns turn with the head)
    nod: f64,
    /// 1 = head in profile, 0 = looking at the viewer
    look: f64,
    /// far hind, near hind, far front, near front
    feet: [P; 4],
    /// length of one leg segment (short when the legs are folded under the body)
    leg: f64,
    mouth: f64,
    /// 1 = eyes closed
    shut: f64,
}

impl Pose {
    fn mix(&self, o: &Pose, k: f64) -> Pose {
        let mut feet = self.feet;
        for (f, g) in feet.iter_mut().zip(o.feet) {
            *f = mixp(*f, g, k);
        }
        Pose {
            hip: mixp(self.hip, o.hip, k),
            sh: mixp(self.sh, o.sh, k),
            head: mixp(self.head, o.head, k),
            nod: lerp(self.nod, o.nod, k),
            look: lerp(self.look, o.look, k),
            feet,
            leg: lerp(self.leg, o.leg, k),
            mouth: lerp(self.mouth, o.mouth, k),
            shut: lerp(self.shut, o.shut, k),
        }
    }

    /// Four-beat walk: near hind, near front, far hind, far front.
    fn walk(step: f64, mv: f64) -> Pose {
        let bob = (step * 2.0).sin() * 0.02 * mv;
        let foot = |base: f64, ph: f64| (base + 0.25 * ph.sin() * mv, 1.0 - 0.15 * ph.cos().max(0.0) * mv);
        Pose {
            hip: (-0.48, 0.05 + bob * 0.5),
            sh: (0.40, 0.02 + bob),
            head: (0.90, -0.70 + bob),
            nod: 0.2,
            look: 1.0,
            feet: [foot(-0.60, step + PI), foot(-0.50, step), foot(0.42, step + 1.5 * PI), foot(0.52, step + 0.5 * PI)],
            leg: LEG,
            mouth: 0.0,
            shut: 0.0,
        }
    }

    /// Standing with the head down in the grass, pulling and chewing.
    fn graze(t: f64, seed: f64) -> Pose {
        let pull = (t * 3.1 + seed).sin();
        let mut p = Pose::walk(0.0, 0.0);
        p.sh = (0.42, 0.05);
        p.head = (0.86 + 0.02 * pull, 0.60 - 0.03 * pull);
        p.nod = 1.30;
        p.mouth = 0.25 + 0.25 * (t * 7.0 + seed).sin();
        p
    }

    /// Waiting: head high and turned to the viewer, a front hoof stamping, bleating.
    fn proud(t: f64, seed: f64) -> Pose {
        let beat = (t * 7.0 + seed).sin().max(0.0).powi(2) * smooth((t * 1.1 + seed).sin() * 2.0 + 0.6);
        let bleat = smooth((t * 1.5 + seed).sin() * 2.2 - 0.5);
        let mut p = Pose::walk(0.0, 0.0);
        p.sh = (0.40, -0.06);
        p.head = (0.66, -0.74 - 0.03 * bleat);
        p.nod = -0.1 * bleat;
        p.look = 0.0;
        p.feet[3] = (0.56 + 0.05 * beat, 1.0 - 0.30 * beat);
        p.mouth = bleat;
        p
    }

    /// Blocked: braced, head and horns lowered.
    fn lowered(t: f64, seed: f64) -> Pose {
        let breath = (t * 1.3 + seed).sin() * 0.012;
        Pose {
            hip: (-0.50, 0.0),
            sh: (0.38, 0.07 + breath),
            head: (0.90, 0.10 + breath),
            nod: 1.0,
            look: 1.0,
            feet: [(-0.68, 1.0), (-0.56, 1.0), (0.50, 1.0), (0.62, 1.0)],
            leg: LEG,
            mouth: 0.0,
            shut: 0.0,
        }
    }

    /// Standing on a ledge, looking out over the valley; now and then the head turns to the viewer.
    fn lookout(t: f64, seed: f64) -> Pose {
        let breath = (t * 1.1 + seed).sin() * 0.01;
        let mut p = Pose::walk(0.0, 0.0);
        p.sh.1 = -0.05 + breath;
        p.head = (0.84, -0.80 + breath);
        p.nod = 0.05;
        p.look = if (t * 0.33 + seed).sin() > 0.72 { 0.0 } else { 1.0 };
        p
    }

    /// Lying with the legs folded under the body, head up, chewing.
    fn lie(t: f64, seed: f64) -> Pose {
        let breath = (t * 1.2 + seed).sin() * 0.012;
        Pose {
            hip: (-0.42, 0.62),
            sh: (0.36, 0.54 - breath),
            head: (0.80, -0.04 - breath),
            nod: 0.18,
            look: 1.0,
            feet: [(-0.14, 0.97), (-0.22, 0.99), (0.60, 0.97), (0.68, 0.99)],
            leg: 0.22,
            mouth: 0.18 + 0.18 * (t * 6.0 + seed).sin(),
            shut: 0.0,
        }
    }

    /// Asleep: lying, the head resting on the ground in front.
    fn sleep(t: f64, seed: f64) -> Pose {
        let breath = (t * 1.0 + seed).sin() * 0.02;
        let mut p = Pose::lie(0.0, 0.0);
        p.sh.1 = 0.54 - breath;
        p.hip.1 = 0.62 - breath * 0.5;
        p.head = (0.92, 0.70);
        p.nod = 0.42;
        p.mouth = 0.0;
        p.shut = 1.0;
        p
    }

    /// Gathering before a leap: the body low, all four legs bent.
    fn crouch() -> Pose {
        let mut p = Pose::walk(0.0, 0.0);
        p.hip = (-0.44, 0.30);
        p.sh = (0.40, 0.20);
        p.head = (0.92, -0.38);
        p.nod = 0.1;
        p
    }

    /// In the air; `u` runs from 0 (pushing off) to 1 (the front hooves reach for the rock).
    fn leap(u: f64) -> Pose {
        let body = |hind: [P; 2], front: [P; 2], head: P, nod: f64| Pose {
            hip: (-0.50, 0.04),
            sh: (0.42, -0.08),
            head,
            nod,
            look: 1.0,
            feet: [hind[0], hind[1], front[0], front[1]],
            leg: LEG,
            mouth: 0.0,
            shut: 0.0,
        };
        let a = body([(-1.02, 0.78), (-0.92, 0.86)], [(0.74, 0.50), (0.84, 0.44)], (0.96, -0.60), 0.05);
        let b = body([(-0.70, 0.60), (-0.60, 0.66)], [(0.98, 0.46), (1.06, 0.38)], (0.98, -0.58), 0.15);
        let c = body([(-0.40, 0.80), (-0.30, 0.86)], [(0.62, 0.94), (0.76, 0.88)], (0.92, -0.62), 0.3);
        if u < 0.5 { a.mix(&b, smooth(u * 2.0)) } else { b.mix(&c, smooth(u * 2.0 - 1.0)) }
    }

    /// Reared up on the hind legs, the front hooves pawing the air, the horns tossing.
    fn rear(t: f64) -> Pose {
        let (sn, cs) = (t * 13.0).sin_cos();
        Pose {
            hip: (-0.36, 0.22),
            sh: (0.14, -0.50),
            head: (0.50, -1.12),
            nod: 0.45 + 0.4 * (t * 10.0).sin(),
            look: 1.0,
            feet: [(-0.52, 1.0), (-0.34, 1.0), (0.66 + 0.08 * sn, -0.12 + 0.10 * cs), (0.74 - 0.08 * sn, 0.04 - 0.10 * cs)],
            leg: LEG,
            mouth: 0.3,
            shut: 0.0,
        }
    }
}

/// One bound: from a place on the ground or a ledge to the next.
#[derive(Clone, Copy, PartialEq)]
struct Jump {
    /// ledge and spot on it; None = the ground
    from: Option<(usize, usize)>,
    to: Option<(usize, usize)>,
    /// seconds of crouching left before the spring
    wind: f64,
    u: f64,
    dur: f64,
    x0: f64,
    y0: f64,
    k0: f64,
    x1: f64,
    y1: f64,
    k1: f64,
    /// height of the arc above the straight line
    arc: f64,
}

/// What an ibex is doing about the cliffs.
#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Ground,
    /// walking to the foot of the cliff below this ledge
    Approach { ledge: usize, spot: usize, since: f64 },
    Leap(Jump),
    OnLedge { ledge: usize, spot: usize, since: f64, until: f64 },
}

/// A kid (sub-agent) following its parent.
#[derive(Clone, Copy)]
struct Kid {
    x: f64,
    y: f64,
    face: f64,
    step: f64,
    moving: f64,
}

#[derive(Clone)]
pub struct Ibex {
    b: Base,
    mode: Mode,
    /// number of the world layout the mode belongs to (a resize makes new ledges)
    epoch: u64,
    next_climb: f64,
    /// how many more ledges it wants to visit before it turns back
    hops: i64,
    prev: Option<usize>,
    pose: Pose,
    /// pitch of the whole body in the air (negative = nose up)
    tilt: f64,
    /// depth factor (set by hand while off the ground)
    k: f64,
    /// where it left the ground (body centre) and the depth factor there: the kids wait here
    ground: P,
    ground_k: f64,
    /// rearing up after a tool call (seconds left)
    rear: f64,
    stamped: bool,
    /// 0..1 head down for a mouthful while working
    nibble: f64,
    /// 0..1 lying down
    lie: f64,
    /// 1..0 after landing
    land: f64,
    blink_at: f64,
    kids: Vec<Kid>,
    /// drawn as a kid: horn stubs, a bigger head, no beard
    kid: bool,
    /// the body is drawn this much above its place (hopping kids)
    lift: f64,
}

/// A rock shelf an ibex can stand on.
struct Ledge {
    x: f64,
    /// the line the hooves stand on
    y: f64,
    half: f64,
    /// direction to the open valley (+1 = right)
    out: f64,
    /// can be reached from the ground: the lowest shelf of its cliff
    ground: bool,
    /// where on the ground the leap up starts
    foot_x: f64,
    links: Vec<usize>,
    /// two places to stand: left and right
    used: [bool; 2],
}

impl Ledge {
    fn spot_x(&self, spot: usize) -> f64 {
        self.x + (spot as f64 * 2.0 - 1.0) * self.half * 0.46
    }

    /// A free place, the one at the valley end first.
    fn free_spot(&self) -> Option<usize> {
        let outer = if self.out > 0.0 { 1 } else { 0 };
        [outer, 1 - outer].into_iter().find(|&k| !self.used[k])
    }
}

#[derive(Clone, Copy, PartialEq)]
enum BitKind {
    Pebble,
    Dust,
    Spark,
}

/// A pebble, a puff of dust or a spark.
struct Bit {
    kind: BitKind,
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    size: f64,
    age: f64,
    life: f64,
    /// pebbles bounce here
    floor: f64,
    c: Rgb,
}

struct Tuft {
    x: f64,
    y: f64,
    size: f64,
    ph: f64,
    col: usize,
}

struct Cloud {
    /// position as a fraction of the way across, height, speed, which picture
    f: f64,
    y: f64,
    sp: f64,
    pic: usize,
}

/// One rock mass: outline, colours and its ledges.
struct Crag {
    outline: Vec<P>,
    lit: Rgb,
    shade: Rgb,
    first: usize,
    count: usize,
}

pub struct Capricorns {
    sky: Option<Picture>,
    land: Option<Picture>,
    cloud_pics: Vec<Picture>,
    shadow_pic: Option<Picture>,
    rock_pic: Option<Picture>,
    epoch: u64,
    ledges: Vec<Ledge>,
    bits: Vec<Bit>,
    clouds: Vec<Cloud>,
    tufts: Vec<Tuft>,
    /// eagles: centre, radii, phase, speed
    eagles: Vec<(f64, f64, f64, f64, f64, f64)>,
    /// the marmot's rock: x and y of its foot
    marmot: P,
    /// waterfall: x, top, bottom
    fall: (f64, f64, f64),
    /// the stream below it: start, bend, end
    stream: [P; 3],
}

impl ActorT<Capricorns> for Ibex {
    const BASE_RADIUS: f64 = 70.0;
    const WALKER: bool = true;
    const COMPANION_SCALE: f64 = 0.46;

    fn new(base: Base, env: &Env, world: &mut Capricorns) -> Self {
        let first = match base.look() {
            Look::Working => env.rng.uniform(4.0, 14.0),
            Look::Background => env.rng.uniform(8.0, 20.0),
            _ => env.rng.uniform(40.0, 90.0),
        };
        Ibex {
            b: base,
            mode: Mode::Ground,
            epoch: world.epoch,
            next_climb: env.t + first,
            hops: 0,
            prev: None,
            pose: Pose::walk(0.0, 0.0),
            tilt: 0.0,
            k: 1.0,
            ground: (0.0, 0.0),
            ground_k: 1.0,
            rear: 0.0,
            stamped: true,
            nibble: 0.0,
            lie: 0.0,
            land: 0.0,
            blink_at: env.rng.uniform(1.0, 4.0),
            kids: Vec::new(),
            kid: false,
            lift: 0.0,
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
        self.rear = 0.0;
        self.nibble = 0.0;
        self.lie = 0.0;
        self.land = 0.0;
        self.lift = 0.0;
        self.kids.clear();
        self.pose = Pose::walk(self.b.step, self.b.moving);
    }

    fn companion_spots(&self, _env: &Env, _t: f64) -> Vec<(f64, f64, f64)> {
        self.kids.iter().map(|k| (k.x, k.y, if k.face >= 0.0 { -PI / 2.0 } else { PI / 2.0 })).collect()
    }

    /// Kids: each has its own place and gait. On the meadow they trot behind; while the parent is
    /// up on the cliff they wait at the foot of it and hop about.
    fn draw_companions(&self, cr: &mut Canvas, env: &Env, world: &Capricorns, t: f64) {
        let b = &self.b;
        if b.alpha < 0.05 || self.kids.is_empty() {
            return;
        }
        let info = Rc::new(Session { id: b.info.id.clone(), state: State::Working, ..Session::default() });
        let up = self.off_ground();
        let big = if up { b.radius / self.k.max(0.1) * self.ground_k } else { b.radius };
        for (i, kid) in self.kids.iter().enumerate() {
            let fi = i as f64;
            let mut c = self.clone();
            c.companion_reset();
            c.kid = true;
            c.b.x = kid.x;
            c.b.y = kid.y;
            c.b.vx = 0.0;
            c.b.vy = 0.0;
            c.b.radius = big * Self::COMPANION_SCALE;
            c.b.color = WORKING_COLOR;
            c.b.info = info.clone();
            c.b.hop = 0.0;
            c.b.graze = 0.0;
            c.b.flicker = 1.0;
            c.b.face = kid.face;
            c.b.seed = b.seed + fi * 3.1;
            // playful hops: a burst of three or four, then a pause
            let mood = smooth((t * 0.9 + fi * 2.3 + b.seed).sin() * 2.0 - if up { 0.2 } else { 1.2 });
            let hop = mood * (t * 7.0 + fi * 1.9).sin().abs();
            c.lift = hop * c.b.radius * 0.55;
            c.pose = Pose::walk(kid.step, kid.moving).mix(&Pose::leap(0.45), hop * 0.8);
            c.draw(cr, env, world, t);
        }
    }

    fn mv(&mut self, env: &mut Env, world: &mut Capricorns, others: &[Other], dt: f64, t: f64, target: Option<(f64, f64)>) {
        let s = env.s;
        let st = self.b.state();
        let look = self.b.look();
        let r0 = self.b.radius; // not yet scaled with the depth
        if self.epoch != world.epoch {
            // the screen changed: the ledges are new, so stand on the meadow again
            self.epoch = world.epoch;
            if self.mode != Mode::Ground {
                self.mode = Mode::Ground;
                self.b.x = clamp(self.b.x, 120.0 * s, env.w - 120.0 * s);
                self.b.y = env.ground_top + r0 * 0.3 + 40.0 * s;
                self.b.vx = 0.0;
                self.b.vy = 0.0;
            }
            self.tilt = 0.0;
            self.kids.clear();
        }
        let rear0 = self.rear;
        self.rear = (self.rear - dt).max(0.0);
        self.land = (self.land - dt * 2.5).max(0.0);
        let may_climb = matches!(st, State::Working | State::Idle) && !self.b.dying;
        let mut want: Pose;
        let mut quick = 10.0;
        let mut lie = false;
        let mut tilt = 0.0;
        match self.mode {
            Mode::Ground => {
                // a mouthful of grass now and then
                let f = if st == State::Working { clamp((t * 0.4 + self.b.seed).sin() * 3.0 - 1.9, 0.0, 1.0) } else { 0.0 };
                self.nibble = lerp(self.nibble, f, (dt * 3.0).min(1.0));
                self.b.walker_move(env, others, dt, t, target, [64.0, 18.0, 55.0, 0.0], [0.9, 0.5, 0.5, 0.1]);
                if self.nibble > 0.5 || self.rear > 0.0 || self.land > 0.3 || self.lie > 0.3 {
                    self.b.x -= self.b.vx * dt * 0.92; // (almost) stands still
                    self.b.y -= self.b.vy * dt * 0.92;
                    self.b.moving *= 0.08;
                }
                let walk = Pose::walk(self.b.step, self.b.moving);
                let graze = Pose::graze(t, self.b.seed);
                want = match st {
                    State::Waiting => {
                        // at its place, or as near as the others let it come
                        let slow = hypot(self.b.vx, self.b.vy) < 30.0 * s;
                        let there = slow || target.is_none_or(|(tx, ty)| hypot(tx - self.b.x, ty - self.b.y) < 220.0 * s);
                        if there { Pose::proud(t, self.b.seed) } else { walk }
                    }
                    State::Blocked => Pose::lowered(t, self.b.seed),
                    State::Rest => Pose::sleep(t, self.b.seed),
                    State::Idle => {
                        // every other pause it lies down instead of grazing
                        let cyc = t * 0.21 + self.b.seed;
                        lie = cyc.sin() < -0.2 && ((cyc / TAU).floor() as i64).rem_euclid(2) == 0;
                        walk.mix(&graze, smooth(self.b.graze * 1.3)).mix(&Pose::lie(t, self.b.seed), smooth(self.lie))
                    }
                    State::Working => walk.mix(&graze, smooth(self.nibble)),
                };
                self.ground = (self.b.x, self.b.y);
                self.ground_k = self.b.depth_scale(env);
                if may_climb && target.is_none() && self.rear <= 0.0 && self.lie < 0.1 && t > self.next_climb {
                    self.next_climb = t + 3.0;
                    if let Some((ledge, spot)) = world.pick_start(env, self.b.x) {
                        world.ledges[ledge].used[spot] = true;
                        self.mode = Mode::Approach { ledge, spot, since: t };
                        self.prev = None;
                        self.hops = match look {
                            Look::Working => env.rng.randint(1, 3),
                            Look::Background => env.rng.randint(0, 2),
                            _ => env.rng.randint(0, 1),
                        };
                    }
                }
            }
            Mode::Approach { ledge, spot, since } => {
                // walk to the foot of the cliff below the first ledge
                let l = &world.ledges[ledge];
                let (fx, fy) = (l.foot_x, env.ground_top + r0 * 0.3 + 10.0 * s);
                let (dx, dy) = (fx - self.b.x, fy - self.b.y);
                let d = hypot(dx, dy).max(0.001);
                let sp = (115.0 * s).min(d * 2.5 + 12.0 * s);
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
                self.nibble = 0.0;
                self.ground = (self.b.x, self.b.y);
                self.ground_k = self.b.depth_scale(env);
                want = Pose::walk(self.b.step, self.b.moving);
                if !may_climb || target.is_some() || t - since > 16.0 {
                    self.mode = Mode::Ground;
                    self.next_climb = t + env.rng.uniform(8.0, 16.0);
                } else if d < 9.0 * s {
                    self.k = self.ground_k;
                    self.mode = Mode::Leap(self.jump(env, world, None, Some((ledge, spot)), r0));
                }
            }
            Mode::Leap(mut j) => {
                let dir = if j.x1 >= j.x0 { 1.0 } else { -1.0 };
                self.b.moving = 0.0;
                self.b.vx = 0.0;
                self.b.vy = 0.0;
                if j.wind > 0.0 {
                    // gather: crouch, facing the way it will go
                    j.wind -= dt;
                    self.b.face = lerp(self.b.face, dir, (dt * 12.0).min(1.0));
                    self.b.x = j.x0;
                    self.b.y = j.y0;
                    self.k = j.k0;
                    want = Pose::crouch();
                    quick = 13.0;
                    if j.wind <= 0.0 {
                        // the spring knocks a pebble or two loose
                        let feet = j.y0 + r0 * j.k0;
                        world.pebbles(env, j.x0 - dir * r0 * 0.4, feet, 2, -dir);
                        world.dust(env, j.x0 - dir * r0 * 0.4, feet, 3);
                    }
                    self.mode = Mode::Leap(j);
                } else {
                    j.u = (j.u + dt / j.dur).min(1.0);
                    let u = j.u;
                    self.b.face = dir;
                    self.b.x = lerp(j.x0, j.x1, u);
                    self.b.y = lerp(j.y0, j.y1, u) - 4.0 * j.arc * u * (1.0 - u);
                    self.k = lerp(j.k0, j.k1, smooth(u));
                    // nose along the flight path
                    let slope = (j.y1 - j.y0) - 4.0 * j.arc * (1.0 - 2.0 * u);
                    tilt = clamp(slope.atan2((j.x1 - j.x0).abs().max(1.0)) * 0.6, -0.65, 0.6);
                    want = Pose::leap(u);
                    quick = 22.0;
                    if u >= 1.0 {
                        // landed: a little dust, a pebble or two tumbling down
                        self.land = 1.0;
                        tilt = 0.0;
                        let feet = j.y1 + r0 * j.k1;
                        world.dust(env, j.x1 + dir * r0 * 0.3, feet, 6);
                        world.pebbles(env, j.x1 + dir * r0 * 0.4, feet, if j.to.is_some() { 3 } else { 2 }, dir);
                        self.mode = match j.to {
                            Some((ledge, spot)) => {
                                let stay = match look {
                                    Look::Idle => env.rng.uniform(25.0, 50.0),
                                    _ if self.hops <= 0 => env.rng.uniform(8.0, 16.0),
                                    Look::Background => env.rng.uniform(4.0, 8.0),
                                    _ => env.rng.uniform(2.0, 4.5),
                                };
                                Mode::OnLedge { ledge, spot, since: t, until: t + stay }
                            }
                            None => {
                                self.b.heading = if dir > 0.0 { 0.0 } else { PI };
                                self.next_climb = t + climb_pause(&env.rng, look);
                                Mode::Ground
                            }
                        };
                    } else {
                        self.mode = Mode::Leap(j);
                    }
                }
            }
            Mode::OnLedge { ledge, spot, since, mut until } => {
                let l = &world.ledges[ledge];
                self.k = LEDGE_K;
                self.b.x = lerp(self.b.x, l.spot_x(spot), (dt * 2.0).min(1.0));
                self.b.y = l.y - r0 * LEDGE_K;
                self.b.vx = 0.0;
                self.b.vy = 0.0;
                self.b.moving = 0.0;
                // look out over the valley, sometimes back at the rock
                let side = if (t * 0.19 + self.b.seed).sin() > 0.75 { -l.out } else { l.out };
                if t - since > 1.2 {
                    self.b.face = lerp(self.b.face, side, (dt * 3.0).min(1.0));
                }
                let stand = Pose::lookout(t, self.b.seed);
                lie = st == State::Idle && t - since > 2.0 && t < until;
                want = match st {
                    State::Blocked => Pose::lowered(t, self.b.seed),
                    _ => stand.mix(&Pose::lie(t, self.b.seed), smooth(self.lie)),
                };
                if st == State::Waiting || self.b.dying {
                    until = until.min(t); // you are needed down on the meadow
                }
                if st == State::Blocked {
                    until = until.max(t + 1.0);
                }
                self.mode = Mode::OnLedge { ledge, spot, since, until };
                if t > until && self.rear <= 0.0 && self.land <= 0.0 && self.lie < 0.08 {
                    let down = self.hops <= 0 || !may_climb;
                    let next = if down { None } else { world.pick_next(env, ledge, self.prev) };
                    let step = match next {
                        Some(n) => Some(Some(n)),
                        None if l.ground => Some(None),
                        // a lower ledge - or, if that stays full for long, one big bound to the meadow
                        None => world.pick_down(ledge).map(Some).or(if t > until + 6.0 { Some(None) } else { None }),
                    };
                    if let Some(to) = step {
                        if next.is_some() {
                            self.hops -= 1;
                        } else {
                            self.hops = 0;
                        }
                        if let Some((n, sp)) = to {
                            world.ledges[n].used[sp] = true;
                        }
                        self.prev = Some(ledge);
                        self.mode = Mode::Leap(self.jump(env, world, Some((ledge, spot)), to, r0));
                    }
                }
            }
        }
        // a tool call: up on the hind legs, then stamp down so that sparks and pebbles fly
        if self.rear > 0.0 && matches!(self.mode, Mode::Ground | Mode::OnLedge { .. }) {
            let p = 1.0 - self.rear / REAR;
            let up = smooth(p / 0.2) * (1.0 - smooth((p - 0.68) / 0.12));
            want = want.mix(&Pose::rear(t), up);
            quick = 16.0;
            if p >= 0.8 && !self.stamped {
                self.stamped = true;
                let rd = r0 * if self.off_ground() { self.k } else { self.b.depth_scale(env) };
                let side = if self.b.face >= 0.0 { 1.0 } else { -1.0 };
                world.sparks(env, self.b.x + side * rd * 0.55, self.b.y + rd, self.b.color);
                world.pebbles(env, self.b.x + side * rd * 0.55, self.b.y + rd, 2, side);
            }
        } else if rear0 > 0.0 {
            self.rear = 0.0;
        }
        self.lie = lerp(self.lie, if lie { 1.0 } else { 0.0 }, (dt * 2.2).min(1.0));
        self.tilt = lerp(self.tilt, tilt, (dt * 14.0).min(1.0));
        if self.mode == Mode::Ground && !may_climb {
            self.next_climb = self.next_climb.max(t + 6.0);
        }
        self.pose = self.pose.mix(&want, (dt * quick).min(1.0));
        if t > self.blink_at + 0.2 {
            self.blink_at = t + env.rng.uniform(2.0, 6.0);
        }
        // Off the ground the size must not follow the height: the stage multiplies the radius with
        // the depth factor of `y` after this, so prepare it to end up at the factor we want.
        let kd = self.b.depth_scale(env);
        if !self.off_ground() {
            self.k = kd;
        }
        self.b.radius *= self.k / kd;
        self.move_kids(env, r0, dt, t);
    }

    fn draw(&self, cr: &mut Canvas, env: &Env, _world: &Capricorns, t: f64) {
        let s = env.s;
        let b = &self.b;
        let (c, st) = (b.color, b.state());
        if b.alpha < 0.01 {
            return;
        }
        let rest = st == State::Rest;
        // the sleeper is a little bigger and fully opaque (at 80 % the legs would show through);
        // blocked: the glow flickers hard, the body only a little
        let r = if rest { b.radius * 1.5 } else { b.radius };
        let a = if rest { (b.alpha / 0.8).min(1.0) } else { b.alpha * b.flicker };
        let body_a = if rest { a } else { b.alpha * lerp(1.0, b.flicker, 0.35) };
        let side = if b.face >= 0.0 { 1.0 } else { -1.0 };
        // in the air the shadow and the glow on the rock are left behind
        let air = match self.mode {
            Mode::Leap(j) if j.wind <= 0.0 => (4.0 * j.u * (1.0 - j.u) * 3.0).min(1.0),
            _ => (self.lift / (r * 0.3)).min(1.0),
        };
        let gy = b.y + r;
        let mut g = Gradient::radial(b.x, gy, 0.0, b.x, gy, r * 1.9);
        g.stop(0.0, c, if rest { 0.30 } else { 0.62 } * a * (1.0 - air));
        g.stop(1.0, c, 0.0);
        cr.save();
        cr.translate(b.x, gy);
        cr.scale(1.0, 0.3);
        cr.translate(-b.x, -gy);
        cr.set_source(&g);
        cr.circle(b.x, gy, r * 1.9);
        cr.fill();
        cr.restore();
        cr.ellipse(b.x, gy, r * 0.95 * (1.0 - 0.4 * air), r * 0.12 * (1.0 - 0.4 * air));
        cr.set_source_rgba(0.05, 0.08, 0.10, 0.34 * a * (1.0 - air));
        cr.fill();
        if !rest {
            cr.glow(b.x, b.y - r * 0.2, r * 1.8, c, lerp(0.20, 0.34, air) * a, 0.0);
        }
        if st == State::Waiting {
            cr.pulse_rings(b.x, b.y - r * 0.2, r, c, a, t, s);
        }
        cr.save();
        cr.translate(b.x, b.y - self.lift);
        // turning round is quick: the figure never gets very thin
        cr.scale(b.face.abs().powf(0.4).max(0.15) * side, 1.0);
        cr.rotate(self.tilt);
        if self.land > 0.0 {
            // the legs give after landing
            let q = (self.land * PI).sin().max(0.0) * 0.5 + self.land * 0.5;
            cr.translate(0.0, r);
            cr.scale(1.0 + 0.08 * q, 1.0 - 0.20 * q);
            cr.translate(0.0, -r);
        }
        let flick = smooth((t * 0.9 + b.seed * 3.0).sin() * 6.0 - 4.6) * (t * 28.0).sin() * 0.5;
        let lk = Looks {
            c,
            kid: self.kid,
            blink: self.blink_at < t && t < self.blink_at + 0.2,
            ear: flick,
            edge: (r * 0.045).min(3.2 * s),
            t,
        };
        figure(cr, &self.pose, r, 3.3 * s, Some(c), 0.92 * a, &lk); // rim light
        figure(cr, &self.pose, r, 0.0, None, body_a, &lk);
        cr.restore();
        if st == State::Waiting && self.pose.mouth > 0.25 && self.pose.look < 0.5 {
            // bleating: little arcs of sound beside the mouth
            let (mx, my) = (b.x + side * self.pose.head.0 * r, b.y + (self.pose.head.1 + 0.26) * r);
            cr.set_line_cap(LineCap::Round);
            cr.set_line_width(2.6 * s);
            for k in 0..3 {
                let f = (t * 2.2 + k as f64 / 3.0) % 1.0;
                let rad = r * (0.28 + 0.34 * f);
                cr.set_rgba(darken(c, 0.25), (1.0 - f) * 0.9 * a * self.pose.mouth);
                cr.new_sub_path();
                if side > 0.0 {
                    cr.arc(mx, my, rad, -0.55, 0.55);
                } else {
                    cr.arc(mx, my, rad, PI - 0.55, PI + 0.55);
                }
                cr.stroke();
            }
        }
        if rest {
            for k in 0..3 {
                let f = (t * 0.3 + k as f64 / 3.0) % 1.0;
                let lay = text::layout("z", (14.0 + 14.0 * f) * s, weight::BOLD, None);
                let zx = b.x + side * r * (0.9 + 0.5 * f) + (f * 6.0).sin() * 4.0 * s;
                let zy = b.y + r * 0.2 - f * r * 1.5;
                text::show(cr, &lay, zx, zy, (1.0, 1.0, 1.0), 0.9 * (PI * f).sin() * a, 0.5);
            }
        }
    }
}

/// How long a rear-up after a tool call takes.
const REAR: f64 = 1.15;

/// How long until the next climb.
fn climb_pause(rng: &Rng, look: Look) -> f64 {
    match look {
        Look::Working => rng.uniform(10.0, 25.0),
        Look::Background => rng.uniform(16.0, 34.0),
        _ => rng.uniform(60.0, 140.0),
    }
}

/// Walker depth factor for a body centre at height `y` on the meadow.
fn ground_k(env: &Env, y: f64) -> f64 {
    let (top, bottom) = (env.ground_top, env.h - env.strip_h);
    lerp(0.65, 1.15, clamp((y - top) / (bottom - top).max(1.0), 0.0, 1.0))
}

impl Ibex {
    fn off_ground(&self) -> bool {
        !matches!(self.mode, Mode::Ground | Mode::Approach { .. })
    }

    /// The places on ledges this ibex stands on or is about to use.
    fn holds(&self) -> [Option<(usize, usize)>; 2] {
        match self.mode {
            Mode::Approach { ledge, spot, .. } | Mode::OnLedge { ledge, spot, .. } => [Some((ledge, spot)), None],
            Mode::Leap(j) => [j.from, j.to],
            Mode::Ground => [None, None],
        }
    }

    /// Plan a bound from where it stands to a ledge, or (None) down to the meadow.
    fn jump(&self, env: &Env, world: &Capricorns, from: Option<(usize, usize)>, to: Option<(usize, usize)>, r0: f64) -> Jump {
        let s = env.s;
        let (x0, y0, k0) = (self.b.x, self.b.y, self.k);
        let (x1, y1, k1) = match to {
            Some((ledge, spot)) => {
                let l = &world.ledges[ledge];
                (l.spot_x(spot) + env.rng.uniform(-6.0, 6.0) * s, l.y - r0 * LEDGE_K, LEDGE_K)
            }
            None => {
                // to the meadow at the foot of the cliff, away from the rock
                let l = from.map(|(i, _)| &world.ledges[i]);
                let out = l.map_or(1.0, |l| l.out);
                let x = l.map_or(x0, |l| if l.ground { l.foot_x } else { x0 + out * 150.0 * s }) + env.rng.uniform(-25.0, 25.0) * s;
                let y = env.ground_top + r0 * 0.3 + env.rng.uniform(10.0, 34.0) * s;
                (clamp(x, 110.0 * s, env.w - 110.0 * s), y, ground_k(env, y))
            }
        };
        let d = hypot(x1 - x0, y1 - y0);
        let arc = if y1 < y0 { 16.0 * s + 0.10 * d } else { 12.0 * s + 0.07 * d };
        Jump { from, to, wind: 0.42, u: 0.0, dur: 0.46 + d / (900.0 * s), x0, y0, k0, x1, y1, k1, arc }
    }

    /// The kids trot behind on the meadow; under a climbing parent they potter about at the foot of the cliff.
    fn move_kids(&mut self, env: &Env, r0: f64, dt: f64, t: f64) {
        let n = self.b.info.subagents.len();
        self.kids.truncate(n);
        let up = self.off_ground();
        let (gx, gy) = if up { self.ground } else { (self.b.x, self.b.y) };
        let rd = r0 * if up { self.ground_k } else { self.k };
        let back = if self.b.face >= 0.0 { -1.0 } else { 1.0 };
        for i in 0..n {
            let fi = i as f64;
            let tx = if up {
                gx + (fi - (n as f64 - 1.0) / 2.0) * rd * 1.0 + (t * 0.45 + fi * 2.1 + self.b.seed).sin() * rd * 0.55
            } else {
                gx + back * rd * (1.3 + fi * 0.75)
            };
            let ty = gy + rd * (0.50 + 0.16 * (i % 2) as f64);
            if i >= self.kids.len() {
                self.kids.push(Kid { x: tx, y: ty, face: -back, step: fi * 2.0, moving: 0.0 });
                continue;
            }
            let kid = &mut self.kids[i];
            let k = 1.0 - (-dt * 3.0).exp();
            let (dx, dy) = ((tx - kid.x) * k, (ty - kid.y) * k);
            kid.x += dx;
            kid.y += dy;
            let v = hypot(dx, dy) / dt.max(1e-4);
            kid.moving = lerp(kid.moving, clamp(v / (22.0 * env.s), 0.0, 1.0), (dt * 6.0).min(1.0));
            kid.step += dt * v / (rd * 0.46 * 0.35).max(1.0);
            if v > 14.0 * env.s {
                kid.face = lerp(kid.face, dx.signum(), (dt * 6.0).min(1.0));
            } else if !up {
                kid.face = lerp(kid.face, -back, (dt * 3.0).min(1.0));
            }
        }
    }
}

/// The things about the head and the fur that do not belong to the pose.
struct Looks {
    c: Rgb,
    kid: bool,
    blink: bool,
    /// angle of an ear flick
    ear: f64,
    /// width of the dark edge around the legs in front (thinner than the rim light)
    edge: f64,
    t: f64,
}

/// The middle joint of a leg from `a` to `b` with two segments `len` long, bending to the side `bend`.
fn joint(a: P, b: P, len: f64, bend: f64) -> P {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let d = hypot(dx, dy).max(1e-6);
    let half = (d / 2.0).min(len);
    let off = (len * len - half * half).max(0.0).sqrt() * bend;
    ((a.0 + b.0) / 2.0 - dy / d * off, (a.1 + b.1) / 2.0 + dx / d * off)
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

/// Centre line of a horn in profile: an arc from `base` up and over to the back.
fn horn_arc(base: P, rad: f64, sweep: f64, lean: f64) -> Vec<P> {
    let (sn, cs) = lean.sin_cos();
    (0..=13)
        .map(|i| {
            let th = sweep * i as f64 / 13.0;
            let (x, y) = (-rad + rad * th.cos(), -rad * th.sin());
            (base.0 + x * cs - y * sn, base.1 + x * sn + y * cs)
        })
        .collect()
}

/// Centre line of a horn seen from the front: up and outwards.
fn horn_front(side: f64) -> Vec<P> {
    let (p0, p1, p2) = ((side * 0.075, -0.13), (side * 0.16, -0.86), (side * 0.66, -1.02));
    (0..=11)
        .map(|i| {
            let f = i as f64 / 11.0;
            let g = 1.0 - f;
            (g * g * p0.0 + 2.0 * g * f * p1.0 + f * f * p2.0, g * g * p0.1 + 2.0 * g * f * p1.1 + f * f * p2.1)
        })
        .collect()
}

/// A tapering horn along `pts` (units of `u`), `w0` wide at the base. With `ridge` the cross
/// ridges are drawn: knobs along the front edge in profile, bars right across from the front (`full`).
#[allow(clippy::too_many_arguments)]
fn horn(cr: &mut Canvas, u: f64, pts: &[P], w0: f64, extra: f64, col: Rgb, ridge: Option<Rgb>, a: f64, full: bool) {
    let n = pts.len();
    let e = extra / u;
    let mut edges: Vec<(P, P, P, f64)> = Vec::with_capacity(n);
    for i in 0..n {
        let (p0, p1) = (pts[i.saturating_sub(1)], pts[(i + 1).min(n - 1)]);
        let (tx, ty) = (p1.0 - p0.0, p1.1 - p0.1);
        let d = hypot(tx, ty).max(1e-9);
        let nrm = (-ty / d, tx / d); // the front (outer) side of the curve
        let wd = w0 * 0.5 * (1.0 - 0.86 * i as f64 / (n - 1) as f64);
        let p = pts[i];
        edges.push(((p.0 + nrm.0 * (wd + e), p.1 + nrm.1 * (wd + e)), (p.0 - nrm.0 * (wd + e), p.1 - nrm.1 * (wd + e)), nrm, wd));
    }
    cr.move_to(edges[0].0 .0 * u, edges[0].0 .1 * u);
    for ed in &edges[1..] {
        cr.line_to(ed.0 .0 * u, ed.0 .1 * u);
    }
    for ed in edges.iter().rev() {
        cr.line_to(ed.1 .0 * u, ed.1 .1 * u);
    }
    cr.close_path();
    cr.set_rgba(col, a);
    cr.fill();
    if !full && w0 > 0.1 {
        // the knobs of the ridges stand out along the front edge
        for (i, ed) in edges.iter().enumerate().take(n - 2).skip(1) {
            let p = pts[i];
            cr.circle((p.0 + ed.2 .0 * ed.3 * 0.9) * u, (p.1 + ed.2 .1 * ed.3 * 0.9) * u, ed.3 * 0.42 * u + extra);
        }
        cr.fill();
    }
    let Some(rc) = ridge else { return };
    for (i, ed) in edges.iter().enumerate().take(n - 1).skip(1) {
        let p = pts[i];
        let back = if full { -1.0 } else { -0.1 };
        cr.move_to((p.0 + ed.2 .0 * ed.3 * 1.05) * u, (p.1 + ed.2 .1 * ed.3 * 1.05) * u);
        cr.line_to((p.0 + ed.2 .0 * ed.3 * back) * u, (p.1 + ed.2 .1 * ed.3 * back) * u);
    }
    cr.set_rgba(rc, 0.9 * a);
    cr.set_line_width(u * 0.024);
    cr.stroke();
    // a light line along the back of the horn
    cr.move_to((pts[0].0 - edges[0].2 .0 * edges[0].3 * 0.45) * u, (pts[0].1 - edges[0].2 .1 * edges[0].3 * 0.45) * u);
    for (i, ed) in edges.iter().enumerate().take(n - 2).skip(1) {
        let k = if full { 0.0 } else { 0.45 };
        cr.line_to((pts[i].0 - ed.2 .0 * ed.3 * k) * u, (pts[i].1 - ed.2 .1 * ed.3 * k) * u);
    }
    cr.set_rgba(lighten(col, 0.45), 0.55 * a);
    cr.set_line_width(u * 0.02);
    cr.stroke();
}

/// The ibex in local coordinates. With `rim` everything is drawn a little bigger in one colour
/// (the rim light); without, the real animal with its details.
fn figure(cr: &mut Canvas, p: &Pose, r: f64, extra: f64, rim: Option<Rgb>, a: f64, lk: &Looks) {
    if a < 0.01 {
        return;
    }
    let col = |base: Rgb| rim.unwrap_or(base);
    let fur = lerp3(FUR, lk.c, 0.06);
    let far = lerp3(FUR_DARK, lk.c, 0.04);
    let e = extra / r;
    cr.set_line_cap(LineCap::Round);
    cr.set_line_join(LineJoin::Round);
    let hipj = (p.hip.0 - 0.08, p.hip.1 + 0.12);
    let shj = (p.sh.0 + 0.06, p.sh.1 + 0.16);
    let leg = |cr: &mut Canvas, k: usize, upper: Rgb, near: bool| {
        // the front legs are a little shorter, so they stand almost straight
        let (top, bend, wid, len) = if k < 2 { (hipj, 1.0, 0.25, p.leg) } else { (shj, -0.5, 0.19, p.leg * 0.91) };
        let foot = p.feet[k];
        // hock and knee sit low: the furred upper part is the longer one
        let j = mixp(joint(top, (foot.0, foot.1 - 0.05), len, bend), foot, if k < 2 { 0.12 } else { 0.26 });
        if rim.is_none() && near {
            cr.set_rgba(far, 0.7 * a); // a dark edge sets the leg off against the body
            cr.move_to(top.0 * r, top.1 * r);
            cr.line_to(j.0 * r, j.1 * r);
            cr.set_line_width(wid * r + lk.edge);
            cr.stroke();
        }
        cr.set_rgba(col(upper), a);
        cr.move_to(top.0 * r, top.1 * r);
        cr.line_to(j.0 * r, j.1 * r);
        cr.set_line_width(wid * r + extra * 2.0);
        cr.stroke();
        // dark lower leg and hoof
        cr.set_rgba(col(LEG_DARK), a);
        cr.move_to(j.0 * r, j.1 * r);
        cr.line_to(foot.0 * r, (foot.1 - 0.05) * r);
        cr.set_line_width(0.11 * r + extra * 2.0);
        cr.stroke();
        cr.ellipse((foot.0 + 0.03) * r, (foot.1 - 0.04) * r, 0.085 * r + extra, 0.05 * r + extra);
        cr.fill();
    };
    leg(cr, 0, far, false);
    leg(cr, 2, far, false);
    // short tail, turned up
    let (ax, ay) = (p.sh.0 - p.hip.0, p.sh.1 - p.hip.1);
    let d = hypot(ax, ay).max(1e-6);
    let n = (ay / d, -ax / d); // towards the back
    let tail = (p.hip.0 - ax / d * 0.30 + n.0 * 0.20, p.hip.1 - ay / d * 0.30 + n.1 * 0.20);
    cr.move_to(tail.0 * r, tail.1 * r);
    cr.line_to((tail.0 - ax / d * 0.10 + n.0 * 0.12) * r, (tail.1 - ay / d * 0.10 + n.1 * 0.12) * r);
    cr.set_rgba(col(LEG_DARK), a);
    cr.set_line_width(0.09 * r + extra * 2.0);
    cr.stroke();
    // torso: deep chest, strong rump, the belly between
    let (srx, sry) = (0.35, 0.40);
    let (hrx, hry) = (0.33, 0.36);
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
    cr.move_to(quad[0].0 * r, quad[0].1 * r);
    for q in &quad[1..] {
        cr.line_to(q.0 * r, q.1 * r);
    }
    cr.close_path();
    cr.set_rgba(col(fur), a);
    cr.fill();
    let mid = mixp(p.hip, p.sh, 0.5);
    let ang = ay.atan2(ax);
    if rim.is_none() {
        let band = |cr: &mut Canvas, along: f64, down: f64, rx: f64, ry: f64| {
            cr.save();
            cr.translate((mid.0 + ax / d * along - n.0 * down) * r, (mid.1 + ay / d * along - n.1 * down) * r);
            cr.rotate(ang);
            cr.ellipse(0.0, 0.0, rx * r, ry * r);
            cr.restore();
        };
        // dark stripe along the flank, the lighter belly below, a light patch on the rump
        band(cr, 0.0, 0.17, 0.52, 0.075);
        cr.set_rgba(far, 0.45 * a);
        cr.fill();
        band(cr, 0.0, 0.27, 0.48, 0.08);
        cr.set_rgba(lerp3(FUR_LIGHT, lk.c, 0.05), 0.92 * a);
        cr.fill();
        band(cr, -0.66, -0.02, 0.07, 0.17);
        cr.set_rgba(FUR_LIGHT, 0.75 * a);
        cr.fill();
        // light on the back and the shoulder
        band(cr, 0.05, -0.23, 0.50, 0.075);
        cr.set_rgba(lighten(fur, 0.22), 0.6 * a);
        cr.fill();
    }
    // strong neck
    let nb = (p.sh.0 + 0.10, p.sh.1 - 0.12);
    let (dx, dy) = (p.head.0 - nb.0, p.head.1 - nb.1);
    let nd = hypot(dx, dy).max(1e-6);
    let nn = (-dy / nd, dx / nd);
    let (w0, w1) = (0.30 + e, 0.16 + e);
    cr.move_to((nb.0 + nn.0 * w0) * r, (nb.1 + nn.1 * w0) * r);
    cr.line_to((p.head.0 + nn.0 * w1) * r, (p.head.1 + nn.1 * w1) * r);
    cr.line_to((p.head.0 - nn.0 * w1) * r, (p.head.1 - nn.1 * w1) * r);
    cr.line_to((nb.0 - nn.0 * w0) * r, (nb.1 - nn.1 * w0) * r);
    cr.close_path();
    cr.set_rgba(col(fur), a);
    cr.fill();
    cr.circle(nb.0 * r, nb.1 * r, w0 * r * 0.96);
    cr.fill();
    // head and horns turn together
    cr.save();
    cr.translate(p.head.0 * r, p.head.1 * r);
    let u = if lk.kid { r * 1.3 } else { r * 1.12 };
    if p.look >= 0.5 {
        cr.rotate(p.nod);
        head_profile(cr, p, u, extra, rim, a, lk, fur);
    } else {
        cr.rotate(p.nod * 0.4);
        head_front(cr, p, u, extra, rim, a, lk, fur);
    }
    cr.restore();
    cr.set_line_cap(LineCap::Round);
    leg(cr, 1, fur, true);
    leg(cr, 3, fur, true);
}

/// Colours of the horns: a strong tint of the state colour, darker for the far horn and the ridges.
fn horn_colors(c: Rgb) -> (Rgb, Rgb, Rgb) {
    let near = lerp3(HORN, c, 0.74);
    (near, darken(near, 0.26), darken(near, 0.52))
}

/// An eye with the level pupil of a goat, or a closed lid.
fn eye(cr: &mut Canvas, x: f64, y: f64, u: f64, shut: bool, a: f64) {
    if shut {
        cr.set_rgba(LEG_DARK, a);
        cr.set_line_width(u * 0.02);
        cr.new_sub_path();
        cr.arc(x, y - u * 0.01, u * 0.035, 0.15 * PI, 0.85 * PI);
        cr.stroke();
        return;
    }
    cr.circle(x, y, u * 0.042);
    cr.set_rgba(LEG_DARK, a);
    cr.fill();
    cr.circle(x, y, u * 0.033);
    cr.set_source_rgba(0.93, 0.70, 0.26, a);
    cr.fill();
    cr.ellipse(x, y, u * 0.024, u * 0.011);
    cr.set_source_rgba(0.05, 0.04, 0.03, a);
    cr.fill();
}

/// Head in profile, in its own coordinates: the muzzle points along +x, `u` is the unit.
#[allow(clippy::too_many_arguments)]
fn head_profile(cr: &mut Canvas, p: &Pose, u: f64, extra: f64, rim: Option<Rgb>, a: f64, lk: &Looks, fur: Rgb) {
    let col = |base: Rgb| rim.unwrap_or(base);
    let (hc, hfar, ridge) = horn_colors(lk.c);
    let (rad, sweep, w0) = if lk.kid { (0.10, 1.1, 0.07) } else { (0.49, 2.2, 0.16) };
    horn(cr, u, &horn_arc((0.045, -0.11), rad, sweep * 0.97, -0.10), w0, extra, col(hfar), None, a, false);
    // skull and muzzle
    cr.circle(0.0, 0.0, 0.17 * u + extra);
    cr.save();
    cr.translate(0.17 * u, 0.04 * u);
    cr.rotate(0.22);
    cr.ellipse(0.0, 0.0, 0.20 * u + extra, 0.105 * u + extra);
    cr.restore();
    cr.set_rgba(col(lerp3(fur, FUR_DARK, 0.18)), a);
    cr.fill();
    if !lk.kid {
        // beard
        let sway = (lk.t * 2.0).sin() * 0.02;
        cr.move_to(0.11 * u, 0.10 * u);
        cr.line_to(0.24 * u, 0.12 * u);
        cr.line_to((0.15 + sway) * u, 0.33 * u);
        cr.close_path();
        cr.set_rgba(col(FUR_DARK), a);
        cr.fill_preserve();
        cr.set_line_width(extra * 2.0 + 0.02 * u);
        cr.stroke();
    }
    if rim.is_none() {
        cr.ellipse(0.27 * u, 0.085 * u, 0.085 * u, 0.05 * u);
        cr.set_rgba(FUR_LIGHT, 0.85 * a);
        cr.fill();
        cr.ellipse(0.345 * u, 0.06 * u, 0.035 * u, 0.032 * u);
        cr.set_rgba(LEG_DARK, a);
        cr.fill();
        if p.mouth > 0.05 {
            cr.ellipse(0.27 * u, 0.125 * u, 0.065 * u, 0.045 * u * p.mouth);
            cr.set_source_rgba(0.30, 0.08, 0.08, a);
            cr.fill();
        } else {
            cr.move_to(0.34 * u, 0.115 * u);
            cr.line_to(0.21 * u, 0.125 * u);
            cr.set_rgba(LEG_DARK, 0.8 * a);
            cr.set_line_width(0.016 * u);
            cr.stroke();
        }
        eye(cr, 0.075 * u, -0.045 * u, u, p.shut > 0.5 || lk.blink, a);
    }
    horn(cr, u, &horn_arc((-0.02, -0.13), rad, sweep, -0.26), w0, extra, col(hc), if rim.is_none() && !lk.kid { Some(ridge) } else { None }, a, false);
    // ear, behind the horn
    leaf_path(cr, -0.09 * u, -0.06 * u, 0.25 * u + extra, 0.065 * u + extra, PI + 0.42 + lk.ear);
    cr.set_rgba(col(lerp3(fur, FUR_DARK, 0.35)), a);
    cr.fill();
}

/// Head seen from the front: both horns rise in a wide V, the ridges face the viewer.
#[allow(clippy::too_many_arguments)]
fn head_front(cr: &mut Canvas, p: &Pose, u: f64, extra: f64, rim: Option<Rgb>, a: f64, lk: &Looks, fur: Rgb) {
    let col = |base: Rgb| rim.unwrap_or(base);
    let (hc, _, ridge) = horn_colors(lk.c);
    for side in [-1.0, 1.0] {
        if lk.kid {
            horn(cr, u, &[(side * 0.07, -0.13), (side * 0.09, -0.21), (side * 0.12, -0.28)], 0.07, extra, col(hc), None, a, true);
        } else {
            let pts = horn_front(side);
            horn(cr, u, &pts, 0.17, extra, col(hc), if rim.is_none() { Some(ridge) } else { None }, a, true);
        }
        leaf_path(cr, side * 0.12 * u, -0.05 * u, 0.27 * u + extra, 0.07 * u + extra, if side > 0.0 { -0.15 - lk.ear } else { PI + 0.15 + lk.ear });
        cr.set_rgba(col(lerp3(fur, FUR_DARK, 0.35)), a);
        cr.fill();
    }
    if !lk.kid {
        let sway = (lk.t * 2.0).sin() * 0.02;
        cr.move_to(-0.055 * u, 0.26 * u);
        cr.line_to(0.055 * u, 0.26 * u);
        cr.line_to(sway * u, 0.50 * u);
        cr.close_path();
        cr.set_rgba(col(FUR_DARK), a);
        cr.fill_preserve();
        cr.set_line_width(extra * 2.0 + 0.02 * u);
        cr.stroke();
    }
    cr.ellipse(0.0, -0.02 * u, 0.165 * u + extra, 0.175 * u + extra);
    cr.ellipse(0.0, 0.16 * u, 0.105 * u + extra, 0.135 * u + extra);
    cr.set_rgba(col(lerp3(fur, FUR_DARK, 0.18)), a);
    cr.fill();
    if rim.is_some() {
        return;
    }
    cr.ellipse(0.0, 0.21 * u, 0.078 * u, 0.07 * u);
    cr.set_rgba(FUR_LIGHT, 0.9 * a);
    cr.fill();
    cr.ellipse(0.0, 0.185 * u, 0.042 * u, 0.026 * u);
    cr.set_rgba(LEG_DARK, a);
    cr.fill();
    if p.mouth > 0.05 {
        cr.ellipse(0.0, 0.255 * u, 0.04 * u, 0.042 * u * p.mouth);
        cr.set_source_rgba(0.30, 0.08, 0.08, a);
        cr.fill();
    }
    let shut = p.shut > 0.5 || lk.blink;
    eye(cr, -0.098 * u, -0.035 * u, u, shut, a);
    eye(cr, 0.098 * u, -0.035 * u, u, shut, a);
}

// ---------------------------------------------------------------------- the painted world

/// A mountain chain: a jagged skyline through the summits, and where the summits are in it.
struct Chain {
    pts: Vec<P>,
    peaks: Vec<usize>,
}

fn chain(rng: &Rng, x0: f64, x1: f64, summits: &[P], foot: f64, rough: f64) -> Chain {
    let mut keys: Vec<P> = vec![(x0, lerp(summits[0].1, foot, 0.55))];
    for (i, &p) in summits.iter().enumerate() {
        keys.push(p);
        keys.push(match summits.get(i + 1) {
            Some(n) => ((p.0 + n.0) / 2.0 + rng.uniform(-0.15, 0.15) * (n.0 - p.0), lerp(p.1.max(n.1), foot, rng.uniform(0.25, 0.5))),
            None => (x1, lerp(p.1, foot, 0.55)),
        });
    }
    let mut pts = Vec::new();
    let mut peaks = Vec::new();
    for (i, win) in keys.windows(2).enumerate() {
        if i % 2 == 1 {
            peaks.push(pts.len());
        }
        pts.push(win[0]);
        for j in 1..4 {
            let f = j as f64 / 4.0;
            pts.push((lerp(win[0].0, win[1].0, f) + rng.uniform(-1.0, 1.0) * rough, lerp(win[0].1, win[1].1, f) + rng.uniform(-1.0, 1.0) * rough));
        }
    }
    pts.push(keys[keys.len() - 1]);
    Chain { pts, peaks }
}

/// Paint a chain: rock, snow caps with a jagged lower edge, and the shaded side of each summit.
#[allow(clippy::too_many_arguments)]
fn paint_chain(cr: &mut Canvas, rng: &Rng, ch: &Chain, bottom: f64, top: Rgb, low: Rgb, snow: f64, s: f64) {
    let (x0, x1) = (ch.pts[0].0, ch.pts[ch.pts.len() - 1].0);
    let high = ch.pts.iter().map(|p| p.1).fold(f64::MAX, f64::min);
    let mut g = Gradient::linear(0.0, high, 0.0, bottom);
    g.stop(0.0, top, 1.0);
    g.stop(1.0, low, 1.0);
    cr.move_to(x0, bottom);
    for p in &ch.pts {
        cr.line_to(p.0, p.1);
    }
    cr.line_to(x1, bottom);
    cr.close_path();
    cr.set_source(&g);
    cr.fill();
    for &pk in &ch.peaks {
        let (px, py) = ch.pts[pk];
        if snow > 0.0 {
            let line = py + snow * rng.uniform(0.8, 1.2);
            let (mut i, mut j) = (pk, pk);
            while i > 0 && ch.pts[i - 1].1 < line && pk - i < 3 {
                i -= 1;
            }
            while j + 1 < ch.pts.len() && ch.pts[j + 1].1 < line && j - pk < 3 {
                j += 1;
            }
            if i < pk || j > pk {
                cr.move_to(ch.pts[i].0, ch.pts[i].1);
                for p in &ch.pts[i + 1..=j] {
                    cr.line_to(p.0, p.1);
                }
                let (a, b) = (ch.pts[j], ch.pts[i]);
                for q in 1..8 {
                    let f = q as f64 / 8.0;
                    let drop = if q % 2 == 1 { rng.uniform(0.35, 0.8) } else { rng.uniform(-0.05, 0.2) };
                    cr.line_to(lerp(a.0, b.0, f), lerp(a.1, b.1, f) + drop * snow * (0.5 + (PI * f).sin()));
                }
                cr.close_path();
                cr.set_source_rgb(0.97, 0.98, 1.0);
                cr.fill();
            }
        }
        // the side away from the sun
        let v = (pk + 4).min(ch.pts.len() - 1);
        cr.move_to(px, py);
        for p in &ch.pts[pk + 1..=v] {
            cr.line_to(p.0, p.1);
        }
        // back up to the summit along a broken ridge that hangs below the skyline
        let (vx, vy) = ch.pts[v];
        let sag = (bottom - vy).min((vx - px) * 0.9);
        for q in (1..7).rev() {
            let f = q as f64 / 7.0;
            let jag = if q % 2 == 0 { 0.10 } else { -0.04 } * (vx - px) + rng.uniform(-5.0, 5.0) * s;
            cr.line_to(lerp(px, vx, f * f) + jag * f, lerp(py, vy, f) + sag * (PI * f).sin().powf(0.7));
        }
        cr.close_path();
        cr.set_source_rgba(0.20, 0.28, 0.48, 0.22);
        cr.fill();
    }
}

/// Turn a hand-made outline (x, y, "the stretch to the next point is a ledge top") into broken rock.
fn roughen(rng: &Rng, pts: &[(f64, f64, bool)], amp: f64, step: f64) -> Vec<P> {
    let mut out = Vec::new();
    for (i, &(x, y, flat)) in pts.iter().enumerate() {
        out.push((x, y));
        let (nx, ny, _) = pts[(i + 1) % pts.len()];
        if flat {
            continue;
        }
        let n = (hypot(nx - x, ny - y) / step).ceil() as usize;
        for j in 1..n {
            let f = j as f64 / n as f64;
            out.push((lerp(x, nx, f) + rng.uniform(-1.0, 1.0) * amp, lerp(y, ny, f) + rng.uniform(-1.0, 1.0) * amp));
        }
    }
    out
}

fn outline_path(cr: &mut Canvas, pts: &[P]) {
    cr.move_to(pts[0].0, pts[0].1);
    for p in &pts[1..] {
        cr.line_to(p.0, p.1);
    }
    cr.close_path();
}

/// A tuft of grass: a fan of blades from one point (path and stroke).
fn tuft(cr: &mut Canvas, x: f64, y: f64, size: f64, sway: f64, col: Rgb) {
    for i in 0..5 {
        let fi = i as f64 - 2.0;
        let ang = -PI / 2.0 + fi * 0.30 + sway;
        let len = size * (1.0 - 0.12 * fi.abs());
        cr.move_to(x + fi * size * 0.07, y);
        cr.quad_to(x + fi * size * 0.10, y - len * 0.55, x + ang.cos() * len, y + ang.sin() * len);
    }
    cr.set_rgb(col);
    cr.set_line_width((size * 0.10).max(1.0));
    cr.stroke();
}

/// Edelweiss: a woolly white star.
fn edelweiss(cr: &mut Canvas, x: f64, y: f64, z: f64) {
    for i in 0..7 {
        let ang = i as f64 * TAU / 7.0 + 0.2;
        leaf_path(cr, x, y, z * 2.3, z * 0.55, ang);
    }
    cr.set_source_rgb(0.97, 0.97, 0.92);
    cr.fill();
    cr.circle(x, y, z * 0.75);
    cr.set_source_rgb(0.90, 0.80, 0.36);
    cr.fill();
}

/// Gentian: a deep blue cup on a short stalk.
fn gentian(cr: &mut Canvas, x: f64, y: f64, z: f64) {
    cr.move_to(x - z * 0.5, y);
    cr.line_to(x - z * 1.2, y - z * 2.4);
    cr.line_to(x - z * 0.4, y - z * 1.9);
    cr.line_to(x, y - z * 2.6);
    cr.line_to(x + z * 0.4, y - z * 1.9);
    cr.line_to(x + z * 1.2, y - z * 2.4);
    cr.line_to(x + z * 0.5, y);
    cr.close_path();
    cr.set_source_rgb(0.16, 0.24, 0.80);
    cr.fill();
    cr.circle(x, y - z * 1.7, z * 0.35);
    cr.set_source_rgb(0.55, 0.66, 1.0);
    cr.fill();
}

/// A boulder with a lit top and a shaded foot.
fn boulder(cr: &mut Canvas, x: f64, y: f64, w: f64, h: f64, lit: Rgb, shade: Rgb) {
    let shape = |cr: &mut Canvas, k: f64| {
        cr.move_to(x - w * 0.5, y);
        cr.line_to(x - w * 0.44, y - h * 0.55 * k);
        cr.line_to(x - w * 0.18, y - h * k);
        cr.line_to(x + w * 0.20, y - h * 0.92 * k);
        cr.line_to(x + w * 0.46, y - h * 0.45 * k);
        cr.line_to(x + w * 0.5, y);
        cr.close_path();
    };
    shape(cr, 1.0);
    cr.set_rgb(shade);
    cr.fill();
    cr.move_to(x - w * 0.44, y - h * 0.55);
    cr.line_to(x - w * 0.18, y - h);
    cr.line_to(x + w * 0.20, y - h * 0.92);
    cr.line_to(x + w * 0.06, y - h * 0.50);
    cr.line_to(x - w * 0.24, y - h * 0.36);
    cr.close_path();
    cr.set_rgb(lit);
    cr.fill();
}

/// A twisted mountain pine: a bent trunk and a few flat cushions of needles.
fn pine(cr: &mut Canvas, x: f64, y: f64, z: f64, lean: f64) {
    cr.set_line_cap(LineCap::Round);
    cr.set_source_rgb(0.30, 0.21, 0.15);
    cr.set_line_width(z * 0.11);
    cr.move_to(x, y);
    cr.curve_to(x + lean * z * 0.10, y - z * 0.30, x + lean * z * 0.55, y - z * 0.32, x + lean * z * 0.30, y - z * 0.72);
    cr.stroke();
    cr.set_line_width(z * 0.06);
    cr.move_to(x + lean * z * 0.36, y - z * 0.40);
    cr.quad_to(x + lean * z * 0.70, y - z * 0.42, x + lean * z * 0.86, y - z * 0.56);
    cr.move_to(x + lean * z * 0.20, y - z * 0.30);
    cr.quad_to(x - lean * z * 0.10, y - z * 0.42, x - lean * z * 0.26, y - z * 0.40);
    cr.move_to(x + lean * z * 0.30, y - z * 0.72);
    cr.quad_to(x + lean * z * 0.20, y - z * 0.90, x + lean * z * 0.02, y - z * 0.98);
    cr.stroke();
    let pads = [(0.86, -0.60, 0.34), (-0.28, -0.44, 0.26), (0.02, -1.00, 0.36), (0.42, -0.80, 0.30), (0.50, -0.50, 0.22)];
    for (k, col) in [(0.07, 0.24, 0.17), (0.14, 0.36, 0.22)].into_iter().enumerate() {
        for &(px, py, pr) in &pads {
            let up = k as f64 * z * 0.05;
            cr.ellipse(x + lean * px * z, y + py * z - up, pr * z * (1.0 - 0.25 * k as f64), pr * z * (0.42 - 0.12 * k as f64));
        }
        cr.set_rgb(col);
        cr.fill();
    }
}

impl Capricorns {
    /// A ledge to start a climb on: one that can be reached from the ground, the nearest first.
    fn pick_start(&self, env: &Env, x: f64) -> Option<(usize, usize)> {
        let mut free: Vec<usize> = (0..self.ledges.len()).filter(|&i| self.ledges[i].ground && self.ledges[i].used == [false, false]).collect();
        if free.is_empty() {
            return None;
        }
        free.sort_by(|&a, &b| (self.ledges[a].foot_x - x).abs().total_cmp(&(self.ledges[b].foot_x - x).abs()));
        let i = if env.rng.chance(0.6) { free[0] } else { *env.rng.choice(&free) };
        self.ledges[i].free_spot().map(|sp| (i, sp))
    }

    /// The next ledge of a climb: a neighbour with room, rather a new and a higher one.
    fn pick_next(&self, env: &Env, from: usize, prev: Option<usize>) -> Option<(usize, usize)> {
        let here = &self.ledges[from];
        let free: Vec<usize> = here.links.iter().copied().filter(|&i| self.ledges[i].used == [false, false]).collect();
        let fresh: Vec<usize> = free.iter().copied().filter(|&i| Some(i) != prev).collect();
        let pool = if fresh.is_empty() { free } else { fresh };
        if pool.is_empty() {
            return None;
        }
        let higher: Vec<usize> = pool.iter().copied().filter(|&i| self.ledges[i].y < here.y).collect();
        let i = if !higher.is_empty() && env.rng.chance(0.75) { *env.rng.choice(&higher) } else { *env.rng.choice(&pool) };
        self.ledges[i].free_spot().map(|sp| (i, sp))
    }

    /// The way down: a lower neighbour with a free place.
    fn pick_down(&self, from: usize) -> Option<(usize, usize)> {
        let here = &self.ledges[from];
        here.links.iter().copied().filter(|&i| self.ledges[i].y > here.y + 1.0).find_map(|i| self.ledges[i].free_spot().map(|sp| (i, sp)))
    }

    /// Pebbles knocked loose: they tumble down to the meadow.
    fn pebbles(&mut self, env: &Env, x: f64, y: f64, n: usize, dir: f64) {
        let (s, rng) = (env.s, &env.rng);
        let floor = y.max(env.ground_top + rng.uniform(34.0, 70.0) * s);
        for _ in 0..n {
            let g = rng.uniform(0.50, 0.72);
            self.bits.push(Bit {
                kind: BitKind::Pebble,
                x: x + rng.uniform(-10.0, 10.0) * s,
                y: y - 2.0 * s,
                vx: (dir * rng.uniform(20.0, 90.0) + rng.uniform(-30.0, 30.0)) * s,
                vy: -rng.uniform(40.0, 170.0) * s,
                size: rng.uniform(2.2, 4.2) * s,
                age: 0.0,
                life: rng.uniform(1.4, 2.4),
                floor: floor + rng.uniform(-8.0, 8.0) * s,
                c: (g, g * 0.98, g * 0.95),
            });
        }
    }

    /// A little dust where hooves hit the rock.
    fn dust(&mut self, env: &Env, x: f64, y: f64, n: usize) {
        let (s, rng) = (env.s, &env.rng);
        for _ in 0..n {
            self.bits.push(Bit {
                kind: BitKind::Dust,
                x: x + rng.uniform(-18.0, 18.0) * s,
                y: y - rng.uniform(0.0, 6.0) * s,
                vx: rng.uniform(-46.0, 46.0) * s,
                vy: -rng.uniform(6.0, 30.0) * s,
                size: rng.uniform(6.0, 12.0) * s,
                age: 0.0,
                life: rng.uniform(0.5, 0.95),
                floor: y,
                c: (0.93, 0.90, 0.84),
            });
        }
    }

    /// Sparks from a stamping hoof.
    fn sparks(&mut self, env: &Env, x: f64, y: f64, c: Rgb) {
        let (s, rng) = (env.s, &env.rng);
        for _ in 0..7 {
            let ang = rng.uniform(-PI * 0.95, -PI * 0.05);
            let sp = rng.uniform(120.0, 300.0) * s;
            self.bits.push(Bit {
                kind: BitKind::Spark,
                x,
                y: y - 3.0 * s,
                vx: ang.cos() * sp,
                vy: ang.sin() * sp,
                size: rng.uniform(1.6, 2.6) * s,
                age: 0.0,
                life: rng.uniform(0.35, 0.65),
                floor: y,
                c: lerp3((1.0, 0.93, 0.62), c, rng.uniform(0.0, 0.6)),
            });
        }
        self.dust(env, x, y, 3);
    }

    /// Far mountains, the rock faces with their ledges, and the meadow.
    fn paint_land(&self, cr: &mut Canvas, env: &Env, crags: &[Crag]) {
        let (w, h, s) = (env.w, env.h, env.s);
        let gt = env.ground_top;
        let rng = &env.rng;
        // three chains, each nearer one darker and lower
        let wide = w + 200.0 * s;
        let spread = |n: usize, lo: f64, hi: f64, off: f64| -> Vec<P> {
            (0..n).map(|i| (-100.0 * s + wide * ((i as f64 + 0.5 + off) / n as f64 + rng.uniform(-0.04, 0.04)), gt - h * rng.uniform(lo, hi))).collect()
        };
        let far = chain(rng, -100.0 * s, w + 100.0 * s, &spread(5, 0.36, 0.50, 0.12), gt - h * 0.04, 5.0 * s);
        paint_chain(cr, rng, &far, gt, (0.66, 0.75, 0.90), (0.84, 0.89, 0.95), h * 0.085, s);
        let mid = chain(rng, -100.0 * s, w + 100.0 * s, &spread(4, 0.26, 0.37, -0.15), gt - h * 0.02, 7.0 * s);
        paint_chain(cr, rng, &mid, gt, (0.50, 0.61, 0.78), (0.74, 0.82, 0.90), h * 0.05, s);
        let hills: Vec<P> = (0..7).map(|i| (-100.0 * s + wide * (i as f64 + 0.5) / 7.0, gt - h * rng.uniform(0.10, 0.17))).collect();
        let near = chain(rng, -100.0 * s, w + 100.0 * s, &hills, gt + h * 0.02, 4.0 * s);
        let mut g = Gradient::linear(0.0, gt - h * 0.17, 0.0, gt);
        g.add_color_stop_rgb(0.0, 0.38, 0.52, 0.52);
        g.add_color_stop_rgb(1.0, 0.62, 0.74, 0.66);
        cr.move_to(-100.0 * s, gt + 10.0 * s);
        for p in &near.pts {
            cr.line_to(p.0, p.1);
        }
        cr.line_to(w + 100.0 * s, gt + 10.0 * s);
        cr.close_path();
        cr.set_source(&g);
        cr.fill();
        // morning haze in the valley
        let mut g = Gradient::linear(0.0, gt - h * 0.30, 0.0, gt);
        g.add_color_stop_rgba(0.0, 1.0, 1.0, 1.0, 0.0);
        g.add_color_stop_rgba(1.0, 1.0, 0.99, 0.95, 0.42);
        cr.rectangle(0.0, gt - h * 0.30, w, h * 0.31);
        cr.set_source(&g);
        cr.fill();
        for crag in crags {
            self.paint_crag(cr, env, crag);
        }
        // the waterfall in a dark cleft of the left face
        let (fx, fy0, fy1) = self.fall;
        cr.move_to(fx - 9.0 * s, fy0 - 8.0 * s);
        cr.line_to(fx + 7.0 * s, fy0 - 14.0 * s);
        cr.line_to(fx + 17.0 * s, fy1);
        cr.line_to(fx - 17.0 * s, fy1);
        cr.close_path();
        cr.set_source_rgba(0.24, 0.26, 0.32, 0.5);
        cr.fill();
        let mut g = Gradient::linear(0.0, fy0, 0.0, fy1);
        g.add_color_stop_rgba(0.0, 0.86, 0.94, 1.0, 0.95);
        g.add_color_stop_rgba(1.0, 0.97, 0.99, 1.0, 0.95);
        cr.move_to(fx - 4.0 * s, fy0);
        cr.line_to(fx + 4.0 * s, fy0);
        cr.line_to(fx + 9.0 * s, fy1);
        cr.line_to(fx - 9.0 * s, fy1);
        cr.close_path();
        cr.set_source(&g);
        cr.fill();
        // the meadow: a soft uneven edge, lighter where the sun lies on it
        let floor = h - env.strip_h;
        let mut g = Gradient::linear(0.0, gt, 0.0, h);
        g.add_color_stop_rgb(0.0, 0.62, 0.74, 0.40);
        g.add_color_stop_rgb(0.35, 0.47, 0.64, 0.30);
        g.add_color_stop_rgb(1.0, 0.27, 0.44, 0.20);
        cr.move_to(0.0, h);
        let n = 24;
        for i in 0..=n {
            let x = w * i as f64 / n as f64;
            cr.line_to(x, gt + ((i as f64 * 1.7).sin() * 5.0 + rng.uniform(-3.0, 3.0)) * s);
        }
        cr.line_to(w, h);
        cr.close_path();
        cr.set_source(&g);
        cr.fill();
        for _ in 0..7 {
            let y = rng.uniform(gt + 30.0 * s, floor);
            let k = ground_k(env, y);
            cr.ellipse(rng.uniform(0.0, w), y, rng.uniform(150.0, 320.0) * s * k, rng.uniform(14.0, 30.0) * s * k);
            cr.set_source_rgba(0.80, 0.86, 0.46, 0.16);
            cr.fill();
        }
        // scree at the foot of the rock
        for crag in crags {
            let (x0, x1) = crag.outline.iter().fold((f64::MAX, f64::MIN), |a, p| (a.0.min(p.0), a.1.max(p.0)));
            let n = ((x1 - x0) / (9.0 * s)) as usize;
            for k in 0..n {
                let y = gt + rng.uniform(0.0, 1.0).powi(2) * 64.0 * s - 4.0 * s;
                let x = rng.uniform(x0.max(-10.0 * s) - 20.0 * s, x1.min(w + 10.0 * s) + 20.0 * s);
                let z = rng.uniform(2.5, 7.5) * s * if k % 9 == 0 { 2.0 } else { 1.0 };
                cr.ellipse(x, y, z * 1.3, z * 0.8);
                cr.set_rgb(if k % 3 == 0 { crag.shade } else { lerp3(crag.lit, crag.shade, 0.3) });
                cr.fill();
                cr.ellipse(x - z * 0.2, y - z * 0.25, z * 0.8, z * 0.4);
                cr.set_rgba(lighten(crag.lit, 0.3), 0.8);
                cr.fill();
            }
        }
        // the stream runs from the fall across the corner of the meadow
        let [p0, p1, p2] = self.stream;
        cr.set_line_cap(LineCap::Round);
        for (wd, col, al) in [(26.0, (0.36, 0.44, 0.34), 0.8), (18.0, (0.62, 0.82, 0.94), 1.0), (5.0, (0.92, 0.97, 1.0), 0.8)] {
            cr.move_to(p0.0, p0.1);
            cr.quad_to(p1.0, p1.1, p2.0, p2.1);
            cr.set_rgba(col, al);
            cr.set_line_width(wd * s);
            cr.stroke();
        }
        cr.ellipse(fx, fy1 + 3.0 * s, 44.0 * s, 10.0 * s);
        cr.set_source_rgb(0.62, 0.82, 0.94);
        cr.fill();
        cr.ellipse(fx, fy1 + 1.0 * s, 24.0 * s, 5.0 * s);
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.8);
        cr.fill();
        // boulders, grass, flowers; smaller towards the back
        for _ in 0..(6.0 * w / (1600.0 * s)) as usize {
            let y = rng.uniform(gt + 20.0 * s, floor - 10.0 * s);
            let k = ground_k(env, y);
            let bw = rng.uniform(34.0, 80.0) * s * k;
            boulder(cr, rng.uniform(0.0, w), y, bw, bw * rng.uniform(0.45, 0.7), (0.80, 0.79, 0.76), (0.50, 0.50, 0.54));
        }
        for (i, col) in GRASS.iter().enumerate() {
            for _ in 0..(120.0 * w / (1600.0 * s)) as usize {
                let y = gt + rng.uniform(0.02, 1.0).powf(0.8) * (h - gt);
                let k = ground_k(env, y);
                let x = rng.uniform(0.0, w);
                let z = rng.uniform(7.0, 15.0) * s * k;
                for d in [-0.4, 0.0, 0.45] {
                    cr.move_to(x + d * z * 0.5, y);
                    cr.line_to(x + d * z, y - z * (1.0 - d * d));
                }
            }
            cr.set_rgba(*col, if i == 1 { 0.55 } else { 0.7 });
            cr.set_line_width(1.6 * s);
            cr.stroke();
        }
        for i in 0..(70.0 * w / (1600.0 * s)) as usize {
            let y = rng.uniform(gt + 14.0 * s, floor + 6.0 * s);
            let k = ground_k(env, y);
            let (x, z) = (rng.uniform(0.0, w), 2.8 * s * k);
            match i % 5 {
                0 | 1 => edelweiss(cr, x, y, z * 1.1),
                2 => gentian(cr, x, y, z * 1.6),
                3 => {
                    // alpine roses: a small pink cushion
                    for _ in 0..4 {
                        cr.circle(x + rng.uniform(-3.0, 3.0) * z, y + rng.uniform(-1.2, 1.2) * z, z * 1.1);
                    }
                    cr.set_source_rgb(0.90, 0.36, 0.52);
                    cr.fill();
                }
                _ => {
                    cr.circle(x, y, z * 1.1);
                    cr.set_source_rgb(0.98, 0.86, 0.30);
                    cr.fill();
                }
            }
        }
    }

    /// One rock mass with facets, cracks, the shelves and what grows on them.
    fn paint_crag(&self, cr: &mut Canvas, env: &Env, crag: &Crag) {
        let (s, gt) = (env.s, env.ground_top);
        let rng = &env.rng;
        let pts = &crag.outline;
        let (mut x0, mut x1, mut top) = (f64::MAX, f64::MIN, f64::MAX);
        for p in pts {
            x0 = x0.min(p.0);
            x1 = x1.max(p.0);
            top = top.min(p.1);
        }
        let (lit, shade) = (crag.lit, crag.shade);
        let mut g = Gradient::linear(0.0, top, 0.0, gt);
        g.stop(0.0, lit, 1.0);
        g.stop(1.0, lerp3(lit, shade, 0.75), 1.0);
        outline_path(cr, pts);
        cr.set_source(&g);
        cr.fill();
        cr.save();
        outline_path(cr, pts);
        cr.clip();
        // tall facets, some in the light and some in shadow
        let area = (x1 - x0) * (gt - top);
        for i in 0..(area / (11000.0 * s * s)) as usize {
            let (x, y) = (rng.uniform(x0, x1), rng.uniform(top, gt));
            let (fw, fh) = (rng.uniform(26.0, 80.0) * s, rng.uniform(70.0, 190.0) * s);
            let (j0, j1) = (rng.uniform(-22.0, 22.0) * s, rng.uniform(-18.0, 18.0) * s);
            cr.move_to(x - fw / 2.0 + j0, y - fh / 2.0);
            cr.line_to(x + fw / 2.0 + j0, y - fh / 2.0 + j1);
            cr.line_to(x + fw / 2.0 - j0 * 0.5, y + fh / 2.0);
            cr.line_to(x - fw / 2.0 - j0 * 0.5, y + fh / 2.0 - j1);
            cr.close_path();
            match i % 3 {
                0 => cr.set_rgba(lighten(lit, 0.35), 0.34),
                1 => cr.set_rgba(shade, 0.36),
                _ => cr.set_rgba(lerp3(shade, (0.52, 0.42, 0.32), 0.5), 0.24), // a brown band
            }
            cr.fill();
        }
        // cracks, mostly running down
        cr.set_line_join(LineJoin::Round);
        cr.set_line_cap(LineCap::Round);
        for _ in 0..(area / (15000.0 * s * s)) as usize {
            let (mut x, mut y) = (rng.uniform(x0, x1), rng.uniform(top, gt - 40.0 * s));
            cr.move_to(x, y);
            for _ in 0..rng.randint(3, 6) {
                x += rng.uniform(-13.0, 13.0) * s;
                y += rng.uniform(16.0, 44.0) * s;
                cr.line_to(x, y);
            }
            cr.set_rgba(darken(shade, 0.45), rng.uniform(0.22, 0.42));
            cr.set_line_width(rng.uniform(1.0, 2.2) * s);
            cr.stroke();
        }
        // shadow under every shelf
        for l in &self.ledges[crag.first..crag.first + crag.count] {
            let y = l.y + 3.0 * s;
            let mut g = Gradient::linear(0.0, y, 0.0, y + 62.0 * s);
            g.add_color_stop_rgba(0.0, 0.16, 0.17, 0.24, 0.50);
            g.add_color_stop_rgba(1.0, 0.16, 0.17, 0.24, 0.0);
            cr.move_to(l.x - l.half - 4.0 * s, y);
            cr.line_to(l.x + l.half + 4.0 * s, y);
            cr.line_to(l.x + l.half - 16.0 * s, y + 62.0 * s);
            cr.line_to(l.x - l.half + 16.0 * s, y + 62.0 * s);
            cr.close_path();
            cr.set_source(&g);
            cr.fill();
        }
        // haze at the foot
        let mut g = Gradient::linear(0.0, gt - 150.0 * s, 0.0, gt);
        g.add_color_stop_rgba(0.0, 0.95, 0.96, 0.94, 0.0);
        g.add_color_stop_rgba(1.0, 0.95, 0.96, 0.94, 0.26);
        cr.rectangle(x0, gt - 150.0 * s, x1 - x0, 160.0 * s);
        cr.set_source(&g);
        cr.fill();
        cr.restore();
        // snow on the summit
        let peak = pts.iter().copied().fold((0.0, f64::MAX), |a, p| if p.1 < a.1 { p } else { a });
        for (dx, dy, rx, ry) in [(0.0, 9.0, 46.0, 9.0), (-58.0, 22.0, 34.0, 7.0), (40.0, 24.0, 26.0, 6.0)] {
            cr.ellipse(peak.0 + dx * s, peak.1 + dy * s, rx * s, ry * s);
        }
        cr.set_source_rgb(0.97, 0.98, 1.0);
        cr.fill();
        for (i, l) in self.ledges[crag.first..crag.first + crag.count].iter().enumerate() {
            let (xa, xb, y) = (l.x - l.half, l.x + l.half, l.y + 3.0 * s);
            // the lip of rock under the edge
            cr.move_to(xa - 7.0 * s, y);
            cr.line_to(xb + 7.0 * s, y);
            for q in 0..=7 {
                let f = q as f64 / 7.0;
                cr.line_to(lerp(xb + 3.0 * s, xa - 3.0 * s, f), y + (9.0 + 13.0 * (PI * f).sin() + rng.uniform(-4.0, 4.0)) * s);
            }
            cr.close_path();
            cr.set_rgb(darken(shade, 0.30));
            cr.fill();
            // the flat top, bright in the sun
            cr.move_to(xa - 7.0 * s, y);
            cr.line_to(xb + 7.0 * s, y);
            cr.line_to(xb - 8.0 * s, y - 10.0 * s);
            cr.line_to(xa + 8.0 * s, y - 10.0 * s);
            cr.close_path();
            cr.set_rgb(lighten(lit, 0.55));
            cr.fill();
            cr.move_to(xa - 7.0 * s, y);
            cr.line_to(xb + 7.0 * s, y);
            cr.set_source_rgba(1.0, 1.0, 0.97, 0.9);
            cr.set_line_width(1.6 * s);
            cr.stroke();
            // a patch of snow at the back, grass at the rock end, a flower or a pine
            let inner = l.x - l.out * l.half * 0.72;
            if i % 2 == 1 {
                cr.ellipse(inner, y - 9.0 * s, 26.0 * s, 5.0 * s);
                cr.set_source_rgb(0.97, 0.98, 1.0);
                cr.fill();
            } else {
                tuft(cr, inner, y - 5.0 * s, 13.0 * s, 0.1, GRASS[0]);
                tuft(cr, inner + 12.0 * s, y - 6.0 * s, 10.0 * s, -0.15, GRASS[1]);
            }
            tuft(cr, l.x - l.out * l.half * 0.1, y - 8.0 * s, 8.0 * s, 0.0, GRASS[2]);
            match i % 3 {
                0 => edelweiss(cr, inner - l.out * 18.0 * s, y - 7.0 * s, 2.6 * s),
                1 => gentian(cr, l.x + l.out * l.half * 0.92, y - 2.0 * s, 3.2 * s),
                _ => {}
            }
        }
    }
}

impl Theme for Capricorns {
    type Actor = Ibex;
    const NAME: &'static str = "capricorns";
    const LABEL_BG: Rgb = (0.04, 0.06, 0.10);
    const EMPTY_COLOR: Rgb = (0.02, 0.05, 0.16);
    const WAITING_ROW: f64 = 0.80;

    fn new(_env: &Env) -> Self {
        Capricorns {
            sky: None,
            land: None,
            cloud_pics: Vec::new(),
            shadow_pic: None,
            rock_pic: None,
            epoch: 0,
            ledges: Vec::new(),
            bits: Vec::new(),
            clouds: Vec::new(),
            tufts: Vec::new(),
            eagles: Vec::new(),
            marmot: (0.0, 0.0),
            fall: (0.0, 0.0, 0.0),
            stream: [(0.0, 0.0); 3],
        }
    }

    fn spawn_point(&self, env: &Env) -> (f64, f64) {
        (env.rng.uniform(env.w * 0.15, env.w * 0.85), env.rng.uniform(env.ground_top + 50.0 * env.s, env.h * 0.88))
    }

    fn on_tool(&mut self, env: &mut Env, actor: &mut Ibex, name: &str, index: usize) {
        if matches!(actor.mode, Mode::Ground | Mode::OnLedge { .. }) && actor.lie < 0.3 {
            actor.rear = REAR;
            actor.stamped = false;
        }
        let b = &actor.b;
        // The names rise in front of a bright sky and pale rock, so they must be dark. The stage
        // lightens the colour of a floater by 30 %: give it the colour that ends up at the dark one.
        let mean = (b.color.0 + b.color.1 + b.color.2) / 3.0;
        let strong = |v: f64| clamp(v + (v - mean) * 0.6, 0.0, 1.0) * 0.6;
        let dark = (strong(b.color.0), strong(b.color.1), strong(b.color.2));
        let pre = |v: f64| (v - 0.3) / 0.7;
        let y = b.y - b.radius * 2.1 - index as f64 * 26.0 * env.s;
        env.spawn_floater(b.x, y, name, (pre(dark.0), pre(dark.1), pre(dark.2)), FloatStyle::Text, 40.0, 3.5);
    }

    fn setup_world(&mut self, env: &mut Env) {
        let (w, h, s) = (env.w, env.h, env.s);
        let gt = h * 0.70;
        env.ground_top = gt;
        self.epoch += 1; // the ibex notice and come down from the old ledges
        let env: &Env = env;
        let rng = &env.rng;
        // The rock is laid out in units of the scale, measured from the ground up and from the
        // screen edges in, so the leaps are the same on every screen: (x, height, ledge top?)
        type Raw = [(f64, f64, bool)];
        let left: &Raw = &[
            (-30.0, 648.0, false),
            (60.0, 720.0, false),
            (150.0, 738.0, false),
            (235.0, 696.0, false),
            (290.0, 600.0, false),
            (300.0, 480.0, false),
            (330.0, 396.0, false),
            (350.0, 327.0, true),
            (500.0, 327.0, false),
            (490.0, 276.0, false),
            (455.0, 216.0, false),
            (450.0, 144.0, false),
            (468.0, 75.0, true),
            (570.0, 75.0, false),
            (560.0, 34.0, false),
            (595.0, 0.0, false),
            (650.0, -42.0, false),
            (650.0, -110.0, false),
            (-30.0, -110.0, false),
        ];
        let right: &Raw = &[
            (-30.0, 560.0, false),
            (70.0, 624.0, false),
            (180.0, 650.0, false),
            (260.0, 606.0, false),
            (300.0, 516.0, false),
            (320.0, 408.0, false),
            (345.0, 321.0, true),
            (492.0, 321.0, false),
            (480.0, 270.0, false),
            (450.0, 204.0, false),
            (455.0, 132.0, false),
            (478.0, 69.0, true),
            (587.0, 69.0, false),
            (575.0, 30.0, false),
            (605.0, 0.0, false),
            (660.0, -42.0, false),
            (660.0, -110.0, false),
            (-30.0, -110.0, false),
        ];
        let middle: &Raw = &[
            (-260.0, -42.0, false),
            (-215.0, 12.0, false),
            (-192.0, 54.0, false),
            (-187.0, 85.0, true),
            (-45.0, 85.0, false),
            (-50.0, 150.0, false),
            (-40.0, 240.0, false),
            (-10.0, 300.0, false),
            (30.0, 322.0, false),
            (55.0, 290.0, false),
            (72.0, 211.0, true),
            (172.0, 211.0, false),
            (160.0, 162.0, false),
            (135.0, 108.0, false),
            (150.0, 48.0, false),
            (195.0, 0.0, false),
            (245.0, -42.0, false),
            (245.0, -110.0, false),
            (-260.0, -110.0, false),
        ];
        let cx = w * 0.53;
        let place = |raw: &Raw, at: &dyn Fn(f64) -> f64| -> Vec<(f64, f64, bool)> { raw.iter().map(|&(x, up, flat)| (at(x), gt - up * s, flat)).collect() };
        let from_left = |x: f64| x * s;
        let from_right = |x: f64| w - x * s;
        let from_mid = |x: f64| cx + x * s;
        // (x, height, half width, valley side, reachable from the ground)
        let mk = |x: f64, up: f64, half: f64, out: f64, ground: bool| Ledge {
            x,
            y: gt - up * s,
            half: half * s,
            out,
            ground,
            foot_x: x + out * (half + 82.0) * s,
            links: Vec::new(),
            used: [false; 2],
        };
        self.ledges = vec![
            mk(from_left(455.0), 72.0, 110.0, 1.0, true),
            mk(from_left(205.0), 198.0, 100.0, 1.0, false),
            mk(from_left(395.0), 324.0, 100.0, 1.0, false),
            mk(from_left(170.0), 450.0, 95.0, 1.0, false),
            mk(from_mid(-85.0), 82.0, 98.0, -1.0, true),
            mk(from_mid(75.0), 208.0, 92.0, 1.0, false),
            mk(from_right(470.0), 66.0, 112.0, -1.0, true),
            mk(from_right(215.0), 192.0, 100.0, -1.0, false),
            mk(from_right(385.0), 318.0, 102.0, -1.0, false),
        ];
        // neighbouring ledges of one face, and across the gaps where the screen is narrow enough
        let mut links = vec![(0, 1), (1, 2), (2, 3), (4, 5), (6, 7), (7, 8)];
        for (a, b) in [(0, 4), (5, 8), (5, 6)] {
            let (la, lb) = (&self.ledges[a], &self.ledges[b]);
            if hypot(lb.x - la.x, (lb.y - la.y) * 1.3) < 335.0 * s {
                links.push((a, b));
            }
        }
        for (a, b) in links {
            self.ledges[a].links.push(b);
            self.ledges[b].links.push(a);
        }
        let crags = [
            Crag { outline: roughen(rng, &place(middle, &from_mid), 5.0 * s, 34.0 * s), lit: (0.80, 0.80, 0.80), shade: (0.60, 0.62, 0.68), first: 4, count: 2 },
            Crag { outline: roughen(rng, &place(left, &from_left), 6.0 * s, 36.0 * s), lit: (0.78, 0.76, 0.72), shade: (0.50, 0.50, 0.55), first: 0, count: 4 },
            Crag { outline: roughen(rng, &place(right, &from_right), 6.0 * s, 36.0 * s), lit: (0.84, 0.80, 0.73), shade: (0.56, 0.54, 0.55), first: 6, count: 3 },
        ];
        self.fall = (38.0 * s, gt - 470.0 * s, gt + 2.0 * s);
        self.stream = [(40.0 * s, gt + 4.0 * s), (78.0 * s, gt + 70.0 * s), (-20.0 * s, gt + 150.0 * s)];
        self.marmot = (w * 0.70, gt + 40.0 * s);
        self.sky = Some(Picture::record(w, h, |cr| {
            let mut g = Gradient::linear(0.0, 0.0, 0.0, gt);
            g.add_color_stop_rgb(0.0, 0.28, 0.54, 0.86);
            g.add_color_stop_rgb(0.55, 0.60, 0.79, 0.94);
            g.add_color_stop_rgb(1.0, 0.90, 0.93, 0.93);
            cr.rectangle(0.0, 0.0, w, h);
            cr.set_source(&g);
            cr.fill();
            // the morning sun, still low
            let (sx, sy) = (w * 0.37, h * 0.13);
            cr.glow(sx, sy, 440.0 * s, (1.0, 0.97, 0.84), 0.50, 0.0);
            cr.glow(sx, sy, 90.0 * s, (1.0, 0.99, 0.92), 0.85, 0.3);
        }));
        let this: &Capricorns = self;
        let land = Picture::record(w, h, |cr| {
            this.paint_land(cr, env, &crags);
            // twisted pines on the rock
            pine(cr, 232.0 * s, gt - 694.0 * s, 74.0 * s, 1.0);
            pine(cr, w - 292.0 * s, gt - 530.0 * s, 66.0 * s, -1.0);
            pine(cr, cx + 46.0 * s, gt - 296.0 * s, 46.0 * s, 1.0);
        });
        self.land = Some(land);
        // clouds, painted once
        self.cloud_pics = (0..2)
            .map(|v| {
                Picture::record(420.0 * s, 150.0 * s, |cr| {
                    let blobs: &[(f64, f64, f64)] = if v == 0 {
                        &[(90.0, 96.0, 44.0), (150.0, 72.0, 58.0), (220.0, 80.0, 52.0), (285.0, 98.0, 40.0), (330.0, 106.0, 30.0)]
                    } else {
                        &[(70.0, 102.0, 34.0), (120.0, 84.0, 46.0), (190.0, 66.0, 56.0), (260.0, 86.0, 48.0), (320.0, 100.0, 36.0)]
                    };
                    for (pass, col) in [(0.86, 0.90, 0.96), (0.99, 0.99, 1.0)].into_iter().enumerate() {
                        for &(x, y, r) in blobs {
                            let lift = pass as f64 * 8.0;
                            cr.circle(x * s, (y - lift) * s, (r - lift * 0.6) * s);
                        }
                        cr.rectangle(70.0 * s, (100.0 - pass as f64 * 14.0) * s, 270.0 * s, 26.0 * s);
                        cr.set_rgb(col);
                        cr.fill();
                    }
                })
            })
            .collect();
        self.clouds = (0..5)
            .map(|i| Cloud { f: (i as f64 + rng.uniform(0.0, 0.7)) / 5.0, y: h * rng.uniform(0.03, 0.30), sp: rng.uniform(0.004, 0.009), pic: i % 2 })
            .collect();
        self.shadow_pic = Some(Picture::record(640.0 * s, 170.0 * s, |cr| {
            cr.translate(320.0 * s, 85.0 * s);
            cr.scale(3.7, 1.0);
            cr.glow(0.0, 0.0, 84.0 * s, (0.04, 0.12, 0.10), 0.20, 0.3);
        }));
        self.rock_pic = Some(Picture::record(150.0 * s, 80.0 * s, |cr| {
            boulder(cr, 75.0 * s, 72.0 * s, 130.0 * s, 58.0 * s, (0.82, 0.81, 0.78), (0.52, 0.52, 0.56));
            tuft(cr, 18.0 * s, 74.0 * s, 16.0 * s, -0.2, GRASS[0]);
            tuft(cr, 132.0 * s, 75.0 * s, 14.0 * s, 0.2, GRASS[2]);
        }));
        self.bits.clear();
        let floor = h - env.strip_h;
        self.tufts = (0..(16.0 * w / (1600.0 * s)) as usize)
            .map(|_| {
                let y = rng.uniform(gt + 40.0 * s, floor + 8.0 * s);
                Tuft { x: rng.uniform(0.0, w), y, size: rng.uniform(20.0, 30.0) * s * ground_k(env, y), ph: rng.uniform(0.0, TAU), col: rng.index(3) }
            })
            .collect();
        self.tufts.sort_by(|a, b| a.y.total_cmp(&b.y));
        self.eagles = vec![
            (w * 0.56, h * 0.17, 210.0 * s, 46.0 * s, rng.uniform(0.0, TAU), 0.17),
            (w * 0.30, h * 0.09, 130.0 * s, 30.0 * s, rng.uniform(0.0, TAU), -0.13),
        ];
    }

    fn update_world(&mut self, env: &mut Env, actors: &mut [Ibex], dt: f64) {
        let s = env.s;
        // which places on the ledges are taken right now (also frees those of ibex that have gone)
        for l in &mut self.ledges {
            l.used = [false; 2];
        }
        for a in actors.iter().filter(|a| a.epoch == self.epoch) {
            for (i, spot) in a.holds().into_iter().flatten() {
                if let Some(l) = self.ledges.get_mut(i) {
                    l.used[spot] = true;
                }
            }
        }
        for p in &mut self.bits {
            p.age += dt;
            match p.kind {
                BitKind::Dust => {
                    p.vx *= 1.0 - (dt * 2.5).min(1.0);
                    p.size += dt * 16.0 * s;
                }
                BitKind::Spark => p.vy += 620.0 * s * dt,
                BitKind::Pebble => {
                    p.vy += 900.0 * s * dt;
                    if p.y >= p.floor && p.vy > 0.0 {
                        // bounce, lower each time, and roll out
                        p.y = p.floor;
                        p.vy *= -0.38;
                        p.vx *= 0.6;
                    }
                }
            }
            p.x += p.vx * dt;
            p.y += p.vy * dt;
        }
        self.bits.retain(|p| p.age < p.life);
    }

    fn draw_world(&mut self, cr: &mut Canvas, env: &Env, actors: &[Ibex], t: f64) {
        let (w, h, s) = (env.w, env.h, env.s);
        let gt = env.ground_top;
        if let Some(sky) = &self.sky {
            cr.picture(sky, 0.0, 0.0, 1.0);
        }
        // clouds drift behind the peaks
        let span = w + 520.0 * s;
        for c in &self.clouds {
            let x = ((c.f + t * c.sp) % 1.0) * span - 470.0 * s;
            cr.picture(&self.cloud_pics[c.pic], x.round(), c.y.round(), 1.0);
        }
        if let Some(land) = &self.land {
            cr.picture(land, 0.0, 0.0, 1.0);
        }
        // their shadows wander over the meadow
        if let Some(pic) = &self.shadow_pic {
            for (i, c) in self.clouds.iter().take(2).enumerate() {
                let x = ((c.f + 0.3 + t * c.sp * 1.4) % 1.0) * (w + 700.0 * s) - 670.0 * s;
                cr.picture(pic, x.round(), (gt + (34.0 + 150.0 * i as f64) * s).round(), 1.0);
            }
        }
        // the waterfall: streaks of white running down, spray at the foot, glints on the stream
        let (fx, fy0, fy1) = self.fall;
        cr.set_line_cap(LineCap::Round);
        for i in 0..8 {
            let fi = i as f64;
            let f = (t * 0.8 + fi / 8.0 + (fi * 1.7).sin() * 0.05).rem_euclid(1.0);
            let y = lerp(fy0, fy1 - 30.0 * s, f * f * 0.6 + f * 0.4);
            let x = fx + (fi * 2.4).sin() * (2.0 + 4.0 * f) * s;
            cr.move_to(x, y);
            cr.line_to(x, y + (16.0 + 22.0 * f) * s);
        }
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.8);
        cr.set_line_width(2.2 * s);
        cr.stroke();
        for i in 0..3 {
            let fi = i as f64;
            let f = (t * 0.55 + fi / 3.0) % 1.0;
            cr.ellipse(fx + (fi - 1.0) * 12.0 * s, fy1 - (4.0 + 16.0 * f) * s, (14.0 + 18.0 * f) * s, (7.0 + 8.0 * f) * s);
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.34 * (1.0 - f));
            cr.fill();
        }
        let [p0, p1, p2] = self.stream;
        for i in 0..5 {
            let f = (t * 0.22 + i as f64 / 5.0) % 1.0;
            let g = 1.0 - f;
            let (x, y) = (g * g * p0.0 + 2.0 * g * f * p1.0 + f * f * p2.0, g * g * p0.1 + 2.0 * g * f * p1.1 + f * f * p2.1);
            cr.move_to(x - 4.0 * s, y + (i % 2) as f64 * 3.0 * s - 2.0 * s);
            cr.rel_line_to(8.0 * s, 1.5 * s);
        }
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.75);
        cr.set_line_width(1.8 * s);
        cr.stroke();
        // eagles circling far up
        for &(ex, ey, rx, ry, ph, sp) in &self.eagles {
            let ang = t * sp + ph;
            let (x, y) = (ex + ang.cos() * rx, ey + ang.sin() * ry);
            let bank = ang.sin() * sp.signum();
            let span = 22.0 * s * (0.62 + 0.38 * ang.cos().abs());
            let dip = (t * 0.7 + ph).sin() * 0.06;
            leaf_path(cr, x, y, span, 3.6 * s, PI - 0.22 + bank * 0.25 + dip);
            leaf_path(cr, x, y, span, 3.6 * s, 0.22 + bank * 0.25 - dip);
            cr.ellipse(x, y + 1.0 * s, 4.2 * s, 2.4 * s);
            cr.set_source_rgba(0.17, 0.14, 0.14, 0.88);
            cr.fill();
        }
        // the marmot pops up behind its rock, looks about and ducks again
        let (mx, my) = self.marmot;
        let cyc = (t * 0.047 + 0.63).rem_euclid(1.0);
        let pop = smooth(cyc / 0.035) * (1.0 - smooth((cyc - 0.30) / 0.03));
        if pop > 0.01 {
            let peek = (t * 1.3).sin() * 5.0 * s * pop;
            let (bx, by) = (mx + 18.0 * s, my - 34.0 * s - pop * 36.0 * s);
            cr.ellipse(bx, by + 30.0 * s, 15.0 * s, 28.0 * s);
            cr.set_source_rgb(0.52, 0.38, 0.24);
            cr.fill();
            cr.ellipse(bx + 3.0 * s, by + 34.0 * s, 9.0 * s, 20.0 * s);
            cr.set_source_rgb(0.76, 0.62, 0.42);
            cr.fill();
            cr.circle(bx + peek - 9.0 * s, by - 7.0 * s, 3.6 * s);
            cr.circle(bx + peek + 9.0 * s, by - 7.0 * s, 3.6 * s);
            cr.circle(bx + peek, by, 12.0 * s);
            cr.set_source_rgb(0.46, 0.33, 0.21);
            cr.fill();
            cr.ellipse(bx + peek * 1.3, by + 4.0 * s, 6.5 * s, 5.0 * s);
            cr.set_source_rgb(0.80, 0.68, 0.50);
            cr.fill();
            cr.circle(bx + peek * 1.3 - 5.0 * s, by - 2.5 * s, 1.8 * s);
            cr.circle(bx + peek * 1.3 + 5.0 * s, by - 2.5 * s, 1.8 * s);
            cr.circle(bx + peek * 1.4, by + 2.5 * s, 1.9 * s);
            cr.set_source_rgb(0.08, 0.06, 0.05);
            cr.fill();
        }
        if let Some(rock) = &self.rock_pic {
            cr.picture(rock, (mx - 75.0 * s).round(), (my - 72.0 * s).round(), 1.0);
        }
        // ibex and the grass tufts of the meadow, back to front
        let mut ti = 0;
        let tuft_at = |cr: &mut Canvas, f: &Tuft| {
            tuft(cr, f.x, f.y, f.size, (t * 1.1 + f.ph).sin() * 0.16, darken(GRASS[f.col], 0.12));
        };
        for a in sorted_actors(actors) {
            let foot = if a.off_ground() { 0.0 } else { a.b.y + a.b.radius };
            while ti < self.tufts.len() && self.tufts[ti].y < foot {
                tuft_at(cr, &self.tufts[ti]);
                ti += 1;
            }
            a.draw(cr, env, self, t);
        }
        while ti < self.tufts.len() {
            tuft_at(cr, &self.tufts[ti]);
            ti += 1;
        }
        // pebbles, dust and sparks
        for p in &self.bits {
            let f = p.age / p.life;
            match p.kind {
                BitKind::Pebble => {
                    cr.circle(p.x, p.y, p.size);
                    cr.set_rgba(darken(p.c, 0.35), (1.0 - f).min(0.25) * 4.0);
                    cr.fill();
                    cr.circle(p.x - p.size * 0.2, p.y - p.size * 0.25, p.size * 0.62);
                    cr.set_rgba(lighten(p.c, 0.2), (1.0 - f).min(0.25) * 4.0);
                    cr.fill();
                }
                BitKind::Dust => {
                    cr.circle(p.x, p.y, p.size);
                    cr.set_rgba(p.c, 0.42 * (1.0 - f));
                    cr.fill();
                }
                BitKind::Spark => {
                    cr.move_to(p.x, p.y);
                    cr.line_to(p.x - p.vx * 0.035, p.y - p.vy * 0.035);
                    cr.set_rgba(p.c, 1.0 - f * f);
                    cr.set_line_width(p.size);
                    cr.stroke();
                }
            }
        }
        // grass and a flower or two right in front
        for (i, fx) in [0.035, 0.12, 0.90, 0.975].into_iter().enumerate() {
            let fi = i as f64;
            let x = w * fx;
            tuft(cr, x, h - 4.0 * s, 62.0 * s, (t * 0.9 + fi * 1.9).sin() * 0.10, (0.14, 0.30, 0.13));
            tuft(cr, x + 26.0 * s, h - 2.0 * s, 44.0 * s, (t * 0.9 + fi * 1.9 + 0.6).sin() * 0.12, (0.20, 0.38, 0.16));
        }
    }
}
