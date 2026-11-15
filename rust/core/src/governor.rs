//! Adaptive frame rate: keeps the animation smooth when the computer is busy.
//!
//! The scenes are slow-moving, so 30 fps is plenty - and 15 fps that arrives evenly looks better than
//! 30 fps that stutters. The governor watches how long each frame takes to produce. When producing
//! frames eats too large a share of the time, it steps the frame rate down, which also leaves more of
//! the machine to everything else. When there is plenty of headroom again it steps back up, slowly,
//! and waits longer each time it has to step down again soon after.
//!
//! Used by the render loop:  if gov.due(waited) { ...render...; gov.record(seconds, now) }

pub const LEVELS: [u32; 3] = [30, 20, 15]; // frames per second, best first
const DOWN_AT: f64 = 0.60; // step down when (mean + spread of frame time) * fps exceeds this share of a second
const UP_BELOW: f64 = 0.35; // step up only if the same figure would stay below this at the higher rate
const SMOOTH: f64 = 0.10; // weight of each new frame in the running averages (~10 frames)
const MIN_FRAMES: u32 = 20; // frames needed after a change before judging again
const DOWN_WAIT: f64 = 1.0; // s between steps down
const UP_WAIT: f64 = 20.0; // s of calm before the first step up (doubles after a quick relapse, up to 5 min)

#[derive(Clone, Debug)]
pub struct Governor {
    levels: Vec<u32>,
    base_fps: u32,
    pub enabled: bool,
    level: usize,
    stats: Option<(f64, f64)>, // running mean and mean of squares
    frames: u32,
    changed: f64,
    up_wait: f64,
    last_up: Option<f64>,
}

impl Governor {
    /// `now` is any steadily increasing clock in seconds; pass the same clock to `record`.
    pub fn new(base_fps: u32, levels: &[u32], enabled: bool, now: f64) -> Self {
        let mut lv: Vec<u32> = levels.iter().copied().filter(|f| *f <= base_fps).collect();
        if lv.is_empty() {
            lv.push(base_fps);
        }
        Governor {
            levels: lv,
            base_fps,
            enabled,
            level: 0,
            stats: None,
            frames: 0,
            changed: now,
            up_wait: UP_WAIT,
            last_up: None,
        }
    }

    pub fn fps(&self) -> u32 {
        if self.enabled { self.levels[self.level] } else { self.base_fps }
    }

    /// True when a new frame should be produced after `waited` seconds since the last one.
    pub fn due(&self, waited: f64) -> bool {
        waited >= 1.0 / self.fps() as f64 - 0.5 / self.base_fps as f64
    }

    fn stress(&self, fps: u32) -> f64 {
        let (mean, sq) = self.stats.unwrap_or((0.0, 0.0));
        let spread = (sq - mean * mean).max(0.0).sqrt();
        (mean + spread) * fps as f64
    }

    /// Report the time one frame took to produce.
    pub fn record(&mut self, seconds: f64, now: f64) {
        if !self.enabled {
            return;
        }
        self.stats = Some(match self.stats {
            None => (seconds, seconds * seconds),
            Some((m, q)) => (m + SMOOTH * (seconds - m), q + SMOOTH * (seconds * seconds - q)),
        });
        self.frames += 1;
        if self.frames < MIN_FRAMES {
            return;
        }
        if self.level < self.levels.len() - 1
            && now - self.changed >= DOWN_WAIT
            && self.stress(self.levels[self.level]) > DOWN_AT
        {
            if self.last_up.is_some_and(|u| now - u < 60.0) {
                self.up_wait = (self.up_wait * 2.0).min(300.0); // that did not hold: be more careful next time
            }
            self.level += 1;
            self.changed = now;
            self.frames = 0;
        } else if self.level > 0
            && now - self.changed >= self.up_wait
            && self.stress(self.levels[self.level - 1]) < UP_BELOW
        {
            self.level -= 1;
            self.last_up = Some(now);
            self.changed = now;
            self.frames = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_down_and_up() {
        let mut g = Governor::new(30, &LEVELS, true, 0.0);
        let mut now = 0.0;
        for _ in 0..200 {
            now += 1.0 / 30.0;
            g.record(0.005, now);
        }
        assert_eq!(g.fps(), 30);
        for _ in 0..200 {
            now += 1.0 / 20.0;
            g.record(0.040, now);
        }
        assert_eq!(g.fps(), 15);
        for _ in 0..3000 {
            now += 1.0 / 15.0;
            g.record(0.004, now);
        }
        assert_eq!(g.fps(), 30);
        let off = Governor::new(30, &LEVELS, false, 0.0);
        assert_eq!(off.fps(), 30);
        assert!(off.due(1.0 / 30.0) && !off.due(0.01));
    }
}
