//! The rotating show: switches theme every N minutes with a fade through black.
use std::rc::Rc;

use crate::canvas::Canvas;
use crate::scene::{Scene, SceneOpts};
use crate::sessions::Session;
use crate::sysmon::SysData;
use crate::util::Rng;

/// Builds the scene for a theme name.
pub type Factory = fn(&str, SceneOpts) -> Option<Box<dyn Scene>>;

/// Theme selection from the settings -> list of valid theme names (never empty).
///
///   all                         every theme
///   pasture, seabed, forest     only these
///   all, -synthwave, -space     every theme except these
pub fn parse_list(value: &str, all: &[&'static str]) -> Vec<&'static str> {
    let value = if value.trim().is_empty() { "all" } else { value };
    let words: Vec<String> =
        value.replace(';', ",").split(',').map(|n| n.trim().to_lowercase()).filter(|n| !n.is_empty()).collect();
    let include: Vec<&String> = words.iter().filter(|n| !n.starts_with('-')).collect();
    let exclude: Vec<&str> = words.iter().filter(|n| n.starts_with('-')).map(|n| n[1..].trim()).collect();
    let mut names: Vec<&'static str> =
        if include.is_empty() || include.iter().any(|n| matches!(n.as_str(), "all" | "random" | "*")) {
            all.to_vec()
        } else {
            include.iter().filter_map(|n| all.iter().find(|a| *a == n).copied()).collect()
        };
    names.retain(|n| !exclude.contains(n));
    if names.is_empty() { all.to_vec() } else { names }
}

/// Looks like a Scene to the screensaver; rotates through the configured themes in random order.
///
/// The list is shuffled, played through, then reshuffled - every theme comes once per round and
/// the same theme never plays twice in a row.
pub struct Show {
    factory: Factory,
    pub names: Vec<&'static str>,
    minutes: f64,
    opts: SceneOpts,
    queue: Vec<&'static str>,
    current: &'static str,
    scene: Box<dyn Scene>,
    sessions: Vec<Rc<Session>>,
    system: Option<Rc<SysData>>,
    elapsed: f64,
    fade: f64,
    fading: i8, // +1 fading out, -1 fading in
    rng: Rng,
}

const FADE: f64 = 1.2; // seconds to fade out (and again to fade in)

impl Show {
    /// `avoid`: the theme of the main monitor, which this one should not start with.
    pub fn new(factory: Factory, names: Vec<&'static str>, minutes: f64, opts: SceneOpts, avoid: Option<&str>) -> Show {
        let rng = Rng::new(opts.seed);
        let mut queue = Vec::new();
        let current = next_name(&names, &mut queue, "", avoid, &rng);
        let scene = factory(current, opts).expect("theme names come from the registry");
        Show {
            factory,
            names,
            minutes,
            opts,
            queue,
            current,
            scene,
            sessions: Vec::new(),
            system: None,
            elapsed: 0.0,
            fade: 0.0,
            fading: 0,
            rng,
        }
    }

    pub fn name(&self) -> &'static str {
        self.current
    }

    pub fn set_sessions(&mut self, sessions: &[Rc<Session>]) {
        self.sessions = sessions.to_vec();
        self.scene.set_sessions(sessions);
    }

    pub fn set_system(&mut self, data: Option<Rc<SysData>>) {
        self.system = data.clone();
        self.scene.set_system(data);
    }

    pub fn show_hint(&mut self, text: &str, seconds: f64) {
        self.scene.show_hint(text, seconds);
    }

    pub fn next_theme(&mut self) {
        if self.names.len() > 1 && self.fading == 0 {
            self.fading = 1;
            self.elapsed = 0.0; // the new theme gets its full time
        }
    }

    /// `avoid`: the theme currently on the main monitor (extra monitors show something different).
    pub fn update(&mut self, dt: f64, avoid: Option<&str>) {
        self.scene.update(dt);
        self.elapsed += dt;
        if self.minutes > 0.0 && self.fading == 0 && self.elapsed >= self.minutes * 60.0 {
            self.next_theme();
            self.elapsed = 0.0;
        }
        if self.fading != 0 {
            self.fade += self.fading as f64 * dt / FADE;
            if self.fade >= 1.0 {
                self.fade = 1.0;
                let (hint, left) = self.scene.hint_left();
                self.current = next_name(&self.names, &mut self.queue, self.current, avoid, &self.rng);
                self.scene = (self.factory)(self.current, self.opts).expect("theme names come from the registry");
                self.scene.set_sessions(&self.sessions);
                self.scene.set_system(self.system.clone());
                if left > 0.0 {
                    self.scene.show_hint(&hint, left);
                }
                self.fading = -1;
            } else if self.fade <= 0.0 {
                self.fade = 0.0;
                self.fading = 0;
            }
        }
    }

    pub fn draw(&mut self, cr: &mut Canvas, w: f64, h: f64) {
        self.scene.draw(cr, w, h);
        cr.finish();
        if self.fade > 0.0 {
            cr.set_source_rgba(0.0, 0.0, 0.0, self.fade);
            cr.paint();
        }
    }
}

fn next_name(
    names: &[&'static str],
    queue: &mut Vec<&'static str>,
    current: &str,
    avoid: Option<&str>,
    rng: &Rng,
) -> &'static str {
    if queue.is_empty() {
        *queue = names.to_vec();
        rng.shuffle(queue);
        if queue.len() > 1 && queue[0] == current {
            let first = queue.remove(0);
            queue.push(first);
        }
    }
    if let Some(av) = avoid {
        if queue.len() > 1 && queue[0] == av {
            let first = queue.remove(0);
            queue.push(first); // show something different from the main monitor
        }
    }
    queue.remove(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [&str; 4] = ["aquarium", "space", "synthwave", "pasture"];

    #[test]
    fn lists() {
        assert_eq!(parse_list("all", &ALL), ALL.to_vec());
        assert_eq!(parse_list("", &ALL), ALL.to_vec());
        assert_eq!(parse_list("Pasture; space, nope", &ALL), vec!["pasture", "space"]);
        assert_eq!(parse_list("all, -synthwave, -space", &ALL), vec!["aquarium", "pasture"]);
        assert_eq!(parse_list("-space", &ALL), vec!["aquarium", "synthwave", "pasture"]);
        assert_eq!(parse_list("nope", &ALL), ALL.to_vec());
    }
}
