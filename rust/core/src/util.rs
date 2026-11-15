//! Small helpers shared by everything: colours, interpolation, a random generator.
use std::cell::Cell;
use std::f64::consts::PI;
use std::time::{SystemTime, UNIX_EPOCH};

pub const TAU: f64 = std::f64::consts::TAU;

/// Colour as (red, green, blue), each 0..1 - the same tuples the Python version used.
pub type Rgb = (f64, f64, f64);

pub const CLAUDE_ORANGE: Rgb = (0.85, 0.47, 0.34);

pub fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

pub fn lerp3(c1: Rgb, c2: Rgb, t: f64) -> Rgb {
    (lerp(c1.0, c2.0, t), lerp(c1.1, c2.1, t), lerp(c1.2, c2.2, t))
}

pub fn lighten(c: Rgb, f: f64) -> Rgb {
    lerp3(c, (1.0, 1.0, 1.0), f)
}

pub fn darken(c: Rgb, f: f64) -> Rgb {
    lerp3(c, (0.0, 0.0, 0.0), f)
}

pub fn clamp(v: f64, lo: f64, hi: f64) -> f64 {
    // like Python's max(lo, min(hi, v)): never panics when lo > hi
    lo.max(hi.min(v))
}

/// Python's `%` for floats: the result has the sign of `m`.
pub fn pymod(a: f64, m: f64) -> f64 {
    a.rem_euclid(m)
}

pub fn angle_lerp(a: f64, b: f64, t: f64) -> f64 {
    let d = pymod(b - a + PI, TAU) - PI;
    a + d * t
}

pub fn hypot(x: f64, y: f64) -> f64 {
    x.hypot(y)
}

/// Seconds since 1970 (Python's `time.time()`).
pub fn now() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

pub fn fmt_duration(sec: f64) -> String {
    let sec = sec.max(0.0) as u64;
    if sec < 60 {
        return format!("{sec}s");
    }
    let (m, s) = (sec / 60, sec % 60);
    if m < 60 {
        return format!("{m}m {s:02}s");
    }
    format!("{}h {:02}m", m / 60, m % 60)
}

/// Local time formatted with strftime codes (English names, like Python's default locale).
pub fn strftime(fmt: &str) -> String {
    chrono::Local::now().format(fmt).to_string()
}

/// Today's (month, day) in local time.
pub fn local_month_day() -> (u32, u32) {
    use chrono::Datelike;
    let n = chrono::Local::now();
    (n.month(), n.day())
}

/// Local time of day in hours (0.0 .. 24.0).
pub fn local_hours() -> f64 {
    use chrono::Timelike;
    let n = chrono::Local::now();
    n.hour() as f64 + n.minute() as f64 / 60.0 + n.second() as f64 / 3600.0
}

/// Random numbers with the method names of Python's `random.Random`.
/// Works through a shared reference, so drawing code can use it too.
#[derive(Debug)]
pub struct Rng {
    s: Cell<[u64; 2]>,
}

impl Clone for Rng {
    fn clone(&self) -> Self {
        Rng { s: Cell::new(self.s.get()) }
    }
}

impl Rng {
    pub fn new(seed: Option<u64>) -> Self {
        let seed = seed.unwrap_or_else(|| {
            let t = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1);
            t ^ (&t as *const u64 as u64).rotate_left(32)
        });
        // splitmix64 to spread the seed over the state
        let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut next = || {
            z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut x = z;
            x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            x ^ (x >> 31)
        };
        let a = next();
        let b = next() | 1;
        Rng { s: Cell::new([a, b]) }
    }

    fn next_u64(&self) -> u64 {
        // xoroshiro128+
        let [s0, mut s1] = self.s.get();
        let r = s0.wrapping_add(s1);
        s1 ^= s0;
        self.s.set([s0.rotate_left(24) ^ s1 ^ (s1 << 16), s1.rotate_left(37)]);
        r
    }

    /// 0.0 <= x < 1.0
    pub fn random(&self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn uniform(&self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.random()
    }

    /// Integer in a..=b (both ends included, like Python's randint).
    pub fn randint(&self, a: i64, b: i64) -> i64 {
        if b <= a {
            return a;
        }
        a + (self.next_u64() % ((b - a + 1) as u64)) as i64
    }

    /// Index in 0..n.
    pub fn index(&self, n: usize) -> usize {
        if n == 0 { 0 } else { (self.next_u64() % n as u64) as usize }
    }

    pub fn choice<'a, T>(&self, items: &'a [T]) -> &'a T {
        &items[self.index(items.len())]
    }

    pub fn chance(&self, p: f64) -> bool {
        self.random() < p
    }

    pub fn gauss(&self, mu: f64, sigma: f64) -> f64 {
        let u1 = (1.0 - self.random()).max(1e-12);
        let u2 = self.random();
        mu + sigma * (-2.0 * u1.ln()).sqrt() * (TAU * u2).cos()
    }

    pub fn shuffle<T>(&self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            items.swap(i, self.index(i + 1));
        }
    }
}
