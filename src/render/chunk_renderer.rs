//! GPU side of chunk rendering: pipelines, per-sub-chunk vertex buffers, shared index buffer.

use crate::render::atlas::AtlasGpu;
use crate::render::camera::Frustum;
use crate::render::gpu::{Gpu, DEPTH_FORMAT};
use crate::render::mesher::{ChunkVertex, MeshData};
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use std::collections::HashMap;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Globals {
    pub view_proj: [[f32; 4]; 4],
    pub cam_pos: [f32; 4],
    pub fog_color: [f32; 4],
    pub params: [f32; 4],
    pub tints: [[f32; 4]; 8],
}

impl Globals {
    pub fn new(view_proj: Mat4, cam_pos: Vec3, fog_color: [f32; 3], fog_start: f32, fog_end: f32, sun_level: f32, time: f32) -> Globals {
        Globals {
            view_proj: view_proj.to_cols_array_2d(),
            cam_pos: [cam_pos.x, cam_pos.y, cam_pos.z, 1.0],
            fog_color: [fog_color[0], fog_color[1], fog_color[2], 1.0],
            params: [fog_start, fog_end, sun_level, time],
            tints: [
                [1.0, 1.0, 1.0, 1.0],
                [0.55, 0.82, 0.32, 1.0], // grass
                [0.42, 0.72, 0.28, 1.0], // foliage
                [0.35, 0.55, 1.0, 1.0],  // water
                [1.0, 1.0, 1.0, 1.0],
                [1.0, 1.0, 1.0, 1.0],
                [1.0, 1.0, 1.0, 1.0],
                [1.0, 1.0, 1.0, 1.0],
            ],
        }
    }
}

pub const MAX_QUADS: usize = 131072;

pub struct SubMesh {
    pub opaque: Option<(wgpu::Buffer, u32)>,
    pub translucent: Option<(wgpu::Buffer, u32)>,
    pub version: u32,
    pub min: Vec3,
    pub max: Vec3,
}

pub struct ChunkRenderer {
    pub pipeline_opaque: wgpu::RenderPipeline,
    pub pipeline_translucent: wgpu::RenderPipeline,
    pub globals_buf: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
    pub bind_layout: wgpu::BindGroupLayout,
    pub index_buf: wgpu::Buffer,
    pub meshes: HashMap<(i32, i32, i32), SubMesh>,
    pub stats_drawn: usize,
    pub stats_quads: usize,
    scratch_visible: Vec<(f32, (i32, i32, i32))>,
}

pub fn vertex_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<ChunkVertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute { offset: 0, shader_location: 0, format: wgpu::VertexFormat::Float32x3 },
            wgpu::VertexAttribute { offset: 12, shader_location: 1, format: wgpu::VertexFormat::Float32x2 },
            wgpu::VertexAttribute { offset: 20, shader_location: 2, format: wgpu::VertexFormat::Uint32 },
        ],
    }
}

impl ChunkRenderer {
    pub fn new(gpu: &Gpu, atlas: &AtlasGpu) -> ChunkRenderer {
        let device = &gpu.device;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("chunk shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/chunk.wgsl").into()),
        });
        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("chunk bind layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2Array, multisampled: false },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let globals_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("chunk bind group"),
            layout: &bind_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: globals_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&atlas.view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&atlas.sampler) },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("chunk pipeline layout"),
            bind_group_layouts: &[&bind_layout],
            push_constant_ranges: &[],
        });
        let make = |entry: &str, blend: Option<wgpu::BlendState>, depth_write: bool, cull: Option<wgpu::Face>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(entry),
                layout: Some(&layout),
                vertex: wgpu::VertexState { module: &shader, entry_point: "vs_main", buffers: &[vertex_layout()] },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: entry,
                    targets: &[Some(wgpu::ColorTargetState { format: gpu.config.format, blend, write_mask: wgpu::ColorWrites::ALL })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: cull,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: depth_write,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                multiview: None,
            })
        };
        let pipeline_opaque = make("fs_opaque", None, true, Some(wgpu::Face::Back));
        let pipeline_translucent = make("fs_translucent", Some(wgpu::BlendState::ALPHA_BLENDING), false, None);
        // shared quad index buffer
        let mut indices: Vec<u32> = Vec::with_capacity(MAX_QUADS * 6);
        for q in 0..MAX_QUADS as u32 {
            let b = q * 4;
            indices.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
        }
        let index_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("quad indices"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        ChunkRenderer {
            pipeline_opaque,
            pipeline_translucent,
            globals_buf,
            bind_group,
            bind_layout,
            index_buf,
            meshes: HashMap::new(),
            stats_drawn: 0,
            stats_quads: 0,
            scratch_visible: Vec::new(),
        }
    }

    pub fn write_globals(&self, gpu: &Gpu, g: &Globals) {
        gpu.queue.write_buffer(&self.globals_buf, 0, bytemuck::bytes_of(g));
    }

    pub fn upload(&mut self, gpu: &Gpu, cx: i32, sy: usize, cz: i32, version: u32, mesh: MeshData) {
        let key = (cx, sy as i32, cz);
        if mesh.is_empty() {
            self.meshes.remove(&key);
            // keep a tombstone with the version so we don't re-mesh forever
            self.meshes.insert(key, SubMesh { opaque: None, translucent: None, version, min: Vec3::ZERO, max: Vec3::ZERO });
            return;
        }
        let mk = |v: &Vec<ChunkVertex>, label: &str| -> Option<(wgpu::Buffer, u32)> {
            if v.is_empty() {
                return None;
            }
            let quads = (v.len() / 4).min(MAX_QUADS);
            let buf = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::cast_slice(&v[..quads * 4]),
                usage: wgpu::BufferUsages::VERTEX,
            });
            Some((buf, (quads * 6) as u32))
        };
        let min = Vec3::new(cx as f32 * 16.0, sy as f32 * 16.0, cz as f32 * 16.0);
        let max = min + Vec3::splat(16.0);
        let sm = SubMesh { opaque: mk(&mesh.opaque, "opaque"), translucent: mk(&mesh.translucent, "translucent"), version, min, max };
        self.meshes.insert(key, sm);
    }

    pub fn remove_column(&mut self, cx: i32, cz: i32) {
        for sy in 0..16 {
            self.meshes.remove(&(cx, sy, cz));
        }
    }

    pub fn mesh_version(&self, cx: i32, sy: usize, cz: i32) -> Option<u32> {
        self.meshes.get(&(cx, sy as i32, cz)).map(|m| m.version)
    }

    /// Collect visible sub-meshes sorted front-to-back.
    pub fn cull(&mut self, frustum: &Frustum, cam: Vec3, max_dist: f32) {
        self.scratch_visible.clear();
        let md2 = max_dist * max_dist;
        for (key, m) in &self.meshes {
            if m.opaque.is_none() && m.translucent.is_none() {
                continue;
            }
            let center = (m.min + m.max) * 0.5;
            let d2 = center.distance_squared(cam);
            if d2 > md2 {
                continue;
            }
            if !frustum.intersects_aabb(m.min, m.max) {
                continue;
            }
            self.scratch_visible.push((d2, *key));
        }
        self.scratch_visible.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    }

    pub fn draw_opaque<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) -> (usize, usize) {
        pass.set_pipeline(&self.pipeline_opaque);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_index_buffer(self.index_buf.slice(..), wgpu::IndexFormat::Uint32);
        let mut drawn = 0;
        let mut quads = 0;
        for (_, key) in &self.scratch_visible {
            if let Some(m) = self.meshes.get(key) {
                if let Some((buf, n)) = &m.opaque {
                    pass.set_vertex_buffer(0, buf.slice(..));
                    pass.draw_indexed(0..*n, 0, 0..1);
                    drawn += 1;
                    quads += *n as usize / 6;
                }
            }
        }
        (drawn, quads)
    }

    pub fn draw_translucent<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) -> usize {
        pass.set_pipeline(&self.pipeline_translucent);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_index_buffer(self.index_buf.slice(..), wgpu::IndexFormat::Uint32);
        let mut drawn = 0;
        for (_, key) in self.scratch_visible.iter().rev() {
            if let Some(m) = self.meshes.get(key) {
                if let Some((buf, n)) = &m.translucent {
                    pass.set_vertex_buffer(0, buf.slice(..));
                    pass.draw_indexed(0..*n, 0, 0..1);
                    drawn += 1;
                }
            }
        }
        drawn
    }

    /// Draw an arbitrary quad list (entities, overlays) with the given pipeline.
    pub fn draw_dynamic<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>, buf: &'a wgpu::Buffer, quads: u32, translucent: bool) {
        if quads == 0 {
            return;
        }
        pass.set_pipeline(if translucent { &self.pipeline_translucent } else { &self.pipeline_opaque });
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_index_buffer(self.index_buf.slice(..), wgpu::IndexFormat::Uint32);
        pass.set_vertex_buffer(0, buf.slice(..));
        pass.draw_indexed(0..quads.min(MAX_QUADS as u32) * 6, 0, 0..1);
    }
}

/// A growable vertex buffer for per-frame geometry (entities, held item, overlays).
pub struct DynamicBuffer {
    pub buf: wgpu::Buffer,
    pub capacity: usize,
    pub quads: u32,
}

impl DynamicBuffer {
    pub fn new(device: &wgpu::Device, capacity: usize) -> DynamicBuffer {
        let buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("dynamic vertices"),
            size: (capacity * std::mem::size_of::<ChunkVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        DynamicBuffer { buf, capacity, quads: 0 }
    }
    pub fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, verts: &[ChunkVertex]) {
        if verts.len() > self.capacity {
            let cap = (verts.len() * 3 / 2).max(1024);
            *self = DynamicBuffer::new(device, cap);
        }
        if !verts.is_empty() {
            queue.write_buffer(&self.buf, 0, bytemuck::cast_slice(verts));
        }
        self.quads = (verts.len() / 4) as u32;
    }
}
