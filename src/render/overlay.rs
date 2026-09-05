//! Block selection outline, break-crack overlay and the first-person hand / held item.

use crate::player::items::{ItemKind, ItemStack};
use crate::render::atlas::Tile;
use crate::render::camera::Camera;
use crate::render::chunk_renderer::{ChunkRenderer, DynamicBuffer};
use crate::render::gpu::{Gpu, DEPTH_FORMAT};
use crate::render::mesher::{pack, ChunkVertex};
use crate::world::block::{self, face_tiles, Shape};
use bytemuck::{Pod, Zeroable};
use glam::Vec3;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct LineVertex {
    pub pos: [f32; 3],
    pub color: [f32; 4],
}

pub struct OverlayRenderer {
    line_pipeline: wgpu::RenderPipeline,
    line_buf: wgpu::Buffer,
    line_count: u32,
    pub crack: DynamicBuffer,
    pub hand: DynamicBuffer,
}

const LINE_CAP: usize = 4096;

impl OverlayRenderer {
    pub fn new(gpu: &Gpu, chunks: &ChunkRenderer) -> OverlayRenderer {
        let device = &gpu.device;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("lines"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/lines.wgsl").into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[&chunks.bind_layout], push_constant_ranges: &[] });
        let line_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("line pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<LineVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute { offset: 0, shader_location: 0, format: wgpu::VertexFormat::Float32x3 },
                        wgpu::VertexAttribute { offset: 12, shader_location: 1, format: wgpu::VertexFormat::Float32x4 },
                    ],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState { format: gpu.config.format, blend: Some(wgpu::BlendState::ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })],
            }),
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::LineList, cull_mode: None, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
        });
        let line_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("lines"),
            size: (LINE_CAP * std::mem::size_of::<LineVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        OverlayRenderer { line_pipeline, line_buf, line_count: 0, crack: DynamicBuffer::new(device, 256), hand: DynamicBuffer::new(device, 256) }
    }

    pub fn upload_lines(&mut self, gpu: &Gpu, lines: &[LineVertex]) {
        let n = lines.len().min(LINE_CAP);
        if n > 0 {
            gpu.queue.write_buffer(&self.line_buf, 0, bytemuck::cast_slice(&lines[..n]));
        }
        self.line_count = n as u32;
    }

    pub fn draw_lines<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>, chunks: &'a ChunkRenderer) {
        if self.line_count == 0 {
            return;
        }
        pass.set_pipeline(&self.line_pipeline);
        pass.set_bind_group(0, &chunks.bind_group, &[]);
        pass.set_vertex_buffer(0, self.line_buf.slice(..));
        pass.draw(0..self.line_count, 0..1);
    }
}

/// 12 edges of a box.
pub fn box_lines(min: Vec3, max: Vec3, color: [f32; 4], out: &mut Vec<LineVertex>) {
    let c = [
        Vec3::new(min.x, min.y, min.z),
        Vec3::new(max.x, min.y, min.z),
        Vec3::new(max.x, min.y, max.z),
        Vec3::new(min.x, min.y, max.z),
        Vec3::new(min.x, max.y, min.z),
        Vec3::new(max.x, max.y, min.z),
        Vec3::new(max.x, max.y, max.z),
        Vec3::new(min.x, max.y, max.z),
    ];
    let edges = [(0, 1), (1, 2), (2, 3), (3, 0), (4, 5), (5, 6), (6, 7), (7, 4), (0, 4), (1, 5), (2, 6), (3, 7)];
    for (a, b) in edges {
        out.push(LineVertex { pos: c[a].to_array(), color });
        out.push(LineVertex { pos: c[b].to_array(), color });
    }
}

/// Selection box for a block (uses the collision shape where it is smaller than a cube).
pub fn selection_box(v: u16, pos: (i32, i32, i32)) -> (Vec3, Vec3) {
    let e = 0.003;
    if let Some(b) = crate::player::physics::block_aabb(v, pos.0, pos.1, pos.2) {
        return (b.min - Vec3::splat(e), b.max + Vec3::splat(e));
    }
    let p = block::props(block::vox_id(v));
    let base = Vec3::new(pos.0 as f32, pos.1 as f32, pos.2 as f32);
    let (min, max) = match p.shape {
        Shape::Cross => (Vec3::new(0.15, 0.0, 0.15), Vec3::new(0.85, 0.85, 0.85)),
        Shape::Torch => (Vec3::new(0.35, 0.0, 0.35), Vec3::new(0.65, 0.65, 0.65)),
        Shape::Wire => (Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.07, 1.0)),
        Shape::Plate => (Vec3::new(0.06, 0.0, 0.06), Vec3::new(0.94, 0.08, 0.94)),
        Shape::Button => (Vec3::new(0.2, 0.2, 0.2), Vec3::new(0.8, 0.8, 0.8)),
        Shape::Fluid => (Vec3::ZERO, Vec3::new(1.0, 0.875, 1.0)),
        _ => (Vec3::ZERO, Vec3::ONE),
    };
    (base + min - Vec3::splat(e), base + max + Vec3::splat(e))
}

/// Six faces of a unit cube at `pos`, slightly inflated, textured with a crack stage.
pub fn crack_quads(pos: (i32, i32, i32), progress: f32, light: (u8, u8)) -> Vec<ChunkVertex> {
    let stage = ((progress * 10.0) as u16).min(9);
    let tile = Tile::Crack0.index() + stage;
    let e = 0.004;
    let min = Vec3::new(pos.0 as f32 - e, pos.1 as f32 - e, pos.2 as f32 - e);
    let max = Vec3::new(pos.0 as f32 + 1.0 + e, pos.1 as f32 + 1.0 + e, pos.2 as f32 + 1.0 + e);
    let mut out = Vec::with_capacity(24);
    box_quads_world(min, max, [tile; 6], light, &mut out);
    out
}

/// Axis-aligned world-space box with the standard face order.
pub fn box_quads_world(min: Vec3, max: Vec3, tiles: [u16; 6], light: (u8, u8), out: &mut Vec<ChunkVertex>) {
    let basis = [Vec3::X, Vec3::Y, Vec3::Z];
    let center = (min + max) * 0.5;
    let half = (max - min) * 0.5;
    box_quads(center, half, basis, tiles, light, [0, 1, 2, 3, 4, 5], out);
}

/// Generic oriented box. `basis` must be right-handed. `normals` gives the shading code per face.
pub fn box_quads(center: Vec3, half: Vec3, basis: [Vec3; 3], tiles: [u16; 6], light: (u8, u8), normals: [u8; 6], out: &mut Vec<ChunkVertex>) {
    // face: (axis, positive, u_axis, v_axis, corners (u,v) bits) -- same conventions as the mesher
    const FACES: [(usize, bool, usize, usize, [(u8, u8); 4]); 6] = [
        (0, false, 2, 1, [(0, 0), (1, 0), (1, 1), (0, 1)]),
        (0, true, 2, 1, [(0, 0), (0, 1), (1, 1), (1, 0)]),
        (1, false, 0, 2, [(0, 0), (1, 0), (1, 1), (0, 1)]),
        (1, true, 0, 2, [(0, 0), (0, 1), (1, 1), (1, 0)]),
        (2, false, 0, 1, [(0, 0), (0, 1), (1, 1), (1, 0)]),
        (2, true, 0, 1, [(0, 0), (1, 0), (1, 1), (0, 1)]),
    ];
    for (fi, (axis, positive, ua, va, corners)) in FACES.iter().enumerate() {
        let data = pack(tiles[fi], normals[fi], 0, 3, light.0, light.1);
        for (ub, vb) in corners {
            let mut p = center + basis[*axis] * (if *positive { half[*axis] } else { -half[*axis] });
            p += basis[*ua] * (if *ub == 1 { half[*ua] } else { -half[*ua] });
            p += basis[*va] * (if *vb == 1 { half[*va] } else { -half[*va] });
            let u = if *ub == 1 { 1.0 } else { 0.0 };
            let v = if *axis == 1 {
                if *vb == 1 { 1.0 } else { 0.0 }
            } else if *vb == 1 {
                0.0
            } else {
                1.0
            };
            let u = if fi == 1 || fi == 4 { 1.0 - u } else { u };
            out.push(ChunkVertex { pos: p.to_array(), uv: [u, v], data });
        }
    }
}

/// First-person hand: an arm, a held block cube, or a flat item sprite. `swing` in 0..1.
pub fn held_quads(cam: &Camera, held: &ItemStack, swing: f32, light: (u8, u8), eating: f32) -> Vec<ChunkVertex> {
    let f = cam.forward();
    let r = f.cross(Vec3::Y).normalize_or_zero();
    let r = if r == Vec3::ZERO { cam.right() } else { r };
    let u = r.cross(f).normalize();
    // swing animation: dip down/forward and rotate
    let s = swing.clamp(0.0, 1.0);
    let dip = (s * std::f32::consts::PI).sin();
    let mut anchor = cam.pos + f * 0.62 + r * 0.42 - u * (0.34 + dip * 0.22) - f * dip * 0.15;
    if eating > 0.0 {
        anchor += u * (eating * 8.0).sin().abs() * 0.05 - r * 0.15;
    }
    let mut out = Vec::new();
    let normals = [0, 1, 2, 3, 4, 5];
    match held.as_block() {
        Some(b) if !held.is_empty() => {
            let p = block::props(b.id());
            if p.shape == Shape::Cube || p.shape == Shape::Cactus || p.shape == Shape::Farmland || p.shape == Shape::Fluid {
                let tiles = face_tiles(b, 0).map(|t| t.index());
                let a = 0.55f32;
                let rr = r * a.cos() - f * a.sin();
                let ff = r * a.sin() + f * a.cos();
                let bb = [rr, u, -ff];
                box_quads(anchor, Vec3::splat(0.16), bb, tiles, light, normals, &mut out);
                return out;
            }
            // non-cube blocks (torch, plants, doors...) as sprites of their item icon
            let tile = held.props().tile.index();
            sprite(anchor + u * 0.05, r, u, 0.26, tile, light, &mut out);
            out
        }
        _ if !held.is_empty() => {
            let tile = held.props().tile.index();
            let big = matches!(held.props().kind, ItemKind::Tool { .. });
            sprite(anchor + u * 0.1 - r * 0.05, r, u, if big { 0.34 } else { 0.26 }, tile, light, &mut out);
            out
        }
        _ => {
            // bare arm: a long box angled up toward the camera
            let a = 0.5f32;
            let ff = f * a.cos() + u * a.sin();
            let uu = u * a.cos() - f * a.sin();
            let bb = [r, uu, -ff];
            let arm_center = anchor + r * 0.05 - u * 0.12 - f * 0.1;
            let skin = Tile::PlayerSkin.index();
            let shirt = Tile::PlayerShirt.index();
            box_quads(arm_center, Vec3::new(0.09, 0.09, 0.36), bb, [skin, skin, skin, skin, skin, shirt], light, normals, &mut out);
            out
        }
    }
}

fn sprite(center: Vec3, r: Vec3, u: Vec3, size: f32, tile: u16, light: (u8, u8), out: &mut Vec<ChunkVertex>) {
    let data = pack(tile, 3, 0, 3, light.0, light.1);
    let h = size * 0.5;
    let p = [center - r * h - u * h, center + r * h - u * h, center + r * h + u * h, center - r * h + u * h];
    let uv = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
    for k in 0..4 {
        out.push(ChunkVertex { pos: p[k].to_array(), uv: uv[k], data });
    }
    for k in [3, 2, 1, 0] {
        out.push(ChunkVertex { pos: p[k].to_array(), uv: uv[k], data });
    }
}

/// Item drop as a small spinning cube (blocks) or sprite (items).
pub fn drop_quads(pos: Vec3, stack: &ItemStack, age: f32, light: (u8, u8), out: &mut Vec<ChunkVertex>) {
    let bob = (age * 2.0).sin() * 0.05 + 0.15;
    let spin = age * 1.5;
    let r = Vec3::new(spin.cos(), 0.0, -spin.sin());
    let f = Vec3::new(spin.sin(), 0.0, spin.cos());
    let center = pos + Vec3::new(0.0, bob, 0.0);
    match stack.as_block() {
        Some(b) if matches!(block::props(b.id()).shape, Shape::Cube | Shape::Cactus | Shape::Farmland) => {
            let tiles = face_tiles(b, 0).map(|t| t.index());
            box_quads(center, Vec3::splat(0.125), [r, Vec3::Y, f], tiles, light, [0, 1, 2, 3, 4, 5], out);
        }
        _ => {
            let tile = stack.props().tile.index();
            sprite(center + Vec3::new(0.0, 0.05, 0.0), r, Vec3::Y, 0.3, tile, light, out);
        }
    }
}
