//! Text: laid out with the system fonts (with fallback for symbols and other scripts) and turned into
//! outlines once per string. Drawing a text is then an ordinary filled path, on any renderer.
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use cosmic_text::{
    Attrs, Buffer, CacheKey, CacheKeyFlags, Ellipsize, EllipsizeHeightLimit, Family as CtFamily, FontSystem, Metrics,
    Shaping, SwashCache, Wrap,
};
use kurbo::{BezPath, Point};

use crate::canvas::Canvas;
use crate::util::Rgb;

/// Font weights used by the themes (same numbers as Pango.Weight).
pub mod weight {
    pub const LIGHT: u16 = 300;
    pub const NORMAL: u16 = 400;
    pub const SEMIBOLD: u16 = 600;
    pub const BOLD: u16 = 700;
    pub const HEAVY: u16 = 900;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Family {
    Sans,
    Mono,
}

#[cfg(target_os = "macos")]
const FACES: (&str, &str) = ("Helvetica Neue", "Menlo");
#[cfg(target_os = "windows")]
const FACES: (&str, &str) = ("Segoe UI", "Consolas");
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const FACES: (&str, &str) = ("Ubuntu Sans", "Ubuntu Sans Mono");

pub fn font_names() -> (&'static str, &'static str) {
    FACES
}

#[derive(Debug)]
pub struct TextLayout {
    pub width: f64,
    pub height: f64,
    pub ascent: f64,
    /// glyph outlines, origin = top left corner of the text
    pub path: BezPath,
}

impl TextLayout {
    /// Size in whole pixels (as Pango reports it).
    pub fn get_pixel_size(&self) -> (f64, f64) {
        ((self.width + 0.5).floor(), (self.height + 0.5).floor())
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct Key {
    text: String,
    size10: i64,
    weight: u16,
    max_width: Option<i64>,
    family: Family,
}

struct Engine {
    fonts: FontSystem,
    swash: SwashCache,
    cache: HashMap<Key, Rc<TextLayout>>,
    /// which of the wanted families exist (index 0 sans, 1 mono)
    have: [bool; 2],
}

impl Engine {
    fn new() -> Self {
        let fonts = FontSystem::new();
        let has = |name: &str| fonts.db().faces().any(|f| f.families.iter().any(|(n, _)| n.eq_ignore_ascii_case(name)));
        let have = [has(FACES.0), has(FACES.1)];
        Engine { fonts, swash: SwashCache::new(), cache: HashMap::new(), have }
    }

    fn layout(&mut self, text: &str, size: f64, weight: u16, max_width: Option<f64>, family: Family) -> Rc<TextLayout> {
        let key = Key {
            text: text.to_string(),
            size10: (size * 10.0).round() as i64,
            weight,
            max_width: max_width.map(|w| w.round() as i64),
            family,
        };
        if let Some(l) = self.cache.get(&key) {
            return l.clone();
        }
        let lay = Rc::new(self.build(text, size.max(1.0), weight, max_width, family));
        if self.cache.len() > 3000 {
            self.cache.clear();
        }
        self.cache.insert(key, lay.clone());
        lay
    }

    fn build(&mut self, text: &str, size: f64, weight: u16, max_width: Option<f64>, family: Family) -> TextLayout {
        let size32 = size as f32;
        let fam = match family {
            Family::Sans if self.have[0] => CtFamily::Name(FACES.0),
            Family::Sans => CtFamily::SansSerif,
            Family::Mono if self.have[1] => CtFamily::Name(FACES.1),
            Family::Mono => CtFamily::Monospace,
        };
        let attrs = Attrs::new()
            .family(fam)
            .weight(cosmic_text::Weight(weight))
            .cache_key_flags(CacheKeyFlags::DISABLE_HINTING);
        let mut buf = Buffer::new_empty(Metrics::new(size32, (size32 * 1.25).max(1.0)));
        buf.set_wrap(Wrap::None);
        if let Some(mw) = max_width {
            buf.set_ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1)));
            buf.set_size(Some(mw.max(1.0) as f32), None);
        } else {
            buf.set_size(None, None);
        }
        let one_line: String = text.chars().map(|c| if c == '\n' || c == '\r' || c == '\t' { ' ' } else { c }).collect();
        buf.set_text(&one_line, &attrs, Shaping::Advanced, None);
        buf.shape_until_scroll(&mut self.fonts, false);

        let mut glyphs = Vec::new();
        let (mut width, mut ascent, mut descent) = (0.0f32, 0.0f32, 0.0f32);
        if let Some(lines) = buf.line_layout(&mut self.fonts, 0) {
            if let Some(line) = lines.first() {
                width = line.w;
                ascent = line.max_ascent;
                descent = line.max_descent;
                for g in &line.glyphs {
                    glyphs.push((
                        g.font_id,
                        g.glyph_id,
                        g.font_size,
                        g.font_weight,
                        g.x + g.font_size * g.x_offset,
                        g.y - g.font_size * g.y_offset,
                    ));
                }
            }
        }
        if ascent <= 0.0 {
            ascent = size32 * 0.95;
            descent = size32 * 0.25;
        }
        let mut path = BezPath::new();
        for (font_id, glyph_id, font_size, font_weight, gx, gy) in glyphs {
            let (ck, _, _) =
                CacheKey::new(font_id, glyph_id, font_size, (0.0, 0.0), font_weight, CacheKeyFlags::DISABLE_HINTING);
            let Some(cmds) = self.swash.get_outline_commands(&mut self.fonts, ck) else { continue };
            let (ox, oy) = (gx as f64, (ascent + gy) as f64);
            let p = |v: &zeno::Vector| Point::new(ox + v.x as f64, oy - v.y as f64);
            use zeno::Command as C;
            for c in cmds {
                match c {
                    C::MoveTo(a) => path.move_to(p(a)),
                    C::LineTo(a) => path.line_to(p(a)),
                    C::QuadTo(a, b) => path.quad_to(p(a), p(b)),
                    C::CurveTo(a, b, d) => path.curve_to(p(a), p(b), p(d)),
                    C::Close => path.close_path(),
                }
            }
        }
        let mut width = width as f64;
        if let Some(mw) = max_width {
            width = width.min(mw);
        }
        TextLayout { width, height: (ascent + descent) as f64, ascent: ascent as f64, path }
    }
}

thread_local! {
    static ENGINE: RefCell<Option<Engine>> = const { RefCell::new(None) };
}

/// Lay out one line of text (cached). `max_width` cuts it with "…".
pub fn layout(text: &str, size: f64, weight: u16, max_width: Option<f64>) -> Rc<TextLayout> {
    layout_family(text, size, weight, max_width, Family::Sans)
}

pub fn layout_family(text: &str, size: f64, weight: u16, max_width: Option<f64>, family: Family) -> Rc<TextLayout> {
    ENGINE.with(|e| {
        let mut e = e.borrow_mut();
        e.get_or_insert_with(Engine::new).layout(text, size, weight, max_width, family)
    })
}

/// Load the system fonts now (takes a moment) instead of at the first text.
pub fn warm_up() {
    layout("Claude", 16.0, weight::NORMAL, None);
}

/// Names of the font families that were found for (sans, mono) - for the self test.
pub fn resolved_fonts() -> (String, String) {
    ENGINE.with(|e| {
        let mut e = e.borrow_mut();
        let e = e.get_or_insert_with(Engine::new);
        let f = |i: usize, name: &str, generic: &str| {
            if e.have[i] { name.to_string() } else { format!("{name} not found, using the system {generic} font") }
        };
        (f(0, FACES.0, "sans-serif"), f(1, FACES.1, "monospace"))
    })
}

/// Draw a layout with its top left corner at (x, y). `shadow` > 0 adds a dark copy below.
pub fn show(cr: &mut Canvas, lay: &TextLayout, x: f64, y: f64, rgb: Rgb, alpha: f64, shadow: f64) {
    if shadow > 0.0 {
        cr.fill_path_at(&lay.path, x + 1.0, y + 2.0, (0.0, 0.0, 0.0), shadow * alpha);
    }
    cr.fill_path_at(&lay.path, x, y, rgb, alpha);
}
