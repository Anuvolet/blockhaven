//! Non-mob entities: item drops, arrows, primed TNT.

use crate::player::items::ItemStack;
use crate::player::physics::{self, Aabb};
use crate::world::noise::Rng;
use crate::world::{ChunkCache, World};
use glam::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ItemDrop {
    pub pos: [f32; 3],
    pub vel: [f32; 3],
    pub stack: ItemStack,
    pub age: f32,
    pub pickup_delay: f32,
    pub on_ground: bool,
}

pub const DROP_LIFETIME: f32 = 300.0;

impl ItemDrop {
    pub fn new(pos: Vec3, stack: ItemStack, vel: Vec3) -> ItemDrop {
        ItemDrop { pos: pos.to_array(), vel: vel.to_array(), stack, age: 0.0, pickup_delay: 0.5, on_ground: false }
    }
    pub fn position(&self) -> Vec3 {
        Vec3::from(self.pos)
    }
    pub fn aabb(&self) -> Aabb {
        Aabb::from_center(self.position(), 0.125, 0.25)
    }
    pub fn update(&mut self, world: &World, dt: f32) {
        self.age += dt;
        self.pickup_delay = (self.pickup_delay - dt).max(0.0);
        let mut vel = Vec3::from(self.vel);
        let in_water = physics::fluid_at(world, self.position() + Vec3::new(0.0, 0.1, 0.0)).is_some();
        if in_water {
            vel.y += (1.5 - vel.y) * (1.0 - (-dt * 4.0).exp());
            vel.x *= 0.9f32.powf(dt * 20.0);
            vel.z *= 0.9f32.powf(dt * 20.0);
        } else {
            vel.y -= physics::GRAVITY * 0.6 * dt;
        }
        let mut cache = ChunkCache::new(world);
        let mut aabb = self.aabb();
        let (_, res) = physics::move_aabb(&mut cache, &mut aabb, vel * dt);
        self.on_ground = res.on_ground;
        if res.on_ground {
            vel.y = 0.0;
            let f = 0.6f32.powf(dt * 20.0);
            vel.x *= f;
            vel.z *= f;
        }
        if res.hit_x {
            vel.x = 0.0;
        }
        if res.hit_z {
            vel.z = 0.0;
        }
        if res.hit_ceiling {
            vel.y = 0.0;
        }
        self.pos = [aabb.center().x, aabb.min.y, aabb.center().z];
        self.vel = vel.to_array();
        // unstuck: if inside a solid block, float up
        if physics::overlaps_solid(&mut cache, &Aabb::from_center(self.position() + Vec3::new(0.0, 0.05, 0.0), 0.1, 0.15)) {
            self.pos[1] += dt * 2.0;
        }
    }
}

/// Spawn a drop with a small random velocity.
pub fn spawn_drop(drops: &mut Vec<ItemDrop>, pos: Vec3, stack: ItemStack, rng: &mut Rng) {
    if stack.is_empty() {
        return;
    }
    let vel = Vec3::new(rng.f32() - 0.5, 2.0 + rng.f32() * 1.5, rng.f32() - 0.5) * 1.5;
    drops.push(ItemDrop::new(pos, stack, vel));
}

/// Throw a drop in a direction (Q key).
pub fn throw_drop(drops: &mut Vec<ItemDrop>, pos: Vec3, dir: Vec3, stack: ItemStack) {
    let mut d = ItemDrop::new(pos, stack, dir * 6.0 + Vec3::new(0.0, 1.5, 0.0));
    d.pickup_delay = 1.5;
    drops.push(d);
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Arrow {
    pub pos: [f32; 3],
    pub vel: [f32; 3],
    pub age: f32,
    pub stuck: bool,
    pub damage: f32,
    /// 0 = shot by a player (can hurt mobs), 1 = shot by a skeleton (hurts players).
    pub owner: u8,
}

impl Arrow {
    pub fn new(pos: Vec3, vel: Vec3, damage: f32, owner: u8) -> Arrow {
        Arrow { pos: pos.to_array(), vel: vel.to_array(), age: 0.0, stuck: false, damage, owner }
    }
    pub fn position(&self) -> Vec3 {
        Vec3::from(self.pos)
    }
    pub fn velocity(&self) -> Vec3 {
        Vec3::from(self.vel)
    }
    /// Move; returns true if it hit a block this step.
    pub fn update(&mut self, world: &World, dt: f32) -> bool {
        self.age += dt;
        if self.stuck {
            return false;
        }
        let mut vel = self.velocity();
        vel.y -= physics::GRAVITY * 0.5 * dt;
        vel *= 0.995f32.powf(dt * 20.0);
        let start = self.position();
        let end = start + vel * dt;
        let mut cache = ChunkCache::new(world);
        // sample along the segment
        let steps = ((end - start).length() / 0.25).ceil().max(1.0) as i32;
        for s in 1..=steps {
            let p = start.lerp(end, s as f32 / steps as f32);
            if cache.is_solid(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32) {
                let prev = start.lerp(end, (s - 1) as f32 / steps as f32);
                self.pos = prev.to_array();
                self.vel = [0.0; 3];
                self.stuck = true;
                return true;
            }
        }
        self.pos = end.to_array();
        self.vel = vel.to_array();
        false
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrimedTnt {
    pub pos: [f32; 3],
    pub vel: [f32; 3],
    pub fuse: f32,
}

impl PrimedTnt {
    pub fn new(pos: Vec3) -> PrimedTnt {
        PrimedTnt { pos: pos.to_array(), vel: [0.0, 2.0, 0.0], fuse: 4.0 }
    }
    pub fn position(&self) -> Vec3 {
        Vec3::from(self.pos)
    }
    /// Returns true when it should explode.
    pub fn update(&mut self, world: &World, dt: f32) -> bool {
        self.fuse -= dt;
        let mut vel = Vec3::from(self.vel);
        vel.y -= physics::GRAVITY * 0.6 * dt;
        let mut cache = ChunkCache::new(world);
        let mut aabb = Aabb::from_center(self.position(), 0.49, 0.98);
        let (_, res) = physics::move_aabb(&mut cache, &mut aabb, vel * dt);
        if res.on_ground {
            vel.y = 0.0;
            vel.x *= 0.7f32.powf(dt * 20.0);
            vel.z *= 0.7f32.powf(dt * 20.0);
        }
        self.pos = [aabb.center().x, aabb.min.y, aabb.center().z];
        self.vel = vel.to_array();
        self.fuse <= 0.0
    }
}
