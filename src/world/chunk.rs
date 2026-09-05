//! Column chunk (16 x 256 x 16) made of 16 sub-chunks (16^3).

use crate::world::block::{self, Block};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const CHUNK_SIZE: usize = 16;
pub const CHUNK_HEIGHT: usize = 256;
pub const SUB_COUNT: usize = CHUNK_HEIGHT / CHUNK_SIZE;
pub const SUB_VOLUME: usize = CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE;

#[inline]
pub const fn sub_index(x: usize, y: usize, z: usize) -> usize {
    (y * CHUNK_SIZE + z) * CHUNK_SIZE + x
}

#[derive(Clone)]
pub struct SubChunk {
    /// None = all air.
    pub blocks: Option<Box<[u16; SUB_VOLUME]>>,
    /// Low nibble sky light, high nibble block light.
    pub light: Box<[u8; SUB_VOLUME]>,
    /// Incremented on every block change; meshes carry the version they were built from.
    pub version: u32,
    pub non_air: u16,
}

impl SubChunk {
    pub fn new() -> SubChunk {
        SubChunk { blocks: None, light: Box::new([0; SUB_VOLUME]), version: 0, non_air: 0 }
    }
    #[inline]
    pub fn get(&self, x: usize, y: usize, z: usize) -> u16 {
        match &self.blocks {
            Some(b) => b[sub_index(x, y, z)],
            None => 0,
        }
    }
    #[inline]
    pub fn set(&mut self, x: usize, y: usize, z: usize, v: u16) {
        if self.blocks.is_none() {
            if v == 0 {
                return;
            }
            self.blocks = Some(Box::new([0; SUB_VOLUME]));
        }
        let b = self.blocks.as_mut().unwrap();
        let i = sub_index(x, y, z);
        let old = b[i];
        if old == v {
            return;
        }
        if old == 0 {
            self.non_air += 1;
        }
        if v == 0 {
            self.non_air -= 1;
        }
        b[i] = v;
        self.version = self.version.wrapping_add(1);
    }
    #[inline]
    pub fn sky(&self, x: usize, y: usize, z: usize) -> u8 {
        self.light[sub_index(x, y, z)] & 0xf
    }
    #[inline]
    pub fn block_light(&self, x: usize, y: usize, z: usize) -> u8 {
        self.light[sub_index(x, y, z)] >> 4
    }
    #[inline]
    pub fn set_sky(&mut self, x: usize, y: usize, z: usize, v: u8) {
        let i = sub_index(x, y, z);
        self.light[i] = (self.light[i] & 0xf0) | (v & 0xf);
    }
    #[inline]
    pub fn set_block_light(&mut self, x: usize, y: usize, z: usize, v: u8) {
        let i = sub_index(x, y, z);
        self.light[i] = (self.light[i] & 0x0f) | (v << 4);
    }
    pub fn is_empty(&self) -> bool {
        self.non_air == 0
    }
    /// Recompute `non_air` after bulk writes.
    pub fn recount(&mut self) {
        self.non_air = match &self.blocks {
            Some(b) => b.iter().filter(|v| **v != 0).count() as u16,
            None => 0,
        };
        if self.non_air == 0 {
            self.blocks = None;
        }
    }
}

impl Default for SubChunk {
    fn default() -> Self {
        SubChunk::new()
    }
}

/// Inventory-like block entities stored per chunk at a local position.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum BlockEntity {
    Chest { items: Vec<crate::player::items::ItemStack> },
    Furnace(crate::player::furnace::FurnaceState),
    Spawner { mob: u8, cooldown: u32 },
}

#[derive(Clone)]
pub struct Chunk {
    pub cx: i32,
    pub cz: i32,
    pub subs: Vec<SubChunk>,
    /// Highest non-air block per column + 1 (0 if none).
    pub heightmap: [u16; CHUNK_SIZE * CHUNK_SIZE],
    pub biome: [u8; CHUNK_SIZE * CHUNK_SIZE],
    pub block_entities: HashMap<(u8, u8, u8), BlockEntity>,
    /// Chunk was modified after generation and must be saved.
    pub dirty_save: bool,
    /// True once cross-chunk light seams with all 4 neighbours have been resolved.
    pub seams_done: [bool; 4],
}

impl Chunk {
    pub fn new(cx: i32, cz: i32) -> Chunk {
        Chunk {
            cx,
            cz,
            subs: (0..SUB_COUNT).map(|_| SubChunk::new()).collect(),
            heightmap: [0; CHUNK_SIZE * CHUNK_SIZE],
            biome: [0; CHUNK_SIZE * CHUNK_SIZE],
            block_entities: HashMap::new(),
            dirty_save: false,
            seams_done: [false; 4],
        }
    }
    #[inline]
    pub fn get(&self, x: usize, y: usize, z: usize) -> u16 {
        if y >= CHUNK_HEIGHT {
            return 0;
        }
        self.subs[y / CHUNK_SIZE].get(x, y & 15, z)
    }
    #[inline]
    pub fn get_block(&self, x: usize, y: usize, z: usize) -> Block {
        block::vox_block(self.get(x, y, z))
    }
    /// Set without any bookkeeping beyond the sub-chunk version (generation and edits).
    #[inline]
    pub fn set(&mut self, x: usize, y: usize, z: usize, v: u16) {
        if y >= CHUNK_HEIGHT {
            return;
        }
        self.subs[y / CHUNK_SIZE].set(x, y & 15, z, v);
    }
    /// Set a block and keep the heightmap up to date. Returns the old voxel.
    pub fn set_tracked(&mut self, x: usize, y: usize, z: usize, v: u16) -> u16 {
        let old = self.get(x, y, z);
        if old == v {
            return old;
        }
        self.set(x, y, z, v);
        let hi = z * CHUNK_SIZE + x;
        if v != 0 {
            if (y as u16 + 1) > self.heightmap[hi] {
                self.heightmap[hi] = y as u16 + 1;
            }
        } else if self.heightmap[hi] == y as u16 + 1 {
            let mut h = y;
            while h > 0 && self.get(x, h - 1, z) == 0 {
                h -= 1;
            }
            self.heightmap[hi] = h as u16;
        }
        self.dirty_save = true;
        old
    }
    #[inline]
    pub fn sky(&self, x: usize, y: usize, z: usize) -> u8 {
        if y >= CHUNK_HEIGHT {
            return 15;
        }
        self.subs[y / CHUNK_SIZE].sky(x, y & 15, z)
    }
    #[inline]
    pub fn block_light(&self, x: usize, y: usize, z: usize) -> u8 {
        if y >= CHUNK_HEIGHT {
            return 0;
        }
        self.subs[y / CHUNK_SIZE].block_light(x, y & 15, z)
    }
    #[inline]
    pub fn set_sky(&mut self, x: usize, y: usize, z: usize, v: u8) {
        if y < CHUNK_HEIGHT {
            self.subs[y / CHUNK_SIZE].set_sky(x, y & 15, z, v);
        }
    }
    #[inline]
    pub fn set_block_light(&mut self, x: usize, y: usize, z: usize, v: u8) {
        if y < CHUNK_HEIGHT {
            self.subs[y / CHUNK_SIZE].set_block_light(x, y & 15, z, v);
        }
    }
    #[inline]
    pub fn height(&self, x: usize, z: usize) -> usize {
        self.heightmap[z * CHUNK_SIZE + x] as usize
    }
    #[inline]
    pub fn biome_at(&self, x: usize, z: usize) -> u8 {
        self.biome[z * CHUNK_SIZE + x]
    }
    pub fn recompute_heightmap(&mut self) {
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let mut h = CHUNK_HEIGHT;
                while h > 0 && self.get(x, h - 1, z) == 0 {
                    h -= 1;
                }
                self.heightmap[z * CHUNK_SIZE + x] = h as u16;
            }
        }
    }
}
