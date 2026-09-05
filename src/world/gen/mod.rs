//! Terrain generation: layered noise heightmap, biomes, caves, ores, fluids, decorations.

pub mod decor;
pub mod structures;

use crate::world::block::{voxel, Block};
use crate::world::chunk::{Chunk, CHUNK_HEIGHT, CHUNK_SIZE};
use crate::world::noise::{Perlin, Rng};
use crate::world::SEA_LEVEL;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Biome {
    Ocean = 0,
    Plains = 1,
    Forest = 2,
    Desert = 3,
    SnowyTaiga = 4,
    Swamp = 5,
    Mountains = 6,
    Beach = 7,
    River = 8,
}

impl Biome {
    pub fn from_id(id: u8) -> Biome {
        match id {
            1 => Biome::Plains,
            2 => Biome::Forest,
            3 => Biome::Desert,
            4 => Biome::SnowyTaiga,
            5 => Biome::Swamp,
            6 => Biome::Mountains,
            7 => Biome::Beach,
            8 => Biome::River,
            _ => Biome::Ocean,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Biome::Ocean => "Ocean",
            Biome::Plains => "Plains",
            Biome::Forest => "Forest",
            Biome::Desert => "Desert",
            Biome::SnowyTaiga => "Snowy Taiga",
            Biome::Swamp => "Swamp",
            Biome::Mountains => "Mountains",
            Biome::Beach => "Beach",
            Biome::River => "River",
        }
    }
    pub fn is_cold(self) -> bool {
        matches!(self, Biome::SnowyTaiga)
    }
}

/// Climate values are kept for debugging / future biome features.
#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub struct ColumnInfo {
    pub height: i32,
    pub biome: Biome,
    pub temperature: f32,
    pub humidity: f32,
    pub mountain: f32,
    pub river: bool,
}

pub struct Generator {
    pub seed: u64,
    pub flat: bool,
    continent: Perlin,
    hills: Perlin,
    erosion: Perlin,
    mountain: Perlin,
    detail: Perlin,
    river: Perlin,
    temp: Perlin,
    humid: Perlin,
    swamp: Perlin,
    cheese: Perlin,
    worm_a: Perlin,
    worm_b: Perlin,
}

pub const FLAT_HEIGHT: i32 = 64;

impl Generator {
    pub fn new(seed: u64) -> Generator {
        let p = |k: u64| Perlin::new(seed.wrapping_add(k.wrapping_mul(0x9E3779B97F4A7C15)));
        Generator {
            seed,
            flat: false,
            continent: p(1),
            hills: p(2),
            erosion: p(3),
            mountain: p(4),
            detail: p(5),
            river: p(6),
            temp: p(7),
            humid: p(8),
            swamp: p(9),
            cheese: p(10),
            worm_a: p(11),
            worm_b: p(12),
        }
    }

    /// Flat grass world used by tests and the "flat" world type.
    pub fn flat(seed: u64) -> Generator {
        let mut g = Generator::new(seed);
        g.flat = true;
        g
    }

    /// Pure function of (x, z): surface height and biome.
    pub fn column(&self, x: i32, z: i32) -> ColumnInfo {
        if self.flat {
            return ColumnInfo { height: FLAT_HEIGHT, biome: Biome::Plains, temperature: 0.2, humidity: 0.0, mountain: 0.0, river: false };
        }
        let fx = x as f64;
        let fz = z as f64;
        let cont = self.continent.fbm2(fx / 1500.0, fz / 1500.0, 4, 2.0, 0.5);
        let hill = self.hills.fbm2(fx / 140.0, fz / 140.0, 5, 2.0, 0.5);
        let erosion = self.erosion.fbm2(fx / 480.0, fz / 480.0, 3, 2.0, 0.5);
        let mtn = self.mountain.ridge2(fx / 650.0, fz / 650.0, 4);
        let det = self.detail.fbm2(fx / 28.0, fz / 28.0, 3, 2.0, 0.5);
        let land = smoothstep(-0.25, 0.1, cont);
        let mtn_mask = smoothstep(0.42, 0.62, mtn) * smoothstep(0.0, 0.3, cont);
        let hilliness = 0.35 + 0.65 * smoothstep(-0.3, 0.5, erosion);
        let peaks = smoothstep(0.5, 0.9, mtn);
        let ridges = self.detail.ridge2(fx / 90.0, fz / 90.0, 3) * 14.0 * mtn_mask;
        let mut h = 40.0 + land * 26.0 + cont.max(0.0) * 14.0 + hill * 12.0 * hilliness * land + mtn_mask * (30.0 + 95.0 * peaks) + ridges + det * 2.5;
        // rivers
        let rv = self.river.fbm2(fx / 420.0, fz / 420.0, 3, 2.0, 0.5).abs();
        let river_w = 0.045;
        let mut river = false;
        if land > 0.5 && h > (SEA_LEVEL - 1) as f64 {
            let t = smoothstep(river_w, river_w * 2.2, rv);
            let river_h = (SEA_LEVEL - 3) as f64 - (1.0 - (rv / river_w).min(1.0)) * 3.0;
            h = river_h + (h - river_h) * t;
            river = t < 0.55;
        }
        let temp = self.temp.fbm2(fx / 1100.0, fz / 1100.0, 3, 2.0, 0.5) as f32 - ((h as f32 - 70.0).max(0.0) / 120.0);
        let hum = self.humid.fbm2(fx / 900.0, fz / 900.0, 3, 2.0, 0.5) as f32;
        let mut height = h.round() as i32;
        let sea = SEA_LEVEL;
        let biome = if height < sea - 3 {
            Biome::Ocean
        } else if river && height <= sea {
            Biome::River
        } else if mtn_mask > 0.5 || height > 98 {
            Biome::Mountains
        } else if height <= sea + 1 && temp > -0.3 {
            Biome::Beach
        } else if temp < -0.28 {
            Biome::SnowyTaiga
        } else if temp > 0.3 && hum < -0.05 {
            Biome::Desert
        } else if hum > 0.25 && temp > -0.1 && height < sea + 8 {
            Biome::Swamp
        } else if hum > 0.05 {
            Biome::Forest
        } else {
            Biome::Plains
        };
        if biome == Biome::Swamp {
            let sn = self.swamp.fbm2(fx / 40.0, fz / 40.0, 2, 2.0, 0.5);
            if sn > 0.25 && height <= sea + 3 {
                height = sea - 1;
            }
        }
        ColumnInfo { height: height.clamp(5, CHUNK_HEIGHT as i32 - 20), biome, temperature: temp, humidity: hum, mountain: mtn_mask as f32, river }
    }

    /// Height of the terrain at (x,z) plus water: the y a mob or player would stand on.
    pub fn surface_height(&self, x: i32, z: i32) -> i32 {
        self.column(x, z).height.max(SEA_LEVEL)
    }

    pub fn generate(&self, cx: i32, cz: i32) -> Chunk {
        let mut chunk = Chunk::new(cx, cz);
        let mut infos = [ColumnInfo { height: 0, biome: Biome::Plains, temperature: 0.0, humidity: 0.0, mountain: 0.0, river: false }; CHUNK_SIZE * CHUNK_SIZE];
        for lz in 0..CHUNK_SIZE {
            for lx in 0..CHUNK_SIZE {
                let info = self.column(cx * 16 + lx as i32, cz * 16 + lz as i32);
                infos[lz * CHUNK_SIZE + lx] = info;
                chunk.biome[lz * CHUNK_SIZE + lx] = info.biome as u8;
            }
        }
        let mut rng = Rng::at(self.seed, cx as i64, cz as i64, 0xC0FFEE);
        self.fill_terrain(&mut chunk, &infos, &mut rng);
        if !self.flat {
            self.carve_caves(&mut chunk, &infos);
            self.place_ores(&mut chunk, &mut rng);
        }
        chunk.recompute_heightmap();
        decor::decorate(self, &mut chunk);
        structures::apply(self, &mut chunk);
        chunk.recompute_heightmap();
        chunk
    }

    fn fill_terrain(&self, chunk: &mut Chunk, infos: &[ColumnInfo], rng: &mut Rng) {
        let stone = voxel(Block::Stone, 0);
        let sea = SEA_LEVEL;
        for lz in 0..CHUNK_SIZE {
            for lx in 0..CHUNK_SIZE {
                let info = infos[lz * CHUNK_SIZE + lx];
                let h = info.height;
                let (top, filler, filler_depth) = surface_blocks(info, rng);
                for y in 0..=h.max(sea) {
                    let v = if y == 0 || (y < 4 && rng.chance(0.5 - y as f32 * 0.12)) {
                        voxel(Block::Bedrock, 0)
                    } else if y < h - filler_depth {
                        stone
                    } else if y < h {
                        filler
                    } else if y == h {
                        top
                    } else {
                        // between terrain and sea level: water
                        voxel(Block::Water, 0)
                    };
                    chunk.set(lx, y as usize, lz, v);
                }
                // ice over cold water
                if h < sea && info.biome.is_cold() {
                    chunk.set(lx, sea as usize, lz, voxel(Block::Ice, 0));
                }
            }
        }
    }

    fn carve_caves(&self, chunk: &mut Chunk, infos: &[ColumnInfo]) {
        let air = 0u16;
        let lava = voxel(Block::Lava, 0);
        for lz in 0..CHUNK_SIZE {
            for lx in 0..CHUNK_SIZE {
                let info = infos[lz * CHUNK_SIZE + lx];
                let wx = (chunk.cx * 16 + lx as i32) as f64;
                let wz = (chunk.cz * 16 + lz as i32) as f64;
                let h = info.height;
                // don't open caves into the sea floor
                let top = if h <= SEA_LEVEL { h - 6 } else { h + 1 };
                for y in 5..top.min(CHUNK_HEIGHT as i32 - 1) {
                    let fy = y as f64;
                    let a = self.worm_a.get3(wx / 95.0, fy / 60.0, wz / 95.0);
                    let b = self.worm_b.get3(wx / 95.0 + 31.0, fy / 60.0, wz / 95.0 + 17.0);
                    let worm = a * a + b * b < 0.0032;
                    // big cheese caves stay well below the surface; worms may break through
                    let cheese = if !worm && y < h - 12 { self.cheese.fbm3(wx / 70.0, fy / 45.0, wz / 70.0, 3, 2.0, 0.5) > 0.44 } else { false };
                    if worm || cheese {
                        let cur = chunk.get(lx, y as usize, lz);
                        if cur == 0 || crate::world::block::vox_block(cur) == Block::Bedrock || crate::world::block::is_water(cur) {
                            continue;
                        }
                        chunk.set(lx, y as usize, lz, if y <= 10 { lava } else { air });
                    }
                }
            }
        }
    }

    fn place_ores(&self, chunk: &mut Chunk, rng: &mut Rng) {
        let veins: [(Block, u32, i32, i32, u32); 7] = [
            (Block::Dirt, 6, 20, 110, 18),
            (Block::Gravel, 4, 10, 100, 18),
            (Block::CoalOre, 20, 10, 128, 10),
            (Block::IronOre, 18, 5, 64, 7),
            (Block::GoldOre, 4, 5, 32, 6),
            (Block::RedstoneOre, 8, 5, 16, 7),
            (Block::DiamondOre, 2, 5, 14, 5),
        ];
        let stone_id = Block::Stone.id();
        for (block, count, ymin, ymax, size) in veins {
            for _ in 0..count {
                let mut x = rng.range(0, 16);
                let mut y = rng.range(ymin, ymax);
                let mut z = rng.range(0, 16);
                for _ in 0..size {
                    if (0..16).contains(&x) && (0..16).contains(&z) && (1..CHUNK_HEIGHT as i32).contains(&y) {
                        let cur = chunk.get(x as usize, y as usize, z as usize);
                        if crate::world::block::vox_id(cur) == stone_id {
                            chunk.set(x as usize, y as usize, z as usize, voxel(block, 0));
                        }
                    }
                    match rng.below(6) {
                        0 => x += 1,
                        1 => x -= 1,
                        2 => y += 1,
                        3 => y -= 1,
                        4 => z += 1,
                        _ => z -= 1,
                    }
                }
            }
        }
    }
}

/// (top block, filler block, filler depth) for a column.
fn surface_blocks(info: ColumnInfo, rng: &mut Rng) -> (u16, u16, i32) {
    let h = info.height;
    let sea = SEA_LEVEL;
    let grass = voxel(Block::Grass, 0);
    let dirt = voxel(Block::Dirt, 0);
    let sand = voxel(Block::Sand, 0);
    let depth = 3 + rng.range(0, 2);
    match info.biome {
        Biome::Ocean => {
            if h > sea - 9 {
                (sand, sand, 2)
            } else if rng.chance(0.15) {
                (voxel(Block::Clay, 0), voxel(Block::Gravel, 0), 2)
            } else {
                (voxel(Block::Gravel, 0), dirt, 2)
            }
        }
        Biome::River => {
            if rng.chance(0.2) {
                (voxel(Block::Gravel, 0), dirt, 2)
            } else {
                (sand, sand, 2)
            }
        }
        Biome::Beach => (sand, sand, depth),
        Biome::Desert => (sand, voxel(Block::Sandstone, 0), depth + 2),
        Biome::SnowyTaiga => {
            if h <= sea {
                (voxel(Block::Gravel, 0), dirt, 2)
            } else if rng.chance(0.15) {
                (voxel(Block::Podzol, 0), dirt, depth)
            } else {
                (voxel(Block::SnowyGrass, 0), dirt, depth)
            }
        }
        Biome::Swamp => {
            if h < sea {
                if rng.chance(0.3) {
                    (voxel(Block::Clay, 0), dirt, 2)
                } else {
                    (dirt, dirt, 2)
                }
            } else {
                (grass, dirt, depth)
            }
        }
        Biome::Mountains => {
            if h > 108 {
                (voxel(Block::Snow, 0), voxel(Block::Stone, 0), 1)
            } else if h > 92 || info.mountain > 0.8 {
                (voxel(Block::Stone, 0), voxel(Block::Stone, 0), 1)
            } else if h <= sea {
                (voxel(Block::Gravel, 0), dirt, 2)
            } else {
                (grass, dirt, 2)
            }
        }
        Biome::Plains | Biome::Forest => {
            if h < sea {
                (dirt, dirt, 2)
            } else {
                (grass, dirt, depth)
            }
        }
    }
}

pub fn smoothstep(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_is_deterministic() {
        let a = Generator::new(2024);
        let b = Generator::new(2024);
        let ca = a.generate(3, -2);
        let cb = b.generate(3, -2);
        for y in 0..CHUNK_HEIGHT {
            for z in 0..16 {
                for x in 0..16 {
                    assert_eq!(ca.get(x, y, z), cb.get(x, y, z));
                }
            }
        }
        assert_eq!(ca.heightmap, cb.heightmap);
        assert_eq!(ca.biome, cb.biome);
    }

    #[test]
    fn terrain_has_bedrock_and_a_surface() {
        let g = Generator::new(7);
        let c = g.generate(0, 0);
        for z in 0..16 {
            for x in 0..16 {
                assert_eq!(c.get_block(x, 0, z), Block::Bedrock);
                assert!(c.height(x, z) > 5);
            }
        }
    }

    #[test]
    fn biomes_vary_across_the_world() {
        let g = Generator::new(99);
        let mut seen = std::collections::HashSet::new();
        for i in 0..400 {
            let x = (i % 20) * 400 - 4000;
            let z = (i / 20) * 400 - 4000;
            seen.insert(g.column(x, z).biome as u8);
        }
        assert!(seen.len() >= 5, "expected several biomes, got {:?}", seen);
    }

    #[test]
    fn flat_world_is_flat() {
        let g = Generator::flat(1);
        let c = g.generate(5, 5);
        for z in 0..16 {
            for x in 0..16 {
                assert_eq!(c.get_block(x, FLAT_HEIGHT as usize, z), Block::Grass);
                assert_eq!(c.get_block(x, FLAT_HEIGHT as usize + 1, z), Block::Air);
            }
        }
    }
}
