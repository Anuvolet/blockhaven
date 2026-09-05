pub mod crafting;
pub mod furnace;
pub mod interact;
pub mod inventory;
pub mod items;
pub mod physics;
pub mod raycast;

use crate::player::inventory::Inventory;
use crate::player::physics::{self as phys, Aabb};
use crate::render::camera::Camera;
use crate::world::block::Block;
use crate::world::{ChunkCache, World};
use glam::Vec3;
use serde::{Deserialize, Serialize};

pub const WIDTH: f32 = 0.6;
pub const HEIGHT: f32 = 1.8;
pub const EYE: f32 = 1.62;
pub const EYE_SNEAK: f32 = 1.42;
pub const REACH: f32 = 5.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum GameMode {
    Survival,
    Creative,
}

/// Abstract per-frame input for one player (keyboard/mouse or gamepad).
#[derive(Clone, Copy, Debug, Default)]
pub struct PlayerInput {
    pub forward: f32,
    pub strafe: f32,
    pub jump: bool,
    pub jump_pressed: bool,
    pub sneak: bool,
    pub sprint: bool,
    pub look_dx: f32,
    pub look_dy: f32,
    pub attack: bool,
    pub attack_pressed: bool,
    pub use_pressed: bool,
    pub use_held: bool,
    pub hotbar: Option<usize>,
    pub scroll: i32,
    pub inventory: bool,
    pub drop: bool,
    pub pick_block: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlayerEvent {
    Landed(f32),
    Jumped,
    Step,
    EnteredWater,
    Hurt,
}

/// Which UI screen this player has open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenUi {
    None,
    Inventory,
    CraftingTable,
    Chest((i32, i32, i32)),
    Furnace((i32, i32, i32)),
    Dead,
}

#[derive(Clone, Debug)]
pub struct Player {
    pub id: usize,
    pub name: String,
    pub pos: Vec3,
    pub vel: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
    pub flying: bool,
    pub sneaking: bool,
    pub sprinting: bool,
    pub in_water: bool,
    pub head_in_water: bool,
    pub in_lava: bool,
    pub on_ladder: bool,
    pub health: f32,
    pub hunger: f32,
    pub saturation: f32,
    pub exhaustion: f32,
    pub regen_timer: f32,
    pub starve_timer: f32,
    pub lava_timer: f32,
    pub fire_ticks: f32,
    pub drown_timer: f32,
    pub air: f32,
    pub dead: bool,
    pub fall_start: f32,
    pub inventory: Inventory,
    pub mode: GameMode,
    pub breaking: Option<((i32, i32, i32), f32)>,
    pub place_cooldown: f32,
    pub attack_cooldown: f32,
    pub swing: f32,
    pub last_jump_press: f32,
    pub time: f32,
    pub spawn: Vec3,
    pub bed_spawn: Option<Vec3>,
    pub hurt_timer: f32,
    pub bob: f32,
    pub step_dist: f32,
    pub ui: OpenUi,
    pub eating: f32,
    pub fov_mult: f32,
    pub sleeping: bool,
}

impl Player {
    pub fn new(id: usize, name: &str, spawn: Vec3, mode: GameMode) -> Player {
        Player {
            id,
            name: name.to_string(),
            pos: spawn,
            vel: Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: false,
            flying: false,
            sneaking: false,
            sprinting: false,
            in_water: false,
            head_in_water: false,
            in_lava: false,
            on_ladder: false,
            health: 20.0,
            hunger: 20.0,
            saturation: 5.0,
            exhaustion: 0.0,
            regen_timer: 0.0,
            starve_timer: 0.0,
            lava_timer: 0.0,
            fire_ticks: 0.0,
            drown_timer: 0.0,
            air: 10.0,
            dead: false,
            fall_start: spawn.y,
            inventory: Inventory::new(),
            mode,
            breaking: None,
            place_cooldown: 0.0,
            attack_cooldown: 0.0,
            swing: 1.0,
            last_jump_press: -10.0,
            time: 0.0,
            spawn,
            bed_spawn: None,
            hurt_timer: 0.0,
            bob: 0.0,
            step_dist: 0.0,
            ui: OpenUi::None,
            eating: 0.0,
            fov_mult: 1.0,
            sleeping: false,
        }
    }

    pub fn eye_height(&self) -> f32 {
        if self.sneaking && !self.flying {
            EYE_SNEAK
        } else {
            EYE
        }
    }

    pub fn eye(&self) -> Vec3 {
        self.pos + Vec3::new(0.0, self.eye_height(), 0.0)
    }

    pub fn aabb(&self) -> Aabb {
        Aabb::from_center(self.pos, WIDTH * 0.5, HEIGHT)
    }

    pub fn camera(&self, aspect: f32, fov: f32) -> Camera {
        let mut c = Camera::new();
        c.pos = self.eye();
        // view bobbing
        if self.on_ground && !self.flying {
            c.pos.y += (self.bob * 2.0).sin().abs() * 0.05;
            c.pos += self.right() * (self.bob).sin() * 0.03;
        }
        c.yaw = self.yaw;
        c.pitch = self.pitch;
        c.aspect = aspect;
        c.fov_deg = fov * self.fov_mult;
        c
    }

    pub fn forward_flat(&self) -> Vec3 {
        let (s, c) = self.yaw.sin_cos();
        Vec3::new(-s, 0.0, -c)
    }
    pub fn right(&self) -> Vec3 {
        let (s, c) = self.yaw.sin_cos();
        Vec3::new(c, 0.0, -s)
    }
    pub fn look_dir(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        Vec3::new(-sy * cp, sp, -cy * cp)
    }

    pub fn reach(&self) -> f32 {
        if self.mode == GameMode::Creative {
            REACH + 1.0
        } else {
            REACH
        }
    }

    pub fn can_sprint(&self) -> bool {
        self.hunger > 6.0 || self.mode == GameMode::Creative
    }

    /// Integrate movement for one frame.
    pub fn update_physics(&mut self, world: &World, input: &PlayerInput, dt: f32, sensitivity: f32) -> Vec<PlayerEvent> {
        let mut events = Vec::new();
        self.time += dt;
        if self.dead {
            return events;
        }
        // look
        self.yaw -= input.look_dx * sensitivity;
        self.pitch -= input.look_dy * sensitivity;
        let lim = 89.5f32.to_radians();
        self.pitch = self.pitch.clamp(-lim, lim);
        self.yaw = self.yaw.rem_euclid(std::f32::consts::TAU);

        // flight toggle by double tap
        if input.jump_pressed {
            if self.mode == GameMode::Creative && self.time - self.last_jump_press < 0.3 {
                self.flying = !self.flying;
                self.vel.y = 0.0;
            }
            self.last_jump_press = self.time;
        }
        if self.mode != GameMode::Creative {
            self.flying = false;
        }

        // environment
        let feet = self.pos + Vec3::new(0.0, 0.4, 0.0);
        let fluid_feet = phys::fluid_at(world, feet);
        let fluid_eye = phys::fluid_at(world, self.eye());
        let was_in_water = self.in_water;
        self.in_water = fluid_feet == Some(Block::Water) || fluid_eye == Some(Block::Water);
        self.head_in_water = fluid_eye == Some(Block::Water);
        self.in_lava = fluid_feet == Some(Block::Lava) || fluid_eye == Some(Block::Lava);
        if self.in_water && !was_in_water && self.vel.y < -3.0 {
            events.push(PlayerEvent::EnteredWater);
        }
        self.on_ladder = phys::is_ladder(world, self.pos + Vec3::new(0.0, 0.5, 0.0)) || phys::is_ladder(world, self.pos + Vec3::new(0.0, 1.2, 0.0));

        self.sneaking = input.sneak && !self.flying;
        self.sprinting = input.sprint && input.forward > 0.5 && !self.sneaking && self.can_sprint() && !self.head_in_water;

        // movement
        let speed = if self.flying {
            if input.sprint { 22.0 } else { 11.0 }
        } else if self.in_water || self.in_lava {
            2.4
        } else if self.sneaking {
            1.4
        } else if self.sprinting {
            5.7
        } else {
            4.3
        };
        let mut wish = self.forward_flat() * input.forward + self.right() * input.strafe;
        if wish.length_squared() > 1.0 {
            wish = wish.normalize();
        }
        wish *= speed;
        let k = if self.flying { 8.0 } else if self.on_ground { 18.0 } else if self.in_water { 6.0 } else { 3.0 };
        let a = 1.0 - (-dt * k).exp();
        self.vel.x += (wish.x - self.vel.x) * a;
        self.vel.z += (wish.z - self.vel.z) * a;

        if self.flying {
            let vy = (if input.jump { 1.0 } else { 0.0 }) - (if input.sneak { 1.0 } else { 0.0 });
            let target = vy * (if input.sprint { 16.0 } else { 8.0 });
            self.vel.y += (target - self.vel.y) * (1.0 - (-dt * 10.0).exp());
        } else if self.on_ladder {
            let target = if input.jump || input.forward > 0.3 { 2.8 } else if input.sneak { 0.0 } else { -2.5 };
            self.vel.y += (target - self.vel.y) * (1.0 - (-dt * 12.0).exp());
            self.fall_start = self.pos.y;
        } else if self.in_water || self.in_lava {
            let target = if input.jump { 3.2 } else if input.sneak { -3.0 } else { -1.6 };
            self.vel.y += (target - self.vel.y) * (1.0 - (-dt * 5.0).exp());
            self.fall_start = self.pos.y;
            // jump out of water at the edge
            if input.jump && self.on_ground {
                self.vel.y = phys::JUMP_SPEED * 0.6;
            }
        } else {
            self.vel.y -= phys::GRAVITY * dt;
            if self.vel.y < -phys::TERMINAL {
                self.vel.y = -phys::TERMINAL;
            }
            if input.jump && self.on_ground {
                self.vel.y = phys::JUMP_SPEED;
                self.on_ground = false;
                events.push(PlayerEvent::Jumped);
                self.exhaustion += if self.sprinting { 0.2 } else { 0.05 };
            }
        }

        // integrate with collision
        let mut cache = ChunkCache::new(world);
        let mut aabb = self.aabb();
        let mut delta = self.vel * dt;
        if self.sneaking && self.on_ground {
            // edge protection: cancel horizontal motion that leaves the ground
            let test = aabb.offset(Vec3::new(delta.x, 0.0, 0.0));
            if !phys::has_ground_below(&mut cache, &test) {
                delta.x = 0.0;
                self.vel.x = 0.0;
            }
            let test = aabb.offset(Vec3::new(delta.x, 0.0, delta.z));
            if !phys::has_ground_below(&mut cache, &test) {
                delta.z = 0.0;
                self.vel.z = 0.0;
            }
        }
        let was_on_ground = self.on_ground;
        let (applied, res) = phys::move_aabb(&mut cache, &mut aabb, delta);
        self.pos = Vec3::new(aabb.center().x, aabb.min.y, aabb.center().z);
        if res.hit_x {
            self.vel.x = 0.0;
        }
        if res.hit_z {
            self.vel.z = 0.0;
        }
        if res.hit_ceiling && self.vel.y > 0.0 {
            self.vel.y = 0.0;
        }
        self.on_ground = res.on_ground;
        if self.on_ground && self.vel.y < 0.0 {
            self.vel.y = 0.0;
        }
        // fall tracking
        if self.on_ground && !was_on_ground {
            let fall = self.fall_start - self.pos.y;
            if fall > 3.0 && !self.in_water && !self.flying {
                events.push(PlayerEvent::Landed(fall));
            }
            self.fall_start = self.pos.y;
        }
        if !self.on_ground {
            if self.vel.y >= 0.0 || self.flying || self.in_water {
                self.fall_start = self.pos.y;
            }
        } else {
            self.fall_start = self.pos.y;
        }
        // footsteps / bobbing
        let hspeed = Vec3::new(applied.x, 0.0, applied.z).length();
        if self.on_ground && hspeed > 0.001 {
            self.bob += hspeed * 1.9;
            self.step_dist += hspeed;
            if self.step_dist > 1.6 {
                self.step_dist = 0.0;
                events.push(PlayerEvent::Step);
            }
            self.exhaustion += hspeed * if self.sprinting { 0.02 } else { 0.002 };
        } else {
            self.bob += (0.0 - (self.bob % std::f32::consts::PI)) * 0.1;
        }
        // timers
        self.place_cooldown = (self.place_cooldown - dt).max(0.0);
        self.attack_cooldown = (self.attack_cooldown - dt).max(0.0);
        self.hurt_timer = (self.hurt_timer - dt).max(0.0);
        self.swing = (self.swing + dt * 4.0).min(1.0);
        let target_fov = if self.sprinting { 1.12 } else { 1.0 };
        self.fov_mult += (target_fov - self.fov_mult) * (1.0 - (-dt * 10.0).exp());
        events
    }

    /// Hunger, natural regeneration, starvation, drowning and lava. Returns true if the player died.
    pub fn tick_survival(&mut self, dt: f32) -> Vec<PlayerEvent> {
        let mut events = Vec::new();
        if self.dead || self.mode == GameMode::Creative {
            self.air = 10.0;
            return events;
        }
        // exhaustion -> saturation -> hunger
        self.exhaustion += dt * 0.004;
        while self.exhaustion >= 4.0 {
            self.exhaustion -= 4.0;
            if self.saturation > 0.0 {
                self.saturation = (self.saturation - 1.0).max(0.0);
            } else {
                self.hunger = (self.hunger - 1.0).max(0.0);
            }
        }
        // regeneration
        if self.hunger >= 18.0 && self.health < 20.0 {
            self.regen_timer += dt;
            if self.regen_timer >= 4.0 {
                self.regen_timer = 0.0;
                self.health = (self.health + 1.0).min(20.0);
                self.exhaustion += 0.6;
            }
        } else {
            self.regen_timer = 0.0;
        }
        // starvation
        if self.hunger <= 0.0 {
            self.starve_timer += dt;
            if self.starve_timer >= 4.0 {
                self.starve_timer = 0.0;
                if self.health > 1.0 {
                    self.damage(1.0);
                    events.push(PlayerEvent::Hurt);
                }
            }
        }
        // drowning
        if self.head_in_water {
            self.air -= dt;
            if self.air <= 0.0 {
                self.drown_timer += dt;
                if self.drown_timer >= 1.0 {
                    self.drown_timer = 0.0;
                    self.damage(2.0);
                    events.push(PlayerEvent::Hurt);
                }
            }
        } else {
            self.air = (self.air + dt * 3.0).min(10.0);
        }
        // lava / fire
        if self.in_lava {
            self.fire_ticks = 4.0;
            self.lava_timer += dt;
            if self.lava_timer >= 0.5 {
                self.lava_timer = 0.0;
                self.damage(4.0);
                events.push(PlayerEvent::Hurt);
            }
        } else if self.fire_ticks > 0.0 {
            if self.in_water {
                self.fire_ticks = 0.0;
            } else {
                self.fire_ticks -= dt;
                self.lava_timer += dt;
                if self.lava_timer >= 1.0 {
                    self.lava_timer = 0.0;
                    self.damage(1.0);
                    events.push(PlayerEvent::Hurt);
                }
            }
        }
        events
    }

    /// Apply damage after armour. Returns true if this killed the player.
    pub fn damage(&mut self, amount: f32) -> bool {
        if self.dead || self.mode == GameMode::Creative {
            return false;
        }
        let def = self.inventory.armor_defense() as f32;
        let dmg = amount * (1.0 - (def * 0.04).min(0.8));
        self.health -= dmg;
        self.hurt_timer = 0.5;
        self.inventory.damage_armor(1);
        if self.health <= 0.0 {
            self.health = 0.0;
            self.dead = true;
            self.ui = OpenUi::Dead;
            return true;
        }
        false
    }

    pub fn heal(&mut self, amount: f32) {
        self.health = (self.health + amount).min(20.0);
    }

    pub fn eat(&mut self, hunger: u8, saturation: f32) {
        self.hunger = (self.hunger + hunger as f32).min(20.0);
        self.saturation = (self.saturation + saturation).min(self.hunger);
    }

    pub fn respawn(&mut self) {
        let at = self.bed_spawn.unwrap_or(self.spawn);
        self.pos = at;
        self.vel = Vec3::ZERO;
        self.health = 20.0;
        self.hunger = 20.0;
        self.saturation = 5.0;
        self.exhaustion = 0.0;
        self.air = 10.0;
        self.fire_ticks = 0.0;
        self.dead = false;
        self.ui = OpenUi::None;
        self.fall_start = at.y;
        self.breaking = None;
    }

    /// Saved state.
    pub fn to_save(&self) -> PlayerSave {
        PlayerSave {
            pos: [self.pos.x, self.pos.y, self.pos.z],
            yaw: self.yaw,
            pitch: self.pitch,
            health: self.health,
            hunger: self.hunger,
            saturation: self.saturation,
            inventory: self.inventory.clone(),
            mode: self.mode,
            bed_spawn: self.bed_spawn.map(|v| [v.x, v.y, v.z]),
            flying: self.flying,
        }
    }

    pub fn apply_save(&mut self, s: &PlayerSave) {
        self.pos = Vec3::from(s.pos);
        self.yaw = s.yaw;
        self.pitch = s.pitch;
        self.health = s.health;
        self.hunger = s.hunger;
        self.saturation = s.saturation;
        self.inventory = s.inventory.clone();
        self.mode = s.mode;
        self.bed_spawn = s.bed_spawn.map(Vec3::from);
        self.flying = s.flying && s.mode == GameMode::Creative;
        self.dead = self.health <= 0.0;
        if self.dead {
            self.respawn();
        }
        self.fall_start = self.pos.y;
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlayerSave {
    pub pos: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub health: f32,
    pub hunger: f32,
    pub saturation: f32,
    pub inventory: Inventory,
    pub mode: GameMode,
    pub bed_spawn: Option<[f32; 3]>,
    pub flying: bool,
}
