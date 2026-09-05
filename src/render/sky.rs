//! Sky dome: gradient, sun, moon, stars. Drawn as a fullscreen triangle before the world.

use crate::render::gpu::{Gpu, DEPTH_FORMAT};
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct SkyUniform {
    pub inv_view_proj: [[f32; 4]; 4],
    pub sun_dir: [f32; 4],
    pub zenith: [f32; 4],
    pub horizon: [f32; 4],
    pub params: [f32; 4],
}

pub struct SkyRenderer {
    pipeline: wgpu::RenderPipeline,
    buf: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl SkyRenderer {
    pub fn new(gpu: &Gpu) -> SkyRenderer {
        let device = &gpu.device;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sky shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/sky.wgsl").into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sky layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            }],
        });
        let buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sky uniform"),
            size: std::mem::size_of::<SkyUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sky bind group"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: buf.as_entire_binding() }],
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[&layout], push_constant_ranges: &[] });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sky pipeline"),
            layout: Some(&pl),
            vertex: wgpu::VertexState { module: &shader, entry_point: "vs_main", buffers: &[] },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState { format: gpu.config.format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            }),
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, cull_mode: None, ..Default::default() },
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
        SkyRenderer { pipeline, buf, bind_group }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update(&self, gpu: &Gpu, proj: Mat4, forward: Vec3, sun_dir: Vec3, zenith: [f32; 3], horizon: [f32; 3], sun_level: f32, time: f32, seed: f32) {
        let view_rot = Mat4::look_to_rh(Vec3::ZERO, forward, Vec3::Y);
        let inv = (proj * view_rot).inverse();
        let u = SkyUniform {
            inv_view_proj: inv.to_cols_array_2d(),
            sun_dir: [sun_dir.x, sun_dir.y, sun_dir.z, 0.0],
            zenith: [zenith[0], zenith[1], zenith[2], 1.0],
            horizon: [horizon[0], horizon[1], horizon[2], 1.0],
            params: [sun_level, time, seed, 0.0],
        };
        gpu.queue.write_buffer(&self.buf, 0, bytemuck::bytes_of(&u));
    }

    pub fn draw<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}
