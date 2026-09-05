//! Mobs: passive animals and hostile monsters. State, physics and damage live here; behaviour is
//! in `ai`, box models in `models`, spawning rules in `spawn`.

pub mod ai;
pub mod models;
pub mod spawn;

use crate::player::items::{Item, ItemStack};
use crate::player::physics::{self, Aabb};
use crate::world::block::Block;
use crate::world::noise::Rng;
use crate::world::{ChunkCache, World};
use glam::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, Hash)]
pub enum MobKind {
    Pig,
    Cow,
    Sheep,
    Chicken,
    Zombie,
    Skeleton,
    Creeper,
}

impl MobKind {
    pub fn hostile(self) -> bool {
        matches!(self, MobKind::Zombie | MobKind::Skeleton | MobKind::Creeper)
    }
    /// (half width, height) in blocks.
    pub fn size(self) -> (f32, f32) {
        match self {
            MobKind::Pig => (0.45, 0.9),
            MobKind::Cow => (0.45, 1.4),
            MobKind::Sheep => (0.45, 1.3),
            MobKind::Chicken => (0.2, 0.7),
            MobKind::Zombie => (0.3, 1.95),
            MobKind::Skeleton => (0.3, 1.95),
            MobKind::Creeper => (0.3, 1.7),
        }
    }
    pub fn max_health(self) -> f32 {
        match self {
            MobKind::Pig => 10.0,
            MobKind::Cow => 10.0,
            MobKind::Sheep => 8.0,
            MobKind::Chicken => 4.0,
            MobKind::Zombie => 20.0,
            MobKind::Skeleton => 20.0,
            MobKind::Creeper => 20.0,
        }
    }
    pub fn speed(self) -> f32 {
        match self {
            MobKind::Pig => 2.0,
            MobKind::Cow => 1.8,
            MobKind::Sheep => 1.9,
            MobKind::Chicken => 2.2,
            MobKind::Zombie => 2.6,
            MobKind::Skeleton => 2.6,
            MobKind::Creeper => 2.5,
        }
    }
    pub fn burns_in_daylight(self) -> bool {
        matches!(self, MobKind::Zombie | MobKind::Skeleton)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AiState {
    pub wander: [f32; 2],
    pub wander_timer: f32,
    pub flee_timer: f32,
    pub attack_cooldown: f32,
    pub shoot_cooldown: f32,
    pub fuse: f32,
    pub jump_cooldown: f32,
    pub ambient_timer: f32,
    pub target: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Mob {
    pub kind: MobKind,
    pub pos: [f32; 3],
    pub vel: [f32; 3],
    pub yaw: f32,
    pub head_yaw: f32,
    pub health: f32,
    pub on_ground: bool,
    pub in_water: bool,
    pub age: f32,
    pub hurt_timer: f32,
    pub anim: f32,
    pub anim_speed: f32,
    pub ai: AiState,
    pub burning: f32,
    pub sheared: bool,
    pub regrow: f32,
    pub egg_timer: f32,
    pub dead: bool,
    pub death_timer: f32,
    pub fall_start: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MobEvent {
    AttackPlayer { player: usize, damage: f32, from: Vec3 },
    ShootArrow { origin: Vec3, dir: Vec3 },
    Explode { pos: Vec3, power: f32 },
    Died { kind: MobKind, pos: Vec3, drops: Vec<ItemStack> },
    Hurt { kind: MobKind, pos: Vec3 },
    Ambient { kind: MobKind, pos: Vec3 },
    FuseStart { pos: Vec3 },
    LayEgg { pos: Vec3 },
}

/// Read-only view of the world the AI needs.
pub struct MobCtx<'a> {
    pub world: &'a World,
    /// (position, dead) per player
    pub players: &'a [(Vec3, bool)],
    pub rng: &'a mut Rng,
    pub sun_level: f32,
}

impl Mob {
    pub fn new(kind: MobKind, pos: Vec3, rng: &mut Rng) -> Mob {
        Mob {
            kind,
            pos: pos.to_array(),
            vel: [0.0; 3],
            yaw: rng.f32() * std::f32::consts::TAU,
            head_yaw: 0.0,
            health: kind.max_health(),
            on_ground: false,
            in_water: false,
            age: 0.0,
            hurt_timer: 0.0,
            anim: 0.0,
            anim_speed: 0.0,
            ai: AiState { wander_timer: rng.f32() * 3.0, ambient_timer: 3.0 + rng.f32() * 10.0, ..Default::default() },
            burning: 0.0,
            sheared: false,
            regrow: 0.0,
            egg_timer: 60.0 + rng.f32() * 120.0,
            dead: false,
            death_timer: 0.0,
            fall_start: pos.y,
        }
    }

    pub fn position(&self) -> Vec3 {
        Vec3::from(self.pos)
    }
    pub fn velocity(&self) -> Vec3 {
        Vec3::from(self.vel)
    }
    pub fn aabb(&self) -> Aabb {
        let (hw, h) = self.kind.size();
        Aabb::from_center(self.position(), hw, h)
    }
    pub fn eye(&self) -> Vec3 {
        self.position() + Vec3::new(0.0, self.kind.size().1 * 0.85, 0.0)
    }

    /// Apply damage with optional knockback from a source position. Returns true if the mob died.
    pub fn damage(&mut self, amount: f32, from: Option<Vec3>) -> bool {
        if self.dead {
            return false;
        }
        self.health -= amount;
        self.hurt_timer = 0.5;
        self.ai.flee_timer = 4.0;
        if let Some(f) = from {
            let d = (self.position() - f).normalize_or_zero();
            let mut v = self.velocity();
            v += Vec3::new(d.x, 0.0, d.z) * 6.0 + Vec3::new(0.0, 4.5, 0.0);
            self.vel = v.to_array();
        }
        if self.health <= 0.0 {
            self.health = 0.0;
            self.dead = true;
            return true;
        }
        false
    }

    pub fn drops(&self, rng: &mut Rng) -> Vec<ItemStack> {
        let mut v = Vec::new();
        let n = |rng: &mut Rng, lo: u32, hi: u32| lo + rng.below(hi - lo + 1);
        match self.kind {
            MobKind::Pig => {
                let k = n(rng, 1, 3) as u8;
                v.push(ItemStack::item(if self.burning > 0.0 { Item::PorkchopCooked } else { Item::PorkchopRaw }, k));
            }
            MobKind::Cow => {
                v.push(ItemStack::item(if self.burning > 0.0 { Item::BeefCooked } else { Item::BeefRaw }, n(rng, 1, 3) as u8));
                let l = n(rng, 0, 2) as u8;
                if l > 0 {
                    v.push(ItemStack::item(Item::Leather, l));
                }
            }
            MobKind::Sheep => {
                if !self.sheared {
                    v.push(ItemStack::block(Block::Wool, 1));
                }
            }
            MobKind::Chicken => {
                v.push(ItemStack::item(if self.burning > 0.0 { Item::ChickenCooked } else { Item::ChickenRaw }, 1));
                let f = n(rng, 0, 2) as u8;
                if f > 0 {
                    v.push(ItemStack::item(Item::Feather, f));
                }
            }
            MobKind::Zombie => {
                let k = n(rng, 0, 2) as u8;
                if k > 0 {
                    v.push(ItemStack::item(Item::RottenFlesh, k));
                }
                if rng.chance(0.03) {
                    v.push(ItemStack::item(Item::IronIngot, 1));
                }
            }
            MobKind::Skeleton => {
                let b = n(rng, 0, 2) as u8;
                if b > 0 {
                    v.push(ItemStack::item(Item::Bone, b));
                }
                let a = n(rng, 0, 2) as u8;
                if a > 0 {
                    v.push(ItemStack::item(Item::Arrow, a));
                }
                if rng.chance(0.08) {
                    v.push(ItemStack::item(Item::Bow, 1));
                }
            }
            MobKind::Creeper => {
                let g = n(rng, 0, 2) as u8;
                if g > 0 {
                    v.push(ItemStack::item(Item::Gunpowder, g));
                }
            }
        }
        v
    }

    /// One frame of simulation.
    pub fn update(&mut self, ctx: &mut MobCtx, dt: f32) -> Vec<MobEvent> {
        let mut events = Vec::new();
        self.age += dt;
        self.hurt_timer = (self.hurt_timer - dt).max(0.0);
        if self.dead {
            self.death_timer += dt;
            let mut v = self.velocity();
            v.x *= 0.5f32.powf(dt * 20.0);
            v.z *= 0.5f32.powf(dt * 20.0);
            v.y -= physics::GRAVITY * dt;
            let mut cache = ChunkCache::new(ctx.world);
            let mut aabb = self.aabb();
            let (_, res) = physics::move_aabb(&mut cache, &mut aabb, v * dt);
            if res.on_ground {
                v.y = 0.0;
            }
            self.pos = [aabb.center().x, aabb.min.y, aabb.center().z];
            self.vel = v.to_array();
            return events;
        }
        // AI decides a wish direction + jump
        let (wish, jump, ev) = ai::think(self, ctx, dt);
        events.extend(ev);
        // environment
        let world = ctx.world;
        let feet = self.position() + Vec3::new(0.0, 0.3, 0.0);
        let fluid = physics::fluid_at(world, feet);
        self.in_water = fluid == Some(Block::Water);
        let in_lava = fluid == Some(Block::Lava);
        if in_lava {
            self.burning = 5.0;
            if (self.age * 2.0) as i32 % 2 == 0 && self.hurt_timer <= 0.0 {
                if self.damage(4.0, None) {
                    events.push(MobEvent::Died { kind: self.kind, pos: self.position(), drops: self.drops(ctx.rng) });
                }
            }
        }
        // daylight burning
        if self.kind.burns_in_daylight() && ctx.sun_level > 0.62 {
            let e = self.eye();
            let sky = world.sky_light(e.x.floor() as i32, e.y.floor() as i32, e.z.floor() as i32);
            if sky >= 15 && !self.in_water {
                self.burning = 1.0;
            }
        }
        if self.burning > 0.0 {
            if self.in_water {
                self.burning = 0.0;
            } else {
                self.burning -= dt;
                let before = (self.age * 1.0) as i32;
                let after = ((self.age + dt) * 1.0) as i32;
                if after != before {
                    self.health -= 1.0;
                    self.hurt_timer = 0.3;
                    events.push(MobEvent::Hurt { kind: self.kind, pos: self.position() });
                    if self.health <= 0.0 {
                        self.dead = true;
                        events.push(MobEvent::Died { kind: self.kind, pos: self.position(), drops: self.drops(ctx.rng) });
                        return events;
                    }
                }
                if self.kind.burns_in_daylight() && ctx.sun_level > 0.62 {
                    self.burning = self.burning.max(0.5);
                }
            }
        }
        // movement
        let speed = self.kind.speed() * if self.ai.flee_timer > 0.0 && !self.kind.hostile() { 1.5 } else { 1.0 } * if self.in_water { 0.6 } else { 1.0 };
        let target_v = Vec3::new(wish.x, 0.0, wish.y) * speed;
        let k = if self.on_ground { 10.0 } else { 2.5 };
        let a = 1.0 - (-dt * k).exp();
        let mut v = self.velocity();
        v.x += (target_v.x - v.x) * a;
        v.z += (target_v.z - v.z) * a;
        if self.in_water {
            let target_y = if jump || wish.length() > 0.1 { 2.5 } else { -0.5 };
            v.y += (target_y - v.y) * (1.0 - (-dt * 4.0).exp());
        } else {
            v.y -= physics::GRAVITY * dt;
            if self.kind == MobKind::Chicken && v.y < -2.0 {
                v.y = -2.0;
            }
            if jump && self.on_ground && self.ai.jump_cooldown <= 0.0 {
                v.y = physics::JUMP_SPEED * 0.92;
                self.ai.jump_cooldown = 0.6;
            }
        }
        self.ai.jump_cooldown = (self.ai.jump_cooldown - dt).max(0.0);
        if v.y < -physics::TERMINAL {
            v.y = -physics::TERMINAL;
        }
        let mut cache = ChunkCache::new(world);
        let mut aabb = self.aabb();
        let was_ground = self.on_ground;
        let (applied, res) = physics::move_aabb(&mut cache, &mut aabb, v * dt);
        self.on_ground = res.on_ground;
        if res.hit_x {
            v.x = 0.0;
        }
        if res.hit_z {
            v.z = 0.0;
        }
        if res.hit_ceiling && v.y > 0.0 {
            v.y = 0.0;
        }
        if self.on_ground && v.y < 0.0 {
            v.y = 0.0;
        }
        // auto-jump when walking into a wall
        if (res.hit_x || res.hit_z) && self.on_ground && wish.length() > 0.1 && self.ai.jump_cooldown <= 0.0 {
            v.y = physics::JUMP_SPEED * 0.92;
            self.on_ground = false;
            self.ai.jump_cooldown = 0.6;
        }
        self.pos = [aabb.center().x, aabb.min.y, aabb.center().z];
        // fall damage
        if self.on_ground && !was_ground {
            let fall = self.fall_start - self.pos[1];
            if fall > 3.5 && !self.in_water && self.kind != MobKind::Chicken {
                if self.damage((fall - 3.0).floor(), None) {
                    events.push(MobEvent::Died { kind: self.kind, pos: self.position(), drops: self.drops(ctx.rng) });
                }
            }
            self.fall_start = self.pos[1];
        }
        if !self.on_ground {
            if v.y >= 0.0 || self.in_water {
                self.fall_start = self.pos[1];
            }
        } else {
            self.fall_start = self.pos[1];
        }
        self.vel = v.to_array();
        // facing + animation
        let h = Vec3::new(applied.x, 0.0, applied.z);
        let hs = h.length() / dt.max(1e-4);
        if hs > 0.05 {
            let target_yaw = (-h.x).atan2(-h.z);
            let mut d = target_yaw - self.yaw;
            while d > std::f32::consts::PI {
                d -= std::f32::consts::TAU;
            }
            while d < -std::f32::consts::PI {
                d += std::f32::consts::TAU;
            }
            self.yaw += d * (1.0 - (-dt * 10.0).exp());
        }
        self.anim_speed += ((hs / self.kind.speed()).min(1.5) - self.anim_speed) * (1.0 - (-dt * 8.0).exp());
        self.anim += dt * 9.0 * self.anim_speed;
        // sheep wool regrowth, chicken eggs
        if self.kind == MobKind::Sheep && self.sheared {
            self.regrow -= dt;
            if self.regrow <= 0.0 {
                self.sheared = false;
            }
        }
        if self.kind == MobKind::Chicken {
            self.egg_timer -= dt;
            if self.egg_timer <= 0.0 {
                self.egg_timer = 120.0 + ctx.rng.f32() * 180.0;
                events.push(MobEvent::LayEgg { pos: self.position() });
            }
        }
        // ambient sounds
        self.ai.ambient_timer -= dt;
        if self.ai.ambient_timer <= 0.0 {
            self.ai.ambient_timer = 4.0 + ctx.rng.f32() * 12.0;
            events.push(MobEvent::Ambient { kind: self.kind, pos: self.position() });
        }
        events
    }

    /// Ray/AABB test used for player attacks. Returns distance along the ray.
    pub fn ray_hit(&self, origin: Vec3, dir: Vec3, max: f32) -> Option<f32> {
        let b = self.aabb();
        let mut tmin = 0.0f32;
        let mut tmax = max;
        for axis in 0..3 {
            let o = origin[axis];
            let d = dir[axis];
            if d.abs() < 1e-8 {
                if o < b.min[axis] || o > b.max[axis] {
                    return None;
                }
                continue;
            }
            let inv = 1.0 / d;
            let mut t1 = (b.min[axis] - o) * inv;
            let mut t2 = (b.max[axis] - o) * inv;
            if t1 > t2 {
                std::mem::swap(&mut t1, &mut t2);
            }
            tmin = tmin.max(t1);
            tmax = tmax.min(t2);
            if tmin > tmax {
                return None;
            }
        }
        Some(tmin)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::block::voxel;
    use crate::world::chunk::Chunk;

    fn flat() -> std::sync::Arc<World> {
        let w = World::new(0);
        for cz in -1..=1 {
            for cx in -1..=1 {
                let mut c = Chunk::new(cx, cz);
                for z in 0..16 {
                    for x in 0..16 {
                        for y in 0..10 {
                            c.set(x, y, z, voxel(Block::Grass, 0));
                        }
                    }
                }
                c.recompute_heightmap();
                crate::world::light::init_chunk_light(&mut c);
                w.insert_chunk(c);
            }
        }
        w
    }

    #[test]
    fn zombie_chases_and_attacks_the_player() {
        let world = flat();
        let mut rng = Rng::new(3);
        let mut mob = Mob::new(MobKind::Zombie, Vec3::new(2.5, 10.0, 8.5), &mut rng);
        let players = vec![(Vec3::new(10.5, 10.0, 8.5), false)];
        let mut attacked = false;
        for _ in 0..600 {
            let mut ctx = MobCtx { world: &world, players: &players, rng: &mut rng, sun_level: 0.3 };
            for e in mob.update(&mut ctx, 0.05) {
                if let MobEvent::AttackPlayer { .. } = e {
                    attacked = true;
                }
            }
            if attacked {
                break;
            }
        }
        assert!(attacked, "zombie never reached the player: pos {:?}", mob.pos);
        assert!(mob.position().x > 8.0);
    }

    #[test]
    fn zombies_burn_in_daylight_and_die() {
        let world = flat();
        let mut rng = Rng::new(3);
        let mut mob = Mob::new(MobKind::Zombie, Vec3::new(2.5, 10.0, 8.5), &mut rng);
        let players = vec![];
        let mut died = false;
        for _ in 0..1200 {
            let mut ctx = MobCtx { world: &world, players: &players, rng: &mut rng, sun_level: 1.0 };
            for e in mob.update(&mut ctx, 0.05) {
                if let MobEvent::Died { .. } = e {
                    died = true;
                }
            }
            if died {
                break;
            }
        }
        assert!(died);
    }

    #[test]
    fn pigs_drop_porkchops() {
        let mut rng = Rng::new(1);
        let mob = Mob::new(MobKind::Pig, Vec3::ZERO, &mut rng);
        let d = mob.drops(&mut rng);
        assert_eq!(d[0].as_item(), Some(Item::PorkchopRaw));
    }
}
