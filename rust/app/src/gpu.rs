//! Drawing on the GPU with vello (Metal on macOS, Direct3D 12 on Windows, Vulkan on Linux via wgpu).
//!
//! `VelloSink` turns the Canvas operations of one frame into a vello scene; `Gpu` owns the devices and
//! renders a scene into a window surface or (without any window) into pixels for PNG files.
use std::collections::HashMap;

use vello::kurbo::{self, Affine, BezPath, Rect};
use vello::peniko::{self, Brush, Color, Fill, Mix};
use vello::util::{RenderContext, RenderSurface};
use vello::wgpu;
use vello::{AaConfig, AaSupport, RenderParams, Renderer, RendererOptions};
use vivarium_core::canvas::{FillRule, GradientKind, LineCap, LineJoin, Paint, Picture, Sink, StrokeStyle};

struct CachedPicture {
    scene: vello::Scene,
    used: u64,
}

/// Collects one frame. Recorded pictures (static backgrounds, the monitor panel) stay encoded between
/// frames, so showing them again costs a copy instead of new drawing work.
pub struct VelloSink {
    pub scene: vello::Scene,
    pics: HashMap<u64, CachedPicture>,
    frame: u64,
}

impl Default for VelloSink {
    fn default() -> Self {
        Self::new()
    }
}

impl VelloSink {
    pub fn new() -> Self {
        VelloSink { scene: vello::Scene::new(), pics: HashMap::new(), frame: 0 }
    }

    /// Start a new frame (forgets pictures that were not shown for a while).
    pub fn begin(&mut self) {
        self.scene.reset();
        self.frame += 1;
        let frame = self.frame;
        if frame.is_multiple_of(64) {
            self.pics.retain(|_, p| frame - p.used < 240);
        }
    }
}

fn fill_of(rule: FillRule) -> Fill {
    match rule {
        FillRule::Winding => Fill::NonZero,
        FillRule::EvenOdd => Fill::EvenOdd,
    }
}

fn brush_of(paint: &Paint) -> Brush {
    match paint {
        Paint::Solid(c) => Brush::Solid(Color::new(*c)),
        Paint::Gradient(g) => {
            let mut stops: Vec<peniko::ColorStop> =
                g.stops.iter().map(|(o, c)| peniko::ColorStop::from((o.clamp(0.0, 1.0), Color::new(*c)))).collect();
            stops.sort_by(|a, b| a.offset.partial_cmp(&b.offset).unwrap_or(std::cmp::Ordering::Equal));
            let grad = match g.kind {
                GradientKind::Linear { x0, y0, x1, y1 } => peniko::Gradient::new_linear((x0, y0), (x1, y1)),
                GradientKind::Radial { cx0, cy0, r0, cx1, cy1, r1 } => {
                    peniko::Gradient::new_two_point_radial((cx0, cy0), r0 as f32, (cx1, cy1), r1 as f32)
                }
            };
            Brush::Gradient(grad.with_stops(stops.as_slice()))
        }
    }
}

fn brush_xf(paint: &Paint, xf: Affine, paint_xf: Affine) -> Option<Affine> {
    match paint {
        Paint::Solid(_) => None,
        // vello positions the brush by (path transform * brush transform); we want paint_xf in the end
        Paint::Gradient(_) => Some(xf.inverse() * paint_xf),
    }
}

impl Sink for VelloSink {
    fn fill(&mut self, path: &BezPath, xf: Affine, rule: FillRule, paint: &Paint, paint_xf: Affine) {
        let brush = brush_of(paint);
        self.scene.fill(fill_of(rule), xf, &brush, brush_xf(paint, xf, paint_xf), path);
    }

    fn stroke(&mut self, path: &BezPath, xf: Affine, style: &StrokeStyle, paint: &Paint, paint_xf: Affine) {
        let cap = match style.cap {
            LineCap::Butt => kurbo::Cap::Butt,
            LineCap::Round => kurbo::Cap::Round,
            LineCap::Square => kurbo::Cap::Square,
        };
        let join = match style.join {
            LineJoin::Miter => kurbo::Join::Miter,
            LineJoin::Round => kurbo::Join::Round,
            LineJoin::Bevel => kurbo::Join::Bevel,
        };
        let mut st = kurbo::Stroke::new(style.width).with_caps(cap).with_join(join);
        st.miter_limit = 10.0; // Cairo's default
        let brush = brush_of(paint);
        self.scene.stroke(&st, xf, &brush, brush_xf(paint, xf, paint_xf), path);
    }

    fn push_clip(&mut self, path: &BezPath, xf: Affine, rule: FillRule) {
        self.scene.push_clip_layer(fill_of(rule), xf, path);
    }

    fn push_alpha(&mut self, alpha: f64, bounds: Rect, xf: Affine) {
        self.scene.push_layer(Fill::NonZero, Mix::Normal, alpha as f32, xf, &bounds);
    }

    fn pop(&mut self) {
        self.scene.pop_layer();
    }

    fn picture(&mut self, pic: &Picture, xf: Affine, alpha: f64) {
        let id = pic.id();
        if !self.pics.contains_key(&id) {
            let mut sub = VelloSink { scene: vello::Scene::new(), pics: std::mem::take(&mut self.pics), frame: self.frame };
            pic.replay(&mut sub, Affine::IDENTITY);
            self.pics = sub.pics;
            self.pics.insert(id, CachedPicture { scene: sub.scene, used: self.frame });
        }
        let Some(entry) = self.pics.get_mut(&id) else { return };
        entry.used = self.frame;
        if alpha < 0.999 {
            let bounds = Rect::new(0.0, 0.0, pic.0.width, pic.0.height);
            self.scene.push_layer(Fill::NonZero, Mix::Normal, alpha as f32, xf, &bounds);
            self.scene.append(&entry.scene, Some(xf));
            self.scene.pop_layer();
        } else {
            self.scene.append(&entry.scene, Some(xf));
        }
    }
}

pub struct Gpu {
    pub ctx: RenderContext,
    renderers: Vec<Option<Renderer>>,
}

fn params(width: u32, height: u32) -> RenderParams {
    RenderParams { base_color: Color::new([0.0, 0.0, 0.0, 1.0]), width, height, antialiasing_method: AaConfig::Area }
}

impl Gpu {
    pub fn new() -> Self {
        Gpu { ctx: RenderContext::new(), renderers: Vec::new() }
    }

    fn renderer(&mut self, dev_id: usize) -> Result<&mut Renderer, String> {
        if self.renderers.len() <= dev_id {
            self.renderers.resize_with(dev_id + 1, || None);
        }
        if self.renderers[dev_id].is_none() {
            let opts = RendererOptions { antialiasing_support: AaSupport::area_only(), ..Default::default() };
            let r = Renderer::new(&self.ctx.devices[dev_id].device, opts).map_err(|e| format!("GPU renderer: {e}"))?;
            self.renderers[dev_id] = Some(r);
        }
        Ok(self.renderers[dev_id].as_mut().expect("just created"))
    }

    /// Name of the graphics adapter in use (for the log).
    pub fn adapter_name(&self, dev_id: usize) -> String {
        let info = self.ctx.devices[dev_id].adapter().get_info();
        format!("{} ({:?})", info.name, info.backend)
    }

    pub fn create_surface<'w>(
        &mut self,
        target: impl Into<wgpu::SurfaceTarget<'w>>,
        width: u32,
        height: u32,
    ) -> Result<RenderSurface<'w>, String> {
        // Linux: with vsync, presenting can block for as long as the window is not shown (e.g. once the
        // screen is locked), and a blocked loop could never notice the lock. Frames are paced by our own
        // timer anyway, so the newest frame is simply handed over without waiting.
        let mode = if cfg!(target_os = "linux") { wgpu::PresentMode::AutoNoVsync } else { wgpu::PresentMode::AutoVsync };
        let surface = pollster::block_on(self.ctx.create_surface(target, width.max(1), height.max(1), mode))
            .map_err(|e| format!("no usable graphics adapter: {e}"))?;
        self.renderer(surface.dev_id)?; // compile the shaders now, not at the first frame
        Ok(surface)
    }

    pub fn resize(&self, surface: &mut RenderSurface<'_>, width: u32, height: u32) {
        if width > 0 && height > 0 && (surface.config.width, surface.config.height) != (width, height) {
            self.ctx.resize_surface(surface, width, height);
        }
    }

    /// Draw the scene into the window. Returns false if the frame had to be skipped.
    pub fn present(&mut self, surface: &mut RenderSurface<'_>, scene: &vello::Scene) -> Result<bool, String> {
        let (w, h) = (surface.config.width, surface.config.height);
        let dev_id = surface.dev_id;
        self.renderer(dev_id)?;
        let handle = &self.ctx.devices[dev_id];
        let renderer = self.renderers[dev_id].as_mut().expect("created above");
        renderer
            .render_to_texture(&handle.device, &handle.queue, scene, &surface.target_view, &params(w, h))
            .map_err(|e| format!("render: {e}"))?;
        let frame = match surface.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.ctx.configure_surface(surface);
                return Ok(false);
            }
            _ => return Ok(false), // occluded, timeout: try again with the next frame
        };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut enc = handle.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("blit") });
        surface.blitter.copy(&handle.device, &mut enc, &surface.target_view, &view);
        handle.queue.submit([enc.finish()]);
        handle.queue.present(frame);
        Ok(true)
    }

    /// Draw the scene without a window and return the pixels (RGBA, 4 bytes each, top row first).
    pub fn render_pixels(&mut self, scene: &vello::Scene, width: u32, height: u32) -> Result<Vec<u8>, String> {
        let dev_id = pollster::block_on(self.ctx.device(None)).ok_or("no usable graphics adapter")?;
        self.renderer(dev_id)?;
        let handle = &self.ctx.devices[dev_id];
        let (device, queue) = (&handle.device, &handle.queue);
        let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let renderer = self.renderers[dev_id].as_mut().expect("created above");
        renderer.render_to_texture(device, queue, scene, &view, &params(width, height)).map_err(|e| format!("render: {e}"))?;
        let row = (width * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: row as u64 * height as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("readback") });
        enc.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: None },
            },
            size,
        );
        queue.submit([enc.finish()]);
        let slice = buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        device.poll(wgpu::PollType::wait_indefinitely()).map_err(|e| format!("GPU poll: {e}"))?;
        rx.recv().map_err(|e| e.to_string())?.map_err(|e| format!("read back: {e}"))?;
        let data = slice.get_mapped_range().map_err(|e| format!("read back: {e}"))?;
        let mut out = Vec::with_capacity((width * height * 4) as usize);
        for y in 0..height as usize {
            let start = y * row as usize;
            out.extend_from_slice(&data[start..start + width as usize * 4]);
        }
        drop(data);
        buffer.unmap();
        Ok(out)
    }
}

pub fn write_png(path: &std::path::Path, pixels: &[u8], width: u32, height: u32) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut w = enc.write_header().map_err(|e| e.to_string())?;
    w.write_image_data(pixels).map_err(|e| e.to_string())
}
