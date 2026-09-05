//! Frame rendering: one or two viewports (split-screen), HUD per viewport, menu overlay.

use crate::app::App;
use crate::player::OpenUi;
use crate::render::atlas::Tile;
use crate::render::camera::Frustum;
use crate::render::chunk_renderer::Globals;
use crate::render::overlay;
use crate::render::ui2d::UiBatch;
use crate::ui::screens;
use glam::Vec3;

#[derive(Clone, Copy)]
pub struct Viewport {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl App {
    pub(crate) fn viewport(&self, idx: usize) -> Viewport {
        let (w, h) = (self.gpu.config.width, self.gpu.config.height);
        if self.players.len() < 2 {
            Viewport { x: 0, y: 0, w, h }
        } else {
            let half = w / 2;
            if idx == 0 {
                Viewport { x: 0, y: 0, w: half, h }
            } else {
                Viewport { x: half, y: 0, w: w - half, h }
            }
        }
    }

    pub(crate) fn viewport_size(&self, idx: usize) -> (f32, f32) {
        let v = self.viewport(idx);
        (v.w as f32, v.h as f32)
    }

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
    }

    fn render_to(&mut self, view: &wgpu::TextureView) {
        let n = self.players.len();
        for i in 0..n {
            self.render_view(view, i, i == 0);
        }
        self.render_overlay_ui(view);
        let p = &self.players[0];
        self.stats = format!(
            "Blockhaven | {:.0} fps | pos {:.1} {:.1} {:.1} | hp {:.0} food {:.0} | chunks {} | drawn {} subs, {} quads | pending gen {} mesh {} | rd {} | {}",
            self.fps,
            p.pos.x,
            p.pos.y,
            p.pos.z,
            p.health,
            p.hunger,
            self.world.chunk_count(),
            self.chunk_renderer.stats_drawn,
            self.chunk_renderer.stats_quads,
            self.pending_gen.len(),
            self.pending_mesh.len(),
            self.settings.render_distance,
            self.daytime.clock()
        );
    }

    /// Render the world + HUD for one player into its viewport.
    fn render_view(&mut self, view: &wgpu::TextureView, idx: usize, first: bool) {
        let vp = self.viewport(idx);
        let mut enc = self.gpu.device.create_command_encoder(&Default::default());
        let player = &self.players[idx];
        let aspect = vp.w as f32 / vp.h.max(1) as f32;
        let camera = player.camera(aspect, self.settings.fov);
        let vpm = camera.view_proj();
        let frustum = Frustum::from_matrix(vpm);
        let rd = self.settings.render_distance;
        let fog_end = (rd as f32 * 16.0 - 8.0).max(32.0);
        let fog_start = fog_end * 0.6;
        let (zenith, horizon) = self.daytime.sky_colors();
        let (fog_color, fog_start, fog_end) = if player.head_in_water {
            ([0.1, 0.25, 0.55], 2.0, 24.0)
        } else if player.in_lava {
            ([0.9, 0.3, 0.05], 0.5, 3.0)
        } else {
            (horizon, fog_start, fog_end)
        };
        let globals = Globals::new(vpm, camera.pos, fog_color, fog_start, fog_end, self.daytime.sun_level(), self.daytime.time_of_day());
        self.chunk_renderer.write_globals(&self.gpu, &globals);
        self.sky.update(&self.gpu, camera.proj(), camera.forward(), self.daytime.sun_dir(), zenith, horizon, self.daytime.sun_level(), self.daytime.time_of_day(), 0.37);
        self.chunk_renderer.cull(&frustum, camera.pos, fog_end + 24.0);

        // entities
        let mut ent = Vec::new();
        for d in &self.drops {
            let p = d.position();
            let l = self.world.light_at(p.x.floor() as i32, (p.y + 0.2).floor() as i32, p.z.floor() as i32);
            overlay::drop_quads(p, &d.stack, d.age, l, &mut ent);
        }
        for m in &self.mobs {
            let p = m.position();
            if p.distance(camera.pos) < 64.0 {
                let l = self.world.light_at(p.x.floor() as i32, (p.y + 0.5).floor() as i32, p.z.floor() as i32);
                crate::mobs::models::mob_quads(m, l, &mut ent);
            }
        }
        for (j, other) in self.players.iter().enumerate() {
            if j == idx || other.dead {
                continue;
            }
            let p = other.pos;
            let l = self.world.light_at(p.x.floor() as i32, (p.y + 1.0).floor() as i32, p.z.floor() as i32);
            let hs = Vec3::new(other.vel.x, 0.0, other.vel.z).length();
            let amp = (hs / 4.3).min(1.0);
            let body_pos = if other.sneaking { p - Vec3::new(0.0, 0.15, 0.0) } else { p };
            crate::mobs::models::player_quads(body_pos, other.yaw, other.yaw, other.bob, amp, l, other.hurt_timer > 0.0, &mut ent);
        }
        for a in &self.arrows {
            let p = a.position();
            let l = self.world.light_at(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
            let dir = if a.stuck { Vec3::X } else { a.velocity().normalize_or_zero() };
            let side = dir.cross(Vec3::Y).normalize_or_zero();
            let side = if side == Vec3::ZERO { Vec3::Z } else { side };
            let up = side.cross(dir);
            overlay::box_quads(p, Vec3::new(0.03, 0.03, 0.3), [side, up, -dir], [Tile::ArrowSide.index(); 6], l, [0, 1, 2, 3, 4, 5], &mut ent);
        }
        for t in &self.tnt {
            let p = t.position() + Vec3::new(0.0, 0.5, 0.0);
            let l = self.world.light_at(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
            let flash = ((t.fuse * 8.0).sin() > 0.0) && t.fuse < 3.0;
            let tiles = if flash { [Tile::TntPrimed.index(); 6] } else { crate::world::block::face_tiles(crate::world::block::Block::Tnt, 0).map(|t| t.index()) };
            overlay::box_quads_world(p - Vec3::splat(0.49), p + Vec3::splat(0.49), tiles, l, &mut ent);
        }
        self.entity_buf.upload(&self.gpu.device, &self.gpu.queue, &ent);

        // selection outline + crack + hand
        let mut lines = Vec::new();
        let mut crack = Vec::new();
        let interactive = !self.demo && player.ui == OpenUi::None && !player.dead;
        if interactive {
            let mut cache = crate::world::ChunkCache::new(&self.world);
            if let Some(hit) = crate::player::raycast::raycast(&mut cache, player.eye(), player.look_dir(), player.reach(), false) {
                let v = self.world.get(hit.pos.0, hit.pos.1, hit.pos.2);
                let (min, max) = overlay::selection_box(v, hit.pos);
                overlay::box_lines(min, max, [0.0, 0.0, 0.0, 0.75], &mut lines);
                if let Some((bp, prog)) = player.breaking {
                    if bp == hit.pos {
                        let l = self.world.light_at(hit.pos.0 + hit.normal.0, hit.pos.1 + hit.normal.1, hit.pos.2 + hit.normal.2);
                        crack = overlay::crack_quads(hit.pos, prog, l);
                    }
                }
            }
        }
        self.overlay.upload_lines(&self.gpu, &lines);
        self.overlay.crack.upload(&self.gpu.device, &self.gpu.queue, &crack);
        let eye = player.eye();
        let hl = self.world.light_at(eye.x.floor() as i32, eye.y.floor() as i32, eye.z.floor() as i32);
        let hand = if interactive { overlay::held_quads(&camera, &player.inventory.held(), player.swing, hl, player.eating) } else { Vec::new() };
        self.overlay.hand.upload(&self.gpu.device, &self.gpu.queue, &hand);

        let set_vp = |pass: &mut wgpu::RenderPass| {
            pass.set_viewport(vp.x as f32, vp.y as f32, vp.w as f32, vp.h as f32, 0.0, 1.0);
            pass.set_scissor_rect(vp.x, vp.y, vp.w, vp.h);
        };
        let stats;
        {
            let load = if first { wgpu::LoadOp::Clear(wgpu::Color::BLACK) } else { wgpu::LoadOp::Load };
            let dload = if first { wgpu::LoadOp::Clear(1.0) } else { wgpu::LoadOp::Load };
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment { view, resolve_target: None, ops: wgpu::Operations { load, store: wgpu::StoreOp::Store } })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.gpu.depth_view,
                    depth_ops: Some(wgpu::Operations { load: dload, store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            set_vp(&mut pass);
            self.sky.draw(&mut pass);
            let (drawn, quads) = self.chunk_renderer.draw_opaque(&mut pass);
            stats = (drawn, quads);
            self.chunk_renderer.draw_dynamic(&mut pass, &self.entity_buf.buf, self.entity_buf.quads, false);
            self.chunk_renderer.draw_translucent(&mut pass);
            self.chunk_renderer.draw_dynamic(&mut pass, &self.overlay.crack.buf, self.overlay.crack.quads, true);
            self.overlay.draw_lines(&mut pass, &self.chunk_renderer);
        }
        // hand pass: depth cleared inside the viewport region only (scissored clear via a full-depth
        // quad is unnecessary: the hand is drawn with depth test off against a far-cleared buffer)
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("hand"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment { view, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store } })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.gpu.depth_view,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            set_vp(&mut pass);
            self.chunk_renderer.draw_dynamic(&mut pass, &self.overlay.hand.buf, self.overlay.hand.quads, false);
        }
        if first {
            self.chunk_renderer.stats_drawn = stats.0;
            self.chunk_renderer.stats_quads = stats.1;
        }
        // HUD for this viewport
        if !self.demo {
            let scale = self.gui_scale();
            let mut hud = UiBatch::new(vp.w as f32, vp.h as f32, scale);
            let debug_lines = if self.show_debug && idx == 0 { Some(self.debug_lines()) } else { None };
            let p = &self.players[idx];
            crate::ui::hud::draw_hud(&mut hud, p, debug_lines.as_deref(), true);
            if self.players.len() > 1 {
                crate::ui::hud::draw_label(&mut hud, &p.name);
            }
            if idx == 0 && self.last_save_msg > 0.0 {
                hud.text_shadow(hud.width - 40.0, 4.0, 1.0, "Saved", [0.8, 1.0, 0.8, 1.0]);
            }
            if p.ui != OpenUi::None && p.ui != OpenUi::Dead {
                screens::draw(&mut hud, &self.world, p, p.cursor.0, p.cursor.1);
                if idx == 1 {
                    hud.tile(p.cursor.0 - 1.0, p.cursor.1 - 1.0, 6.0, 6.0, Tile::Crosshair, [1.0, 1.0, 0.3, 1.0]);
                }
            }
            self.ui.prepare(&self.gpu, &[&hud]);
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ui"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment { view, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store } })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            set_vp(&mut pass);
            self.ui.draw(&mut pass, &self.chunk_renderer, 0);
        }
        self.gpu.queue.submit(Some(enc.finish()));
    }

    /// Menu (and split-screen divider) drawn over the whole window.
    fn render_overlay_ui(&mut self, view: &wgpu::TextureView) {
        let scale = self.gui_scale();
        let mut batch = UiBatch::new(self.gpu.config.width as f32, self.gpu.config.height as f32, scale);
        if self.players.len() > 1 {
            let x = (batch.width * 0.5).floor();
            batch.rect(x - 1.0, 0.0, 2.0, batch.height, [0.0, 0.0, 0.0, 1.0]);
        }
        if self.in_menu {
            let (mx, my) = (self.input.cursor.0 / scale, self.input.cursor.1 / scale);
            self.menu.draw(&mut batch, &self.settings, mx, my);
        }
        if batch.verts.is_empty() {
            return;
        }
        let mut enc = self.gpu.device.create_command_encoder(&Default::default());
        self.ui.prepare(&self.gpu, &[&batch]);
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("menu"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment { view, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store } })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            self.ui.draw(&mut pass, &self.chunk_renderer, 0);
        }
        self.gpu.queue.submit(Some(enc.finish()));
    }
}
