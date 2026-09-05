//! Application: chunk streaming, per-frame update and rendering.

use crate::daytime::DayCycle;
use crate::input::Input;
use crate::render::atlas;
use crate::render::camera::{Camera, Frustum};
use crate::render::chunk_renderer::{ChunkRenderer, Globals};
use crate::render::gpu::Gpu;
use crate::render::sky::SkyRenderer;
use crate::world::gen::Generator;
use crate::world::worker::{Job, JobResult, WorkerPool};
use crate::world::World;
use glam::Vec3;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;
use winit::keyboard::KeyCode;

pub struct App {
    pub gpu: Gpu,
    pub chunk_renderer: ChunkRenderer,
    pub sky: SkyRenderer,
    pub world: Arc<World>,
    pub generator: Arc<Generator>,
    pub pool: WorkerPool,
    pub camera: Camera,
    pub input: Input,
    pub daytime: DayCycle,
    pub render_distance: i32,
    pub cursor_grabbed: bool,
    pending_gen: HashSet<(i32, i32)>,
    pending_mesh: HashMap<(i32, i32, i32), u32>,
    last_player_chunk: (i32, i32),
    frame: u64,
    last_frame: Instant,
    fps_accum: f32,
    fps_frames: u32,
    pub fps: f32,
    pub stats: String,
    threads: usize,
}

impl App {
    pub fn new(gpu: Gpu, seed: u64) -> App {
        let atlas_data = atlas::generate();
        let atlas_gpu = atlas::upload(&gpu.device, &gpu.queue, &atlas_data);
        let chunk_renderer = ChunkRenderer::new(&gpu, &atlas_gpu);
        let sky = SkyRenderer::new(&gpu);
        let world = World::new(seed);
        let generator = Arc::new(Generator::new(seed));
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).saturating_sub(1).max(2);
        let pool = WorkerPool::new(world.clone(), generator.clone(), threads);
        let mut camera = Camera::new();
        camera.aspect = gpu.aspect();
        let spawn_h = generator.surface_height(0, 0) as f32 + 2.0;
        camera.pos = Vec3::new(0.5, spawn_h + 20.0, 0.5);
        App {
            gpu,
            chunk_renderer,
            sky,
            world,
            generator,
            pool,
            camera,
            input: Input::new(),
            daytime: DayCycle::new(),
            render_distance: 12,
            cursor_grabbed: false,
            pending_gen: HashSet::new(),
            pending_mesh: HashMap::new(),
            last_player_chunk: (i32::MAX, i32::MAX),
            frame: 0,
            last_frame: Instant::now(),
            fps_accum: 0.0,
            fps_frames: 0,
            fps: 0.0,
            stats: String::new(),
            threads,
        }
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        self.gpu.resize(w, h);
        self.camera.aspect = self.gpu.aspect();
    }

    pub fn update(&mut self) {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;
        self.frame += 1;
        self.fps_accum += dt;
        self.fps_frames += 1;
        if self.fps_accum >= 0.5 {
            self.fps = self.fps_frames as f32 / self.fps_accum;
            self.fps_accum = 0.0;
            self.fps_frames = 0;
        }
        self.daytime.advance(dt as f64);

        // --- free-fly camera ---
        if self.cursor_grabbed {
            let (dx, dy) = self.input.mouse_delta;
            let sens = 0.0025;
            self.camera.yaw -= dx * sens;
            self.camera.pitch -= dy * sens;
            self.camera.clamp_pitch();
        }
        let speed = if self.input.pressed(KeyCode::ControlLeft) { 60.0 } else { 18.0 };
        let mut mv = Vec3::ZERO;
        let fwd = self.camera.forward_flat();
        let right = self.camera.right();
        if self.input.pressed(KeyCode::KeyW) {
            mv += fwd;
        }
        if self.input.pressed(KeyCode::KeyS) {
            mv -= fwd;
        }
        if self.input.pressed(KeyCode::KeyD) {
            mv += right;
        }
        if self.input.pressed(KeyCode::KeyA) {
            mv -= right;
        }
        if self.input.pressed(KeyCode::Space) {
            mv.y += 1.0;
        }
        if self.input.pressed(KeyCode::ShiftLeft) {
            mv.y -= 1.0;
        }
        if mv.length_squared() > 0.0 {
            self.camera.pos += mv.normalize() * speed * dt;
        }
        if self.input.just(KeyCode::BracketRight) {
            self.render_distance = (self.render_distance + 1).min(32);
        }
        if self.input.just(KeyCode::BracketLeft) {
            self.render_distance = (self.render_distance - 1).max(2);
        }

        self.stream_chunks();
        self.collect_results();
        self.input.end_frame();
    }

    fn player_chunk(&self) -> (i32, i32) {
        ((self.camera.pos.x.floor() as i32) >> 4, (self.camera.pos.z.floor() as i32) >> 4)
    }

    fn stream_chunks(&mut self) {
        let (pcx, pcz) = self.player_chunk();
        let rd = self.render_distance;
        let moved = (pcx, pcz) != self.last_player_chunk;
        self.last_player_chunk = (pcx, pcz);

        // unload far chunks
        if self.frame % 60 == 0 {
            let far = rd + 3;
            for (cx, cz) in self.world.chunk_keys() {
                if (cx - pcx).abs() > far || (cz - pcz).abs() > far {
                    if self.pending_mesh.keys().any(|k| k.0 == cx && k.2 == cz) {
                        continue;
                    }
                    self.world.remove_chunk(cx, cz);
                    self.chunk_renderer.remove_column(cx, cz);
                }
            }
        }

        // generation requests
        let max_gen_inflight = self.threads * 3;
        if (moved || self.frame % 15 == 0) && self.pending_gen.len() < max_gen_inflight {
            let mut wanted: Vec<(i32, (i32, i32))> = Vec::new();
            let gr = rd + 1;
            for dz in -gr..=gr {
                for dx in -gr..=gr {
                    let key = (pcx + dx, pcz + dz);
                    if self.pending_gen.contains(&key) || self.world.has_chunk(key.0, key.1) {
                        continue;
                    }
                    wanted.push((dx * dx + dz * dz, key));
                }
            }
            wanted.sort_by_key(|w| w.0);
            for (_, key) in wanted.into_iter().take(max_gen_inflight - self.pending_gen.len()) {
                self.pending_gen.insert(key);
                self.pool.submit(Job::Generate { cx: key.0, cz: key.1 });
            }
        }

        // mesh requests
        let max_mesh_inflight = self.threads * 4;
        if self.frame % 3 == 0 && self.pending_mesh.len() < max_mesh_inflight {
            let mut wanted: Vec<(i32, (i32, i32, i32), u32)> = Vec::new();
            let chunks = self.world.chunks.read().unwrap();
            for dz in -rd..=rd {
                for dx in -rd..=rd {
                    let (cx, cz) = (pcx + dx, pcz + dz);
                    let Some(c) = chunks.get(&(cx, cz)) else { continue };
                    // need all 8 neighbours for correct culling / AO / seams
                    let mut ok = true;
                    'n: for nz in -1..=1 {
                        for nx in -1..=1 {
                            if (nx != 0 || nz != 0) && !chunks.contains_key(&(cx + nx, cz + nz)) {
                                ok = false;
                                break 'n;
                            }
                        }
                    }
                    if !ok {
                        continue;
                    }
                    let c = c.read().unwrap();
                    for (sy, sub) in c.subs.iter().enumerate() {
                        let key = (cx, sy as i32, cz);
                        if let Some(v) = self.pending_mesh.get(&key) {
                            if *v == sub.version {
                                continue;
                            }
                        }
                        if self.chunk_renderer.mesh_version(cx, sy, cz) == Some(sub.version) {
                            continue;
                        }
                        let dy = (sy as i32) - ((self.camera.pos.y as i32) >> 4);
                        wanted.push((dx * dx + dz * dz + dy * dy / 2, key, sub.version));
                    }
                }
            }
            drop(chunks);
            wanted.sort_by_key(|w| w.0);
            let budget = max_mesh_inflight - self.pending_mesh.len();
            for (_, key, version) in wanted.into_iter().take(budget) {
                self.pending_mesh.insert(key, version);
                self.pool.submit(Job::Mesh { cx: key.0, cz: key.2, sy: key.1 as usize, version });
            }
        }
    }

    fn collect_results(&mut self) {
        let (pcx, pcz) = self.player_chunk();
        for r in self.pool.poll() {
            match r {
                JobResult::Generated { chunk } => {
                    let key = (chunk.cx, chunk.cz);
                    self.pending_gen.remove(&key);
                    if (key.0 - pcx).abs() > self.render_distance + 3 || (key.1 - pcz).abs() > self.render_distance + 3 {
                        continue;
                    }
                    self.world.insert_chunk(chunk);
                }
                JobResult::Meshed { cx, cz, sy, version, mesh } => {
                    let key = (cx, sy as i32, cz);
                    if self.pending_mesh.get(&key) == Some(&version) {
                        self.pending_mesh.remove(&key);
                    }
                    if !self.world.has_chunk(cx, cz) {
                        continue;
                    }
                    self.chunk_renderer.upload(&self.gpu, cx, sy, cz, version, mesh);
                }
            }
        }
    }

    fn render_to(&mut self, view: &wgpu::TextureView) {
        let mut enc = self.gpu.device.create_command_encoder(&Default::default());

        let vp = self.camera.view_proj();
        let frustum = Frustum::from_matrix(vp);
        let fog_end = (self.render_distance as f32 * 16.0 - 8.0).max(32.0);
        let fog_start = fog_end * 0.6;
        let (zenith, horizon) = self.daytime.sky_colors();
        let globals = Globals::new(vp, self.camera.pos, horizon, fog_start, fog_end, self.daytime.sun_level(), self.daytime.time_of_day());
        self.chunk_renderer.write_globals(&self.gpu, &globals);
        self.sky.update(&self.gpu, self.camera.proj(), self.camera.forward(), self.daytime.sun_dir(), zenith, horizon, self.daytime.sun_level(), self.daytime.time_of_day(), 0.37);
        self.chunk_renderer.cull(&frustum, self.camera.pos, fog_end + 24.0);
        let stats;
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.gpu.depth_view,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            self.sky.draw(&mut pass);
            let (drawn, quads) = self.chunk_renderer.draw_opaque(&mut pass);
            stats = (drawn, quads);
            self.chunk_renderer.draw_translucent(&mut pass);
        }
        self.chunk_renderer.stats_drawn = stats.0;
        self.chunk_renderer.stats_quads = stats.1;
        self.gpu.queue.submit(Some(enc.finish()));
    }

    pub fn is_idle(&self) -> bool {
        self.pending_gen.is_empty() && self.pending_mesh.is_empty() && self.pool.in_flight == 0
    }

    /// Render one frame into an offscreen target and save it as PNG.
    pub fn screenshot(&mut self, path: &str) {
        let cap = crate::render::screenshot::Capture::new(&self.gpu);
        self.render_to(&cap.view);
        let px = cap.read(&self.gpu);
        match crate::render::screenshot::save_png(path, cap.width, cap.height, &px) {
            Ok(()) => println!("screenshot saved to {path}"),
            Err(e) => eprintln!("screenshot failed: {e}"),
        }
    }

    pub fn render(&mut self) {
        let frame = match self.gpu.surface.get_current_texture() {
            Ok(f) => f,
            Err(wgpu::SurfaceError::Outdated) | Err(wgpu::SurfaceError::Lost) => {
                self.gpu.resize(self.gpu.config.width, self.gpu.config.height);
                return;
            }
            Err(_) => return,
        };
        let view = frame.texture.create_view(&Default::default());
        self.render_to(&view);
        frame.present();
        self.stats = format!(
            "Blockhaven | {:.0} fps | pos {:.1} {:.1} {:.1} | chunks {} | drawn {} subs, {} quads | pending gen {} mesh {} | rd {} | {}",
            self.fps,
            self.camera.pos.x,
            self.camera.pos.y,
            self.camera.pos.z,
            self.world.chunk_count(),
            self.chunk_renderer.stats_drawn,
            self.chunk_renderer.stats_quads,
            self.pending_gen.len(),
            self.pending_mesh.len(),
            self.render_distance,
            self.daytime.clock()
        );
    }
}
