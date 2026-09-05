//! AABB physics against the voxel world: gravity, collision, swimming, ladders, flight.

use crate::world::block::{self, Block, Shape};
use crate::world::{ChunkCache, World};
use glam::Vec3;

pub const GRAVITY: f32 = 32.0;
pub const TERMINAL: f32 = 78.0;
pub const JUMP_SPEED: f32 = 9.0;

#[derive(Clone, Copy, Debug)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    pub fn from_center(pos: Vec3, half_w: f32, height: f32) -> Aabb {
        Aabb { min: Vec3::new(pos.x - half_w, pos.y, pos.z - half_w), max: Vec3::new(pos.x + half_w, pos.y + height, pos.z + half_w) }
    }
    pub fn intersects(&self, o: &Aabb) -> bool {
        self.min.x < o.max.x && self.max.x > o.min.x && self.min.y < o.max.y && self.max.y > o.min.y && self.min.z < o.max.z && self.max.z > o.min.z
    }
    pub fn offset(&self, d: Vec3) -> Aabb {
        Aabb { min: self.min + d, max: self.max + d }
    }
    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }
}

/// Collision box of a voxel (None = no collision).
pub fn block_aabb(v: u16, x: i32, y: i32, z: i32) -> Option<Aabb> {
    let b = block::vox_block(v);
    let p = block::props(b.id());
    if !p.solid {
        return None;
    }
    let (fx, fy, fz) = (x as f32, y as f32, z as f32);
    let full = Aabb { min: Vec3::new(fx, fy, fz), max: Vec3::new(fx + 1.0, fy + 1.0, fz + 1.0) };
    match p.shape {
        Shape::Door => {
            let meta = block::vox_meta(v);
            let facing = meta & 3;
            let open = meta & 4 != 0;
            let side = if open { (facing + 1) & 3 } else { facing };
            let th = 3.0 / 16.0;
            Some(match side {
                0 => Aabb { min: Vec3::new(fx, fy, fz), max: Vec3::new(fx + 1.0, fy + 1.0, fz + th) },
                1 => Aabb { min: Vec3::new(fx + 1.0 - th, fy, fz), max: Vec3::new(fx + 1.0, fy + 1.0, fz + 1.0) },
                2 => Aabb { min: Vec3::new(fx, fy, fz + 1.0 - th), max: Vec3::new(fx + 1.0, fy + 1.0, fz + 1.0) },
                _ => Aabb { min: Vec3::new(fx, fy, fz), max: Vec3::new(fx + th, fy + 1.0, fz + 1.0) },
            })
        }
        Shape::Bed => Some(Aabb { min: full.min, max: Vec3::new(fx + 1.0, fy + 9.0 / 16.0, fz + 1.0) }),
        Shape::Farmland => Some(Aabb { min: full.min, max: Vec3::new(fx + 1.0, fy + 15.0 / 16.0, fz + 1.0) }),
        Shape::Cactus => Some(Aabb { min: Vec3::new(fx + 1.0 / 16.0, fy, fz + 1.0 / 16.0), max: Vec3::new(fx + 15.0 / 16.0, fy + 1.0, fz + 15.0 / 16.0) }),
        _ => Some(full),
    }
}

/// Collect solid block boxes overlapping `area`.
pub fn collect_boxes(cache: &mut ChunkCache, area: &Aabb, out: &mut Vec<Aabb>) {
    out.clear();
    let x0 = area.min.x.floor() as i32;
    let x1 = area.max.x.ceil() as i32;
    let y0 = area.min.y.floor() as i32;
    let y1 = area.max.y.ceil() as i32;
    let z0 = area.min.z.floor() as i32;
    let z1 = area.max.z.ceil() as i32;
    for y in y0..y1 {
        for z in z0..z1 {
            for x in x0..x1 {
                let v = cache.get(x, y, z);
                if v == 0 {
                    continue;
                }
                if let Some(b) = block_aabb(v, x, y, z) {
                    if b.intersects(area) {
                        out.push(b);
                    }
                }
            }
        }
    }
}

pub struct MoveResult {
    pub on_ground: bool,
    pub hit_x: bool,
    pub hit_z: bool,
    pub hit_ceiling: bool,
}

/// Move `aabb` by `delta` with axis-separated sweeps. Returns the applied delta and collision flags.
pub fn move_aabb(cache: &mut ChunkCache, aabb: &mut Aabb, delta: Vec3) -> (Vec3, MoveResult) {
    let mut res = MoveResult { on_ground: false, hit_x: false, hit_z: false, hit_ceiling: false };
    let mut boxes = Vec::new();
    let mut applied = Vec3::ZERO;
    // sub-step to avoid tunnelling at high speed
    let steps = ((delta.abs().max_element() / 0.4).ceil() as i32).clamp(1, 8);
    let step = delta / steps as f32;
    for _ in 0..steps {
        // Y
        let mut dy = step.y;
        if dy != 0.0 {
            let sweep = Aabb { min: aabb.min.min(aabb.min + Vec3::new(0.0, dy, 0.0)), max: aabb.max.max(aabb.max + Vec3::new(0.0, dy, 0.0)) };
            collect_boxes(cache, &sweep, &mut boxes);
            for b in &boxes {
                if aabb.max.x > b.min.x && aabb.min.x < b.max.x && aabb.max.z > b.min.z && aabb.min.z < b.max.z {
                    if dy < 0.0 && aabb.min.y >= b.max.y {
                        let d = b.max.y - aabb.min.y;
                        if d > dy {
                            dy = d;
                            res.on_ground = true;
                        }
                    } else if dy > 0.0 && aabb.max.y <= b.min.y {
                        let d = b.min.y - aabb.max.y;
                        if d < dy {
                            dy = d;
                            res.hit_ceiling = true;
                        }
                    }
                }
            }
            *aabb = aabb.offset(Vec3::new(0.0, dy, 0.0));
            applied.y += dy;
        }
        // X
        let mut dx = step.x;
        if dx != 0.0 {
            let sweep = Aabb { min: aabb.min.min(aabb.min + Vec3::new(dx, 0.0, 0.0)), max: aabb.max.max(aabb.max + Vec3::new(dx, 0.0, 0.0)) };
            collect_boxes(cache, &sweep, &mut boxes);
            for b in &boxes {
                if aabb.max.y > b.min.y && aabb.min.y < b.max.y && aabb.max.z > b.min.z && aabb.min.z < b.max.z {
                    if dx > 0.0 && aabb.max.x <= b.min.x {
                        let d = b.min.x - aabb.max.x;
                        if d < dx {
                            dx = d;
                            res.hit_x = true;
                        }
                    } else if dx < 0.0 && aabb.min.x >= b.max.x {
                        let d = b.max.x - aabb.min.x;
                        if d > dx {
                            dx = d;
                            res.hit_x = true;
                        }
                    }
                }
            }
            *aabb = aabb.offset(Vec3::new(dx, 0.0, 0.0));
            applied.x += dx;
        }
        // Z
        let mut dz = step.z;
        if dz != 0.0 {
            let sweep = Aabb { min: aabb.min.min(aabb.min + Vec3::new(0.0, 0.0, dz)), max: aabb.max.max(aabb.max + Vec3::new(0.0, 0.0, dz)) };
            collect_boxes(cache, &sweep, &mut boxes);
            for b in &boxes {
                if aabb.max.y > b.min.y && aabb.min.y < b.max.y && aabb.max.x > b.min.x && aabb.min.x < b.max.x {
                    if dz > 0.0 && aabb.max.z <= b.min.z {
                        let d = b.min.z - aabb.max.z;
                        if d < dz {
                            dz = d;
                            res.hit_z = true;
                        }
                    } else if dz < 0.0 && aabb.min.z >= b.max.z {
                        let d = b.max.z - aabb.min.z;
                        if d > dz {
                            dz = d;
                            res.hit_z = true;
                        }
                    }
                }
            }
            *aabb = aabb.offset(Vec3::new(0.0, 0.0, dz));
            applied.z += dz;
        }
    }
    // ground probe when not moving down (standing still)
    if !res.on_ground && delta.y <= 0.0 {
        let probe = Aabb { min: aabb.min - Vec3::new(0.0, 0.02, 0.0), max: Vec3::new(aabb.max.x, aabb.min.y, aabb.max.z) };
        collect_boxes(cache, &probe, &mut boxes);
        res.on_ground = boxes.iter().any(|b| (b.max.y - aabb.min.y).abs() < 0.03);
    }
    (applied, res)
}

/// True if the AABB overlaps any solid block.
pub fn overlaps_solid(cache: &mut ChunkCache, aabb: &Aabb) -> bool {
    let mut boxes = Vec::new();
    collect_boxes(cache, aabb, &mut boxes);
    !boxes.is_empty()
}

/// Is there ground beneath any part of this AABB (used for sneak edge protection)?
pub fn has_ground_below(cache: &mut ChunkCache, aabb: &Aabb) -> bool {
    let probe = Aabb { min: Vec3::new(aabb.min.x, aabb.min.y - 0.6, aabb.min.z), max: Vec3::new(aabb.max.x, aabb.min.y + 0.01, aabb.max.z) };
    let mut boxes = Vec::new();
    collect_boxes(cache, &probe, &mut boxes);
    !boxes.is_empty()
}

/// Fluid at a world point.
pub fn fluid_at(world: &World, p: Vec3) -> Option<Block> {
    let v = world.get(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
    if block::is_fluid(v) {
        Some(block::vox_block(v))
    } else {
        None
    }
}

pub fn is_ladder(world: &World, p: Vec3) -> bool {
    world.get_block(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32) == Block::Ladder
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::block::voxel;
    use crate::world::chunk::Chunk;

    fn flat() -> std::sync::Arc<World> {
        let w = World::new(0);
        let mut c = Chunk::new(0, 0);
        for z in 0..16 {
            for x in 0..16 {
                for y in 0..10 {
                    c.set(x, y, z, voxel(Block::Stone, 0));
                }
            }
        }
        c.set(8, 10, 8, voxel(Block::Stone, 0)); // an obstacle
        c.recompute_heightmap();
        w.insert_chunk(c);
        w
    }

    #[test]
    fn falls_and_lands_on_ground() {
        let w = flat();
        let mut cache = ChunkCache::new(&w);
        let mut a = Aabb::from_center(Vec3::new(4.0, 15.0, 4.0), 0.3, 1.8);
        let (_, r) = move_aabb(&mut cache, &mut a, Vec3::new(0.0, -20.0, 0.0));
        assert!(r.on_ground);
        assert!((a.min.y - 10.0).abs() < 1e-4);
    }

    #[test]
    fn walks_into_a_wall_and_stops() {
        let w = flat();
        let mut cache = ChunkCache::new(&w);
        let mut a = Aabb::from_center(Vec3::new(6.0, 10.0, 8.5), 0.3, 1.8);
        let (applied, r) = move_aabb(&mut cache, &mut a, Vec3::new(5.0, 0.0, 0.0));
        assert!(r.hit_x);
        assert!(applied.x < 5.0);
        assert!((a.max.x - 8.0).abs() < 1e-4);
    }
}
