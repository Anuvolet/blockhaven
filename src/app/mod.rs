//! Application state: world lifecycle (menu demo world, new/load/save), players, per-frame update.
//! Simulation lives in `sim`, chunk streaming in `stream`, drawing in `render`, input in `input_map`.

mod input_map;
mod render;
mod sim;
mod stream;

use crate::audio::{Audio, Sound};
use crate::daytime::DayCycle;
use crate::entity::{Arrow, ItemDrop, PrimedTnt};
use crate::input::Input;
use crate::mobs::{Mob, MobKind};
use crate::player::interact::Interaction;
use crate::player::{GameMode, OpenUi, Player};
use crate::redstone::Redstone;
use crate::render::atlas;
use crate::render::chunk_renderer::{ChunkRenderer, DynamicBuffer};
use crate::render::gpu::Gpu;
use crate::render::overlay::OverlayRenderer;
use crate::render::sky::SkyRenderer;
use crate::render::ui2d::UiRenderer;
use crate::save::{LevelData, SaveManager};
use crate::settings::Settings;
use crate::ui::menu::{Menu, MenuAction, Screen};
use crate::world::fluid::FluidSim;
use crate::world::gen::Generator;
use crate::world::noise::Rng;
use crate::world::worker::WorkerPool;
use crate::world::{World, SEA_LEVEL};
use glam::Vec3;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use winit::keyboard::KeyCode;

pub const TICK_DT: f32 = 0.05;
pub const AUTOSAVE_SECS: f32 = 120.0;

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
    pub mobs: Vec<Mob>,
    pub spawners: HashSet<(i32, i32, i32)>,
    pub furnaces: HashSet<(i32, i32, i32)>,
    pub redstone: Redstone,
    pub rng: Rng,
    pub tick_accum: f32,
    pub ticks: u64,
    pub(crate) pending_gen: HashSet<(i32, i32)>,
    pub(crate) pending_mesh: HashMap<(i32, i32, i32), u32>,
    pub(crate) last_player_chunk: (i32, i32),
    pub(crate) frame: u64,
    last_frame: Instant,
    fps_accum: f32,
    fps_frames: u32,
    pub fps: f32,
    pub stats: String,
    pub(crate) threads: usize,
    pub interactions: Vec<Interaction>,
    pub show_debug: bool,
    pub audio: Option<Audio>,
    pub save: Option<Arc<Mutex<SaveManager>>>,
    pub world_name: String,
    pub flat: bool,
    pub autosave_timer: f32,
    pub menu: Menu,
    pub in_menu: bool,
    /// Menu backdrop world: no simulation, slow camera orbit.
    pub demo: bool,
    pub gilrs: Option<gilrs::Gilrs>,
    pub(crate) pad_prev: HashSet<gilrs::Button>,
    pub want_fullscreen: Option<bool>,
    pub quit: bool,
    pub last_save_msg: f32,
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

fn worker_count() -> usize {
    std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).saturating_sub(1).max(2)
}

impl App {
    /// Creates the renderer and a random demo world shown behind the main menu.
    pub fn new(gpu: Gpu, settings: Settings) -> App {
        let atlas_data = atlas::generate();
        let atlas_gpu = atlas::upload(&gpu.device, &gpu.queue, &atlas_data);
        let chunk_renderer = ChunkRenderer::new(&gpu, &atlas_gpu);
        let sky = SkyRenderer::new(&gpu);
        let overlay = OverlayRenderer::new(&gpu, &chunk_renderer);
        let font_view = crate::ui::font::upload(&gpu.device, &gpu.queue);
        let ui = UiRenderer::new(&gpu, &atlas_gpu, &font_view);
        let entity_buf = DynamicBuffer::new(&gpu.device, 4096);
        let seed = crate::world::noise::seed_from_str("");
        let world = World::new(seed);
        let generator = Arc::new(Generator::new(seed));
        let threads = worker_count();
        let pool = WorkerPool::new(world.clone(), generator.clone(), None, threads);
        let spawn = find_spawn(&generator);
        let mut p1 = Player::new(0, "Player 1", spawn + Vec3::new(0.0, 22.0, 0.0), GameMode::Creative);
        p1.flying = true;
        p1.pitch = -0.35;
        let audio = Audio::new(settings.volume);
        let mut menu = Menu::new();
        menu.refresh_worlds();
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
            mobs: Vec::new(),
            spawners: HashSet::new(),
            furnaces: HashSet::new(),
            redstone: Redstone::new(),
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
            audio,
            save: None,
            world_name: String::new(),
            flat: false,
            autosave_timer: 0.0,
            menu,
            in_menu: true,
            demo: true,
            gilrs: gilrs::Gilrs::new().ok(),
            pad_prev: HashSet::new(),
            want_fullscreen: None,
            quit: false,
            last_save_msg: 0.0,
        }
    }

    /// Reset every per-world container and swap in a fresh world.
    fn reset_world(&mut self, seed: u64, flat: bool, persist: Option<&str>) {
        self.world = World::new(seed);
        self.generator = Arc::new(if flat { Generator::flat(seed) } else { Generator::new(seed) });
        self.save = persist.map(|name| Arc::new(Mutex::new(SaveManager::open(name))));
        self.pool = WorkerPool::new(self.world.clone(), self.generator.clone(), self.save.clone(), self.threads);
        self.chunk_renderer.meshes.clear();
        self.pending_gen.clear();
        self.pending_mesh.clear();
        self.last_player_chunk = (i32::MAX, i32::MAX);
        self.fluids = FluidSim::new();
        self.drops.clear();
        self.arrows.clear();
        self.tnt.clear();
        self.mobs.clear();
        self.spawners.clear();
        self.furnaces.clear();
        self.redstone = Redstone::new();
        self.rng = Rng::new(seed ^ 0xABCDEF);
        self.tick_accum = 0.0;
        self.ticks = 0;
        self.flat = flat;
        self.world_name = persist.unwrap_or("").to_string();
        self.autosave_timer = 0.0;
    }

    /// Create a new world and start playing. `persist == false` keeps it in memory only.
    pub fn new_world(&mut self, name: &str, seed: u64, mode: GameMode, flat: bool, persist: bool) {
        self.reset_world(seed, flat, if persist { Some(name) } else { None });
        let spawn = find_spawn(&self.generator);
        let mut p1 = Player::new(0, "Player 1", spawn, mode);
        p1.spawn = spawn;
        self.players = vec![p1];
        self.daytime = DayCycle::new();
        self.in_menu = false;
        self.demo = false;
        self.menu.in_game = true;
        if persist {
            self.save_world();
        }
    }

    /// Load an existing world from disk.
    pub fn load_world(&mut self, name: &str) -> bool {
        let Some(level) = crate::save::load_level(name) else { return false };
        self.reset_world(level.seed, level.flat, Some(name));
        let spawn = Vec3::from(level.spawn);
        let mut players = Vec::new();
        for (i, ps) in level.players.iter().enumerate() {
            let mut p = Player::new(i, if i == 0 { "Player 1" } else { "Player 2" }, spawn, ps.mode);
            p.spawn = spawn;
            p.apply_save(ps);
            players.push(p);
        }
        if players.is_empty() {
            let mut p = Player::new(0, "Player 1", spawn, level.mode);
            p.spawn = spawn;
            players.push(p);
        }
        println!("loaded world '{}' (seed {}, {} players, day {})", name, level.seed, players.len(), (level.time / crate::daytime::DAY_LENGTH_SECS) as u32);
        self.players = players;
        self.drops = level.drops;
        self.daytime = DayCycle { time: level.time };
        self.in_menu = false;
        self.demo = false;
        self.menu.in_game = true;
        true
    }

    /// Write level data + every modified chunk to disk.
    pub fn save_world(&mut self) {
        let Some(save) = self.save.clone() else { return };
        let level = LevelData {
            version: crate::save::FORMAT_VERSION,
            name: self.world_name.clone(),
            seed: self.world.seed,
            time: self.daytime.time,
            spawn: self.players[0].spawn.to_array(),
            players: self.players.iter().map(|p| p.to_save()).collect(),
            mode: self.players[0].mode,
            flat: self.flat,
            drops: self.drops.clone(),
        };
        let mut sm = save.lock().unwrap();
        if let Err(e) = sm.save_level(&level) {
            eprintln!("failed to save level: {e}");
        }
        let keys = self.world.chunk_keys();
        for (cx, cz) in keys {
            if let Some(c) = self.world.get_chunk(cx, cz) {
                let mut c = c.write().unwrap();
                if c.dirty_save {
                    sm.store_chunk(&c);
                    c.dirty_save = false;
                }
            }
        }
        if let Err(e) = sm.flush() {
            eprintln!("failed to write region files: {e}");
        }
        self.last_save_msg = 2.0;
    }

    /// Save (if playing) and return to the title screen with a fresh demo world.
    pub fn quit_to_title(&mut self) {
        if !self.demo {
            self.save_world();
        }
        let seed = crate::world::noise::seed_from_str("");
        self.reset_world(seed, false, None);
        let spawn = find_spawn(&self.generator);
        let mut p1 = Player::new(0, "Player 1", spawn + Vec3::new(0.0, 22.0, 0.0), GameMode::Creative);
        p1.flying = true;
        p1.pitch = -0.35;
        self.players = vec![p1];
        self.daytime = DayCycle::new();
        self.demo = true;
        self.in_menu = true;
        self.menu.in_game = false;
        self.menu.screen = Screen::Main;
        self.menu.refresh_worlds();
    }

    /// Called when the window closes.
    pub fn on_quit(&mut self) {
        if !self.demo {
            self.save_world();
        }
        self.settings.save();
    }

    pub fn add_player(&mut self) {
        if self.players.len() >= 2 || self.demo {
            return;
        }
        let p1 = &self.players[0];
        let mut p2 = Player::new(1, "Player 2", p1.pos, p1.mode);
        p2.spawn = p1.spawn;
        p2.yaw = p1.yaw;
        self.players.push(p2);
        self.players[0].say("Player 2 joined");
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
        !self.in_menu && self.players[0].ui == OpenUi::None
    }

    pub fn gui_scale(&self) -> f32 {
        crate::ui::gui_scale(self.gpu.config.height)
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        self.gpu.resize(w, h);
    }

    pub fn is_idle(&self) -> bool {
        self.pending_gen.is_empty() && self.pending_mesh.is_empty() && self.pool.in_flight == 0
    }

    /// Per-frame entry point.
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
        self.poll_gamepad();
        if self.input.just(KeyCode::F11) {
            self.settings.fullscreen = !self.settings.fullscreen;
            self.want_fullscreen = Some(self.settings.fullscreen);
            self.settings.save();
        }
        if self.input.just(KeyCode::F3) {
            self.show_debug = !self.show_debug;
        }
        self.last_save_msg = (self.last_save_msg - dt).max(0.0);

        if self.in_menu {
            self.update_menu(dt);
        } else {
            // open the pause menu
            let pause_pressed = (self.input.just(KeyCode::Escape) && self.players[0].ui == OpenUi::None) || self.pad_just(gilrs::Button::Start) || self.input.just(KeyCode::Numpad8);
            if pause_pressed {
                self.in_menu = true;
                self.menu.in_game = true;
                self.menu.screen = Screen::Pause;
            } else {
                if self.input.just(KeyCode::F2) {
                    self.add_player();
                }
                self.update_playing(dt);
            }
        }
        if self.demo {
            // slow orbit of the demo camera
            self.players[0].yaw += dt * 0.04;
            self.daytime.advance(dt as f64 * 8.0);
        }
        self.stream_chunks();
        self.collect_results();
        self.input.end_frame();
    }

    fn update_menu(&mut self, dt: f32) {
        let scale = self.gui_scale();
        let batch = crate::render::ui2d::UiBatch::new(self.gpu.config.width as f32, self.gpu.config.height as f32, scale);
        let min = self.menu_input(scale);
        let action = self.menu.update(&min, &batch, &mut self.settings, dt);
        match action {
            MenuAction::None => {}
            MenuAction::NewWorld { name, seed, mode, flat } => {
                let seed = crate::world::noise::seed_from_str(&seed);
                if crate::save::list_worlds().iter().any(|w| w.eq_ignore_ascii_case(&crate::save::sanitize(&name))) {
                    self.menu.status = "A world with that name already exists".to_string();
                } else {
                    self.menu.status.clear();
                    self.new_world(&name, seed, mode, flat, true);
                }
            }
            MenuAction::LoadWorld(name) => {
                if !self.load_world(&name) {
                    self.menu.status = "Could not load that world".to_string();
                }
            }
            MenuAction::DeleteWorld(name) => {
                crate::save::delete_world(&name);
                self.menu.refresh_worlds();
            }
            MenuAction::Resume => {
                self.in_menu = false;
            }
            MenuAction::SaveAndQuit => self.quit_to_title(),
            MenuAction::QuitApp => self.quit = true,
            MenuAction::SettingsChanged => {
                self.settings.save();
                self.want_fullscreen = Some(self.settings.fullscreen);
                self.gpu.set_vsync(self.settings.vsync);
                if let Some(a) = self.audio.as_mut() {
                    a.set_volume(self.settings.volume);
                }
            }
            MenuAction::AddPlayer => {
                self.add_player();
                self.in_menu = false;
            }
        }
        if min.click {
            self.sound(Sound::Click, 0.6, 1.0);
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
            format!("Render distance {}  fov {:.0}  seed {}  world '{}'", self.settings.render_distance, self.settings.fov, self.world.seed, self.world_name),
        ]
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
