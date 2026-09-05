//! 2D UI batch renderer: solid rects, atlas tiles, bitmap text. Coordinates are GUI units
//! (window pixels / gui scale) converted to NDC on the CPU so no per-viewport uniform is needed.

use crate::player::items::ItemStack;
use crate::render::atlas::{AtlasGpu, Tile};
use crate::render::chunk_renderer::ChunkRenderer;
use crate::render::gpu::Gpu;
use crate::ui::font;
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct UiVertex {
    pub pos: [f32; 2],
    pub uv: [f32; 2],
    pub color: [f32; 4],
    pub data: u32,
}

pub const MODE_SOLID: u32 = 0;
pub const MODE_ATLAS: u32 = 1;
pub const MODE_FONT: u32 = 2;

pub struct UiBatch {
    pub verts: Vec<UiVertex>,
    pub width: f32,
    pub height: f32,
}

impl UiBatch {
    /// `w`/`h` in physical pixels; `scale` = gui pixels per unit.
    pub fn new(w: f32, h: f32, scale: f32) -> UiBatch {
        UiBatch { verts: Vec::new(), width: w / scale, height: h / scale }
    }

    #[inline]
    fn ndc(&self, x: f32, y: f32) -> [f32; 2] {
        [x / self.width * 2.0 - 1.0, 1.0 - y / self.height * 2.0]
    }

    #[allow(clippy::too_many_arguments)]
    pub fn quad(&mut self, x: f32, y: f32, w: f32, h: f32, uv: [f32; 4], color: [f32; 4], data: u32) {
        let p = [self.ndc(x, y), self.ndc(x, y + h), self.ndc(x + w, y + h), self.ndc(x + w, y)];
        let t = [[uv[0], uv[1]], [uv[0], uv[3]], [uv[2], uv[3]], [uv[2], uv[1]]];
        for k in 0..4 {
            self.verts.push(UiVertex { pos: p[k], uv: t[k], color, data });
        }
    }

    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
        self.quad(x, y, w, h, [0.0; 4], color, MODE_SOLID);
    }

    pub fn rect_outline(&mut self, x: f32, y: f32, w: f32, h: f32, t: f32, color: [f32; 4]) {
        self.rect(x, y, w, t, color);
        self.rect(x, y + h - t, w, t, color);
        self.rect(x, y, t, h, color);
        self.rect(x + w - t, y, t, h, color);
    }

    /// Classic bevelled panel.
    pub fn panel(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.rect(x, y, w, h, [0.78, 0.78, 0.78, 1.0]);
        self.rect(x, y, w, 2.0, [1.0, 1.0, 1.0, 1.0]);
        self.rect(x, y, 2.0, h, [1.0, 1.0, 1.0, 1.0]);
        self.rect(x, y + h - 2.0, w, 2.0, [0.33, 0.33, 0.33, 1.0]);
        self.rect(x + w - 2.0, y, 2.0, h, [0.33, 0.33, 0.33, 1.0]);
        self.rect_outline(x - 1.0, y - 1.0, w + 2.0, h + 2.0, 1.0, [0.0, 0.0, 0.0, 1.0]);
    }

    /// Inset slot background.
    pub fn slot(&mut self, x: f32, y: f32, size: f32) {
        self.rect(x, y, size, size, [0.55, 0.55, 0.55, 1.0]);
        self.rect(x, y, size, 1.0, [0.22, 0.22, 0.22, 1.0]);
        self.rect(x, y, 1.0, size, [0.22, 0.22, 0.22, 1.0]);
        self.rect(x, y + size - 1.0, size, 1.0, [1.0, 1.0, 1.0, 1.0]);
        self.rect(x + size - 1.0, y, 1.0, size, [1.0, 1.0, 1.0, 1.0]);
    }

    pub fn button(&mut self, x: f32, y: f32, w: f32, h: f32, label: &str, hovered: bool, enabled: bool) {
        let base = if !enabled { [0.35, 0.35, 0.35, 1.0] } else if hovered { [0.55, 0.62, 0.85, 1.0] } else { [0.45, 0.45, 0.45, 1.0] };
        self.rect(x, y, w, h, base);
        self.rect(x, y, w, 1.0, [0.75, 0.75, 0.75, 1.0]);
        self.rect(x, y, 1.0, h, [0.75, 0.75, 0.75, 1.0]);
        self.rect(x, y + h - 1.0, w, 1.0, [0.2, 0.2, 0.2, 1.0]);
        self.rect(x + w - 1.0, y, 1.0, h, [0.2, 0.2, 0.2, 1.0]);
        self.rect_outline(x - 1.0, y - 1.0, w + 2.0, h + 2.0, 1.0, [0.0, 0.0, 0.0, 1.0]);
        let col = if !enabled { [0.6, 0.6, 0.6, 1.0] } else if hovered { [1.0, 1.0, 0.6, 1.0] } else { [1.0, 1.0, 1.0, 1.0] };
        let tw = font::text_width(label, 1.0);
        self.text_shadow(x + (w - tw) * 0.5, y + (h - 7.0) * 0.5, 1.0, label, col);
    }

    pub fn tile(&mut self, x: f32, y: f32, w: f32, h: f32, tile: Tile, color: [f32; 4]) {
        self.quad(x, y, w, h, [0.0, 0.0, 1.0, 1.0], color, MODE_ATLAS | ((tile.index() as u32) << 8));
    }

    /// Part of a tile (u/v in 0..1).
    #[allow(clippy::too_many_arguments)]
    pub fn tile_part(&mut self, x: f32, y: f32, w: f32, h: f32, tile: Tile, uv: [f32; 4], color: [f32; 4]) {
        self.quad(x, y, w, h, uv, color, MODE_ATLAS | ((tile.index() as u32) << 8));
    }

    pub fn text(&mut self, x: f32, y: f32, scale: f32, s: &str, color: [f32; 4]) -> f32 {
        let mut cx = x;
        for ch in s.chars() {
            if ch != ' ' {
                let uv = font::glyph_uv(ch);
                self.quad(cx, y, 8.0 * scale, 8.0 * scale, uv, color, MODE_FONT);
            }
            cx += font::ADVANCE * scale;
        }
        cx - x
    }

    pub fn text_shadow(&mut self, x: f32, y: f32, scale: f32, s: &str, color: [f32; 4]) -> f32 {
        self.text(x + scale, y + scale, scale, s, [0.0, 0.0, 0.0, color[3] * 0.7]);
        self.text(x, y, scale, s, color)
    }

    pub fn text_centered(&mut self, cx: f32, y: f32, scale: f32, s: &str, color: [f32; 4]) {
        let w = font::text_width(s, scale);
        self.text_shadow(cx - w * 0.5, y, scale, s, color);
    }

    /// Item icon with count / durability bar.
    pub fn item(&mut self, x: f32, y: f32, size: f32, stack: &ItemStack) {
        if stack.is_empty() {
            return;
        }
        let tile = stack.props().tile;
        self.tile(x, y, size, size, tile, [1.0; 4]);
        if stack.count > 1 {
            let s = stack.count.to_string();
            let w = font::text_width(&s, 1.0);
            self.text_shadow(x + size - w + 1.0, y + size - 7.0, 1.0, &s, [1.0; 4]);
        }
        let max = stack.max_durability();
        if max > 0 && stack.damage > 0 {
            let frac = 1.0 - stack.damage as f32 / max as f32;
            let bw = size - 4.0;
            self.rect(x + 2.0, y + size - 3.0, bw, 2.0, [0.0, 0.0, 0.0, 1.0]);
            let col = [1.0 - frac, frac, 0.0, 1.0];
            self.rect(x + 2.0, y + size - 3.0, bw * frac, 1.0, col);
        }
    }
}

pub struct UiRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    buf: wgpu::Buffer,
    capacity: usize,
    ranges: Vec<(u32, u32)>,
}

impl UiRenderer {
    pub fn new(gpu: &Gpu, atlas: &AtlasGpu, font_view: &wgpu::TextureView) -> UiRenderer {
        let device = &gpu.device;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ui"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/ui.wgsl").into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ui layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2Array, multisampled: false },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                    count: None,
                },
            ],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ui bind group"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&atlas.view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&atlas.sampler_ui) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(font_view) },
            ],
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[&layout], push_constant_ranges: &[] });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ui pipeline"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<UiVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute { offset: 0, shader_location: 0, format: wgpu::VertexFormat::Float32x2 },
                        wgpu::VertexAttribute { offset: 8, shader_location: 1, format: wgpu::VertexFormat::Float32x2 },
                        wgpu::VertexAttribute { offset: 16, shader_location: 2, format: wgpu::VertexFormat::Float32x4 },
                        wgpu::VertexAttribute { offset: 32, shader_location: 3, format: wgpu::VertexFormat::Uint32 },
                    ],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState { format: gpu.config.format, blend: Some(wgpu::BlendState::ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })],
            }),
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, cull_mode: None, ..Default::default() },
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
        });
        let capacity = 65536;
        let buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ui vertices"),
            size: (capacity * std::mem::size_of::<UiVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        UiRenderer { pipeline, bind_group, buf, capacity, ranges: Vec::new() }
    }

    /// Upload all batches; returns nothing, batches are drawn by index with `draw`.
    pub fn prepare(&mut self, gpu: &Gpu, batches: &[&UiBatch]) {
        let total: usize = batches.iter().map(|b| b.verts.len()).sum();
        if total > self.capacity {
            self.capacity = (total * 3 / 2).max(1024);
            self.buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("ui vertices"),
                size: (self.capacity * std::mem::size_of::<UiVertex>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        let mut all: Vec<UiVertex> = Vec::with_capacity(total);
        self.ranges.clear();
        for b in batches {
            let start = all.len() as u32 / 4;
            all.extend_from_slice(&b.verts);
            self.ranges.push((start, b.verts.len() as u32 / 4));
        }
        if !all.is_empty() {
            gpu.queue.write_buffer(&self.buf, 0, bytemuck::cast_slice(&all));
        }
    }

    pub fn draw<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>, chunks: &'a ChunkRenderer, batch_index: usize) {
        let Some((start, quads)) = self.ranges.get(batch_index).copied() else { return };
        if quads == 0 {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_index_buffer(chunks.index_buf.slice(..), wgpu::IndexFormat::Uint32);
        pass.set_vertex_buffer(0, self.buf.slice(..));
        // base vertex = start * 4 so the shared quad index buffer lines up
        pass.draw_indexed(0..quads * 6, (start * 4) as i32, 0..1);
    }
}
