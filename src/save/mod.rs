//! World persistence: `saves/<name>/level.bin` + region files with RLE + zlib chunk blobs.

pub mod region;

use crate::entity::ItemDrop;
use crate::player::{GameMode, PlayerSave};
use crate::world::chunk::{BlockEntity, Chunk, CHUNK_SIZE, SUB_COUNT, SUB_VOLUME};
use region::Region;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub const SAVES_DIR: &str = "saves";
pub const FORMAT_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LevelData {
    pub version: u32,
    pub name: String,
    pub seed: u64,
    pub time: f64,
    pub spawn: [f32; 3],
    pub players: Vec<PlayerSave>,
    pub mode: GameMode,
    pub flat: bool,
    pub drops: Vec<ItemDrop>,
}

#[derive(Serialize, Deserialize)]
struct SubData {
    blocks: Vec<(u16, u16)>,
    light: Vec<(u8, u16)>,
}

#[derive(Serialize, Deserialize)]
struct ChunkData {
    version: u32,
    subs: Vec<Option<SubData>>,
    heightmap: Vec<u16>,
    biome: Vec<u8>,
    entities: Vec<((u8, u8, u8), BlockEntity)>,
}

fn rle_encode<T: Copy + PartialEq>(data: &[T]) -> Vec<(T, u16)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < data.len() {
        let v = data[i];
        let mut run = 1usize;
        while i + run < data.len() && data[i + run] == v && run < u16::MAX as usize {
            run += 1;
        }
        out.push((v, run as u16));
        i += run;
    }
    out
}

fn rle_decode<T: Copy>(runs: &[(T, u16)], out: &mut [T]) -> bool {
    let mut i = 0;
    for (v, run) in runs {
        let run = *run as usize;
        if i + run > out.len() {
            return false;
        }
        for o in out.iter_mut().skip(i).take(run) {
            *o = *v;
        }
        i += run;
    }
    i == out.len()
}

pub fn chunk_to_bytes(chunk: &Chunk) -> Vec<u8> {
    let subs = chunk
        .subs
        .iter()
        .map(|s| {
            if s.is_empty() && s.light.iter().all(|l| *l == 15 || *l == 0) && s.light.iter().all(|l| *l == s.light[0]) {
                // fully empty with uniform light: encode as light-only sub with no blocks
                Some(SubData { blocks: Vec::new(), light: rle_encode(&s.light[..]) })
            } else {
                let blocks = match &s.blocks {
                    Some(b) => rle_encode(&b[..]),
                    None => Vec::new(),
                };
                Some(SubData { blocks, light: rle_encode(&s.light[..]) })
            }
        })
        .collect();
    let data = ChunkData { version: FORMAT_VERSION, subs, heightmap: chunk.heightmap.to_vec(), biome: chunk.biome.to_vec(), entities: chunk.block_entities.iter().map(|(k, v)| (*k, v.clone())).collect() };
    let raw = bincode::serialize(&data).unwrap_or_default();
    let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    let _ = enc.write_all(&raw);
    enc.finish().unwrap_or_default()
}

pub fn bytes_to_chunk(cx: i32, cz: i32, bytes: &[u8]) -> Option<Chunk> {
    let mut dec = flate2::read::ZlibDecoder::new(bytes);
    let mut raw = Vec::new();
    dec.read_to_end(&mut raw).ok()?;
    let data: ChunkData = bincode::deserialize(&raw).ok()?;
    if data.subs.len() != SUB_COUNT || data.heightmap.len() != CHUNK_SIZE * CHUNK_SIZE || data.biome.len() != CHUNK_SIZE * CHUNK_SIZE {
        return None;
    }
    let mut chunk = Chunk::new(cx, cz);
    for (i, sd) in data.subs.into_iter().enumerate() {
        let Some(sd) = sd else { continue };
        let sub = &mut chunk.subs[i];
        if !sd.blocks.is_empty() {
            let mut arr = Box::new([0u16; SUB_VOLUME]);
            if !rle_decode(&sd.blocks, &mut arr[..]) {
                return None;
            }
            sub.blocks = Some(arr);
            sub.recount();
        }
        let mut light = Box::new([0u8; SUB_VOLUME]);
        if !rle_decode(&sd.light, &mut light[..]) {
            return None;
        }
        sub.light = light;
    }
    chunk.heightmap.copy_from_slice(&data.heightmap);
    chunk.biome.copy_from_slice(&data.biome);
    chunk.block_entities = data.entities.into_iter().collect();
    chunk.seams_done = [true; 4];
    Some(chunk)
}

pub fn world_dir(name: &str) -> PathBuf {
    Path::new(SAVES_DIR).join(sanitize(name))
}

pub fn sanitize(name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '_' }).collect();
    let s = s.trim().to_string();
    if s.is_empty() {
        "world".to_string()
    } else {
        s
    }
}

pub fn list_worlds() -> Vec<String> {
    let mut v: Vec<String> = match std::fs::read_dir(SAVES_DIR) {
        Ok(rd) => rd.filter_map(|e| e.ok()).filter(|e| e.path().join("level.bin").exists()).filter_map(|e| e.file_name().into_string().ok()).collect(),
        Err(_) => Vec::new(),
    };
    v.sort();
    v
}

pub fn delete_world(name: &str) {
    let _ = std::fs::remove_dir_all(world_dir(name));
}

pub fn load_level(name: &str) -> Option<LevelData> {
    let bytes = std::fs::read(world_dir(name).join("level.bin")).ok()?;
    bincode::deserialize(&bytes).ok()
}

/// Region cache + level writer for one world.
pub struct SaveManager {
    pub dir: PathBuf,
    regions: HashMap<(i32, i32), Region>,
    dirty: HashSet<(i32, i32)>,
    pub chunks_written: u64,
}

impl SaveManager {
    pub fn open(name: &str) -> SaveManager {
        let dir = world_dir(name);
        let _ = std::fs::create_dir_all(dir.join("region"));
        SaveManager { dir, regions: HashMap::new(), dirty: HashSet::new(), chunks_written: 0 }
    }

    fn region_key(cx: i32, cz: i32) -> ((i32, i32), u32) {
        let rx = cx >> 5;
        let rz = cz >> 5;
        let lx = (cx & 31) as u32;
        let lz = (cz & 31) as u32;
        ((rx, rz), lz * 32 + lx)
    }

    fn region_path(&self, rx: i32, rz: i32) -> PathBuf {
        self.dir.join("region").join(format!("r.{rx}.{rz}.bin"))
    }

    fn region_mut(&mut self, rx: i32, rz: i32) -> &mut Region {
        if !self.regions.contains_key(&(rx, rz)) {
            let path = self.region_path(rx, rz);
            let r = Region::load(&path).unwrap_or_default();
            self.regions.insert((rx, rz), r);
        }
        self.regions.get_mut(&(rx, rz)).unwrap()
    }

    pub fn has_chunk(&mut self, cx: i32, cz: i32) -> bool {
        let ((rx, rz), idx) = Self::region_key(cx, cz);
        self.region_mut(rx, rz).get(idx).is_some()
    }

    pub fn load_chunk(&mut self, cx: i32, cz: i32) -> Option<Chunk> {
        let ((rx, rz), idx) = Self::region_key(cx, cz);
        let bytes = self.region_mut(rx, rz).get(idx)?.to_vec();
        bytes_to_chunk(cx, cz, &bytes)
    }

    pub fn store_chunk(&mut self, chunk: &Chunk) {
        let ((rx, rz), idx) = Self::region_key(chunk.cx, chunk.cz);
        let bytes = chunk_to_bytes(chunk);
        self.region_mut(rx, rz).set(idx, bytes);
        self.dirty.insert((rx, rz));
        self.chunks_written += 1;
    }

    /// Write every modified region file (atomically).
    pub fn flush(&mut self) -> std::io::Result<()> {
        let dirty: Vec<(i32, i32)> = self.dirty.drain().collect();
        for (rx, rz) in dirty {
            let path = self.region_path(rx, rz);
            if let Some(r) = self.regions.get(&(rx, rz)) {
                r.save(&path)?;
            }
        }
        Ok(())
    }

    pub fn save_level(&self, level: &LevelData) -> std::io::Result<()> {
        let bytes = bincode::serialize(level).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        let path = self.dir.join("level.bin");
        let tmp = self.dir.join("level.bin.tmp");
        std::fs::write(&tmp, bytes)?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::items::ItemStack;
    use crate::world::block::{voxel, Block};

    #[test]
    fn chunk_round_trip_preserves_blocks_light_and_entities() {
        let g = crate::world::gen::Generator::new(77);
        let mut c = g.generate(2, -3);
        crate::world::light::init_chunk_light(&mut c);
        c.set(3, 70, 3, voxel(Block::Torch, 2));
        c.block_entities.insert((1, 60, 1), BlockEntity::Chest { items: vec![ItemStack::block(Block::Stone, 5); 27] });
        let bytes = chunk_to_bytes(&c);
        let d = bytes_to_chunk(2, -3, &bytes).expect("decode");
        for y in 0..256 {
            for z in 0..16 {
                for x in 0..16 {
                    assert_eq!(c.get(x, y, z), d.get(x, y, z), "block mismatch at {x},{y},{z}");
                    assert_eq!(c.sky(x, y, z), d.sky(x, y, z));
                    assert_eq!(c.block_light(x, y, z), d.block_light(x, y, z));
                }
            }
        }
        assert_eq!(c.heightmap, d.heightmap);
        assert_eq!(c.biome, d.biome);
        assert!(matches!(d.block_entities.get(&(1, 60, 1)), Some(BlockEntity::Chest { items }) if items.len() == 27));
        assert!(bytes.len() < 40_000, "compressed chunk unexpectedly large: {}", bytes.len());
    }

    #[test]
    fn region_files_round_trip_on_disk() {
        let dir = std::env::temp_dir().join(format!("blockhaven_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(dir.join("region"));
        let mut sm = SaveManager { dir: dir.clone(), regions: HashMap::new(), dirty: HashSet::new(), chunks_written: 0 };
        let mut c = Chunk::new(-1, 40);
        c.set(0, 10, 0, voxel(Block::GoldBlock, 0));
        c.recompute_heightmap();
        sm.store_chunk(&c);
        sm.flush().unwrap();
        let mut sm2 = SaveManager { dir: dir.clone(), regions: HashMap::new(), dirty: HashSet::new(), chunks_written: 0 };
        assert!(sm2.has_chunk(-1, 40));
        assert!(!sm2.has_chunk(0, 40));
        let d = sm2.load_chunk(-1, 40).unwrap();
        assert_eq!(d.get_block(0, 10, 0), Block::GoldBlock);
        let _ = std::fs::remove_dir_all(dir);
    }
}
