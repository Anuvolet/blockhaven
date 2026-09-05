//! Finite fluid spreading (classic style): sources spread up to 7 blocks for water and 3 for
//! lava, flowing blocks dry out when their supply disappears. Driven by scheduled ticks.

use crate::world::block::{self, props, voxel, Block};
use crate::world::World;
use std::collections::{BTreeMap, HashSet};

pub const WATER_DELAY: u64 = 5;
pub const LAVA_DELAY: u64 = 20;
const FALLING: u8 = 8;

pub struct FluidSim {
    queue: BTreeMap<u64, Vec<(i32, i32, i32)>>,
    scheduled: HashSet<(i32, i32, i32)>,
    pub tick: u64,
}

impl Default for FluidSim {
    fn default() -> Self {
        Self::new()
    }
}

impl FluidSim {
    pub fn new() -> FluidSim {
        FluidSim { queue: BTreeMap::new(), scheduled: HashSet::new(), tick: 0 }
    }

    pub fn schedule(&mut self, x: i32, y: i32, z: i32, delay: u64) {
        if self.scheduled.insert((x, y, z)) {
            self.queue.entry(self.tick + delay).or_default().push((x, y, z));
        }
    }

    /// Schedule the position and its 6 neighbours if they hold fluid.
    pub fn touch(&mut self, world: &World, x: i32, y: i32, z: i32) {
        for (dx, dy, dz) in [(0, 0, 0), (-1, 0, 0), (1, 0, 0), (0, -1, 0), (0, 1, 0), (0, 0, -1), (0, 0, 1)] {
            let (nx, ny, nz) = (x + dx, y + dy, z + dz);
            let v = world.get(nx, ny, nz);
            if block::is_fluid(v) {
                let d = if block::is_water(v) { WATER_DELAY } else { LAVA_DELAY };
                self.schedule(nx, ny, nz, d);
            }
        }
    }

    pub fn pending(&self) -> usize {
        self.scheduled.len()
    }

    /// Run one world tick. Returns the positions that changed.
    pub fn step(&mut self, world: &World) -> Vec<(i32, i32, i32)> {
        self.tick += 1;
        let mut due = Vec::new();
        let keys: Vec<u64> = self.queue.range(..=self.tick).map(|(k, _)| *k).collect();
        for k in keys {
            if let Some(v) = self.queue.remove(&k) {
                due.extend(v);
            }
        }
        let mut changed = Vec::new();
        for (x, y, z) in due {
            self.scheduled.remove(&(x, y, z));
            self.update(world, x, y, z, &mut changed);
        }
        changed
    }

    fn update(&mut self, world: &World, x: i32, y: i32, z: i32, changed: &mut Vec<(i32, i32, i32)>) {
        let v = world.get(x, y, z);
        if !block::is_fluid(v) {
            return;
        }
        let fluid = block::vox_block(v);
        let is_water = fluid == Block::Water;
        let delay = if is_water { WATER_DELAY } else { LAVA_DELAY };
        let max_level: u8 = if is_water { 7 } else { 3 };
        let meta = block::vox_meta(v);
        let level = meta & 7;
        let same = |w: u16| block::vox_block(w) == fluid;

        // --- recompute level of flowing blocks from neighbours ---
        if meta != 0 {
            let mut best: Option<u8> = None;
            let above = world.get(x, y + 1, z);
            if same(above) {
                best = Some(FALLING | 1);
            }
            let mut sources = 0;
            for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                let n = world.get(x + dx, y, z + dz);
                if same(n) {
                    let nm = block::vox_meta(n);
                    if nm == 0 {
                        sources += 1;
                    }
                    let nl = (nm & 7) + 1;
                    if nl <= max_level {
                        best = Some(match best {
                            Some(b) if (b & 7) <= nl => b,
                            _ => nl,
                        });
                    }
                }
            }
            // two adjacent sources make a new water source
            let below = world.get(x, y - 1, z);
            if is_water && sources >= 2 && (props(block::vox_id(below)).solid || same(below)) {
                best = Some(0);
            }
            match best {
                None => {
                    world.set_block(x, y, z, 0);
                    changed.push((x, y, z));
                    self.touch_neighbors(world, x, y, z);
                    return;
                }
                Some(nm) if nm != meta => {
                    world.set_block(x, y, z, voxel(fluid, nm));
                    changed.push((x, y, z));
                    self.touch_neighbors(world, x, y, z);
                    // continue spreading with the new level
                    self.schedule(x, y, z, delay);
                    return;
                }
                _ => {}
            }
        }

        // --- spread ---
        let below = world.get(x, y - 1, z);
        let can_flow_into = |w: u16| -> bool {
            let b = block::vox_block(w);
            if b == fluid {
                return false;
            }
            props(block::vox_id(w)).replaceable && !block::is_fluid(w)
        };
        if y > 0 {
            if can_flow_into(below) {
                world.set_block(x, y - 1, z, voxel(fluid, FALLING | 1));
                changed.push((x, y - 1, z));
                self.schedule(x, y - 1, z, delay);
                return;
            }
            if block::is_fluid(below) && !same(below) {
                // lava meets water below / water meets lava below
                self.interact(world, x, y - 1, z, is_water, changed);
                return;
            }
            if same(below) {
                // keep falling, no horizontal spread from mid-air columns
                let bm = block::vox_meta(below);
                if bm & FALLING != 0 && !props(block::vox_id(world.get(x, y - 2, z))).solid && y >= 2 && !same(world.get(x, y - 2, z)) {
                    return;
                }
            }
        }
        let next = level + 1;
        if meta & FALLING != 0 {
            // a falling block that landed spreads like level 1
        }
        if next > max_level {
            return;
        }
        for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            let (nx, nz) = (x + dx, z + dz);
            let n = world.get(nx, y, nz);
            if block::is_fluid(n) && !same(n) {
                self.interact(world, nx, y, nz, is_water, changed);
                continue;
            }
            if can_flow_into(n) {
                world.set_block(nx, y, nz, voxel(fluid, next));
                changed.push((nx, y, nz));
                self.schedule(nx, y, nz, delay);
            } else if same(n) {
                let nm = block::vox_meta(n);
                if nm != 0 && (nm & 7) > next {
                    world.set_block(nx, y, nz, voxel(fluid, next));
                    changed.push((nx, y, nz));
                    self.schedule(nx, y, nz, delay);
                }
            }
        }
    }

    fn touch_neighbors(&mut self, world: &World, x: i32, y: i32, z: i32) {
        for (dx, dy, dz) in [(-1, 0, 0), (1, 0, 0), (0, -1, 0), (0, 1, 0), (0, 0, -1), (0, 0, 1)] {
            let (nx, ny, nz) = (x + dx, y + dy, z + dz);
            let v = world.get(nx, ny, nz);
            if block::is_fluid(v) {
                let d = if block::is_water(v) { WATER_DELAY } else { LAVA_DELAY };
                self.schedule(nx, ny, nz, d);
            }
        }
    }

    /// Water/lava contact at the *other* fluid's position.
    fn interact(&mut self, world: &World, x: i32, y: i32, z: i32, from_water: bool, changed: &mut Vec<(i32, i32, i32)>) {
        let v = world.get(x, y, z);
        let other_is_source = block::vox_meta(v) == 0;
        let result = if from_water {
            // water touching lava: lava source -> obsidian, flowing lava -> cobblestone
            if other_is_source { Block::Obsidian } else { Block::Cobblestone }
        } else {
            // lava touching water: water turns to stone/cobble
            Block::Cobblestone
        };
        world.set_block(x, y, z, voxel(result, 0));
        changed.push((x, y, z));
        self.touch_neighbors(world, x, y, z);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::chunk::Chunk;

    fn flat_world() -> std::sync::Arc<World> {
        let w = World::new(0);
        for cz in -1..=1 {
            for cx in -1..=1 {
                let mut c = Chunk::new(cx, cz);
                for z in 0..16 {
                    for x in 0..16 {
                        for y in 0..10 {
                            c.set(x, y, z, voxel(Block::Stone, 0));
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
    fn water_source_spreads_seven_blocks_on_flat_ground() {
        let w = flat_world();
        let mut sim = FluidSim::new();
        w.set_block(8, 10, 8, voxel(Block::Water, 0));
        sim.touch(&w, 8, 10, 8);
        for _ in 0..400 {
            sim.step(&w);
        }
        assert_eq!(w.get_block(15, 10, 8), Block::Water);
        assert_eq!(block::vox_meta(w.get(15, 10, 8)), 7);
        assert_eq!(w.get_block(16, 10, 8), Block::Air);
        assert_eq!(w.get_block(8, 10, 15), Block::Water);
    }

    #[test]
    fn removing_the_source_dries_the_flow() {
        let w = flat_world();
        let mut sim = FluidSim::new();
        w.set_block(8, 10, 8, voxel(Block::Water, 0));
        sim.touch(&w, 8, 10, 8);
        for _ in 0..200 {
            sim.step(&w);
        }
        assert_eq!(w.get_block(10, 10, 8), Block::Water);
        w.set_block(8, 10, 8, 0);
        sim.touch(&w, 8, 10, 8);
        for _ in 0..400 {
            sim.step(&w);
        }
        assert_eq!(w.get_block(10, 10, 8), Block::Air);
        assert_eq!(w.get_block(15, 10, 8), Block::Air);
    }

    #[test]
    fn water_falls_and_lava_makes_obsidian() {
        let w = flat_world();
        let mut sim = FluidSim::new();
        w.set_block(8, 9, 8, 0); // a hole
        w.set_block(8, 8, 8, 0);
        w.set_block(8, 12, 8, voxel(Block::Water, 0));
        sim.touch(&w, 8, 12, 8);
        for _ in 0..100 {
            sim.step(&w);
        }
        assert_eq!(w.get_block(8, 8, 8), Block::Water);
        // lava source next to water
        w.set_block(3, 10, 3, voxel(Block::Lava, 0));
        w.set_block(4, 10, 3, voxel(Block::Water, 0));
        sim.touch(&w, 4, 10, 3);
        for _ in 0..100 {
            sim.step(&w);
        }
        assert_eq!(w.get_block(3, 10, 3), Block::Obsidian);
    }
}
