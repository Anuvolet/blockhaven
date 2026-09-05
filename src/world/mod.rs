pub mod block;
pub mod chunk;
pub mod fluid;
pub mod gen;
pub mod light;
pub mod noise;
pub mod worker;

use block::{Block, BlockProps};
use chunk::{BlockEntity, Chunk, CHUNK_HEIGHT, CHUNK_SIZE};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub type ChunkRef = Arc<RwLock<Chunk>>;

#[inline]
pub fn chunk_coord(v: i32) -> i32 {
    v >> 4
}
#[inline]
pub fn local_coord(v: i32) -> usize {
    (v & 15) as usize
}

/// Thread-shared voxel world.
pub struct World {
    pub chunks: RwLock<HashMap<(i32, i32), ChunkRef>>,
    pub seed: u64,
}

impl World {
    pub fn new(seed: u64) -> Arc<World> {
        Arc::new(World { chunks: RwLock::new(HashMap::new()), seed })
    }

    pub fn get_chunk(&self, cx: i32, cz: i32) -> Option<ChunkRef> {
        self.chunks.read().unwrap().get(&(cx, cz)).cloned()
    }

    pub fn has_chunk(&self, cx: i32, cz: i32) -> bool {
        self.chunks.read().unwrap().contains_key(&(cx, cz))
    }

    pub fn insert_chunk(&self, chunk: Chunk) -> ChunkRef {
        let key = (chunk.cx, chunk.cz);
        let r = Arc::new(RwLock::new(chunk));
        self.chunks.write().unwrap().insert(key, r.clone());
        r
    }

    pub fn remove_chunk(&self, cx: i32, cz: i32) -> Option<ChunkRef> {
        self.chunks.write().unwrap().remove(&(cx, cz))
    }

    pub fn chunk_count(&self) -> usize {
        self.chunks.read().unwrap().len()
    }

    pub fn chunk_keys(&self) -> Vec<(i32, i32)> {
        self.chunks.read().unwrap().keys().copied().collect()
    }

    /// Raw voxel at world position (0 = air outside loaded chunks / height range).
    pub fn get(&self, x: i32, y: i32, z: i32) -> u16 {
        if !(0..CHUNK_HEIGHT as i32).contains(&y) {
            return 0;
        }
        match self.get_chunk(chunk_coord(x), chunk_coord(z)) {
            Some(c) => c.read().unwrap().get(local_coord(x), y as usize, local_coord(z)),
            None => 0,
        }
    }

    pub fn get_block(&self, x: i32, y: i32, z: i32) -> Block {
        block::vox_block(self.get(x, y, z))
    }

    pub fn props_at(&self, x: i32, y: i32, z: i32) -> &'static BlockProps {
        block::props(block::vox_id(self.get(x, y, z)))
    }

    pub fn is_loaded(&self, x: i32, z: i32) -> bool {
        self.has_chunk(chunk_coord(x), chunk_coord(z))
    }

    /// Set voxel without lighting updates (caller handles light + remesh flags). Returns old voxel.
    pub fn set_raw(&self, x: i32, y: i32, z: i32, v: u16) -> Option<u16> {
        if !(0..CHUNK_HEIGHT as i32).contains(&y) {
            return None;
        }
        let c = self.get_chunk(chunk_coord(x), chunk_coord(z))?;
        let mut c = c.write().unwrap();
        Some(c.set_tracked(local_coord(x), y as usize, local_coord(z), v))
    }

    /// Set a voxel with lighting update and mesh dirty marks. Returns the old voxel.
    pub fn set_block(&self, x: i32, y: i32, z: i32, v: u16) -> Option<u16> {
        let old = self.set_raw(x, y, z, v)?;
        if old != v {
            light::on_block_changed(self, x, y, z, old, v);
            self.mark_dirty_around(x, y, z);
        }
        Some(old)
    }

    pub fn sky_light(&self, x: i32, y: i32, z: i32) -> u8 {
        if y >= CHUNK_HEIGHT as i32 {
            return 15;
        }
        if y < 0 {
            return 0;
        }
        match self.get_chunk(chunk_coord(x), chunk_coord(z)) {
            Some(c) => c.read().unwrap().sky(local_coord(x), y as usize, local_coord(z)),
            None => 15,
        }
    }

    pub fn block_light(&self, x: i32, y: i32, z: i32) -> u8 {
        if !(0..CHUNK_HEIGHT as i32).contains(&y) {
            return 0;
        }
        match self.get_chunk(chunk_coord(x), chunk_coord(z)) {
            Some(c) => c.read().unwrap().block_light(local_coord(x), y as usize, local_coord(z)),
            None => 0,
        }
    }

    pub fn light_at(&self, x: i32, y: i32, z: i32) -> (u8, u8) {
        if y >= CHUNK_HEIGHT as i32 {
            return (15, 0);
        }
        if y < 0 {
            return (0, 0);
        }
        match self.get_chunk(chunk_coord(x), chunk_coord(z)) {
            Some(c) => {
                let c = c.read().unwrap();
                let (lx, ly, lz) = (local_coord(x), y as usize, local_coord(z));
                (c.sky(lx, ly, lz), c.block_light(lx, ly, lz))
            }
            None => (15, 0),
        }
    }

    pub fn height_at(&self, x: i32, z: i32) -> Option<i32> {
        let c = self.get_chunk(chunk_coord(x), chunk_coord(z))?;
        let h = c.read().unwrap().height(local_coord(x), local_coord(z));
        Some(h as i32)
    }

    pub fn biome_at(&self, x: i32, z: i32) -> u8 {
        match self.get_chunk(chunk_coord(x), chunk_coord(z)) {
            Some(c) => c.read().unwrap().biome_at(local_coord(x), local_coord(z)),
            None => 0,
        }
    }

    pub fn block_entity(&self, x: i32, y: i32, z: i32) -> Option<BlockEntity> {
        let c = self.get_chunk(chunk_coord(x), chunk_coord(z))?;
        let c = c.read().unwrap();
        c.block_entities.get(&(local_coord(x) as u8, y as u8, local_coord(z) as u8)).cloned()
    }

    pub fn set_block_entity(&self, x: i32, y: i32, z: i32, be: Option<BlockEntity>) {
        if let Some(c) = self.get_chunk(chunk_coord(x), chunk_coord(z)) {
            let mut c = c.write().unwrap();
            let key = (local_coord(x) as u8, y as u8, local_coord(z) as u8);
            match be {
                Some(b) => {
                    c.block_entities.insert(key, b);
                }
                None => {
                    c.block_entities.remove(&key);
                }
            }
            c.dirty_save = true;
        }
    }

    /// Run `f` with mutable access to a block entity; returns None if absent.
    pub fn with_block_entity<R>(&self, x: i32, y: i32, z: i32, f: impl FnOnce(&mut BlockEntity) -> R) -> Option<R> {
        let c = self.get_chunk(chunk_coord(x), chunk_coord(z))?;
        let mut c = c.write().unwrap();
        let key = (local_coord(x) as u8, y as u8, local_coord(z) as u8);
        let r = c.block_entities.get_mut(&key).map(f);
        if r.is_some() {
            c.dirty_save = true;
        }
        r
    }

    /// Bump the mesh version of the sub-chunk containing (x,y,z) and, when the block sits on a
    /// border, of the neighbouring sub-chunks too (their AO / culling depends on it).
    pub fn mark_dirty_around(&self, x: i32, y: i32, z: i32) {
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let (nx, ny, nz) = (x + dx, y + dy, z + dz);
                    if (nx >> 4, ny >> 4, nz >> 4) == (x >> 4, y >> 4, z >> 4) && (dx, dy, dz) != (0, 0, 0) {
                        continue;
                    }
                    if !(0..CHUNK_HEIGHT as i32).contains(&ny) {
                        continue;
                    }
                    if let Some(c) = self.get_chunk(chunk_coord(nx), chunk_coord(nz)) {
                        let mut c = c.write().unwrap();
                        let s = &mut c.subs[(ny >> 4) as usize];
                        s.version = s.version.wrapping_add(1);
                    }
                }
            }
        }
    }
}

/// Small cache of the last-touched chunk to make repeated neighbouring accesses cheap.
pub struct ChunkCache<'a> {
    world: &'a World,
    last: Option<((i32, i32), ChunkRef)>,
}

impl<'a> ChunkCache<'a> {
    pub fn new(world: &'a World) -> Self {
        ChunkCache { world, last: None }
    }
    #[inline]
    pub fn chunk(&mut self, x: i32, z: i32) -> Option<ChunkRef> {
        let key = (chunk_coord(x), chunk_coord(z));
        if let Some((k, c)) = &self.last {
            if *k == key {
                return Some(c.clone());
            }
        }
        let c = self.world.get_chunk(key.0, key.1)?;
        self.last = Some((key, c.clone()));
        Some(c)
    }
    #[inline]
    pub fn get(&mut self, x: i32, y: i32, z: i32) -> u16 {
        if !(0..CHUNK_HEIGHT as i32).contains(&y) {
            return 0;
        }
        match self.chunk(x, z) {
            Some(c) => c.read().unwrap().get(local_coord(x), y as usize, local_coord(z)),
            None => 0,
        }
    }
    #[inline]
    pub fn get_block(&mut self, x: i32, y: i32, z: i32) -> Block {
        block::vox_block(self.get(x, y, z))
    }
    #[inline]
    pub fn props(&mut self, x: i32, y: i32, z: i32) -> &'static BlockProps {
        block::props(block::vox_id(self.get(x, y, z)))
    }
    #[inline]
    pub fn is_solid(&mut self, x: i32, y: i32, z: i32) -> bool {
        self.props(x, y, z).solid
    }
}

pub const SEA_LEVEL: i32 = 62;
pub const CHUNK_SIZE_I: i32 = CHUNK_SIZE as i32;
