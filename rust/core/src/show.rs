//! The rotating show: switches theme every N minutes with a fade through black.
use std::rc::Rc;

use crate::canvas::{Canvas, Picture};
use crate::scene::{Scene, SceneOpts};
use crate::sessions::Session;
use crate::sysmon::SysData;
use crate::text::{self, weight};
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

/// The themes the running screensaver rotates through. Like `parse_list`, but seasonal themes that are
/// out of season (`resting`) only take part when the settings name them: "all" leaves them out,
/// "all, workshop" or "workshop" shows them at any time of the year.
pub fn parse_rotation(value: &str, all: &[&'static str], resting: &[&str]) -> Vec<&'static str> {
    let named: Vec<String> = value.replace(';', ",").split(',').map(|n| n.trim().to_lowercase()).collect();
    let names: Vec<&'static str> =
        parse_list(value, all).into_iter().filter(|n| !resting.contains(n) || named.iter().any(|w| w == n)).collect();
    if names.is_empty() { parse_list(value, all) } else { names }
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
    /// every theme there is (the picker offers them all, also seasonal ones out of season)
    all: Vec<&'static str>,
    /// a theme chosen in the picker, shown next
    pending: Option<&'static str>,
    picker: Option<Picker>,
}

/// The theme picker: a grid of small pictures of every theme, drawn over the scene.
struct Picker {
    selected: usize,
    thumbs: Vec<Picture>,
    /// seconds without a key (it closes by itself)
    quiet: f64,
}

/// A move in the theme picker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickerKey {
    Left,
    Right,
    Up,
    Down,
}

const PICKER_TIMEOUT: f64 = 20.0;
const THUMB: (f64, f64) = (1600.0, 1000.0); // the size a thumbnail scene is laid out for

const FADE: f64 = 1.2; // seconds to fade out (and again to fade in)

impl Show {
    /// `avoid`: the theme of the main monitor, which this one should not start with.
    pub fn new(
        factory: Factory,
        names: Vec<&'static str>,
        all: &[&'static str],
        minutes: f64,
        opts: SceneOpts,
        avoid: Option<&str>,
    ) -> Show {
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
            all: all.to_vec(),
            pending: None,
            picker: None,
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

    /// Switch to this theme now (with the usual fade), whether or not it is in the rotation.
    pub fn show_theme(&mut self, name: &str) {
        let Some(name) = self.all.iter().copied().find(|n| *n == name) else { return };
        if name == self.current && self.fading == 0 {
            return;
        }
        self.pending = Some(name);
        self.elapsed = 0.0;
        if self.fading <= 0 {
            self.fading = 1;
        }
    }

    // ------------------------------------------------------------------ theme picker
    pub fn picker_open(&self) -> bool {
        self.picker.is_some()
    }

    /// Open the picker: a small picture of every theme, the current one selected.
    pub fn open_picker(&mut self) {
        if self.picker.is_some() {
            return;
        }
        let opts = SceneOpts { show_text: false, decor: true, seed: Some(7) };
        let mut thumbs = Vec::new();
        for name in &self.all {
            let Some(mut scene) = (self.factory)(name, opts) else { continue };
            // the first drawing tells the scene its size; then let the creatures settle in
            let _ = Picture::record(THUMB.0, THUMB.1, |cr| scene.draw(cr, THUMB.0, THUMB.1));
            scene.set_sessions(&self.sessions);
            for _ in 0..45 {
                scene.update(0.1);
            }
            thumbs.push(Picture::record(THUMB.0, THUMB.1, |cr| scene.draw(cr, THUMB.0, THUMB.1)));
        }
        let selected = self.all.iter().position(|n| *n == self.current).unwrap_or(0);
        self.picker = Some(Picker { selected, thumbs, quiet: 0.0 });
    }

    pub fn close_picker(&mut self) {
        self.picker = None;
    }

    fn picker_cols(n: usize) -> usize {
        ((n as f64 * 1.6).sqrt().ceil() as usize).clamp(1, 8)
    }

    pub fn picker_move(&mut self, key: PickerKey) {
        let n = self.all.len();
        let cols = Self::picker_cols(n);
        let Some(p) = self.picker.as_mut() else { return };
        p.quiet = 0.0;
        let i = p.selected;
        p.selected = match key {
            PickerKey::Left => (i + n - 1) % n,
            PickerKey::Right => (i + 1) % n,
            PickerKey::Up if i >= cols => i - cols,
            PickerKey::Down if i + cols < n => i + cols,
            PickerKey::Down if i / cols < (n - 1) / cols => n - 1, // short last row: go to its end
            _ => i,
        };
    }

    /// Show the selected theme and close the picker.
    pub fn picker_choose(&mut self) {
        let Some(p) = self.picker.take() else { return };
        if let Some(name) = self.all.get(p.selected).copied() {
            self.show_theme(name);
        }
    }

    fn draw_picker(&self, cr: &mut Canvas, w: f64, h: f64) {
        let Some(p) = &self.picker else { return };
        let s = (h / 1200.0).min(w / 1600.0).clamp(0.4, 3.0);
        cr.set_source_rgba(0.0, 0.0, 0.0, 0.72);
        cr.paint();
        let n = p.thumbs.len().min(self.all.len());
        if n == 0 {
            return;
        }
        let cols = Self::picker_cols(n);
        let rows = n.div_ceil(cols);
        let gap = 22.0 * s;
        let label_h = 34.0 * s;
        let title_h = 70.0 * s;
        // the largest cell that fits both ways
        let cw_max = (w * 0.92 - gap * (cols as f64 - 1.0)) / cols as f64;
        let ch_max = (h * 0.86 - title_h - (gap + label_h) * rows as f64) / rows as f64;
        let cw = cw_max.min(ch_max * THUMB.0 / THUMB.1).max(40.0);
        let ch = cw * THUMB.1 / THUMB.0;
        let total_w = cw * cols as f64 + gap * (cols as f64 - 1.0);
        let total_h = title_h + (ch + label_h + gap) * rows as f64 - gap;
        let (x0, y0) = ((w - total_w) / 2.0, (h - total_h) / 2.0);
        let title = text::layout("Choose a theme", 30.0 * s, weight::BOLD, None);
        text::show(cr, &title, x0, y0, (1.0, 1.0, 1.0), 0.95, 0.0);
        let help = text::layout("arrow keys  ·  Enter shows it  ·  Esc closes", 16.0 * s, weight::NORMAL, None);
        let (hw, hh) = help.get_pixel_size();
        text::show(cr, &help, x0 + total_w - hw, y0 + title.get_pixel_size().1 - hh - 4.0 * s, (0.8, 0.86, 0.95), 0.8, 0.0);
        for i in 0..n {
            let (col, row) = (i % cols, i / cols);
            let x = x0 + col as f64 * (cw + gap);
            let y = y0 + title_h + row as f64 * (ch + label_h + gap);
            let chosen = i == p.selected;
            cr.save();
            cr.new_path();
            cr.rounded_rect(x, y, cw, ch, 8.0 * s);
            cr.clip();
            cr.translate(x, y);
            cr.scale(cw / THUMB.0, ch / THUMB.1);
            cr.picture(&p.thumbs[i], 0.0, 0.0, 1.0);
            cr.restore();
            if !chosen {
                cr.set_source_rgba(0.0, 0.0, 0.0, 0.35); // the others step back a little
                cr.new_path();
                cr.rounded_rect(x, y, cw, ch, 8.0 * s);
                cr.fill();
            }
            cr.new_path();
            cr.rounded_rect(x, y, cw, ch, 8.0 * s);
            if chosen {
                cr.set_source_rgba(1.0, 0.70, 0.22, 1.0);
                cr.set_line_width(4.0 * s);
            } else {
                cr.set_source_rgba(1.0, 1.0, 1.0, 0.25);
                cr.set_line_width(1.5 * s);
            }
            cr.stroke();
            let name = self.all[i];
            let now = if name == self.current { "  (showing)" } else { "" };
            let lay = text::layout(&format!("{name}{now}"), 17.0 * s, if chosen { weight::BOLD } else { weight::NORMAL }, Some(cw));
            let col_rgb = if chosen { (1.0, 0.78, 0.36) } else { (0.88, 0.92, 1.0) };
            text::show(cr, &lay, x + 2.0 * s, y + ch + 7.0 * s, col_rgb, if chosen { 1.0 } else { 0.8 }, 0.0);
        }
    }

    /// `avoid`: the theme currently on the main monitor (extra monitors show something different).
    pub fn update(&mut self, dt: f64, avoid: Option<&str>) {
        if let Some(p) = self.picker.as_mut() {
            p.quiet += dt;
            if p.quiet > PICKER_TIMEOUT {
                self.picker = None; // nobody chose: back to the screensaver
            }
        }
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
                self.current = match self.pending.take() {
                    Some(name) => name,
                    None => next_name(&self.names, &mut self.queue, self.current, avoid, &self.rng),
                };
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
        self.draw_picker(cr, w, h);
        cr.finish();
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
        assert_eq!(parse_rotation("all", &ALL, &["space"]), vec!["aquarium", "synthwave", "pasture"]);
        assert_eq!(parse_rotation("all, space", &ALL, &["space"]), ALL.to_vec());
        assert_eq!(parse_rotation("Space", &ALL, &["space"]), vec!["space"]);
        assert_eq!(parse_rotation("all, -pasture", &ALL, &["space"]), vec!["aquarium", "synthwave"]);
        assert_eq!(parse_rotation("all", &ALL, &[]), ALL.to_vec());
    }
}
