//! A drawing surface with the calls of a Cairo context, so the themes read like the Python originals.
//!
//! The Canvas keeps Cairo's rules: a path is transformed by the matrix that is current when each point
//! is added (not when it is filled), a gradient is positioned by the matrix that is current when it
//! becomes the source, and a stroke uses the matrix that is current when `stroke()` is called.
//! What is drawn goes to a `Sink` - the GPU renderer, or a `Picture` recorder for cached drawings.
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

pub use kurbo::{Affine, BezPath, PathEl, Point, Rect};

use crate::util::{Rgb, TAU};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FillRule {
    Winding,
    EvenOdd,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineCap {
    Butt,
    Round,
    Square,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineJoin {
    Miter,
    Round,
    Bevel,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GradientKind {
    Linear { x0: f64, y0: f64, x1: f64, y1: f64 },
    Radial { cx0: f64, cy0: f64, r0: f64, cx1: f64, cy1: f64, r1: f64 },
}

/// Linear or radial gradient (colours beyond the ends continue with the end colour, like Cairo).
#[derive(Clone, Debug, PartialEq)]
pub struct Gradient {
    pub kind: GradientKind,
    pub stops: Vec<(f32, [f32; 4])>,
}

impl Gradient {
    pub fn linear(x0: f64, y0: f64, x1: f64, y1: f64) -> Self {
        Gradient { kind: GradientKind::Linear { x0, y0, x1, y1 }, stops: Vec::new() }
    }

    pub fn radial(cx0: f64, cy0: f64, r0: f64, cx1: f64, cy1: f64, r1: f64) -> Self {
        Gradient { kind: GradientKind::Radial { cx0, cy0, r0, cx1, cy1, r1 }, stops: Vec::new() }
    }

    pub fn add_color_stop_rgba(&mut self, offset: f64, r: f64, g: f64, b: f64, a: f64) -> &mut Self {
        self.stops.push((offset as f32, [r as f32, g as f32, b as f32, a.clamp(0.0, 1.0) as f32]));
        self
    }

    pub fn add_color_stop_rgb(&mut self, offset: f64, r: f64, g: f64, b: f64) -> &mut Self {
        self.add_color_stop_rgba(offset, r, g, b, 1.0)
    }

    /// Stop from a colour tuple.
    pub fn stop(&mut self, offset: f64, c: Rgb, a: f64) -> &mut Self {
        self.add_color_stop_rgba(offset, c.0, c.1, c.2, a)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Paint {
    Solid([f32; 4]),
    Gradient(Rc<Gradient>),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StrokeStyle {
    pub width: f64,
    pub cap: LineCap,
    pub join: LineJoin,
}

/// Receives finished drawing operations. Paths are given with the transform still to apply.
pub trait Sink {
    fn fill(&mut self, path: &BezPath, xf: Affine, rule: FillRule, paint: &Paint, paint_xf: Affine);
    fn stroke(&mut self, path: &BezPath, xf: Affine, style: &StrokeStyle, paint: &Paint, paint_xf: Affine);
    fn push_clip(&mut self, path: &BezPath, xf: Affine, rule: FillRule);
    /// A group that is drawn as a whole with this opacity (ended by `pop`).
    fn push_alpha(&mut self, alpha: f64, bounds: Rect, xf: Affine);
    fn pop(&mut self);
    fn picture(&mut self, pic: &Picture, xf: Affine, alpha: f64);
}

#[derive(Clone, Debug)]
pub enum Op {
    Fill { path: BezPath, xf: Affine, rule: FillRule, paint: Paint, paint_xf: Affine },
    Stroke { path: BezPath, xf: Affine, style: StrokeStyle, paint: Paint, paint_xf: Affine },
    PushClip { path: BezPath, xf: Affine, rule: FillRule },
    PushAlpha { alpha: f64, bounds: Rect, xf: Affine },
    Pop,
    Picture { pic: Picture, xf: Affine, alpha: f64 },
}

#[derive(Debug)]
pub struct PictureData {
    pub id: u64,
    pub ops: Vec<Op>,
    pub width: f64,
    pub height: f64,
}

/// A recorded drawing that can be shown again cheaply (replaces the Python version's cached image
/// surfaces: static backgrounds, light rays, ...). The GPU renderer keeps it encoded.
#[derive(Clone, Debug)]
pub struct Picture(pub Rc<PictureData>);

static NEXT_PICTURE: AtomicU64 = AtomicU64::new(1);

impl Picture {
    /// Record what `draw` paints. `width`/`height` is the area it covers (used for opacity groups).
    pub fn record(width: f64, height: f64, draw: impl FnOnce(&mut Canvas)) -> Picture {
        let mut rec = Recorder { ops: Vec::new() };
        {
            let mut cr = Canvas::new(&mut rec);
            draw(&mut cr);
            cr.finish();
        }
        Picture(Rc::new(PictureData { id: NEXT_PICTURE.fetch_add(1, Ordering::Relaxed), ops: rec.ops, width, height }))
    }

    pub fn id(&self) -> u64 {
        self.0.id
    }

    /// Send the recorded operations to a sink (used by renderers that do not cache pictures).
    pub fn replay(&self, sink: &mut dyn Sink, outer: Affine) {
        for op in &self.0.ops {
            match op {
                Op::Fill { path, xf, rule, paint, paint_xf } => sink.fill(path, outer * *xf, *rule, paint, *paint_xf),
                Op::Stroke { path, xf, style, paint, paint_xf } => {
                    sink.stroke(path, outer * *xf, style, paint, *paint_xf)
                }
                Op::PushClip { path, xf, rule } => sink.push_clip(path, outer * *xf, *rule),
                Op::PushAlpha { alpha, bounds, xf } => sink.push_alpha(*alpha, *bounds, outer * *xf),
                Op::Pop => sink.pop(),
                Op::Picture { pic, xf, alpha } => sink.picture(pic, outer * *xf, *alpha),
            }
        }
    }
}

struct Recorder {
    ops: Vec<Op>,
}

impl Sink for Recorder {
    fn fill(&mut self, path: &BezPath, xf: Affine, rule: FillRule, paint: &Paint, paint_xf: Affine) {
        self.ops.push(Op::Fill { path: path.clone(), xf, rule, paint: paint.clone(), paint_xf });
    }
    fn stroke(&mut self, path: &BezPath, xf: Affine, style: &StrokeStyle, paint: &Paint, paint_xf: Affine) {
        self.ops.push(Op::Stroke { path: path.clone(), xf, style: *style, paint: paint.clone(), paint_xf });
    }
    fn push_clip(&mut self, path: &BezPath, xf: Affine, rule: FillRule) {
        self.ops.push(Op::PushClip { path: path.clone(), xf, rule });
    }
    fn push_alpha(&mut self, alpha: f64, bounds: Rect, xf: Affine) {
        self.ops.push(Op::PushAlpha { alpha, bounds, xf });
    }
    fn pop(&mut self) {
        self.ops.push(Op::Pop);
    }
    fn picture(&mut self, pic: &Picture, xf: Affine, alpha: f64) {
        self.ops.push(Op::Picture { pic: pic.clone(), xf, alpha });
    }
}

#[derive(Clone)]
struct State {
    ctm: Affine,
    paint: Paint,
    paint_xf: Affine,
    line_width: f64,
    cap: LineCap,
    join: LineJoin,
    rule: FillRule,
    layers: usize,
}

pub struct Canvas<'a> {
    sink: &'a mut dyn Sink,
    st: State,
    stack: Vec<State>,
    path: BezPath,
    /// current point and start of the current sub-path, in device space
    cur: Option<Point>,
    start: Option<Point>,
}

impl<'a> Canvas<'a> {
    pub fn new(sink: &'a mut dyn Sink) -> Self {
        Canvas {
            sink,
            st: State {
                ctm: Affine::IDENTITY,
                paint: Paint::Solid([0.0, 0.0, 0.0, 1.0]),
                paint_xf: Affine::IDENTITY,
                line_width: 2.0,
                cap: LineCap::Butt,
                join: LineJoin::Miter,
                rule: FillRule::Winding,
                layers: 0,
            },
            stack: Vec::new(),
            path: BezPath::new(),
            cur: None,
            start: None,
        }
    }

    /// Close everything that is still open (clips, groups). Called once at the end of a frame.
    pub fn finish(&mut self) {
        while !self.stack.is_empty() {
            self.restore();
        }
        while self.st.layers > 0 {
            self.sink.pop();
            self.st.layers -= 1;
        }
    }

    // ------------------------------------------------------------------ state
    pub fn save(&mut self) {
        self.stack.push(self.st.clone());
    }

    pub fn restore(&mut self) {
        if let Some(prev) = self.stack.pop() {
            while self.st.layers > prev.layers {
                self.sink.pop();
                self.st.layers -= 1;
            }
            self.st = prev;
        }
    }

    pub fn translate(&mut self, x: f64, y: f64) {
        self.st.ctm *= Affine::translate((x, y));
    }

    pub fn scale(&mut self, sx: f64, sy: f64) {
        // a zero scale would make the matrix unusable (Cairo raises an error); keep it tiny instead
        let fix = |v: f64| if v == 0.0 { 1e-9 } else { v };
        self.st.ctm *= Affine::scale_non_uniform(fix(sx), fix(sy));
    }

    pub fn rotate(&mut self, angle: f64) {
        self.st.ctm *= Affine::rotate(angle);
    }

    pub fn transform(&mut self, m: Affine) {
        self.st.ctm *= m;
    }

    pub fn matrix(&self) -> Affine {
        self.st.ctm
    }

    pub fn set_matrix(&mut self, m: Affine) {
        self.st.ctm = m;
    }

    // ------------------------------------------------------------------ source
    pub fn set_source_rgba(&mut self, r: f64, g: f64, b: f64, a: f64) {
        self.st.paint = Paint::Solid([r as f32, g as f32, b as f32, a.clamp(0.0, 1.0) as f32]);
    }

    pub fn set_source_rgb(&mut self, r: f64, g: f64, b: f64) {
        self.set_source_rgba(r, g, b, 1.0);
    }

    /// Colour tuple + opacity (short for `set_source_rgba(c[0], c[1], c[2], a)`).
    pub fn set_rgba(&mut self, c: Rgb, a: f64) {
        self.set_source_rgba(c.0, c.1, c.2, a);
    }

    pub fn set_rgb(&mut self, c: Rgb) {
        self.set_source_rgba(c.0, c.1, c.2, 1.0);
    }

    pub fn set_source(&mut self, g: &Gradient) {
        self.st.paint = Paint::Gradient(Rc::new(g.clone()));
        self.st.paint_xf = self.st.ctm;
    }

    pub fn set_line_width(&mut self, w: f64) {
        self.st.line_width = w;
    }

    pub fn set_line_cap(&mut self, cap: LineCap) {
        self.st.cap = cap;
    }

    pub fn set_line_join(&mut self, join: LineJoin) {
        self.st.join = join;
    }

    pub fn set_fill_rule(&mut self, rule: FillRule) {
        self.st.rule = rule;
    }

    // ------------------------------------------------------------------ path
    fn dev(&self, x: f64, y: f64) -> Point {
        self.st.ctm * Point::new(x, y)
    }

    pub fn new_path(&mut self) {
        self.path.truncate(0);
        self.cur = None;
        self.start = None;
    }

    pub fn new_sub_path(&mut self) {
        self.cur = None;
    }

    pub fn move_to(&mut self, x: f64, y: f64) {
        let p = self.dev(x, y);
        self.path.move_to(p);
        self.cur = Some(p);
        self.start = Some(p);
    }

    pub fn line_to(&mut self, x: f64, y: f64) {
        if self.cur.is_none() {
            return self.move_to(x, y);
        }
        let p = self.dev(x, y);
        self.path.line_to(p);
        self.cur = Some(p);
    }

    pub fn curve_to(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, x3: f64, y3: f64) {
        if self.cur.is_none() {
            self.move_to(x1, y1);
        }
        let p = self.dev(x3, y3);
        self.path.curve_to(self.dev(x1, y1), self.dev(x2, y2), p);
        self.cur = Some(p);
    }

    /// Quadratic curve (not in Cairo, handy for new code).
    pub fn quad_to(&mut self, x1: f64, y1: f64, x2: f64, y2: f64) {
        if self.cur.is_none() {
            self.move_to(x1, y1);
        }
        let p = self.dev(x2, y2);
        self.path.quad_to(self.dev(x1, y1), p);
        self.cur = Some(p);
    }

    pub fn rel_line_to(&mut self, dx: f64, dy: f64) {
        if let Some(c) = self.cur {
            let u = self.st.ctm.inverse() * c;
            self.line_to(u.x + dx, u.y + dy);
        }
    }

    pub fn close_path(&mut self) {
        if self.cur.is_some() {
            self.path.close_path();
            // like Cairo: after closing, the current point is the start of the sub-path
            if let Some(s) = self.start {
                self.path.move_to(s);
                self.cur = Some(s);
            }
        }
    }

    pub fn rectangle(&mut self, x: f64, y: f64, w: f64, h: f64) {
        self.move_to(x, y);
        self.line_to(x + w, y);
        self.line_to(x + w, y + h);
        self.line_to(x, y + h);
        self.close_path();
    }

    /// Arc with increasing angle (clockwise on screen), like `cairo_arc`.
    pub fn arc(&mut self, xc: f64, yc: f64, r: f64, a0: f64, mut a1: f64) {
        if !(a0.is_finite() && a1.is_finite() && r.is_finite()) {
            return;
        }
        if a1 < a0 {
            a1 += ((a0 - a1) / TAU).ceil() * TAU;
        }
        self.arc_segments(xc, yc, r, a0, a1.min(a0 + TAU * 2.0));
    }

    /// Arc with decreasing angle, like `cairo_arc_negative`.
    pub fn arc_negative(&mut self, xc: f64, yc: f64, r: f64, a0: f64, mut a1: f64) {
        if !(a0.is_finite() && a1.is_finite() && r.is_finite()) {
            return;
        }
        if a1 > a0 {
            a1 -= ((a1 - a0) / TAU).ceil() * TAU;
        }
        self.arc_segments(xc, yc, r, a0, a1.max(a0 - TAU * 2.0));
    }

    fn arc_segments(&mut self, xc: f64, yc: f64, r: f64, a0: f64, a1: f64) {
        let (sx, sy) = (xc + r * a0.cos(), yc + r * a0.sin());
        if self.cur.is_some() {
            self.line_to(sx, sy);
        } else {
            self.move_to(sx, sy);
        }
        let sweep = a1 - a0;
        if sweep == 0.0 || r == 0.0 {
            return;
        }
        let n = (sweep.abs() / (TAU / 4.0)).ceil().max(1.0) as usize;
        let step = sweep / n as f64;
        let k = 4.0 / 3.0 * (step / 4.0).tan();
        let mut a = a0;
        for _ in 0..n {
            let b = a + step;
            let (ca, sa, cb, sb) = (a.cos(), a.sin(), b.cos(), b.sin());
            self.curve_to(
                xc + r * (ca - k * sa),
                yc + r * (sa + k * ca),
                xc + r * (cb + k * sb),
                yc + r * (sb - k * cb),
                xc + r * cb,
                yc + r * sb,
            );
            a = b;
        }
    }

    /// Full circle as its own closed sub-path.
    pub fn circle(&mut self, x: f64, y: f64, r: f64) {
        self.new_sub_path();
        self.arc(x, y, r, 0.0, TAU);
        self.close_path();
        self.new_sub_path();
    }

    /// Ellipse with radii rx, ry as its own closed sub-path (replaces the save/translate/scale/arc/restore idiom).
    pub fn ellipse(&mut self, x: f64, y: f64, rx: f64, ry: f64) {
        self.save();
        self.translate(x, y);
        self.scale(rx, ry);
        self.new_sub_path();
        self.arc(0.0, 0.0, 1.0, 0.0, TAU);
        self.close_path();
        self.new_sub_path();
        self.restore();
    }

    pub fn rounded_rect(&mut self, x: f64, y: f64, w: f64, h: f64, r: f64) {
        use std::f64::consts::PI;
        let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
        self.new_sub_path();
        self.arc(x + w - r, y + r, r, -PI / 2.0, 0.0);
        self.arc(x + w - r, y + h - r, r, 0.0, PI / 2.0);
        self.arc(x + r, y + h - r, r, PI / 2.0, PI);
        self.arc(x + r, y + r, r, PI, 1.5 * PI);
        self.close_path();
    }

    pub fn has_current_point(&self) -> bool {
        self.cur.is_some()
    }

    // ------------------------------------------------------------------ painting
    pub fn fill_preserve(&mut self) {
        if !self.path.elements().is_empty() {
            self.sink.fill(&self.path, Affine::IDENTITY, self.st.rule, &self.st.paint, self.st.paint_xf);
        }
    }

    pub fn fill(&mut self) {
        self.fill_preserve();
        self.new_path();
    }

    pub fn stroke_preserve(&mut self) {
        if self.path.elements().is_empty() || self.st.line_width <= 0.0 {
            return;
        }
        let style = StrokeStyle { width: self.st.line_width, cap: self.st.cap, join: self.st.join };
        // the pen has the shape given by the matrix now: stroke in user space, then transform
        let c = self.st.ctm.as_coeffs();
        let lin = Affine::new([c[0], c[1], c[2], c[3], 0.0, 0.0]);
        if lin.determinant().abs() < 1e-12 {
            return;
        }
        let uniform = (c[0] - c[3]).abs() < 1e-9 && (c[1] + c[2]).abs() < 1e-9;
        if uniform {
            let k = (c[0] * c[0] + c[1] * c[1]).sqrt();
            let style = StrokeStyle { width: style.width * k, ..style };
            self.sink.stroke(&self.path, Affine::IDENTITY, &style, &self.st.paint, self.st.paint_xf);
        } else {
            let local = lin.inverse() * self.path.clone();
            self.sink.stroke(&local, lin, &style, &self.st.paint, lin.inverse() * self.st.paint_xf);
        }
    }

    pub fn stroke(&mut self) {
        self.stroke_preserve();
        self.new_path();
    }

    /// Fill everything (the current clip) with the source.
    pub fn paint(&mut self) {
        let big = BezPath::from_vec(vec![
            PathEl::MoveTo(Point::new(-1e5, -1e5)),
            PathEl::LineTo(Point::new(1e5, -1e5)),
            PathEl::LineTo(Point::new(1e5, 1e5)),
            PathEl::LineTo(Point::new(-1e5, 1e5)),
            PathEl::ClosePath,
        ]);
        self.sink.fill(&big, Affine::IDENTITY, FillRule::Winding, &self.st.paint, self.st.paint_xf);
    }

    pub fn clip_preserve(&mut self) {
        self.sink.push_clip(&self.path, Affine::IDENTITY, self.st.rule);
        self.st.layers += 1;
    }

    /// Clip to the current path until the matching `restore()`.
    pub fn clip(&mut self) {
        self.clip_preserve();
        self.new_path();
    }

    /// Everything drawn until the matching `restore()` is blended as one group with this opacity
    /// (Cairo: push_group ... pop_group_to_source, paint_with_alpha). `x, y, w, h` is the area in
    /// user space the group may cover. Call `save()` first.
    pub fn group(&mut self, alpha: f64, x: f64, y: f64, w: f64, h: f64) {
        self.sink.push_alpha(alpha, Rect::new(x, y, x + w, y + h), self.st.ctm);
        self.st.layers += 1;
    }

    /// Show a recorded picture with its origin at (x, y) in user space.
    pub fn picture(&mut self, pic: &Picture, x: f64, y: f64, alpha: f64) {
        if alpha > 0.002 {
            self.sink.picture(pic, self.st.ctm * Affine::translate((x, y)), alpha.min(1.0));
        }
    }

    /// Fill a ready-made path (in its own coordinates) placed at (x, y) in user space - used for text.
    pub fn fill_path_at(&mut self, path: &BezPath, x: f64, y: f64, c: Rgb, alpha: f64) {
        if alpha <= 0.002 || path.elements().is_empty() {
            return;
        }
        let paint = Paint::Solid([c.0 as f32, c.1 as f32, c.2 as f32, alpha.clamp(0.0, 1.0) as f32]);
        self.sink.fill(path, self.st.ctm * Affine::translate((x, y)), FillRule::Winding, &paint, Affine::IDENTITY);
    }

    // ------------------------------------------------------------------ shared shapes
    /// Soft radial glow: colour c, opacity `alpha` in the centre fading linearly to 0 at radius r.
    /// `inner` (0..1, fraction of r) keeps the opacity flat up to that radius before it fades.
    pub fn glow(&mut self, x: f64, y: f64, r: f64, c: Rgb, alpha: f64, inner: f64) {
        if alpha <= 0.004 || r < 1.0 {
            return;
        }
        let mut g = Gradient::radial(x, y, 0.0, x, y, r);
        g.stop(0.0, c, alpha);
        if inner > 0.0 {
            g.stop(inner, c, alpha);
        }
        g.stop(1.0, c, 0.0);
        self.save();
        self.set_source(&g);
        self.new_path();
        self.arc(x, y, r, 0.0, TAU);
        self.fill();
        self.restore();
    }

    /// Expanding rings used to mark a session that needs you.
    #[allow(clippy::too_many_arguments)]
    pub fn pulse_rings(&mut self, x: f64, y: f64, r: f64, c: Rgb, a: f64, t: f64, s: f64) {
        for k in 0..2 {
            let f = ((t * 0.6) + k as f64 * 0.5).rem_euclid(1.0);
            self.set_rgba(c, (1.0 - f) * 0.55 * a);
            self.set_line_width(3.0 * s * (1.0 - f) + 0.5);
            self.new_sub_path();
            self.arc(x, y, r * (1.2 + 2.0 * f), 0.0, TAU);
            self.stroke();
        }
    }
}
