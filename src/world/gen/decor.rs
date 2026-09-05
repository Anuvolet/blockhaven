//! Chunk decoration: trees, plants, cacti, mushrooms. Deterministic per chunk; every chunk also
//! evaluates its neighbours' feature lists and writes the parts that fall inside itself.

use crate::world::block::{voxel, Block};
use crate::world::chunk::{Chunk, CHUNK_HEIGHT};
use crate::world::gen::{Biome, Generator};
use crate::world::noise::Rng;
use crate::world::SEA_LEVEL;

const DECOR_SALT: u64 = 0xDEC0_7A7E;

/// Writes world-space voxels into a single chunk, ignoring anything outside it.
pub struct Writer<'a> {
    pub chunk: &'a mut Chunk,
    x0: i32,
    z0: i32,
}

impl<'a> Writer<'a> {
    pub fn new(chunk: &'a mut Chunk) -> Writer<'a> {
        let x0 = chunk.cx * 16;
        let z0 = chunk.cz * 16;
        Writer { chunk, x0, z0 }
    }
    #[inline]
    pub fn inside(&self, x: i32, z: i32) -> bool {
        x >= self.x0 && x < self.x0 + 16 && z >= self.z0 && z < self.z0 + 16
    }
    #[inline]
    pub fn get(&self, x: i32, y: i32, z: i32) -> Option<u16> {
        if !self.inside(x, z) || !(0..CHUNK_HEIGHT as i32).contains(&y) {
            return None;
        }
        Some(self.chunk.get((x - self.x0) as usize, y as usize, (z - self.z0) as usize))
    }
    /// Unconditional set (inside the chunk).
    #[inline]
    pub fn set(&mut self, x: i32, y: i32, z: i32, v: u16) {
        if self.inside(x, z) && (0..CHUNK_HEIGHT as i32).contains(&y) {
            self.chunk.set((x - self.x0) as usize, y as usize, (z - self.z0) as usize, v);
        }
    }
    /// Set only if the current voxel is air (or replaceable vegetation).
    #[inline]
    pub fn set_if_air(&mut self, x: i32, y: i32, z: i32, v: u16) {
        if let Some(cur) = self.get(x, y, z) {
            if cur == 0 || crate::world::block::props(crate::world::block::vox_id(cur)).replaceable {
                self.set(x, y, z, v);
            }
        }
    }
    pub fn fill(&mut self, x0: i32, y0: i32, z0: i32, x1: i32, y1: i32, z1: i32, v: u16) {
        for y in y0.min(y1)..=y0.max(y1) {
            for z in z0.min(z1)..=z0.max(z1) {
                for x in x0.min(x1)..=x0.max(x1) {
                    self.set(x, y, z, v);
                }
            }
        }
    }
    pub fn block_entity(&mut self, x: i32, y: i32, z: i32, be: crate::world::chunk::BlockEntity) {
        if self.inside(x, z) && (0..CHUNK_HEIGHT as i32).contains(&y) {
            self.chunk.block_entities.insert(((x - self.x0) as u8, y as u8, (z - self.z0) as u8), be);
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum TreeKind {
    Oak,
    Birch,
    Spruce,
    Cactus,
    BigRedMushroom,
    BigBrownMushroom,
}

#[derive(Clone, Copy, Debug)]
pub struct Feature {
    pub x: i32,
    pub z: i32,
    pub kind: TreeKind,
    pub size: i32,
    pub salt: u32,
}

/// Cross-chunk features (trees) for chunk (cx, cz). Pure function of the seed.
pub fn tree_features(g: &Generator, cx: i32, cz: i32) -> Vec<Feature> {
    let mut rng = Rng::at(g.seed, cx as i64, cz as i64, DECOR_SALT);
    let center = g.column(cx * 16 + 8, cz * 16 + 8);
    let mut out = Vec::new();
    let (count, kinds): (i32, &[TreeKind]) = match center.biome {
        Biome::Forest => (rng.range(6, 11), &[TreeKind::Oak, TreeKind::Oak, TreeKind::Oak, TreeKind::Birch]),
        Biome::Plains => (if rng.chance(0.35) { 1 } else { 0 }, &[TreeKind::Oak]),
        Biome::SnowyTaiga => (rng.range(6, 10), &[TreeKind::Spruce]),
        Biome::Swamp => (rng.range(3, 6), &[TreeKind::Oak, TreeKind::Oak, TreeKind::Oak, TreeKind::BigBrownMushroom, TreeKind::BigRedMushroom]),
        Biome::Desert => (rng.range(2, 6), &[TreeKind::Cactus]),
        Biome::Mountains => (rng.range(0, 3), &[TreeKind::Spruce, TreeKind::Oak]),
        _ => (0, &[TreeKind::Oak]),
    };
    for _ in 0..count {
        let x = cx * 16 + rng.range(1, 15);
        let z = cz * 16 + rng.range(1, 15);
        let kind = kinds[rng.below(kinds.len() as u32) as usize];
        let size = match kind {
            TreeKind::Oak => rng.range(4, 7),
            TreeKind::Birch => rng.range(5, 8),
            TreeKind::Spruce => rng.range(6, 11),
            TreeKind::Cactus => rng.range(1, 4),
            TreeKind::BigRedMushroom | TreeKind::BigBrownMushroom => rng.range(4, 7),
        };
        let salt = rng.next_u32();
        // validate against the pure column info of the feature's own position
        let info = g.column(x, z);
        let ok = match kind {
            TreeKind::Cactus => info.biome == Biome::Desert && info.height > SEA_LEVEL,
            _ => info.height > SEA_LEVEL && !matches!(info.biome, Biome::Ocean | Biome::River | Biome::Beach | Biome::Desert) && info.height < 110,
        };
        if ok {
            out.push(Feature { x, z, kind, size, salt });
        }
    }
    out
}

pub fn decorate(g: &Generator, chunk: &mut Chunk) {
    if g.flat {
        return;
    }
    let cx = chunk.cx;
    let cz = chunk.cz;
    // --- trees from this chunk and its neighbours ---
    let mut features = Vec::new();
    for dz in -1..=1 {
        for dx in -1..=1 {
            features.extend(tree_features(g, cx + dx, cz + dz));
        }
    }
    let mut w = Writer::new(chunk);
    for f in &features {
        let base = g.column(f.x, f.z).height + 1;
        let mut frng = Rng::new(f.salt as u64 ^ g.seed);
        match f.kind {
            TreeKind::Oak => oak(&mut w, f.x, base, f.z, f.size, &mut frng, Block::OakLog, Block::OakLeaves),
            TreeKind::Birch => oak(&mut w, f.x, base, f.z, f.size, &mut frng, Block::BirchLog, Block::BirchLeaves),
            TreeKind::Spruce => spruce(&mut w, f.x, base, f.z, f.size, &mut frng),
            TreeKind::Cactus => {
                for y in 0..f.size {
                    w.set_if_air(f.x, base + y, f.z, voxel(Block::Cactus, 0));
                }
            }
            TreeKind::BigRedMushroom => big_mushroom(&mut w, f.x, base, f.z, f.size, Block::RedMushroomBlock),
            TreeKind::BigBrownMushroom => big_mushroom(&mut w, f.x, base, f.z, f.size, Block::BrownMushroomBlock),
        }
    }
    // --- small plants: strictly inside this chunk, use the real surface ---
    let mut rng = Rng::at(g.seed, cx as i64, cz as i64, DECOR_SALT + 1);
    let center = Biome::from_id(w.chunk.biome_at(8, 8));
    let (grass_n, flower_n, mush_n, bush_n) = match center {
        Biome::Plains => (24, 5, 0, 0),
        Biome::Forest => (12, 3, 3, 0),
        Biome::Swamp => (10, 1, 6, 0),
        Biome::SnowyTaiga => (4, 0, 2, 0),
        Biome::Mountains => (4, 1, 0, 0),
        Biome::Desert => (0, 0, 0, 5),
        _ => (0, 0, 0, 0),
    };
    let place = |w: &mut Writer, n: i32, pick: &mut dyn FnMut(&mut Rng) -> Block, rng: &mut Rng, allow: &[Block]| {
        for _ in 0..n {
            let lx = rng.range(0, 16);
            let lz = rng.range(0, 16);
            let h = w.chunk.height(lx as usize, lz as usize) as i32;
            if h == 0 || h >= CHUNK_HEIGHT as i32 - 1 {
                continue;
            }
            let ground = w.chunk.get_block(lx as usize, (h - 1) as usize, lz as usize);
            if !allow.contains(&ground) {
                continue;
            }
            let x = cx * 16 + lx;
            let z = cz * 16 + lz;
            if w.get(x, h, z) == Some(0) {
                let b = pick(rng);
                w.set(x, h, z, voxel(b, 0));
            }
        }
    };
    let grassy = [Block::Grass, Block::SnowyGrass, Block::Podzol];
    place(&mut w, grass_n, &mut |_| Block::TallGrass, &mut rng, &grassy);
    place(&mut w, flower_n, &mut |r: &mut Rng| if r.chance(0.5) { Block::Dandelion } else { Block::Poppy }, &mut rng, &grassy);
    place(&mut w, mush_n, &mut |r: &mut Rng| if r.chance(0.5) { Block::BrownMushroom } else { Block::RedMushroom }, &mut rng, &[Block::Grass, Block::Dirt, Block::Podzol, Block::SnowyGrass]);
    place(&mut w, bush_n, &mut |_| Block::DeadBush, &mut rng, &[Block::Sand]);
    // --- snow layer on cold biomes (a full snow block on top of exposed snowy grass in taiga) ---
    if center == Biome::SnowyTaiga {
        let mut srng = Rng::at(g.seed, cx as i64, cz as i64, DECOR_SALT + 2);
        for lz in 0..16usize {
            for lx in 0..16usize {
                if srng.chance(0.12) {
                    let h = w.chunk.height(lx, lz) as i32;
                    if h > SEA_LEVEL && w.chunk.get_block(lx, (h - 1) as usize, lz) == Block::SnowyGrass {
                        w.set(cx * 16 + lx as i32, h, cz * 16 + lz as i32, voxel(Block::Snow, 0));
                    }
                }
            }
        }
    }
    // --- clay / lily-free swamp puddles: sugar in the form of extra mushrooms on logs is skipped ---
}

#[allow(clippy::too_many_arguments)]
fn oak(w: &mut Writer, x: i32, y: i32, z: i32, h: i32, rng: &mut Rng, log: Block, leaves: Block) {
    let lv = voxel(leaves, 0);
    // canopy
    for dy in (h - 3)..=(h + 1) {
        let r = if dy >= h { 1 } else { 2 };
        for dz in -r..=r as i32 {
            for dx in -r..=r as i32 {
                let corner = dx.abs() == r && dz.abs() == r;
                if corner && (dy >= h || rng.chance(0.35)) {
                    continue;
                }
                if dy == h + 1 && corner {
                    continue;
                }
                w.set_if_air(x + dx, y + dy, z + dz, lv);
            }
        }
    }
    for dy in 0..h {
        w.set(x, y + dy, z, voxel(log, 0));
    }
    // make sure there is dirt under the trunk
    if let Some(b) = w.get(x, y - 1, z) {
        if b == 0 || crate::world::block::props(crate::world::block::vox_id(b)).replaceable {
            w.set(x, y - 1, z, voxel(Block::Dirt, 0));
        }
    }
}

fn spruce(w: &mut Writer, x: i32, y: i32, z: i32, h: i32, rng: &mut Rng) {
    let lv = voxel(Block::SpruceLeaves, 0);
    let mut r = 1;
    let start = 2 + rng.range(0, 2);
    for dy in (start..=h).rev() {
        let rad = if dy == h { 0 } else if dy >= h - 1 { 1 } else { r };
        for dz in -rad..=rad as i32 {
            for dx in -rad..=rad as i32 {
                if rad >= 2 && dx.abs() == rad && dz.abs() == rad {
                    continue;
                }
                w.set_if_air(x + dx, y + dy, z + dz, lv);
            }
        }
        if dy < h - 1 {
            r = if r == 1 { 2 } else if r == 2 && dy % 2 == 0 { 3 } else { 1 };
        }
    }
    w.set_if_air(x, y + h + 1, z, lv);
    for dy in 0..h {
        w.set(x, y + dy, z, voxel(Block::SpruceLog, 0));
    }
    if let Some(b) = w.get(x, y - 1, z) {
        if b == 0 || crate::world::block::props(crate::world::block::vox_id(b)).replaceable {
            w.set(x, y - 1, z, voxel(Block::Dirt, 0));
        }
    }
}

fn big_mushroom(w: &mut Writer, x: i32, y: i32, z: i32, h: i32, cap: Block) {
    let stem = voxel(Block::MushroomStem, 0);
    let capv = voxel(cap, 0);
    for dy in 0..h {
        w.set(x, y + dy, z, stem);
    }
    let top = y + h;
    for dz in -2i32..=2 {
        for dx in -2i32..=2 {
            if dx.abs() == 2 && dz.abs() == 2 {
                continue;
            }
            w.set_if_air(x + dx, top, z + dz, capv);
        }
    }
    if cap == Block::RedMushroomBlock {
        for dz in -2i32..=2 {
            for dx in -2i32..=2 {
                if dx.abs() == 2 || dz.abs() == 2 {
                    if dx.abs() == 2 && dz.abs() == 2 {
                        continue;
                    }
                    w.set_if_air(x + dx, top - 1, z + dz, capv);
                }
            }
        }
    }
}
