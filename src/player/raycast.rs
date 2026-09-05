//! Voxel raycast (DDA) with per-block collision shapes.

use crate::player::physics::{block_aabb, Aabb};
use crate::world::block;
use crate::world::ChunkCache;
use glam::Vec3;

#[derive(Clone, Copy, Debug)]
pub struct RayHit {
    pub pos: (i32, i32, i32),
    pub normal: (i32, i32, i32),
    pub point: Vec3,
    pub dist: f32,
}

/// Ray vs AABB slab test; returns (t_enter, hit normal) if hit within [0, max].
fn ray_aabb(origin: Vec3, dir: Vec3, b: &Aabb, max: f32) -> Option<(f32, (i32, i32, i32))> {
    let mut tmin = 0.0f32;
    let mut tmax = max;
    let mut normal = (0, 0, 0);
    for axis in 0..3 {
        let o = origin[axis];
        let d = dir[axis];
        let lo = b.min[axis];
        let hi = b.max[axis];
        if d.abs() < 1e-8 {
            if o < lo || o > hi {
                return None;
            }
            continue;
        }
        let inv = 1.0 / d;
        let mut t1 = (lo - o) * inv;
        let mut t2 = (hi - o) * inv;
        // the entry face always points against the ray direction along this axis
        let n = if d > 0.0 { -1 } else { 1 };
        if t1 > t2 {
            std::mem::swap(&mut t1, &mut t2);
        }
        if t1 > tmin {
            tmin = t1;
            normal = match axis {
                0 => (n, 0, 0),
                1 => (0, n, 0),
                _ => (0, 0, n),
            };
        }
        tmax = tmax.min(t2);
        if tmin > tmax {
            return None;
        }
    }
    Some((tmin, normal))
}

pub fn raycast(cache: &mut ChunkCache, origin: Vec3, dir: Vec3, max_dist: f32, hit_fluids: bool) -> Option<RayHit> {
    let dir = dir.normalize_or_zero();
    if dir == Vec3::ZERO {
        return None;
    }
    let mut x = origin.x.floor() as i32;
    let mut y = origin.y.floor() as i32;
    let mut z = origin.z.floor() as i32;
    let step_x = if dir.x > 0.0 { 1 } else { -1 };
    let step_y = if dir.y > 0.0 { 1 } else { -1 };
    let step_z = if dir.z > 0.0 { 1 } else { -1 };
    let inv = |d: f32| if d.abs() < 1e-8 { f32::INFINITY } else { 1.0 / d.abs() };
    let t_delta = Vec3::new(inv(dir.x), inv(dir.y), inv(dir.z));
    let frac = |o: f32, d: f32| if d > 0.0 { o.ceil() - o } else { o - o.floor() };
    let mut t_max = Vec3::new(frac(origin.x, dir.x) * t_delta.x, frac(origin.y, dir.y) * t_delta.y, frac(origin.z, dir.z) * t_delta.z);
    let mut normal = (0, 0, 0);
    let mut t = 0.0f32;
    for _ in 0..256 {
        let v = cache.get(x, y, z);
        if v != 0 {
            let is_fluid = block::is_fluid(v);
            if !is_fluid || hit_fluids {
                let p = block::props(block::vox_id(v));
                if p.solid {
                    if let Some(b) = block_aabb(v, x, y, z) {
                        if let Some((th, n)) = ray_aabb(origin, dir, &b, max_dist) {
                            return Some(RayHit { pos: (x, y, z), normal: n, point: origin + dir * th, dist: th });
                        }
                    }
                } else {
                    // non-solid decoration: hit the full cell
                    let n = if t == 0.0 { (0, 1, 0) } else { normal };
                    return Some(RayHit { pos: (x, y, z), normal: n, point: origin + dir * t, dist: t });
                }
            }
        }
        if t_max.x < t_max.y && t_max.x < t_max.z {
            t = t_max.x;
            t_max.x += t_delta.x;
            x += step_x;
            normal = (-step_x, 0, 0);
        } else if t_max.y < t_max.z {
            t = t_max.y;
            t_max.y += t_delta.y;
            y += step_y;
            normal = (0, -step_y, 0);
        } else {
            t = t_max.z;
            t_max.z += t_delta.z;
            z += step_z;
            normal = (0, 0, -step_z);
        }
        if t > max_dist {
            return None;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::block::{voxel, Block};
    use crate::world::chunk::Chunk;
    use crate::world::World;

    #[test]
    fn hits_the_top_face_of_the_ground() {
        let w = World::new(0);
        let mut c = Chunk::new(0, 0);
        for z in 0..16 {
            for x in 0..16 {
                c.set(x, 5, z, voxel(Block::Stone, 0));
            }
        }
        w.insert_chunk(c);
        let mut cache = ChunkCache::new(&w);
        let hit = raycast(&mut cache, Vec3::new(8.5, 10.0, 8.5), Vec3::new(0.3, -1.0, 0.1), 20.0, false).unwrap();
        assert_eq!(hit.pos.1, 5);
        assert_eq!(hit.normal, (0, 1, 0));
        assert!((hit.point.y - 6.0).abs() < 1e-4);
        assert!(raycast(&mut cache, Vec3::new(8.5, 10.0, 8.5), Vec3::new(0.0, 1.0, 0.0), 20.0, false).is_none());
    }
}
