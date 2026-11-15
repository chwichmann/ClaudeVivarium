//! Drawing on the CPU (tiny-skia) for computers without a usable graphics card: virtual machines,
//! remote desktop sessions, or a system that only offers a software stand-in for a GPU. Slower than the
//! GPU renderer, about like the Python version, but it works everywhere.
// plain chunk loops are the clearest way to walk RGBA pixels here
#![allow(clippy::chunks_exact_to_as_chunks)]

use std::collections::HashMap;
use std::num::NonZeroU32;
use std::sync::Arc;

use tiny_skia as ts;
use vello::kurbo::{Affine, BezPath, PathEl, Rect};
use vivarium_core::canvas::{FillRule, GradientKind, LineCap, LineJoin, Paint, Picture, Sink, StrokeStyle};
use winit::window::Window;

/// Above this many pixels a window is drawn at half size and doubled (keeps big screens affordable).
const MAX_PIXELS: u32 = 2_600_000;

fn xf(a: Affine) -> ts::Transform {
    let c = a.as_coeffs();
    ts::Transform::from_row(c[0] as f32, c[1] as f32, c[2] as f32, c[3] as f32, c[4] as f32, c[5] as f32)
}

fn path_of(p: &BezPath) -> Option<ts::Path> {
    let mut b = ts::PathBuilder::new();
    for el in p.elements() {
        match *el {
            PathEl::MoveTo(a) => b.move_to(a.x as f32, a.y as f32),
            PathEl::LineTo(a) => b.line_to(a.x as f32, a.y as f32),
            PathEl::QuadTo(a, c) => b.quad_to(a.x as f32, a.y as f32, c.x as f32, c.y as f32),
            PathEl::CurveTo(a, c, d) => b.cubic_to(a.x as f32, a.y as f32, c.x as f32, c.y as f32, d.x as f32, d.y as f32),
            PathEl::ClosePath => b.close(),
        }
    }
    b.finish()
}

fn rule_of(rule: FillRule) -> ts::FillRule {
    match rule {
        FillRule::Winding => ts::FillRule::Winding,
        FillRule::EvenOdd => ts::FillRule::EvenOdd,
    }
}

fn color_of(c: &[f32; 4]) -> ts::Color {
    let f = |v: f32| if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 };
    ts::Color::from_rgba(f(c[0]), f(c[1]), f(c[2]), f(c[3])).unwrap_or(ts::Color::BLACK)
}

/// The paint for a shape drawn with path transform `path_xf` (tiny-skia moves the shader along with
/// the path, so the gradient is given relative to the path).
fn paint_of(paint: &Paint, path_xf: Affine, paint_xf: Affine) -> Option<ts::Paint<'static>> {
    let shader = match paint {
        Paint::Solid(c) => ts::Shader::SolidColor(color_of(c)),
        Paint::Gradient(g) => {
            let mut stops: Vec<(f32, ts::Color)> = g.stops.iter().map(|(o, c)| (o.clamp(0.0, 1.0), color_of(c))).collect();
            stops.sort_by(|a, b| a.0.total_cmp(&b.0));
            let last = stops.last().map(|s| s.1)?;
            let stops: Vec<ts::GradientStop> = stops.into_iter().map(|(o, c)| ts::GradientStop::new(o, c)).collect();
            let t = xf(path_xf.inverse() * paint_xf);
            let made = match g.kind {
                GradientKind::Linear { x0, y0, x1, y1 } => ts::LinearGradient::new(
                    ts::Point::from_xy(x0 as f32, y0 as f32),
                    ts::Point::from_xy(x1 as f32, y1 as f32),
                    stops,
                    ts::SpreadMode::Pad,
                    t,
                ),
                GradientKind::Radial { cx0, cy0, r0, cx1, cy1, r1 } => ts::RadialGradient::new(
                    ts::Point::from_xy(cx0 as f32, cy0 as f32),
                    r0 as f32,
                    ts::Point::from_xy(cx1 as f32, cy1 as f32),
                    r1 as f32,
                    stops,
                    ts::SpreadMode::Pad,
                    t,
                ),
            };
            // a gradient without extent is its last colour
            made.unwrap_or(ts::Shader::SolidColor(last))
        }
    };
    Some(ts::Paint { shader, anti_alias: true, ..Default::default() })
}

enum Open {
    Clip(Option<ts::Mask>),
    Alpha(f32),
}

struct CachedPicture {
    pixmap: ts::Pixmap,
    /// no transparent pixel: can be copied without blending
    opaque: bool,
    used: u64,
}

/// Copy `src` onto `dst` at whole-pixel position (x, y), blending with `alpha` (both images hold
/// premultiplied RGBA). Much faster than the general image drawing for the big cached backgrounds.
fn blit(dst: &mut ts::Pixmap, src: &ts::Pixmap, opaque: bool, x: i64, y: i64, alpha: f32) {
    let (dw, dh, sw, sh) = (dst.width() as i64, dst.height() as i64, src.width() as i64, src.height() as i64);
    let (x0, y0) = (x.max(0), y.max(0));
    let (x1, y1) = ((x + sw).min(dw), (y + sh).min(dh));
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    let n = ((x1 - x0) * 4) as usize;
    let a = (alpha.clamp(0.0, 1.0) * 256.0) as u32;
    let (sdata, ddata) = (src.data(), dst.data_mut());
    for row in y0..y1 {
        let so = (((row - y) * sw + (x0 - x)) * 4) as usize;
        let d0 = ((row * dw + x0) * 4) as usize;
        let (srow, drow) = (&sdata[so..so + n], &mut ddata[d0..d0 + n]);
        if opaque && a >= 256 {
            drow.copy_from_slice(srow);
            continue;
        }
        // branch-free so the compiler can process several pixels at once
        let div255 = |v: u32| (v + 128 + ((v + 128) >> 8)) >> 8;
        if a >= 256 {
            for (d, s) in drow.chunks_exact_mut(4).zip(srow.chunks_exact(4)) {
                let inv = 255 - s[3] as u32;
                for k in 0..4 {
                    d[k] = (s[k] as u32 + div255(d[k] as u32 * inv)).min(255) as u8;
                }
            }
        } else {
            for (d, s) in drow.chunks_exact_mut(4).zip(srow.chunks_exact(4)) {
                let inv = 255 - ((s[3] as u32 * a) >> 8);
                for k in 0..4 {
                    d[k] = (((s[k] as u32 * a) >> 8) + div255(d[k] as u32 * inv)).min(255) as u8;
                }
            }
        }
    }
}

/// Collects one frame into a pixel image.
pub struct CpuSink {
    /// the image being drawn into is the last one (opacity groups add one on top)
    targets: Vec<ts::Pixmap>,
    mask: Option<ts::Mask>,
    open: Vec<Open>,
    pics: HashMap<(u64, u32, u32), CachedPicture>,
    frame: u64,
}

impl Default for CpuSink {
    fn default() -> Self {
        Self::new()
    }
}

impl CpuSink {
    pub fn new() -> Self {
        CpuSink { targets: Vec::new(), mask: None, open: Vec::new(), pics: HashMap::new(), frame: 0 }
    }

    /// Start a new frame of this size, black.
    pub fn begin(&mut self, width: u32, height: u32) {
        let (width, height) = (width.max(1), height.max(1));
        self.targets.truncate(1);
        let reuse = self.targets.first().is_some_and(|p| p.width() == width && p.height() == height);
        if !reuse {
            self.targets.clear();
            self.targets.push(ts::Pixmap::new(width, height).expect("size is at least 1x1"));
        }
        // transparent black: cheaper to clear than opaque black, and the same picture once shown
        // (the colours are stored already multiplied by their opacity)
        self.targets[0].data_mut().fill(0);
        self.mask = None;
        self.open.clear();
        self.frame += 1;
        let frame = self.frame;
        if frame.is_multiple_of(64) {
            self.pics.retain(|_, p| frame - p.used < 240);
        }
    }

    /// The finished frame as an opaque picture for a PNG file.
    pub fn opaque_pixels(&self) -> Vec<u8> {
        let mut px = self.pixels().to_vec();
        for p in px.chunks_exact_mut(4) {
            p[3] = 255;
        }
        px
    }

    /// The finished frame: RGBA over black, 4 bytes per pixel, top row first (ignore the 4th byte).
    pub fn pixels(&self) -> &[u8] {
        self.targets.first().map(|p| p.data()).unwrap_or(&[])
    }

    pub fn size(&self) -> (u32, u32) {
        self.targets.first().map(|p| (p.width(), p.height())).unwrap_or((0, 0))
    }

    fn target(&mut self) -> &mut ts::Pixmap {
        self.targets.last_mut().expect("begin() was called")
    }
}

impl Sink for CpuSink {
    fn fill(&mut self, path: &BezPath, t: Affine, rule: FillRule, paint: &Paint, paint_xf: Affine) {
        let (Some(p), Some(pt)) = (path_of(path), paint_of(paint, t, paint_xf)) else { return };
        let mask = self.mask.take();
        self.target().fill_path(&p, &pt, rule_of(rule), xf(t), mask.as_ref());
        self.mask = mask;
    }

    fn stroke(&mut self, path: &BezPath, t: Affine, style: &StrokeStyle, paint: &Paint, paint_xf: Affine) {
        let (Some(p), Some(pt)) = (path_of(path), paint_of(paint, t, paint_xf)) else { return };
        let stroke = ts::Stroke {
            width: style.width as f32,
            miter_limit: 10.0,
            line_cap: match style.cap {
                LineCap::Butt => ts::LineCap::Butt,
                LineCap::Round => ts::LineCap::Round,
                LineCap::Square => ts::LineCap::Square,
            },
            line_join: match style.join {
                LineJoin::Miter => ts::LineJoin::Miter,
                LineJoin::Round => ts::LineJoin::Round,
                LineJoin::Bevel => ts::LineJoin::Bevel,
            },
            dash: None,
        };
        let mask = self.mask.take();
        self.target().stroke_path(&p, &pt, &stroke, xf(t), mask.as_ref());
        self.mask = mask;
    }

    fn push_clip(&mut self, path: &BezPath, t: Affine, rule: FillRule) {
        let (w, h) = self.size();
        let old = self.mask.take();
        let mut mask = old.clone().or_else(|| {
            let mut m = ts::Mask::new(w, h)?;
            m.data_mut().fill(255);
            Some(m)
        });
        if let Some(m) = mask.as_mut() {
            match path_of(path) {
                Some(p) => m.intersect_path(&p, rule_of(rule), true, xf(t)),
                None => m.clear(), // an empty clip hides everything
            }
        }
        self.mask = mask;
        self.open.push(Open::Clip(old));
    }

    fn push_alpha(&mut self, alpha: f64, _bounds: Rect, _t: Affine) {
        let (w, h) = self.size();
        if let Some(layer) = ts::Pixmap::new(w, h) {
            self.targets.push(layer);
            self.open.push(Open::Alpha(alpha.clamp(0.0, 1.0) as f32));
        } else {
            self.open.push(Open::Clip(self.mask.clone())); // keeps push/pop balanced
        }
    }

    fn pop(&mut self) {
        match self.open.pop() {
            Some(Open::Clip(old)) => self.mask = old,
            Some(Open::Alpha(alpha)) => {
                if self.targets.len() < 2 {
                    return;
                }
                let layer = self.targets.pop().expect("checked above");
                let paint = ts::PixmapPaint { opacity: alpha, ..Default::default() };
                let mask = self.mask.take();
                self.target().draw_pixmap(0, 0, layer.as_ref(), &paint, ts::Transform::identity(), mask.as_ref());
                self.mask = mask;
            }
            None => {}
        }
    }

    /// Pictures (static backgrounds, the monitor panel) are drawn once into an image and then copied.
    fn picture(&mut self, pic: &Picture, t: Affine, alpha: f64) {
        let c = t.as_coeffs();
        let straight = c[1].abs() < 1e-9 && c[2].abs() < 1e-9 && c[0] > 0.0 && c[3] > 0.0;
        let (pw, ph) = ((pic.0.width * c[0]).ceil(), (pic.0.height * c[3]).ceil());
        if !straight || !(1.0..=8192.0).contains(&pw) || !(1.0..=8192.0).contains(&ph) {
            if alpha < 0.999 {
                self.push_alpha(alpha, Rect::ZERO, t);
                pic.replay(self, t);
                self.pop();
            } else {
                pic.replay(self, t);
            }
            return;
        }
        let key = (pic.id(), (c[0] as f32).to_bits(), (c[3] as f32).to_bits());
        if !self.pics.contains_key(&key) {
            let Some(pixmap) = ts::Pixmap::new(pw as u32, ph as u32) else { return };
            let mut sub =
                CpuSink { targets: vec![pixmap], mask: None, open: Vec::new(), pics: std::mem::take(&mut self.pics), frame: self.frame };
            pic.replay(&mut sub, Affine::scale_non_uniform(c[0], c[3]));
            self.pics = sub.pics;
            if let Some(pixmap) = sub.targets.into_iter().next() {
                let opaque = pixmap.data().chunks_exact(4).all(|px| px[3] == 255);
                self.pics.insert(key, CachedPicture { pixmap, opaque, used: self.frame });
            }
        }
        let mask = self.mask.take();
        let frame = self.frame;
        if let Some(entry) = self.pics.get_mut(&key) {
            entry.used = frame;
            let whole = c[4].fract() == 0.0 && c[5].fract() == 0.0;
            if whole && mask.is_none() {
                let target = self.targets.last_mut().expect("begin() was called");
                blit(target, &entry.pixmap, entry.opaque, c[4] as i64, c[5] as i64, alpha as f32);
                self.mask = mask;
                return;
            }
            let paint = ts::PixmapPaint {
                opacity: alpha.clamp(0.0, 1.0) as f32,
                quality: if whole { ts::FilterQuality::Nearest } else { ts::FilterQuality::Bilinear },
                ..Default::default()
            };
            let place = ts::Transform::from_translate(c[4] as f32, c[5] as f32);
            let target = self.targets.last_mut().expect("begin() was called");
            target.draw_pixmap(0, 0, entry.pixmap.as_ref(), &paint, place, mask.as_ref());
        }
        self.mask = mask;
    }
}

/// A window that is filled from the CPU renderer.
pub struct CpuScreen {
    surface: softbuffer::Surface<Arc<Window>, Arc<Window>>,
    pub sink: CpuSink,
    /// 1 = drawn at full size, 2 = drawn at half size and doubled
    factor: u32,
}

impl CpuScreen {
    pub fn new(window: Arc<Window>) -> Result<Self, String> {
        let context = softbuffer::Context::new(window.clone()).map_err(|e| format!("software display: {e}"))?;
        let surface = softbuffer::Surface::new(&context, window).map_err(|e| format!("software display: {e}"))?;
        Ok(CpuScreen { surface, sink: CpuSink::new(), factor: 1 })
    }

    /// Start a frame for a window of this size; returns the size to draw the scene at.
    pub fn begin(&mut self, width: u32, height: u32) -> (u32, u32) {
        self.factor = if width.saturating_mul(height) > MAX_PIXELS { 2 } else { 1 };
        let (w, h) = ((width / self.factor).max(1), (height / self.factor).max(1));
        self.sink.begin(w, h);
        (w, h)
    }

    /// Show the finished frame in the window.
    pub fn present(&mut self, width: u32, height: u32) -> Result<(), String> {
        let (Some(nw), Some(nh)) = (NonZeroU32::new(width), NonZeroU32::new(height)) else { return Ok(()) };
        self.surface.resize(nw, nh).map_err(|e| format!("software display: {e}"))?;
        let mut buffer = self.surface.buffer_mut().map_err(|e| format!("software display: {e}"))?;
        let (sw, sh) = self.sink.size();
        let src = self.sink.pixels();
        let f = self.factor as usize;
        let (width, height, sw, sh) = (width as usize, height as usize, sw as usize, sh as usize);
        for y in 0..height {
            let sy = (y / f).min(sh.saturating_sub(1));
            let row = &src[sy * sw * 4..(sy + 1) * sw * 4];
            let out = &mut buffer[y * width..(y + 1) * width];
            if f == 1 {
                for (o, px) in out.iter_mut().zip(row.chunks_exact(4)) {
                    *o = ((px[0] as u32) << 16) | ((px[1] as u32) << 8) | px[2] as u32;
                }
            } else {
                for (x, o) in out.iter_mut().enumerate() {
                    let px = &row[(x / f).min(sw.saturating_sub(1)) * 4..];
                    *o = ((px[0] as u32) << 16) | ((px[1] as u32) << 8) | px[2] as u32;
                }
            }
        }
        buffer.present().map_err(|e| format!("software display: {e}"))
    }
}
