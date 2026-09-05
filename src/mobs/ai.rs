//! Mob behaviour: wander, flee, chase, keep-distance shooting, creeper fuse.

use crate::mobs::{Mob, MobCtx, MobEvent, MobKind};
use crate::player::raycast::raycast;
use crate::world::ChunkCache;
use glam::{Vec2, Vec3};

const SIGHT: f32 = 20.0;

fn nearest_player(mob: &Mob, ctx: &MobCtx) -> Option<(usize, Vec3, f32)> {
    let p = mob.position();
    ctx.players
        .iter()
        .enumerate()
        .filter(|(_, (_, dead))| !dead)
        .map(|(i, (pp, _))| (i, *pp, pp.distance(p)))
        .min_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal))
}

fn has_line_of_sight(ctx: &MobCtx, from: Vec3, to: Vec3) -> bool {
    let d = to - from;
    let dist = d.length();
    if dist < 0.01 {
        return true;
    }
    let mut cache = ChunkCache::new(ctx.world);
    match raycast(&mut cache, from, d / dist, dist, false) {
        Some(h) => h.dist >= dist - 0.5,
        None => true,
    }
}

/// Is there a drop of more than 3 blocks right ahead? (keeps animals from walking off cliffs)
fn cliff_ahead(mob: &Mob, ctx: &MobCtx, dir: Vec2) -> bool {
    let p = mob.position() + Vec3::new(dir.x, 0.0, dir.y) * 0.8;
    let mut cache = ChunkCache::new(ctx.world);
    let (x, z) = (p.x.floor() as i32, p.z.floor() as i32);
    let y0 = p.y.floor() as i32;
    for dy in 1..=4 {
        if cache.is_solid(x, y0 - dy, z) {
            return false;
        }
    }
    true
}

/// Returns (wish direction xz, wants jump, events).
pub fn think(mob: &mut Mob, ctx: &mut MobCtx, dt: f32) -> (Vec2, bool, Vec<MobEvent>) {
    let mut events = Vec::new();
    let mut wish = Vec2::ZERO;
    let mut jump = false;
    let ai = &mut mob.ai;
    ai.wander_timer -= dt;
    ai.flee_timer = (ai.flee_timer - dt).max(0.0);
    ai.attack_cooldown = (ai.attack_cooldown - dt).max(0.0);
    ai.shoot_cooldown = (ai.shoot_cooldown - dt).max(0.0);
    let pos = mob.position();
    let nearest = nearest_player(mob, ctx);

    // random wandering shared by everyone
    if mob.ai.wander_timer <= 0.0 {
        mob.ai.wander_timer = 1.5 + ctx.rng.f32() * 4.0;
        if ctx.rng.chance(0.45) {
            mob.ai.wander = [0.0, 0.0];
        } else {
            let a = ctx.rng.f32() * std::f32::consts::TAU;
            mob.ai.wander = [a.cos(), a.sin()];
        }
    }
    let wander = Vec2::new(mob.ai.wander[0], mob.ai.wander[1]);

    match mob.kind {
        MobKind::Pig | MobKind::Cow | MobKind::Sheep | MobKind::Chicken => {
            if mob.ai.flee_timer > 0.0 {
                if let Some((_, pp, _)) = nearest {
                    let away = Vec2::new(pos.x - pp.x, pos.z - pp.z).normalize_or_zero();
                    wish = if away == Vec2::ZERO { wander } else { away };
                    jump = true;
                }
            } else {
                wish = wander * 0.6;
            }
            if wish.length() > 0.1 && cliff_ahead(mob, ctx, wish.normalize()) {
                mob.ai.wander = [-wish.x, -wish.y];
                wish = Vec2::ZERO;
            }
            if let Some((_, pp, d)) = nearest {
                if d < 6.0 {
                    mob.head_yaw = (-(pp.x - pos.x)).atan2(-(pp.z - pos.z));
                } else {
                    mob.head_yaw = mob.yaw;
                }
            }
        }
        MobKind::Zombie => {
            if let Some((i, pp, d)) = nearest.filter(|(_, _, d)| *d < SIGHT) {
                let to = Vec2::new(pp.x - pos.x, pp.z - pos.z);
                wish = to.normalize_or_zero();
                mob.head_yaw = (-to.x).atan2(-to.y);
                if d < 1.7 && (pp.y - pos.y).abs() < 2.0 && mob.ai.attack_cooldown <= 0.0 {
                    mob.ai.attack_cooldown = 1.2;
                    events.push(MobEvent::AttackPlayer { player: i, damage: 3.0, from: pos });
                }
                if pp.y > pos.y + 0.8 {
                    jump = true;
                }
            } else {
                wish = wander * 0.5;
                mob.head_yaw = mob.yaw;
            }
        }
        MobKind::Skeleton => {
            if let Some((_, pp, d)) = nearest.filter(|(_, _, d)| *d < SIGHT) {
                let to = Vec2::new(pp.x - pos.x, pp.z - pos.z).normalize_or_zero();
                mob.head_yaw = (-to.x).atan2(-to.y);
                if d < 6.0 {
                    wish = -to;
                } else if d > 11.0 {
                    wish = to;
                } else {
                    // strafe slowly
                    wish = Vec2::new(-to.y, to.x) * 0.4 * if (mob.age * 0.5) as i32 % 2 == 0 { 1.0 } else { -1.0 };
                }
                if d < 16.0 && mob.ai.shoot_cooldown <= 0.0 {
                    let eye = mob.eye();
                    let target = pp + Vec3::new(0.0, 1.2, 0.0);
                    if has_line_of_sight(ctx, eye, target) {
                        mob.ai.shoot_cooldown = 2.0;
                        let mut dir = (target - eye).normalize_or_zero();
                        // arc + inaccuracy
                        dir.y += d * 0.012;
                        dir += Vec3::new(ctx.rng.f32() - 0.5, ctx.rng.f32() - 0.5, ctx.rng.f32() - 0.5) * 0.08;
                        events.push(MobEvent::ShootArrow { origin: eye, dir: dir.normalize_or_zero() });
                    }
                }
            } else {
                wish = wander * 0.5;
                mob.head_yaw = mob.yaw;
            }
        }
        MobKind::Creeper => {
            if let Some((_, pp, d)) = nearest.filter(|(_, _, d)| *d < SIGHT) {
                let to = Vec2::new(pp.x - pos.x, pp.z - pos.z);
                mob.head_yaw = (-to.x).atan2(-to.y);
                if d < 3.0 {
                    if mob.ai.fuse <= 0.0 {
                        events.push(MobEvent::FuseStart { pos });
                    }
                    mob.ai.fuse += dt;
                    wish = Vec2::ZERO;
                    if mob.ai.fuse >= 1.5 {
                        mob.dead = true;
                        mob.death_timer = 10.0;
                        events.push(MobEvent::Explode { pos: pos + Vec3::new(0.0, 0.5, 0.0), power: 3.0 });
                    }
                } else {
                    if d > 6.0 {
                        mob.ai.fuse = (mob.ai.fuse - dt * 2.0).max(0.0);
                    }
                    wish = to.normalize_or_zero();
                    if pp.y > pos.y + 0.8 {
                        jump = true;
                    }
                }
            } else {
                mob.ai.fuse = (mob.ai.fuse - dt * 2.0).max(0.0);
                wish = wander * 0.5;
                mob.head_yaw = mob.yaw;
            }
        }
    }
    (wish, jump, events)
}
