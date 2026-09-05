//! Natural spawning / despawning and monster spawner blocks.

use crate::mobs::{Mob, MobKind};
use crate::world::block::Block;
use crate::world::chunk::BlockEntity;
use crate::world::noise::Rng;
use crate::world::{ChunkCache, World};
use glam::Vec3;

pub const PASSIVE_CAP: usize = 24;
pub const HOSTILE_CAP: usize = 40;
pub const DESPAWN_DISTANCE: f32 = 110.0;

fn light_level(world: &World, x: i32, y: i32, z: i32, sun_level: f32) -> f32 {
    let (sky, blk) = world.light_at(x, y, z);
    (sky as f32 * sun_level).max(blk as f32)
}

/// Can a mob of this size stand at block (x,y,z)? (air at y and y+1, solid below)
fn standable(cache: &mut ChunkCache, x: i32, y: i32, z: i32, tall: bool) -> bool {
    if !cache.is_solid(x, y - 1, z) {
        return false;
    }
    let a = cache.get(x, y, z);
    let b = cache.get(x, y + 1, z);
    let free = |v: u16| v == 0 || !crate::world::block::props(crate::world::block::vox_id(v)).solid && !crate::world::block::is_fluid(v);
    free(a) && (!tall || free(b))
}

/// Called about once per second.
pub fn natural_spawn(mobs: &mut Vec<Mob>, world: &World, players: &[Vec3], rng: &mut Rng, sun_level: f32, is_day: bool) {
    if players.is_empty() {
        return;
    }
    let passive = mobs.iter().filter(|m| !m.kind.hostile() && !m.dead).count();
    let hostile = mobs.iter().filter(|m| m.kind.hostile() && !m.dead).count();
    let mut cache = ChunkCache::new(world);
    for _ in 0..8 {
        let p = players[rng.below(players.len() as u32) as usize];
        let dx = rng.range(-56, 57);
        let dz = rng.range(-56, 57);
        if dx.abs() < 24 && dz.abs() < 24 {
            continue;
        }
        let x = p.x.floor() as i32 + dx;
        let z = p.z.floor() as i32 + dz;
        if !world.is_loaded(x, z) {
            continue;
        }
        let want_hostile = hostile < HOSTILE_CAP && (rng.chance(0.75) || passive >= PASSIVE_CAP);
        if want_hostile {
            // anywhere dark: caves or the night surface
            let y = if rng.chance(0.5) { world.height_at(x, z).unwrap_or(0) } else { (p.y.floor() as i32 + rng.range(-24, 12)).clamp(2, 250) };
            if !standable(&mut cache, x, y, z, true) {
                continue;
            }
            if light_level(world, x, y, z, sun_level) >= 7.0 {
                continue;
            }
            let kind = match rng.below(10) {
                0..=4 => MobKind::Zombie,
                5..=7 => MobKind::Skeleton,
                _ => MobKind::Creeper,
            };
            let n = 1 + rng.below(3);
            for _ in 0..n {
                let ox = x + rng.range(-3, 4);
                let oz = z + rng.range(-3, 4);
                if standable(&mut cache, ox, y, oz, true) && light_level(world, ox, y, oz, sun_level) < 7.0 {
                    mobs.push(Mob::new(kind, Vec3::new(ox as f32 + 0.5, y as f32, oz as f32 + 0.5), rng));
                }
            }
        } else if passive < PASSIVE_CAP && is_day {
            let Some(h) = world.height_at(x, z) else { continue };
            let y = h;
            if !standable(&mut cache, x, y, z, true) {
                continue;
            }
            if cache.get_block(x, y - 1, z) != Block::Grass || world.sky_light(x, y, z) < 9 {
                continue;
            }
            let kind = match rng.below(4) {
                0 => MobKind::Pig,
                1 => MobKind::Cow,
                2 => MobKind::Sheep,
                _ => MobKind::Chicken,
            };
            let n = 2 + rng.below(3);
            for _ in 0..n {
                let ox = x + rng.range(-4, 5);
                let oz = z + rng.range(-4, 5);
                let oy = world.height_at(ox, oz).unwrap_or(y);
                if standable(&mut cache, ox, oy, oz, true) && cache.get_block(ox, oy - 1, oz) == Block::Grass {
                    mobs.push(Mob::new(kind, Vec3::new(ox as f32 + 0.5, oy as f32, oz as f32 + 0.5), rng));
                }
            }
        }
    }
}

/// Remove mobs far from every player, or whose chunk is unloaded.
pub fn despawn(mobs: &mut Vec<Mob>, world: &World, players: &[Vec3]) {
    mobs.retain(|m| {
        let p = m.position();
        if !world.is_loaded(p.x.floor() as i32, p.z.floor() as i32) {
            return false;
        }
        if p.y < -8.0 {
            return false;
        }
        let d = players.iter().map(|pp| pp.distance(p)).fold(f32::MAX, f32::min);
        if m.kind.hostile() {
            d < DESPAWN_DISTANCE
        } else {
            d < DESPAWN_DISTANCE + 40.0
        }
    });
}

/// Tick a spawner block; `ticks` is the fixed tick counter.
pub fn tick_spawner(world: &World, pos: (i32, i32, i32), mobs: &mut Vec<Mob>, players: &[Vec3], rng: &mut Rng, sun_level: f32) -> bool {
    if world.get_block(pos.0, pos.1, pos.2) != Block::Spawner {
        return false;
    }
    let center = Vec3::new(pos.0 as f32 + 0.5, pos.1 as f32 + 0.5, pos.2 as f32 + 0.5);
    let near = players.iter().any(|p| p.distance(center) < 16.0);
    let mut spawned: Option<u8> = None;
    world.with_block_entity(pos.0, pos.1, pos.2, |be| {
        if let BlockEntity::Spawner { mob, cooldown } = be {
            if *cooldown > 0 {
                *cooldown -= 1;
            } else if near {
                *cooldown = 200 + rng.below(400);
                spawned = Some(*mob);
            }
        }
    });
    let Some(kind_id) = spawned else { return true };
    let kind = if kind_id == 0 { MobKind::Zombie } else { MobKind::Skeleton };
    let nearby = mobs.iter().filter(|m| m.kind == kind && m.position().distance(center) < 8.0).count();
    if nearby >= 6 {
        return true;
    }
    let mut cache = ChunkCache::new(world);
    let n = 1 + rng.below(3);
    for _ in 0..n {
        for _try in 0..6 {
            let x = pos.0 + rng.range(-4, 5);
            let y = pos.1 + rng.range(-1, 2);
            let z = pos.2 + rng.range(-4, 5);
            if standable(&mut cache, x, y, z, true) && light_level(world, x, y, z, sun_level) < 8.0 {
                mobs.push(Mob::new(kind, Vec3::new(x as f32 + 0.5, y as f32, z as f32 + 0.5), rng));
                break;
            }
        }
    }
    true
}
