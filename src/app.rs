//! Application: players, simulation ticks, chunk streaming, per-frame update and rendering.

use crate::audio::{material_sounds, Audio, Sound};
use crate::daytime::DayCycle;
use crate::entity::{Arrow, ItemDrop, PrimedTnt};
use crate::mobs::{spawn as mobspawn, Mob, MobCtx, MobEvent, MobKind};
use crate::input::Input;
use crate::player::interact::{self, Ctx, Interaction};
use crate::player::{GameMode, OpenUi, Player, PlayerEvent, PlayerInput};
use crate::redstone::{Redstone, RsEvent};
use crate::render::atlas;
use crate::render::camera::Frustum;
use crate::render::chunk_renderer::{ChunkRenderer, DynamicBuffer, Globals};
use crate::render::gpu::Gpu;
use crate::render::overlay::{self, OverlayRenderer};
use crate::render::sky::SkyRenderer;
use crate::render::ui2d::{UiBatch, UiRenderer};
use crate::ui::screens::{self, ScreenInput};
use crate::world::chunk::BlockEntity;
use crate::settings::Settings;
use crate::world::fluid::FluidSim;
use crate::world::gen::Generator;
use crate::world::noise::Rng;
use crate::world::worker::{Job, JobResult, WorkerPool};
use crate::world::{World, SEA_LEVEL};
use glam::Vec3;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;
use winit::keyboard::KeyCode;

pub const TICK_DT: f32 = 0.05;

pub struct App {
    pub gpu: Gpu,
    pub chunk_renderer: ChunkRenderer,
    pub sky: SkyRenderer,
    pub overlay: OverlayRenderer,
    pub ui: UiRenderer,
    pub entity_buf: DynamicBuffer,
    pub world: Arc<World>,
    pub generator: Arc<Generator>,
    pub pool: WorkerPool,
    pub input: Input,
    pub daytime: DayCycle,
    pub settings: Settings,
    pub cursor_grabbed: bool,
    pub players: Vec<Player>,
    pub fluids: FluidSim,
    pub drops: Vec<ItemDrop>,
    pub arrows: Vec<Arrow>,
    pub tnt: Vec<PrimedTnt>,
    pub rng: Rng,
    pub tick_accum: f32,
    pub ticks: u64,
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
    pub interactions: Vec<Interaction>,
    pub show_debug: bool,
    pub furnaces: HashSet<(i32, i32, i32)>,
    pub save: Option<Arc<std::sync::Mutex<crate::save::SaveManager>>>,
    pub world_name: String,
    pub flat: bool,
    pub autosave_timer: f32,
    pub mobs: Vec<Mob>,
    pub spawners: HashSet<(i32, i32, i32)>,
    pub audio: Option<Audio>,
    pub redstone: Redstone,
}

/// Find a land spawn near the origin.
pub fn find_spawn(g: &Generator) -> Vec3 {
    for r in (0..2000).step_by(16) {
        for i in 0..12 {
            let ang = i as f32 / 12.0 * std::f32::consts::TAU;
            let x = (ang.cos() * r as f32) as i32;
            let z = (ang.sin() * r as f32) as i32;
            let c = g.column(x, z);
            if c.height > SEA_LEVEL + 1 && !matches!(c.biome, crate::world::gen::Biome::Ocean | crate::world::gen::Biome::River) {
                return Vec3::new(x as f32 + 0.5, c.height as f32 + 1.0, z as f32 + 0.5);
            }
        }
    }
    Vec3::new(0.5, g.surface_height(0, 0) as f32 + 1.0, 0.5)
}

impl App {
    pub fn new(gpu: Gpu, seed: u64, settings: Settings, mode: GameMode) -> App {
        let atlas_data = atlas::generate();
        let atlas_gpu = atlas::upload(&gpu.device, &gpu.queue, &atlas_data);
        let chunk_renderer = ChunkRenderer::new(&gpu, &atlas_gpu);
        let sky = SkyRenderer::new(&gpu);
        let overlay = OverlayRenderer::new(&gpu, &chunk_renderer);
        let font_view = crate::ui::font::upload(&gpu.device, &gpu.queue);
        let ui = UiRenderer::new(&gpu, &atlas_gpu, &font_view);
        let entity_buf = DynamicBuffer::new(&gpu.device, 4096);
        let world = World::new(seed);
        let generator = Arc::new(Generator::new(seed));
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).saturating_sub(1).max(2);
        let pool = WorkerPool::new(world.clone(), generator.clone(), None, threads);
        let spawn = find_spawn(&generator);
        let settings_volume = settings.volume;
        let mut p1 = Player::new(0, "Player 1", spawn, mode);
        p1.spawn = spawn;
        App {
            gpu,
            chunk_renderer,
            sky,
            overlay,
            ui,
            entity_buf,
            world,
            generator,
            pool,
            input: Input::new(),
            daytime: DayCycle::new(),
            settings,
            cursor_grabbed: false,
            players: vec![p1],
            fluids: FluidSim::new(),
            drops: Vec::new(),
            arrows: Vec::new(),
            tnt: Vec::new(),
            rng: Rng::new(seed ^ 0xABCDEF),
            tick_accum: 0.0,
            ticks: 0,
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
            interactions: Vec::new(),
            show_debug: false,
            furnaces: HashSet::new(),
            save: None,
            world_name: String::new(),
            flat: false,
            autosave_timer: 0.0,
            mobs: Vec::new(),
            spawners: HashSet::new(),
            audio: Audio::new(settings_volume),
            redstone: Redstone::new(),
        }
    }

    pub fn sound(&mut self, s: Sound, gain: f32, pitch: f32) {
        if let Some(a) = self.audio.as_mut() {
            a.play(s, gain, pitch);
        }
    }

    pub fn sound_at(&mut self, s: Sound, pos: Vec3, gain: f32, pitch: f32) {
        if let Some(a) = self.audio.as_mut() {
            a.play_at(s, pos, gain, pitch);
        }
    }

    /// Should the mouse be captured for looking around?
    pub fn wants_grab(&self) -> bool {
        self.players[0].ui == OpenUi::None
    }

    pub fn gui_scale(&self) -> f32 {
        crate::ui::gui_scale(self.gpu.config.height)
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        self.gpu.resize(w, h);
    }

    /// Keyboard + mouse mapping for player 1.
    fn keyboard_input(&self) -> PlayerInput {
        let inp = &self.input;
        let k = |c: KeyCode| inp.pressed(c);
        let mut pi = PlayerInput::default();
        if !self.cursor_grabbed {
            return pi;
        }
        pi.forward = (k(KeyCode::KeyW) as i32 - k(KeyCode::KeyS) as i32) as f32;
        pi.strafe = (k(KeyCode::KeyD) as i32 - k(KeyCode::KeyA) as i32) as f32;
        pi.jump = k(KeyCode::Space);
        pi.jump_pressed = inp.just(KeyCode::Space);
        pi.sneak = k(KeyCode::ShiftLeft);
        pi.sprint = k(KeyCode::ControlLeft);
        pi.look_dx = inp.mouse_delta.0;
        pi.look_dy = inp.mouse_delta.1;
        pi.attack = inp.mouse_down[0];
        pi.attack_pressed = inp.mouse_just_pressed[0];
        pi.use_pressed = inp.mouse_just_pressed[1] || (inp.mouse_down[1] && self.frame % 12 == 0);
        pi.use_held = inp.mouse_down[1];
        pi.pick_block = inp.mouse_just_pressed[2];
        pi.scroll = inp.scroll.round() as i32;
        pi.inventory = inp.just(KeyCode::KeyE);
        pi.drop = inp.just(KeyCode::KeyQ);
        let digits = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6, KeyCode::Digit7, KeyCode::Digit8, KeyCode::Digit9];
        for (i, d) in digits.iter().enumerate() {
            if inp.just(*d) {
                pi.hotbar = Some(i);
            }
        }
        pi
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
        if self.input.just(KeyCode::BracketRight) {
            self.settings.render_distance = (self.settings.render_distance + 1).min(32);
        }
        if self.input.just(KeyCode::BracketLeft) {
            self.settings.render_distance = (self.settings.render_distance - 1).max(2);
        }

        if self.input.just(KeyCode::F3) {
            self.show_debug = !self.show_debug;
        }
        if let Some(a) = self.audio.as_mut() {
            let p = &self.players[0];
            a.begin_frame(p.eye(), p.right());
            let night = if self.daytime.is_night() { 1.0 } else { 0.0 };
            let alt = ((p.pos.y - 70.0) / 60.0).clamp(0.0, 1.0);
            let under = if p.head_in_water { 0.2 } else { 1.0 };
            a.set_ambient((0.18 + 0.35 * alt) * under, 0.08 + 0.12 * night, if night > 0.5 { 82.0 } else { 110.0 });
            if a.volume != self.settings.volume {
                a.set_volume(self.settings.volume);
            }
        }
        // --- container screens (player 1: mouse) ---
        {
            let ui_open = self.players[0].ui != OpenUi::None && self.players[0].ui != OpenUi::Dead;
            if ui_open {
                if self.input.just(KeyCode::Escape) || self.input.just(KeyCode::KeyE) {
                    screens::close(&mut self.players[0], &mut self.drops);
                } else {
                    let scale = self.gui_scale();
                    let batch = UiBatch::new(self.gpu.config.width as f32, self.gpu.config.height as f32, scale);
                    let sin = ScreenInput {
                        mx: self.input.cursor.0 / scale,
                        my: self.input.cursor.1 / scale,
                        left: self.input.mouse_just_pressed[0],
                        right: self.input.mouse_just_pressed[1],
                        shift: self.input.pressed(KeyCode::ShiftLeft),
                    };
                    screens::update(&self.world, &mut self.players[0], &sin, &batch, &mut self.drops);
                }
            }
        }
        // --- players ---
        let inputs: Vec<PlayerInput> = (0..self.players.len())
            .map(|i| {
                let mut pi = if i == 0 { self.keyboard_input() } else { PlayerInput::default() };
                if self.players[i].ui != OpenUi::None {
                    let dead = self.players[i].dead;
                    let jump = pi.jump_pressed;
                    let use_p = pi.use_pressed;
                    pi = PlayerInput::default();
                    if dead {
                        pi.jump_pressed = jump;
                        pi.use_pressed = use_p;
                    }
                }
                pi
            })
            .collect();
        let sens = self.settings.look_scale();
        let mut interactions = Vec::new();
        for i in 0..self.players.len() {
            let pin = inputs[i];
            // wait for the ground chunk before simulating
            let p = &self.players[i];
            let (cx, cz) = ((p.pos.x.floor() as i32) >> 4, (p.pos.z.floor() as i32) >> 4);
            if !self.world.has_chunk(cx, cz) {
                continue;
            }
            let events = self.players[i].update_physics(&self.world, &pin, dt, sens);
            let survival = self.players[i].tick_survival(dt);
            let ppos = self.players[i].pos;
            for ev in events.iter().chain(survival.iter()) {
                match ev {
                    PlayerEvent::Landed(d) => {
                        let dmg = (d - 3.0).floor();
                        if dmg > 0.0 {
                            self.players[i].damage(dmg);
                            self.sound_at(Sound::Fall, ppos, 1.0, 1.0);
                            self.sound_at(Sound::Hurt, ppos, 1.0, 1.0);
                        }
                    }
                    PlayerEvent::Step => {
                        let below = self.world.get_block(ppos.x.floor() as i32, (ppos.y - 0.1).floor() as i32, ppos.z.floor() as i32);
                        if below != crate::world::block::Block::Air {
                            let (_, _, step) = material_sounds(below);
                            let pitch = 0.9 + self.rng.f32() * 0.2;
                            self.sound_at(step, ppos, 0.5, pitch);
                        }
                    }
                    PlayerEvent::Hurt => self.sound_at(Sound::Hurt, ppos, 1.0, 1.0),
                    PlayerEvent::EnteredWater => self.sound_at(Sound::Splash, ppos, 0.8, 1.0),
                    _ => {}
                }
            }
            if self.players[i].dead && self.players[i].ui == OpenUi::Dead && self.players[i].hurt_timer > 0.45 {
                self.sound_at(Sound::Death, ppos, 1.0, 1.0);
            }
            // melee attack on mobs (checked before block interaction)
            let mut pin = pin;
            if pin.attack_pressed && !self.players[i].dead && self.players[i].ui == OpenUi::None {
                let eye = self.players[i].eye();
                let dir = self.players[i].look_dir();
                let reach = self.players[i].reach();
                let mut best: Option<(usize, f32)> = None;
                for (mi, m) in self.mobs.iter().enumerate() {
                    if m.dead {
                        continue;
                    }
                    if let Some(t) = m.ray_hit(eye, dir, reach) {
                        if best.map(|b| t < b.1).unwrap_or(true) {
                            best = Some((mi, t));
                        }
                    }
                }
                if let Some((mi, t)) = best {
                    let mut cache = crate::world::ChunkCache::new(&self.world);
                    let blocked = crate::player::raycast::raycast(&mut cache, eye, dir, reach, false).map(|h| h.dist < t).unwrap_or(false);
                    if !blocked {
                        let dmg = self.players[i].inventory.held().attack_damage();
                        let died = self.mobs[mi].damage(dmg, Some(eye));
                        let mpos = self.mobs[mi].position();
                        let kind = self.mobs[mi].kind;
                        self.sound_at(mob_sound(kind), mpos, 0.8, 1.3);
                        if died {
                            let drops = self.mobs[mi].drops(&mut self.rng);
                            for d in drops {
                                crate::entity::spawn_drop(&mut self.drops, mpos + Vec3::new(0.0, 0.5, 0.0), d, &mut self.rng);
                            }
                        }
                        if self.players[i].inventory.held().tool_info().is_some() {
                            self.players[i].inventory.damage_held(1);
                        }
                        self.players[i].swing = 0.0;
                        self.players[i].exhaustion += 0.1;
                        pin.attack = false;
                        pin.attack_pressed = false;
                        self.players[i].breaking = None;
                    }
                }
            }
            if self.players[i].dead {
                if pin.use_pressed || pin.jump_pressed {
                    let items = self.players[i].inventory.drain_all();
                    let pos = self.players[i].pos + Vec3::new(0.0, 0.5, 0.0);
                    for s in items {
                        crate::entity::spawn_drop(&mut self.drops, pos, s, &mut self.rng);
                    }
                    self.players[i].respawn();
                }
                continue;
            }
            let boxes: Vec<_> = self.players.iter().map(|p| p.aabb()).collect();
            let mut ctx = Ctx { world: &self.world, fluids: &mut self.fluids, drops: &mut self.drops, rng: &mut self.rng, player_boxes: &boxes };
            let acts = interact::update(&mut ctx, &mut self.players[i], &pin, dt);
            for a in &acts {
                match a {
                    Interaction::OpenUi(ui) => {
                        self.players[i].ui = *ui;
                        if let OpenUi::Furnace(p) = ui {
                            self.furnaces.insert(*p);
                        }
                    }
                    Interaction::Placed { pos, block: crate::world::block::Block::Furnace } => {
                        self.furnaces.insert(*pos);
                    }
                    Interaction::Sleep { .. } => {
                        let pos = self.players[i].pos;
                        self.players[i].bed_spawn = Some(pos);
                        if self.daytime.is_night() {
                            self.daytime.skip_to_morning();
                        }
                    }
                    Interaction::ShootArrow { origin, dir } => {
                        self.arrows.push(Arrow::new(*origin + *dir * 0.5, *dir * 30.0, 4.0, 0));
                    }
                    Interaction::Explode { pos } => {
                        self.world.set_block(pos.0, pos.1, pos.2, 0);
                        self.tnt.push(PrimedTnt::new(Vec3::new(pos.0 as f32 + 0.5, pos.1 as f32, pos.2 as f32 + 0.5)));
                    }
                    _ => {}
                }
            }
            for a in &acts {
                match a {
                    Interaction::Broke { pos, .. } | Interaction::Placed { pos, .. } => self.redstone.mark(*pos),
                    Interaction::Toggled { pos, block } => {
                        if *block == crate::world::block::Block::Button {
                            self.redstone.press_button(*pos);
                        } else {
                            self.redstone.mark(*pos);
                        }
                    }
                    _ => {}
                }
                let (s, pos, gain, pitch) = match a {
                    Interaction::Broke { pos, block } => (material_sounds(*block).0, *pos, 1.0, 1.0),
                    Interaction::Hit { pos, block } => (material_sounds(*block).2, *pos, 0.6, 0.8),
                    Interaction::Placed { pos, block } => (material_sounds(*block).1, *pos, 1.0, 1.0),
                    Interaction::Toggled { pos, block } => (if *block == crate::world::block::Block::Door { Sound::Door } else { Sound::Lever }, *pos, 1.0, 1.0),
                    Interaction::OpenUi(OpenUi::Chest(p)) => (Sound::ChestOpen, *p, 1.0, 1.0),
                    Interaction::Ate => {
                        self.sound(Sound::Eat, 1.0, 1.0);
                        continue;
                    }
                    Interaction::ShootArrow { .. } => {
                        self.sound(Sound::Bow, 1.0, 1.0);
                        continue;
                    }
                    Interaction::Explode { pos } => (Sound::Fuse, *pos, 1.0, 1.0),
                    _ => continue,
                };
                let p = Vec3::new(pos.0 as f32 + 0.5, pos.1 as f32 + 0.5, pos.2 as f32 + 0.5);
                self.sound_at(s, p, gain, pitch);
            }
            interactions.extend(acts);
            if pin.inventory && self.players[i].ui == OpenUi::None {
                self.players[i].ui = OpenUi::Inventory;
            }
        }
        self.interactions = interactions;

        // --- fixed ticks ---
        self.tick_accum += dt;
        let mut guard = 0;
        while self.tick_accum >= TICK_DT && guard < 5 {
            self.tick_accum -= TICK_DT;
            guard += 1;
            self.tick();
        }

        // --- entities ---
        for d in self.drops.iter_mut() {
            d.update(&self.world, dt);
        }
        let mut picked: Option<Vec3> = None;
        for i in 0..self.drops.len() {
            if self.drops[i].pickup_delay > 0.0 {
                continue;
            }
            let dp = self.drops[i].position();
            for p in self.players.iter_mut() {
                if p.dead {
                    continue;
                }
                if dp.distance(p.pos + Vec3::new(0.0, 0.8, 0.0)) < 1.6 {
                    let rem = p.inventory.add(self.drops[i].stack);
                    if rem.count != self.drops[i].stack.count {
                        picked = Some(dp);
                    }
                    self.drops[i].stack = rem;
                    if rem.is_empty() {
                        break;
                    }
                }
            }
        }
        if let Some(p) = picked {
            let pitch = 0.9 + self.rng.f32() * 0.3;
            self.sound_at(Sound::PickUp, p, 0.7, pitch);
        }
        self.drops.retain(|d| !d.stack.is_empty() && d.age < crate::entity::DROP_LIFETIME);
        self.update_mobs(dt);
        // arrows: move and hit mobs / players
        let mut arrow_hits: Vec<(usize, Vec3)> = Vec::new();
        for (ai, a) in self.arrows.iter_mut().enumerate() {
            let was_stuck = a.stuck;
            let hit_block = a.update(&self.world, dt);
            if hit_block && !was_stuck {
                arrow_hits.push((ai, a.position()));
            }
        }
        for (_, p) in &arrow_hits {
            self.sound_at(Sound::ArrowHit, *p, 0.8, 1.0);
        }
        let mut remove_arrows = Vec::new();
        for ai in 0..self.arrows.len() {
            let a = &self.arrows[ai];
            if a.stuck || a.age < 0.05 {
                continue;
            }
            let p = a.position();
            let dmg = a.damage;
            if a.owner == 0 {
                for m in self.mobs.iter_mut() {
                    if !m.dead && m.aabb().intersects(&crate::player::physics::Aabb::from_center(p - Vec3::new(0.0, 0.1, 0.0), 0.15, 0.2)) {
                        let from = p - a.velocity().normalize_or_zero();
                        let died = m.damage(dmg, Some(from));
                        let mpos = m.position();
                        let kind = m.kind;
                        if died {
                            let drops = m.drops(&mut self.rng);
                            for d in drops {
                                crate::entity::spawn_drop(&mut self.drops, mpos + Vec3::new(0.0, 0.5, 0.0), d, &mut self.rng);
                            }
                        }
                        remove_arrows.push(ai);
                        self.sound_at(mob_sound(kind), mpos, 0.8, 1.3);
                        break;
                    }
                }
            } else {
                for pl in self.players.iter_mut() {
                    if !pl.dead && pl.aabb().intersects(&crate::player::physics::Aabb::from_center(p - Vec3::new(0.0, 0.1, 0.0), 0.15, 0.2)) {
                        pl.damage(dmg);
                        pl.vel += a.velocity().normalize_or_zero() * 3.0;
                        remove_arrows.push(ai);
                        break;
                    }
                }
            }
        }
        remove_arrows.sort_unstable();
        remove_arrows.dedup();
        for ai in remove_arrows.into_iter().rev() {
            self.arrows.remove(ai);
        }
        self.arrows.retain(|a| a.age < 60.0 && a.position().y > -10.0);
        let mut explosions = Vec::new();
        for t in self.tnt.iter_mut() {
            if t.update(&self.world, dt) {
                explosions.push(t.position());
            }
        }
        self.tnt.retain(|t| t.fuse > 0.0);
        for e in explosions {
            self.explode(e, 4.0);
        }

        self.stream_chunks();
        self.collect_results();
        self.input.end_frame();
    }

    /// One 20 TPS world tick.
    fn tick(&mut self) {
        self.ticks += 1;
        self.fluids.step(&self.world);
        if self.ticks % 4 == 0 {
            self.random_ticks();
        }
        self.tick_furnaces();
        self.tick_redstone();
        let players: Vec<Vec3> = self.players.iter().filter(|p| !p.dead).map(|p| p.pos).collect();
        if self.ticks % 20 == 0 {
            let sun = self.daytime.sun_level();
            let day = !self.daytime.is_night();
            mobspawn::natural_spawn(&mut self.mobs, &self.world, &players, &mut self.rng, sun, day);
        }
        if self.ticks % 100 == 0 {
            mobspawn::despawn(&mut self.mobs, &self.world, &players);
        }
        let sun = self.daytime.sun_level();
        let list: Vec<(i32, i32, i32)> = self.spawners.iter().copied().collect();
        for p in list {
            if !mobspawn::tick_spawner(&self.world, p, &mut self.mobs, &players, &mut self.rng, sun) {
                self.spawners.remove(&p);
            }
        }
    }

    fn update_mobs(&mut self, dt: f32) {
        let players: Vec<(Vec3, bool)> = self.players.iter().map(|p| (p.pos, p.dead)).collect();
        let sun = self.daytime.sun_level();
        let mut events = Vec::new();
        for m in self.mobs.iter_mut() {
            let p = m.position();
            if !self.world.is_loaded(p.x.floor() as i32, p.z.floor() as i32) {
                continue;
            }
            let mut ctx = MobCtx { world: &self.world, players: &players, rng: &mut self.rng, sun_level: sun };
            events.extend(m.update(&mut ctx, dt));
        }
        for e in events {
            match e {
                MobEvent::AttackPlayer { player, damage, from } => {
                    let mut snd = None;
                    if let Some(p) = self.players.get_mut(player) {
                        if p.damage(damage) {
                            snd = Some((Sound::Death, p.pos));
                        } else {
                            let push = (p.pos - from).normalize_or_zero();
                            p.vel += Vec3::new(push.x, 0.0, push.z) * 5.0 + Vec3::new(0.0, 3.5, 0.0);
                            snd = Some((Sound::Hurt, p.pos));
                        }
                    }
                    if let Some((s, pos)) = snd {
                        self.sound_at(s, pos, 1.0, 1.0);
                    }
                }
                MobEvent::ShootArrow { origin, dir } => {
                    self.arrows.push(Arrow::new(origin, dir * 22.0, 3.0, 1));
                    self.sound_at(Sound::Bow, origin, 0.8, 1.0);
                }
                MobEvent::Explode { pos, power } => {
                    self.explode(pos, power);
                    self.sound_at(Sound::Explode, pos, 1.0, 1.0);
                }
                MobEvent::Died { pos, drops, kind } => {
                    for d in drops {
                        crate::entity::spawn_drop(&mut self.drops, pos + Vec3::new(0.0, 0.5, 0.0), d, &mut self.rng);
                    }
                    self.sound_at(mob_sound(kind), pos, 0.9, 0.8);
                }
                MobEvent::Hurt { kind, pos } => self.sound_at(mob_sound(kind), pos, 0.6, 1.2),
                MobEvent::Ambient { kind, pos } => {
                    let pitch = 0.95 + self.rng.f32() * 0.1;
                    self.sound_at(mob_sound(kind), pos, 0.7, pitch);
                }
                MobEvent::FuseStart { pos } => self.sound_at(Sound::Fuse, pos, 1.0, 1.0),
                MobEvent::LayEgg { pos } => {
                    crate::entity::spawn_drop(&mut self.drops, pos, crate::player::items::ItemStack::item(crate::player::items::Item::Egg, 1), &mut self.rng);
                    self.sound_at(Sound::Egg, pos, 0.6, 1.0);
                }
            }
        }
        self.mobs.retain(|m| !(m.dead && m.death_timer > 1.2));
    }

    /// Pressure plates + redstone evaluation.
    fn tick_redstone(&mut self) {
        use crate::world::block::{vox_block, vox_meta, voxel, Block};
        // pressure plates: anything standing on them
        let mut feet: Vec<Vec3> = self.players.iter().filter(|p| !p.dead).map(|p| p.pos).collect();
        feet.extend(self.mobs.iter().filter(|m| !m.dead).map(|m| m.position()));
        feet.extend(self.drops.iter().map(|d| d.position()));
        let mut now_pressed: HashSet<(i32, i32, i32)> = HashSet::new();
        for f in &feet {
            for dy in [0.0f32, -0.05] {
                let p = (f.x.floor() as i32, (f.y + dy).floor() as i32, f.z.floor() as i32);
                if self.world.get_block(p.0, p.1, p.2) == Block::PressurePlate {
                    now_pressed.insert(p);
                }
            }
        }
        for p in now_pressed.iter() {
            if !self.redstone.pressed_plates.contains(p) {
                let v = self.world.get(p.0, p.1, p.2);
                self.world.set_block(p.0, p.1, p.2, voxel(Block::PressurePlate, vox_meta(v) | 1));
                self.redstone.mark(*p);
                self.sound_at(Sound::Click, Vec3::new(p.0 as f32 + 0.5, p.1 as f32, p.2 as f32 + 0.5), 0.6, 1.0);
            }
        }
        let released: Vec<(i32, i32, i32)> = self.redstone.pressed_plates.iter().filter(|p| !now_pressed.contains(p)).copied().collect();
        for p in released {
            let v = self.world.get(p.0, p.1, p.2);
            if vox_block(v) == Block::PressurePlate {
                self.world.set_block(p.0, p.1, p.2, voxel(Block::PressurePlate, vox_meta(v) & !1));
            }
            self.redstone.mark(p);
        }
        self.redstone.pressed_plates = now_pressed;
        let events = self.redstone.step(&self.world);
        for e in events {
            match e {
                RsEvent::PrimeTnt(p) => {
                    let c = Vec3::new(p.0 as f32 + 0.5, p.1 as f32, p.2 as f32 + 0.5);
                    self.tnt.push(PrimedTnt::new(c));
                    self.sound_at(Sound::Fuse, c, 1.0, 1.0);
                }
                RsEvent::Piston(p) => self.sound_at(Sound::Piston, Vec3::new(p.0 as f32 + 0.5, p.1 as f32 + 0.5, p.2 as f32 + 0.5), 0.8, 1.0),
                RsEvent::Door(p) => self.sound_at(Sound::Door, Vec3::new(p.0 as f32 + 0.5, p.1 as f32 + 0.5, p.2 as f32 + 0.5), 0.8, 1.0),
                RsEvent::Click(p) => self.sound_at(Sound::Lever, Vec3::new(p.0 as f32 + 0.5, p.1 as f32 + 0.5, p.2 as f32 + 0.5), 0.5, 0.8),
            }
        }
    }

    /// Advance every active furnace and keep its block's lit state in sync.
    fn tick_furnaces(&mut self) {
        let mut done = Vec::new();
        let list: Vec<(i32, i32, i32)> = self.furnaces.iter().copied().collect();
        for p in list {
            let v = self.world.get(p.0, p.1, p.2);
            let b = crate::world::block::vox_block(v);
            if !matches!(b, crate::world::block::Block::Furnace | crate::world::block::Block::FurnaceLit) {
                done.push(p);
                continue;
            }
            let mut lit = false;
            let mut idle = false;
            let r = self.world.with_block_entity(p.0, p.1, p.2, |be| {
                if let BlockEntity::Furnace(f) = be {
                    lit = f.tick();
                    idle = f.burn_left == 0 && f.progress == 0 && f.input.map(|s| crate::player::furnace::smelt_result(s.id).is_none()).unwrap_or(true);
                }
            });
            if r.is_none() {
                done.push(p);
                continue;
            }
            let want = if lit { crate::world::block::Block::FurnaceLit } else { crate::world::block::Block::Furnace };
            if b != want {
                self.world.set_block(p.0, p.1, p.2, crate::world::block::voxel(want, crate::world::block::vox_meta(v)));
            }
            let open = self.players.iter().any(|pl| pl.ui == OpenUi::Furnace(p));
            if idle && !open {
                done.push(p);
            }
        }
        for p in done {
            self.furnaces.remove(&p);
        }
    }

    /// Crop growth near players.
    fn random_ticks(&mut self) {
        let positions: Vec<Vec3> = self.players.iter().map(|p| p.pos).collect();
        for pos in positions {
            for _ in 0..6 {
                let x = pos.x as i32 + self.rng.range(-24, 25);
                let z = pos.z as i32 + self.rng.range(-24, 25);
                let y = pos.y as i32 + self.rng.range(-8, 9);
                let v = self.world.get(x, y, z);
                let b = crate::world::block::vox_block(v);
                let meta = crate::world::block::vox_meta(v);
                if b == crate::world::block::Block::Wheat && meta < 7 && self.rng.chance(0.3) {
                    self.world.set_block(x, y, z, crate::world::block::voxel(b, meta + 1));
                }
            }
        }
    }

    /// Explosion: destroys blocks in a sphere and damages players.
    pub fn explode(&mut self, center: Vec3, power: f32) {
        let r = power.ceil() as i32;
        let (cx, cy, cz) = (center.x.floor() as i32, center.y.floor() as i32, center.z.floor() as i32);
        let boxes: Vec<_> = self.players.iter().map(|p| p.aabb()).collect();
        let mut chain = Vec::new();
        for dy in -r..=r {
            for dz in -r..=r {
                for dx in -r..=r {
                    let d2 = (dx * dx + dy * dy + dz * dz) as f32;
                    if d2 > power * power {
                        continue;
                    }
                    let (x, y, z) = (cx + dx, cy + dy, cz + dz);
                    let v = self.world.get(x, y, z);
                    if v == 0 {
                        continue;
                    }
                    let b = crate::world::block::vox_block(v);
                    let p = crate::world::block::props(b.id());
                    if p.hardness < 0.0 || b == crate::world::block::Block::Obsidian || crate::world::block::is_fluid(v) {
                        continue;
                    }
                    let resist = (p.hardness * 0.3).min(2.0);
                    if self.rng.f32() * (1.0 + resist) < 1.0 - (d2.sqrt() / power) * 0.6 {
                        if b == crate::world::block::Block::Tnt {
                            chain.push(Vec3::new(x as f32 + 0.5, y as f32, z as f32 + 0.5));
                        }
                        let drop = self.rng.chance(0.3);
                        let mut ctx = Ctx { world: &self.world, fluids: &mut self.fluids, drops: &mut self.drops, rng: &mut self.rng, player_boxes: &boxes };
                        interact::destroy_block(&mut ctx, (x, y, z), drop);
                        self.redstone.mark((x, y, z));
                    }
                }
            }
        }
        for c in chain {
            self.world.set_block(c.x.floor() as i32, c.y.floor() as i32, c.z.floor() as i32, 0);
            let mut t = PrimedTnt::new(c);
            t.fuse = 0.3 + self.rng.f32() * 0.7;
            self.tnt.push(t);
        }
        for p in self.players.iter_mut() {
            let d = p.pos.distance(center);
            if d < power * 2.0 {
                let dmg = ((1.0 - d / (power * 2.0)) * power * 5.0).round();
                p.damage(dmg);
                let push = (p.pos - center).normalize_or_zero() * (1.0 - d / (power * 2.0)) * 12.0;
                p.vel += push + Vec3::new(0.0, 4.0, 0.0);
            }
        }
    }

    fn player_chunk(&self) -> (i32, i32) {
        let p = &self.players[0];
        ((p.pos.x.floor() as i32) >> 4, (p.pos.z.floor() as i32) >> 4)
    }

    fn stream_chunks(&mut self) {
        let (pcx, pcz) = self.player_chunk();
        let rd = self.settings.render_distance;
        let moved = (pcx, pcz) != self.last_player_chunk;
        self.last_player_chunk = (pcx, pcz);
        let anchors: Vec<(i32, i32)> = self.players.iter().map(|p| ((p.pos.x.floor() as i32) >> 4, (p.pos.z.floor() as i32) >> 4)).collect();
        let dist = |cx: i32, cz: i32| anchors.iter().map(|(ax, az)| (cx - ax).abs().max((cz - az).abs())).min().unwrap_or(0);

        if self.frame % 60 == 0 {
            let far = rd + 3;
            for (cx, cz) in self.world.chunk_keys() {
                if dist(cx, cz) > far {
                    if self.pending_mesh.keys().any(|k| k.0 == cx && k.2 == cz) {
                        continue;
                    }
                    self.world.remove_chunk(cx, cz);
                    self.chunk_renderer.remove_column(cx, cz);
                }
            }
        }

        let max_gen_inflight = self.threads * 3;
        if (moved || self.frame % 15 == 0) && self.pending_gen.len() < max_gen_inflight {
            let mut wanted: Vec<(i32, (i32, i32))> = Vec::new();
            let gr = rd + 1;
            for (ax, az) in &anchors {
                for dz in -gr..=gr {
                    for dx in -gr..=gr {
                        let key = (ax + dx, az + dz);
                        if self.pending_gen.contains(&key) || self.world.has_chunk(key.0, key.1) {
                            continue;
                        }
                        wanted.push((dx * dx + dz * dz, key));
                    }
                }
            }
            wanted.sort_by_key(|w| w.0);
            wanted.dedup_by_key(|w| w.1);
            for (_, key) in wanted.into_iter().take(max_gen_inflight - self.pending_gen.len()) {
                self.pending_gen.insert(key);
                self.pool.submit(Job::Generate { cx: key.0, cz: key.1 });
            }
        }

        let max_mesh_inflight = self.threads * 4;
        if self.frame % 3 == 0 && self.pending_mesh.len() < max_mesh_inflight {
            let mut wanted: Vec<(i32, (i32, i32, i32), u32)> = Vec::new();
            let chunks = self.world.chunks.read().unwrap();
            let py = (self.players[0].pos.y as i32) >> 4;
            for (cx, cz) in chunks.keys() {
                let d = dist(*cx, *cz);
                if d > rd {
                    continue;
                }
                let (cx, cz) = (*cx, *cz);
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
                let c = chunks.get(&(cx, cz)).unwrap().read().unwrap();
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
                    let dy = (sy as i32) - py;
                    wanted.push((d * d + dy * dy / 2, key, sub.version));
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
        let rd = self.settings.render_distance;
        let anchors: Vec<(i32, i32)> = self.players.iter().map(|p| ((p.pos.x.floor() as i32) >> 4, (p.pos.z.floor() as i32) >> 4)).collect();
        for r in self.pool.poll() {
            match r {
                JobResult::Generated { chunk } => {
                    let key = (chunk.cx, chunk.cz);
                    self.pending_gen.remove(&key);
                    let near = anchors.iter().any(|(ax, az)| (key.0 - ax).abs() <= rd + 3 && (key.1 - az).abs() <= rd + 3);
                    if !near {
                        continue;
                    }
                    for (k, be) in chunk.block_entities.iter() {
                        if let BlockEntity::Spawner { .. } = be {
                            self.spawners.insert((chunk.cx * 16 + k.0 as i32, k.1 as i32, chunk.cz * 16 + k.2 as i32));
                        }
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

    /// Lines for the F3 overlay.
    pub fn debug_lines(&self) -> Vec<String> {
        let p = &self.players[0];
        let (bx, by, bz) = (p.pos.x.floor() as i32, p.pos.y.floor() as i32, p.pos.z.floor() as i32);
        let biome = crate::world::gen::Biome::from_id(self.world.biome_at(bx, bz)).name();
        let (sky, blk) = self.world.light_at(bx, by, bz);
        let facing = match crate::world::block::facing_from_yaw(p.yaw) {
            0 => "north (-Z)",
            1 => "east (+X)",
            2 => "south (+Z)",
            _ => "west (-X)",
        };
        vec![
            format!("Blockhaven 0.1 | {:.0} fps | {} ({})", self.fps, self.gpu.adapter_name, self.gpu.backend),
            format!("XYZ: {:.2} / {:.2} / {:.2}  block {} {} {}", p.pos.x, p.pos.y, p.pos.z, bx, by, bz),
            format!("Chunk: {} {}  facing {}  yaw {:.1} pitch {:.1}", bx >> 4, bz >> 4, facing, p.yaw.to_degrees(), p.pitch.to_degrees()),
            format!("Biome: {}  light sky {} block {}", biome, sky, blk),
            format!("Chunks: {} loaded, {} sub-meshes drawn, {} quads", self.world.chunk_count(), self.chunk_renderer.stats_drawn, self.chunk_renderer.stats_quads),
            format!("Pending: gen {} mesh {}  workers {}  drops {}  arrows {}  mobs {}", self.pending_gen.len(), self.pending_mesh.len(), self.threads, self.drops.len(), self.arrows.len(), self.mobs.len()),
            format!("Time: {} day {}  sun {:.2}  tick {}  fluids {}  redstone {}", self.daytime.clock(), self.daytime.day_number(), self.daytime.sun_level(), self.ticks, self.fluids.pending(), self.redstone.pending()),
            format!("Health {:.1} hunger {:.1} sat {:.1}  mode {:?}{}", p.health, p.hunger, p.saturation, p.mode, if p.flying { " flying" } else { "" }),
            format!("Render distance {}  fov {:.0}  seed {}", self.settings.render_distance, self.settings.fov, self.world.seed),
        ]
    }

    pub fn is_idle(&self) -> bool {
        self.pending_gen.is_empty() && self.pending_mesh.is_empty() && self.pool.in_flight == 0
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
        let mut enc = self.gpu.device.create_command_encoder(&Default::default());
        let player = &self.players[0];
        let camera = player.camera(self.gpu.aspect(), self.settings.fov);
        let vp = camera.view_proj();
        let frustum = Frustum::from_matrix(vp);
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
        let globals = Globals::new(vp, camera.pos, fog_color, fog_start, fog_end, self.daytime.sun_level(), self.daytime.time_of_day());
        self.chunk_renderer.write_globals(&self.gpu, &globals);
        self.sky.update(&self.gpu, camera.proj(), camera.forward(), self.daytime.sun_dir(), zenith, horizon, self.daytime.sun_level(), self.daytime.time_of_day(), 0.37);
        self.chunk_renderer.cull(&frustum, camera.pos, fog_end + 24.0);

        // entities (item drops, arrows, tnt)
        let mut ent = Vec::new();
        for d in &self.drops {
            let p = d.position();
            let l = self.world.light_at(p.x.floor() as i32, (p.y + 0.2).floor() as i32, p.z.floor() as i32);
            overlay::drop_quads(p, &d.stack, d.age, l, &mut ent);
        }
        for m in &self.mobs {
            let p = m.position();
            let l = self.world.light_at(p.x.floor() as i32, (p.y + 0.5).floor() as i32, p.z.floor() as i32);
            if p.distance(camera.pos) < 64.0 {
                crate::mobs::models::mob_quads(m, l, &mut ent);
            }
        }
        for a in &self.arrows {
            let p = a.position();
            let l = self.world.light_at(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
            let dir = if a.stuck { Vec3::X } else { a.velocity().normalize_or_zero() };
            let side = dir.cross(Vec3::Y).normalize_or_zero();
            let side = if side == Vec3::ZERO { Vec3::Z } else { side };
            let up = side.cross(dir);
            overlay::box_quads(p, Vec3::new(0.03, 0.03, 0.3), [side, up, -dir], [crate::render::atlas::Tile::ArrowSide.index(); 6], l, [0, 1, 2, 3, 4, 5], &mut ent);
        }
        for t in &self.tnt {
            let p = t.position() + Vec3::new(0.0, 0.5, 0.0);
            let l = self.world.light_at(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
            let flash = ((t.fuse * 8.0).sin() > 0.0) && t.fuse < 3.0;
            let tiles = if flash {
                [crate::render::atlas::Tile::TntPrimed.index(); 6]
            } else {
                crate::world::block::face_tiles(crate::world::block::Block::Tnt, 0).map(|t| t.index())
            };
            overlay::box_quads_world(p - Vec3::splat(0.49), p + Vec3::splat(0.49), tiles, l, &mut ent);
        }
        self.entity_buf.upload(&self.gpu.device, &self.gpu.queue, &ent);

        // selection outline + crack
        let mut lines = Vec::new();
        let mut crack = Vec::new();
        {
            let mut cache = crate::world::ChunkCache::new(&self.world);
            if player.ui == OpenUi::None && !player.dead {
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
        }
        self.overlay.upload_lines(&self.gpu, &lines);
        self.overlay.crack.upload(&self.gpu.device, &self.gpu.queue, &crack);
        // hand
        let eye = player.eye();
        let hl = self.world.light_at(eye.x.floor() as i32, eye.y.floor() as i32, eye.z.floor() as i32);
        let hand = if player.dead || player.ui != OpenUi::None { Vec::new() } else { overlay::held_quads(&camera, &player.inventory.held(), player.swing, hl, player.eating) };
        self.overlay.hand.upload(&self.gpu.device, &self.gpu.queue, &hand);

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
            self.chunk_renderer.draw_dynamic(&mut pass, &self.entity_buf.buf, self.entity_buf.quads, false);
            self.chunk_renderer.draw_translucent(&mut pass);
            self.chunk_renderer.draw_dynamic(&mut pass, &self.overlay.crack.buf, self.overlay.crack.quads, true);
            self.overlay.draw_lines(&mut pass, &self.chunk_renderer);
        }
        // hand pass: depth cleared so the arm is never clipped by terrain
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
            self.chunk_renderer.draw_dynamic(&mut pass, &self.overlay.hand.buf, self.overlay.hand.quads, false);
        }
        self.chunk_renderer.stats_drawn = stats.0;
        self.chunk_renderer.stats_quads = stats.1;
        // ---- HUD / screens ----
        let scale = self.gui_scale();
        let mut hud = UiBatch::new(self.gpu.config.width as f32, self.gpu.config.height as f32, scale);
        let debug_lines = if self.show_debug { Some(self.debug_lines()) } else { None };
        let p = &self.players[0];
        crate::ui::hud::draw_hud(&mut hud, p, debug_lines.as_deref(), true);
        if p.ui != OpenUi::None && p.ui != OpenUi::Dead {
            screens::draw(&mut hud, &self.world, p, self.input.cursor.0 / scale, self.input.cursor.1 / scale);
        }
        self.ui.prepare(&self.gpu, &[&hud]);
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ui"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment { view, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store } })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            self.ui.draw(&mut pass, &self.chunk_renderer, 0);
        }
        self.gpu.queue.submit(Some(enc.finish()));

        let p = &self.players[0];
        self.stats = format!(
            "Blockhaven | {:.0} fps | pos {:.1} {:.1} {:.1} | hp {:.0} food {:.0} | chunks {} | drawn {} subs, {} quads | pending gen {} mesh {} | rd {} | {} | held {}",
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
            rd,
            self.daytime.clock(),
            p.inventory.held().name()
        );
    }
}

/// Voice for a mob kind.
pub fn mob_sound(kind: MobKind) -> Sound {
    match kind {
        MobKind::Pig => Sound::Pig,
        MobKind::Cow => Sound::Cow,
        MobKind::Sheep => Sound::Sheep,
        MobKind::Chicken => Sound::Chicken,
        MobKind::Zombie => Sound::Zombie,
        MobKind::Skeleton => Sound::Skeleton,
        MobKind::Creeper => Sound::Fuse,
    }
}
