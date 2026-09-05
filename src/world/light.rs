//! Voxel lighting: 16-level sky light and block light, BFS propagation.

use crate::world::block::{self, props};
use crate::world::chunk::{Chunk, CHUNK_HEIGHT, CHUNK_SIZE};
use crate::world::{chunk_coord, local_coord, ChunkRef, World};
use std::collections::{HashSet, VecDeque};

const CH: i32 = CHUNK_HEIGHT as i32;
const CS: i32 = CHUNK_SIZE as i32;

#[inline]
fn spread(level: u8, filter: u8, sky: bool, down: bool) -> u8 {
    if sky && down && level == 15 && filter == 0 {
        15
    } else {
        level.saturating_sub(1 + filter)
    }
}

const DIRS: [(i32, i32, i32); 6] = [(-1, 0, 0), (1, 0, 0), (0, -1, 0), (0, 1, 0), (0, 0, -1), (0, 0, 1)];

/// Compute sky + block light for a freshly generated chunk in isolation.
/// Cross-chunk seams are resolved later by `propagate_seams`.
pub fn init_chunk_light(chunk: &mut Chunk) {
    // --- sky light: column fill ---
    let mut queue: VecDeque<(i32, i32, i32, u8)> = VecDeque::new();
    for z in 0..CHUNK_SIZE {
        for x in 0..CHUNK_SIZE {
            let mut l = 15u8;
            let top = chunk.height(x, z);
            for y in (0..CHUNK_HEIGHT).rev() {
                if y >= top {
                    chunk.set_sky(x, y, z, 15);
                    continue;
                }
                let v = chunk.get(x, y, z);
                let p = props(block::vox_id(v));
                if p.opaque {
                    l = 0;
                } else if l > 0 {
                    l = spread(l, p.light_filter, true, true);
                }
                chunk.set_sky(x, y, z, l);
                if l == 0 {
                    // everything below is dark until an opening is found by BFS
                    for yy in 0..y {
                        chunk.set_sky(x, yy, z, 0);
                    }
                    break;
                }
            }
        }
    }
    // seeds: lit voxels next to a darker transparent voxel (horizontal only; vertical handled by fill)
    for z in 0..CS {
        for x in 0..CS {
            let top = chunk.height(x as usize, z as usize) as i32;
            // Only voxels at or below the *highest neighbouring* column top can spread sideways.
            let mut max_nb = top;
            for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                let nx = x + dx;
                let nz = z + dz;
                if (0..CS).contains(&nx) && (0..CS).contains(&nz) {
                    max_nb = max_nb.max(chunk.height(nx as usize, nz as usize) as i32);
                }
            }
            for y in 0..max_nb.min(CH) {
                let l = chunk.sky(x as usize, y as usize, z as usize);
                if l < 2 {
                    continue;
                }
                let mut push = false;
                for (dx, dy, dz) in DIRS {
                    let (nx, ny, nz) = (x + dx, y + dy, z + dz);
                    if !(0..CS).contains(&nx) || !(0..CS).contains(&nz) || !(0..CH).contains(&ny) {
                        continue;
                    }
                    let nv = chunk.get(nx as usize, ny as usize, nz as usize);
                    let np = props(block::vox_id(nv));
                    if np.opaque {
                        continue;
                    }
                    let want = spread(l, np.light_filter, true, dy < 0);
                    if chunk.sky(nx as usize, ny as usize, nz as usize) < want {
                        push = true;
                        break;
                    }
                }
                if push {
                    queue.push_back((x, y, z, l));
                }
            }
        }
    }
    bfs_local(chunk, &mut queue, true);

    // --- block light ---
    let mut queue: VecDeque<(i32, i32, i32, u8)> = VecDeque::new();
    for sy in 0..chunk.subs.len() {
        if chunk.subs[sy].is_empty() {
            continue;
        }
        for y in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    let v = chunk.subs[sy].get(x, y, z);
                    if v == 0 {
                        continue;
                    }
                    let e = props(block::vox_id(v)).light;
                    if e > 0 {
                        let wy = sy * CHUNK_SIZE + y;
                        chunk.set_block_light(x, wy, z, e);
                        queue.push_back((x as i32, wy as i32, z as i32, e));
                    }
                }
            }
        }
    }
    bfs_local(chunk, &mut queue, false);
}

fn bfs_local(chunk: &mut Chunk, queue: &mut VecDeque<(i32, i32, i32, u8)>, sky: bool) {
    while let Some((x, y, z, l)) = queue.pop_front() {
        let cur = if sky { chunk.sky(x as usize, y as usize, z as usize) } else { chunk.block_light(x as usize, y as usize, z as usize) };
        if cur != l {
            continue;
        }
        for (dx, dy, dz) in DIRS {
            let (nx, ny, nz) = (x + dx, y + dy, z + dz);
            if !(0..CS).contains(&nx) || !(0..CS).contains(&nz) || !(0..CH).contains(&ny) {
                continue;
            }
            let nv = chunk.get(nx as usize, ny as usize, nz as usize);
            let np = props(block::vox_id(nv));
            if np.opaque {
                continue;
            }
            let nl = spread(l, np.light_filter, sky, dy < 0);
            if nl == 0 {
                continue;
            }
            let (ux, uy, uz) = (nx as usize, ny as usize, nz as usize);
            let old = if sky { chunk.sky(ux, uy, uz) } else { chunk.block_light(ux, uy, uz) };
            if old < nl {
                if sky {
                    chunk.set_sky(ux, uy, uz, nl);
                } else {
                    chunk.set_block_light(ux, uy, uz, nl);
                }
                queue.push_back((nx, ny, nz, nl));
            }
        }
    }
}

/// World-level light updater. Collects touched sub-chunks and bumps their mesh versions on `finish`.
pub struct LightUpdate<'a> {
    world: &'a World,
    add_queue: VecDeque<(i32, i32, i32, u8)>,
    remove_queue: VecDeque<(i32, i32, i32, u8)>,
    dirty: HashSet<(i32, i32, i32)>,
    cache: Option<((i32, i32), ChunkRef)>,
    sky: bool,
}

impl<'a> LightUpdate<'a> {
    pub fn new(world: &'a World, sky: bool) -> Self {
        LightUpdate { world, add_queue: VecDeque::new(), remove_queue: VecDeque::new(), dirty: HashSet::new(), cache: None, sky }
    }

    #[inline]
    fn chunk(&mut self, x: i32, z: i32) -> Option<ChunkRef> {
        let key = (chunk_coord(x), chunk_coord(z));
        if let Some((k, c)) = &self.cache {
            if *k == key {
                return Some(c.clone());
            }
        }
        let c = self.world.get_chunk(key.0, key.1)?;
        self.cache = Some((key, c.clone()));
        Some(c)
    }

    /// Returns (voxel, light) at a position; None if unloaded / out of range.
    #[inline]
    fn read(&mut self, x: i32, y: i32, z: i32) -> Option<(u16, u8)> {
        if !(0..CH).contains(&y) {
            return None;
        }
        let c = self.chunk(x, z)?;
        let c = c.read().unwrap();
        let (lx, ly, lz) = (local_coord(x), y as usize, local_coord(z));
        let l = if self.sky { c.sky(lx, ly, lz) } else { c.block_light(lx, ly, lz) };
        Some((c.get(lx, ly, lz), l))
    }

    #[inline]
    fn write(&mut self, x: i32, y: i32, z: i32, l: u8) {
        if let Some(c) = self.chunk(x, z) {
            let mut c = c.write().unwrap();
            let (lx, ly, lz) = (local_coord(x), y as usize, local_coord(z));
            if self.sky {
                c.set_sky(lx, ly, lz, l);
            } else {
                c.set_block_light(lx, ly, lz, l);
            }
        }
        self.mark(x, y, z);
    }

    fn mark(&mut self, x: i32, y: i32, z: i32) {
        let (sx, sy, sz) = (x >> 4, y >> 4, z >> 4);
        self.dirty.insert((sx, sy, sz));
        // borders affect neighbouring meshes through smooth lighting
        let lx = x & 15;
        let ly = y & 15;
        let lz = z & 15;
        if lx == 0 {
            self.dirty.insert((sx - 1, sy, sz));
        }
        if lx == 15 {
            self.dirty.insert((sx + 1, sy, sz));
        }
        if ly == 0 {
            self.dirty.insert((sx, sy - 1, sz));
        }
        if ly == 15 {
            self.dirty.insert((sx, sy + 1, sz));
        }
        if lz == 0 {
            self.dirty.insert((sx, sy, sz - 1));
        }
        if lz == 15 {
            self.dirty.insert((sx, sy, sz + 1));
        }
    }

    /// Seed a position with a light level (sets it if higher than current).
    pub fn seed(&mut self, x: i32, y: i32, z: i32, level: u8) {
        if level == 0 {
            return;
        }
        if let Some((_, cur)) = self.read(x, y, z) {
            if cur < level {
                self.write(x, y, z, level);
            }
            self.add_queue.push_back((x, y, z, level.max(cur)));
        }
    }

    /// Re-seed from the current value at a position (used after a block became transparent).
    pub fn reseed_neighbors(&mut self, x: i32, y: i32, z: i32) {
        for (dx, dy, dz) in DIRS {
            if let Some((_, l)) = self.read(x + dx, y + dy, z + dz) {
                if l > 0 {
                    self.add_queue.push_back((x + dx, y + dy, z + dz, l));
                }
            }
        }
    }

    /// Remove light starting at a position (its current level is cleared and un-propagated).
    pub fn remove(&mut self, x: i32, y: i32, z: i32) {
        if let Some((_, l)) = self.read(x, y, z) {
            if l > 0 {
                self.write(x, y, z, 0);
                self.remove_queue.push_back((x, y, z, l));
            }
        }
    }

    pub fn propagate(&mut self) {
        let sky = self.sky;
        // removal pass first
        while let Some((x, y, z, l)) = self.remove_queue.pop_front() {
            for (dx, dy, dz) in DIRS {
                let (nx, ny, nz) = (x + dx, y + dy, z + dz);
                let Some((nv, nl)) = self.read(nx, ny, nz) else { continue };
                if nl == 0 {
                    continue;
                }
                let removes = nl < l || (sky && dy < 0 && l == 15 && nl == 15);
                if removes {
                    self.write(nx, ny, nz, 0);
                    self.remove_queue.push_back((nx, ny, nz, nl));
                } else {
                    let _ = nv;
                    self.add_queue.push_back((nx, ny, nz, nl));
                }
            }
        }
        // addition pass
        while let Some((x, y, z, l)) = self.add_queue.pop_front() {
            let Some((_, cur)) = self.read(x, y, z) else { continue };
            if cur != l || l == 0 {
                continue;
            }
            for (dx, dy, dz) in DIRS {
                let (nx, ny, nz) = (x + dx, y + dy, z + dz);
                let Some((nv, nl)) = self.read(nx, ny, nz) else { continue };
                let np = props(block::vox_id(nv));
                if np.opaque {
                    continue;
                }
                let want = spread(l, np.light_filter, sky, dy < 0);
                if want > nl {
                    self.write(nx, ny, nz, want);
                    self.add_queue.push_back((nx, ny, nz, want));
                }
            }
        }
    }

    /// Bump mesh versions of every touched sub-chunk. Returns the number touched.
    pub fn finish(self) -> usize {
        let n = self.dirty.len();
        for (sx, sy, sz) in self.dirty {
            if !(0..(CHUNK_HEIGHT / CHUNK_SIZE) as i32).contains(&sy) {
                continue;
            }
            if let Some(c) = self.world.get_chunk(sx, sz) {
                let mut c = c.write().unwrap();
                let s = &mut c.subs[sy as usize];
                s.version = s.version.wrapping_add(1);
            }
        }
        n
    }
}

/// Update lighting after a single voxel changed from `old` to `new`.
pub fn on_block_changed(world: &World, x: i32, y: i32, z: i32, old: u16, new: u16) {
    let op = props(block::vox_id(old));
    let np = props(block::vox_id(new));
    // ---- block light ----
    {
        let mut u = LightUpdate::new(world, false);
        if op.light > 0 || (np.opaque && !op.opaque) || np.light_filter > op.light_filter {
            u.remove(x, y, z);
        }
        if np.light > 0 {
            u.propagate();
            u.seed(x, y, z, np.light);
        } else if !np.opaque {
            u.propagate();
            u.reseed_neighbors(x, y, z);
        }
        u.propagate();
        u.finish();
    }
    // ---- sky light ----
    {
        let mut u = LightUpdate::new(world, true);
        if np.opaque || np.light_filter > op.light_filter {
            u.remove(x, y, z);
        }
        if !np.opaque {
            u.propagate();
            u.reseed_neighbors(x, y, z);
        }
        u.propagate();
        u.finish();
    }
}

/// Propagate light across the seams between chunk (cx,cz) and its 4 side neighbours that exist.
/// Safe to call from worker threads; light BFS is monotonic so concurrent runs converge.
pub fn propagate_seams(world: &World, cx: i32, cz: i32) {
    let Some(center) = world.get_chunk(cx, cz) else { return };
    let sides: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];
    for (i, (dx, dz)) in sides.iter().enumerate() {
        if center.read().unwrap().seams_done[i] {
            continue;
        }
        let Some(nb) = world.get_chunk(cx + dx, cz + dz) else { continue };
        // collect seeds on both sides where light needs to cross
        let mut seeds_sky: Vec<(i32, i32, i32, u8)> = Vec::new();
        let mut seeds_blk: Vec<(i32, i32, i32, u8)> = Vec::new();
        {
            let c = center.read().unwrap();
            let n = nb.read().unwrap();
            for y in 0..CHUNK_HEIGHT {
                for t in 0..CHUNK_SIZE {
                    // local coords of the border voxel in the center and its counterpart in the neighbour
                    let (cxl, czl, nxl, nzl) = match i {
                        0 => (0, t, CHUNK_SIZE - 1, t),
                        1 => (CHUNK_SIZE - 1, t, 0, t),
                        2 => (t, 0, t, CHUNK_SIZE - 1),
                        _ => (t, CHUNK_SIZE - 1, t, 0),
                    };
                    let cv = c.get(cxl, y, czl);
                    let nv = n.get(nxl, y, nzl);
                    let cp = props(block::vox_id(cv));
                    let np = props(block::vox_id(nv));
                    let wc = (cx * CS + cxl as i32, y as i32, cz * CS + czl as i32);
                    let wn = ((cx + dx) * CS + nxl as i32, y as i32, (cz + dz) * CS + nzl as i32);
                    for sky in [true, false] {
                        let (cl, nl) = if sky { (c.sky(cxl, y, czl), n.sky(nxl, y, nzl)) } else { (c.block_light(cxl, y, czl), n.block_light(nxl, y, nzl)) };
                        let list = if sky { &mut seeds_sky } else { &mut seeds_blk };
                        if !np.opaque && cl > 1 && spread(cl, np.light_filter, sky, false) > nl {
                            list.push((wc.0, wc.1, wc.2, cl));
                        }
                        if !cp.opaque && nl > 1 && spread(nl, cp.light_filter, sky, false) > cl {
                            list.push((wn.0, wn.1, wn.2, nl));
                        }
                    }
                }
            }
        }
        for (sky, seeds) in [(true, seeds_sky), (false, seeds_blk)] {
            if seeds.is_empty() {
                continue;
            }
            let mut u = LightUpdate::new(world, sky);
            for (x, y, z, l) in seeds {
                u.add_queue.push_back((x, y, z, l));
            }
            u.propagate();
            u.finish();
        }
        center.write().unwrap().seams_done[i] = true;
        let opp = i ^ 1;
        nb.write().unwrap().seams_done[opp] = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::block::{voxel, Block};

    fn flat_chunk(h: usize) -> Chunk {
        let mut c = Chunk::new(0, 0);
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                for y in 0..h {
                    c.set(x, y, z, voxel(Block::Stone, 0));
                }
            }
        }
        c.recompute_heightmap();
        c
    }

    #[test]
    fn sky_light_fills_open_air_and_stops_at_ground() {
        let mut c = flat_chunk(10);
        init_chunk_light(&mut c);
        assert_eq!(c.sky(5, 10, 5), 15);
        assert_eq!(c.sky(5, 200, 5), 15);
        assert_eq!(c.sky(5, 5, 5), 0);
    }

    #[test]
    fn sky_light_spreads_into_a_cave_with_decay() {
        let mut c = flat_chunk(20);
        // horizontal tunnel at y=10 from x=0..8, open to the sky at x=0 via a shaft
        for x in 0..8 {
            c.set(x, 10, 5, 0);
        }
        for y in 10..20 {
            c.set(0, y, 5, 0);
        }
        c.recompute_heightmap();
        init_chunk_light(&mut c);
        assert_eq!(c.sky(0, 10, 5), 15);
        assert_eq!(c.sky(1, 10, 5), 14);
        assert_eq!(c.sky(7, 10, 5), 8);
        assert_eq!(c.sky(7, 9, 5), 0);
    }

    #[test]
    fn torch_light_propagates_and_is_removed() {
        let world = World::new(1);
        let mut c = flat_chunk(4);
        c.set(8, 4, 8, voxel(Block::Torch, 0));
        c.recompute_heightmap();
        init_chunk_light(&mut c);
        assert_eq!(c.block_light(8, 4, 8), 14);
        assert_eq!(c.block_light(10, 4, 8), 12);
        assert_eq!(c.block_light(8, 6, 8), 12);
        world.insert_chunk(c);
        // remove the torch through the world API
        let old = world.set_raw(8, 4, 8, 0).unwrap();
        on_block_changed(&world, 8, 4, 8, old, 0);
        assert_eq!(world.light_at(8, 4, 8).1, 0);
        assert_eq!(world.light_at(10, 4, 8).1, 0);
        // place a glowstone: 15 at the source, decays by 1 per block
        let old = world.set_raw(8, 4, 8, voxel(Block::Glowstone, 0)).unwrap();
        on_block_changed(&world, 8, 4, 8, old, voxel(Block::Glowstone, 0));
        assert_eq!(world.light_at(8, 5, 8).1, 14);
        assert_eq!(world.light_at(8, 4, 12).1, 11);
    }

    #[test]
    fn placing_a_block_under_the_sky_darkens_below() {
        let world = World::new(1);
        let mut c = flat_chunk(4);
        init_chunk_light(&mut c);
        world.insert_chunk(c);
        let v = voxel(Block::Stone, 0);
        let old = world.set_raw(8, 20, 8, v).unwrap();
        on_block_changed(&world, 8, 20, 8, old, v);
        assert_eq!(world.sky_light(8, 21, 8), 15);
        // directly below is lit sideways from neighbours (14), not 15
        assert_eq!(world.sky_light(8, 19, 8), 14);
        // removing it restores full sky light
        let old = world.set_raw(8, 20, 8, 0).unwrap();
        on_block_changed(&world, 8, 20, 8, old, 0);
        assert_eq!(world.sky_light(8, 19, 8), 15);
        assert_eq!(world.sky_light(8, 4, 8), 15);
    }

    #[test]
    fn seams_carry_light_between_chunks() {
        let world = World::new(1);
        let mut a = flat_chunk(4);
        // a torch at the +X edge of chunk (0,0)
        a.set(15, 4, 8, voxel(Block::Torch, 0));
        a.recompute_heightmap();
        init_chunk_light(&mut a);
        let mut b = Chunk::new(1, 0);
        for z in 0..16 {
            for x in 0..16 {
                for y in 0..4 {
                    b.set(x, y, z, voxel(Block::Stone, 0));
                }
            }
        }
        b.recompute_heightmap();
        init_chunk_light(&mut b);
        world.insert_chunk(a);
        world.insert_chunk(b);
        assert_eq!(world.light_at(16, 4, 8).1, 0);
        propagate_seams(&world, 0, 0);
        assert_eq!(world.light_at(16, 4, 8).1, 13);
        assert_eq!(world.light_at(18, 4, 8).1, 11);
    }
}
