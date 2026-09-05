//! Structures: villages (huts, houses, farms, well), underground dungeons, and surface ruins.
//! Every structure is a pure function of the seed, so each chunk builds only the parts inside it.

use crate::player::items::{Item, ItemStack};
use crate::world::block::{voxel, Block};
use crate::world::chunk::{BlockEntity, Chunk};
use crate::world::gen::decor::Writer;
use crate::world::gen::{Biome, Generator};
use crate::world::noise::Rng;
use crate::world::SEA_LEVEL;

const VILLAGE_SALT: u64 = 0x5111A6E;
const DUNGEON_SALT: u64 = 0xD0176E0;
const RUIN_SALT: u64 = 0x2011115;

pub const VILLAGE_CELL: i32 = 12; // chunks
pub const RUIN_CELL: i32 = 9;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PieceKind {
    Well,
    Hut,
    House,
    Farm,
    LampPost,
    Ruin,
}

#[derive(Clone, Copy, Debug)]
pub struct Piece {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub w: i32,
    pub h: i32,
    pub d: i32,
    pub kind: PieceKind,
    pub facing: u8,
    pub seed: u64,
    pub desert: bool,
    pub cold: bool,
}

impl Piece {
    fn overlaps_chunk(&self, cx: i32, cz: i32) -> bool {
        let x0 = cx * 16;
        let z0 = cz * 16;
        self.x < x0 + 16 && self.x + self.w > x0 && self.z < z0 + 16 && self.z + self.d > z0
    }
}

fn div_floor(a: i32, b: i32) -> i32 {
    (a as f64 / b as f64).floor() as i32
}

/// Village pieces for a village cell, if a village exists there.
pub fn village(g: &Generator, cellx: i32, cellz: i32) -> Vec<Piece> {
    let mut rng = Rng::at(g.seed, cellx as i64, cellz as i64, VILLAGE_SALT);
    if !rng.chance(0.45) {
        return Vec::new();
    }
    let cx = cellx * VILLAGE_CELL * 16 + rng.range(40, VILLAGE_CELL * 16 - 40);
    let cz = cellz * VILLAGE_CELL * 16 + rng.range(40, VILLAGE_CELL * 16 - 40);
    let center = g.column(cx, cz);
    if !matches!(center.biome, Biome::Plains | Biome::Forest | Biome::Desert | Biome::SnowyTaiga) || center.height <= SEA_LEVEL + 1 || center.height > 95 {
        return Vec::new();
    }
    // flatness check
    for (dx, dz) in [(-14, 0), (14, 0), (0, -14), (0, 14), (10, 10), (-10, -10)] {
        let c = g.column(cx + dx, cz + dz);
        if (c.height - center.height).abs() > 6 || c.height <= SEA_LEVEL {
            return Vec::new();
        }
    }
    let desert = center.biome == Biome::Desert;
    let cold = center.biome == Biome::SnowyTaiga;
    let mut pieces = vec![Piece { x: cx - 2, y: center.height + 1, z: cz - 2, w: 5, h: 5, d: 5, kind: PieceKind::Well, facing: 0, seed: rng.next_u64(), desert, cold }];
    let n = rng.range(5, 9);
    let mut placed: Vec<(i32, i32, i32, i32)> = vec![(cx - 3, cz - 3, cx + 3, cz + 3)];
    for i in 0..n {
        let kind = match rng.below(10) {
            0..=3 => PieceKind::Hut,
            4..=6 => PieceKind::House,
            7..=8 => PieceKind::Farm,
            _ => PieceKind::LampPost,
        };
        let (w, d, h) = match kind {
            PieceKind::Hut => (5, 5, 5),
            PieceKind::House => (7, 7, 6),
            PieceKind::Farm => (7, 9, 2),
            PieceKind::LampPost => (1, 1, 4),
            _ => (5, 5, 5),
        };
        let mut ok = None;
        for _try in 0..12 {
            let ang = (i as f32 + rng.f32()) / n as f32 * std::f32::consts::TAU;
            let r = rng.range(9, 30) as f32;
            let px = cx + (ang.cos() * r) as i32 - w / 2;
            let pz = cz + (ang.sin() * r) as i32 - d / 2;
            let rect = (px - 1, pz - 1, px + w + 1, pz + d + 1);
            let collides = placed.iter().any(|p| rect.0 < p.2 && rect.2 > p.0 && rect.1 < p.3 && rect.3 > p.1);
            if collides {
                continue;
            }
            let col = g.column(px + w / 2, pz + d / 2);
            if (col.height - center.height).abs() > 5 || col.height <= SEA_LEVEL {
                continue;
            }
            ok = Some((px, pz, col.height + 1, rect));
            break;
        }
        let Some((px, pz, py, rect)) = ok else { continue };
        placed.push(rect);
        // face the well
        let facing = if (px + w / 2 - cx).abs() > (pz + d / 2 - cz).abs() {
            if px + w / 2 > cx { 3 } else { 1 }
        } else if pz + d / 2 > cz {
            0
        } else {
            2
        };
        pieces.push(Piece { x: px, y: py, z: pz, w, h, d, kind, facing, seed: rng.next_u64(), desert, cold });
    }
    pieces
}

pub fn ruin(g: &Generator, cellx: i32, cellz: i32) -> Option<Piece> {
    let mut rng = Rng::at(g.seed, cellx as i64, cellz as i64, RUIN_SALT);
    if !rng.chance(0.4) {
        return None;
    }
    let cx = cellx * RUIN_CELL * 16 + rng.range(12, RUIN_CELL * 16 - 12);
    let cz = cellz * RUIN_CELL * 16 + rng.range(12, RUIN_CELL * 16 - 12);
    let c = g.column(cx, cz);
    if c.height <= SEA_LEVEL + 1 || matches!(c.biome, Biome::Ocean | Biome::River | Biome::Beach) || c.height > 100 {
        return None;
    }
    Some(Piece { x: cx - 8, y: c.height + 1, z: cz - 8, w: 17, h: 8, d: 17, kind: PieceKind::Ruin, facing: 0, seed: rng.next_u64(), desert: c.biome == Biome::Desert, cold: c.biome.is_cold() })
}

pub fn apply(g: &Generator, chunk: &mut Chunk) {
    if g.flat {
        return;
    }
    let cx = chunk.cx;
    let cz = chunk.cz;
    let mut pieces: Vec<Piece> = Vec::new();
    // villages: cells whose village could reach this chunk (radius ~ 40 blocks = 3 chunks)
    for dcz in div_floor(cz - 3, VILLAGE_CELL)..=div_floor(cz + 3, VILLAGE_CELL) {
        for dcx in div_floor(cx - 3, VILLAGE_CELL)..=div_floor(cx + 3, VILLAGE_CELL) {
            pieces.extend(village(g, dcx, dcz));
        }
    }
    for dcz in div_floor(cz - 2, RUIN_CELL)..=div_floor(cz + 2, RUIN_CELL) {
        for dcx in div_floor(cx - 2, RUIN_CELL)..=div_floor(cx + 2, RUIN_CELL) {
            pieces.extend(ruin(g, dcx, dcz));
        }
    }
    let mut w = Writer::new(chunk);
    for p in pieces.iter().filter(|p| p.overlaps_chunk(cx, cz)) {
        build_piece(g, &mut w, p);
    }
    dungeon(g, &mut w);
}

fn foundation(g: &Generator, w: &mut Writer, p: &Piece, fill: u16) {
    for z in p.z..p.z + p.d {
        for x in p.x..p.x + p.w {
            if !w.inside(x, z) {
                continue;
            }
            let ground = g.column(x, z).height;
            // clear above the floor
            for y in p.y..p.y + p.h + 2 {
                w.set(x, y, z, 0);
            }
            // fill below the floor down to terrain
            let mut y = p.y - 1;
            while y > ground.min(p.y - 1) - 1 && y >= 0 {
                w.set(x, y, z, fill);
                y -= 1;
            }
            // also make sure there is something directly under the floor even if terrain is higher
            w.set(x, p.y - 1, z, fill);
        }
    }
}

fn village_loot(rng: &mut Rng) -> Vec<ItemStack> {
    let mut items = vec![ItemStack::EMPTY; 27];
    let pool: [(ItemStack, f32); 12] = [
        (ItemStack::item(Item::Bread, 1), 0.9),
        (ItemStack::item(Item::Bread, 2), 0.5),
        (ItemStack::item(Item::Wheat, 3), 0.7),
        (ItemStack::item(Item::Apple, 2), 0.5),
        (ItemStack::item(Item::IronIngot, 1), 0.45),
        (ItemStack::item(Item::GoldIngot, 1), 0.2),
        (ItemStack::item(Item::Leather, 2), 0.4),
        (ItemStack::block(Block::Torch, 4), 0.8),
        (ItemStack::block(Block::OakPlanks, 8), 0.6),
        (ItemStack::item(Item::IronPickaxe, 1), 0.15),
        (ItemStack::item(Item::Diamond, 1), 0.06),
        (ItemStack::item(Item::Coal, 4), 0.5),
    ];
    for (stack, chance) in pool {
        if rng.chance(chance) {
            let slot = rng.below(27) as usize;
            items[slot] = stack;
        }
    }
    items
}

fn dungeon_loot(rng: &mut Rng) -> Vec<ItemStack> {
    let mut items = vec![ItemStack::EMPTY; 27];
    let pool: [(ItemStack, f32); 12] = [
        (ItemStack::item(Item::IronIngot, 2), 0.6),
        (ItemStack::item(Item::Bread, 1), 0.7),
        (ItemStack::item(Item::Redstone, 4), 0.6),
        (ItemStack::item(Item::Gunpowder, 3), 0.5),
        (ItemStack::item(Item::String, 3), 0.5),
        (ItemStack::item(Item::Bone, 4), 0.6),
        (ItemStack::item(Item::Arrow, 8), 0.5),
        (ItemStack::item(Item::Diamond, 1), 0.12),
        (ItemStack::item(Item::GoldIngot, 2), 0.3),
        (ItemStack::item(Item::Coal, 5), 0.5),
        (ItemStack::item(Item::Bow, 1), 0.2),
        (ItemStack::item(Item::IronSword, 1), 0.15),
    ];
    for (stack, chance) in pool {
        if rng.chance(chance) {
            let slot = rng.below(27) as usize;
            items[slot] = stack;
        }
    }
    items
}

fn build_piece(g: &Generator, w: &mut Writer, p: &Piece) {
    let mut rng = Rng::new(p.seed);
    let wall = if p.desert { voxel(Block::Sandstone, 0) } else if p.cold { voxel(Block::SprucePlanks, 0) } else { voxel(Block::OakPlanks, 0) };
    let corner = if p.desert { voxel(Block::Sandstone, 0) } else if p.cold { voxel(Block::SpruceLog, 0) } else { voxel(Block::OakLog, 0) };
    let floor = if p.desert { voxel(Block::Sandstone, 0) } else { voxel(Block::Cobblestone, 0) };
    let roof = if p.desert { voxel(Block::Sandstone, 0) } else if p.cold { voxel(Block::SpruceLog, 1) } else { voxel(Block::OakLog, 1) };
    let glass = voxel(Block::Glass, 0);
    match p.kind {
        PieceKind::Well => {
            foundation(g, w, p, floor);
            let (x0, y0, z0) = (p.x, p.y, p.z);
            // water basin
            w.fill(x0, y0 - 2, z0, x0 + 4, y0, z0 + 4, floor);
            w.fill(x0 + 1, y0 - 1, z0 + 1, x0 + 3, y0, z0 + 3, voxel(Block::Water, 0));
            // posts + roof
            for (dx, dz) in [(0, 0), (4, 0), (0, 4), (4, 4)] {
                w.fill(x0 + dx, y0 + 1, z0 + dz, x0 + dx, y0 + 3, z0 + dz, corner);
            }
            w.fill(x0, y0 + 4, z0, x0 + 4, y0 + 4, z0 + 4, floor);
            w.set(x0 + 2, y0 + 3, z0 + 2, voxel(Block::Torch, 0));
        }
        PieceKind::Hut | PieceKind::House => {
            foundation(g, w, p, floor);
            let (x0, y0, z0, x1, z1) = (p.x, p.y, p.z, p.x + p.w - 1, p.z + p.d - 1);
            let wall_h = if p.kind == PieceKind::Hut { 3 } else { 4 };
            // walls
            w.fill(x0, y0, z0, x1, y0 + wall_h - 1, z0, wall);
            w.fill(x0, y0, z1, x1, y0 + wall_h - 1, z1, wall);
            w.fill(x0, y0, z0, x0, y0 + wall_h - 1, z1, wall);
            w.fill(x1, y0, z0, x1, y0 + wall_h - 1, z1, wall);
            for (x, z) in [(x0, z0), (x1, z0), (x0, z1), (x1, z1)] {
                w.fill(x, y0, z, x, y0 + wall_h - 1, z, corner);
            }
            // roof
            w.fill(x0, y0 + wall_h, z0, x1, y0 + wall_h, z1, roof);
            if p.kind == PieceKind::House {
                w.fill(x0 + 1, y0 + wall_h + 1, z0 + 1, x1 - 1, y0 + wall_h + 1, z1 - 1, roof);
            }
            // door on the facing side
            let (dx, dz) = match p.facing {
                0 => ((x0 + x1) / 2, z0),
                1 => (x1, (z0 + z1) / 2),
                2 => ((x0 + x1) / 2, z1),
                _ => (x0, (z0 + z1) / 2),
            };
            let door_facing = p.facing;
            w.set(dx, y0, dz, voxel(Block::Door, door_facing));
            w.set(dx, y0 + 1, dz, voxel(Block::Door, door_facing | 8));
            // windows on the two sides that are not the door side
            for side in 0..4u8 {
                if side == p.facing {
                    continue;
                }
                let (wx, wz) = match side {
                    0 => ((x0 + x1) / 2, z0),
                    1 => (x1, (z0 + z1) / 2),
                    2 => ((x0 + x1) / 2, z1),
                    _ => (x0, (z0 + z1) / 2),
                };
                w.set(wx, y0 + 1, wz, glass);
            }
            // interior: torch, chest, bed, crafting table, furnace
            let ix0 = x0 + 1;
            let iz0 = z0 + 1;
            let ix1 = x1 - 1;
            let iz1 = z1 - 1;
            w.set(ix0, y0 + 2, iz0, voxel(Block::Torch, 0));
            w.set(ix0, y0 + 1, iz0, wall);
            let chest_pos = (ix1, y0, iz0);
            w.set(chest_pos.0, chest_pos.1, chest_pos.2, voxel(Block::Chest, 2));
            w.block_entity(chest_pos.0, chest_pos.1, chest_pos.2, BlockEntity::Chest { items: village_loot(&mut rng) });
            // bed along the back wall: foot then head
            let bed_facing = 0u8;
            if iz1 - 1 > iz0 {
                w.set(ix0, y0, iz1 - 1, voxel(Block::Bed, bed_facing));
                w.set(ix0, y0, iz1, voxel(Block::Bed, bed_facing | 4));
            }
            if p.kind == PieceKind::House {
                w.set(ix1, y0, iz1, voxel(Block::CraftingTable, 0));
                w.set(ix1 - 1, y0, iz1, voxel(Block::Furnace, 0));
                w.set(ix1, y0, iz1 - 1, voxel(Block::Bookshelf, 0));
                w.set(ix1, y0 + 2, iz1, voxel(Block::Torch, 0));
                w.set(ix1, y0 + 1, iz1, wall);
            }
        }
        PieceKind::Farm => {
            // farmland at floor level (p.y - 1), crops at p.y
            let (x0, z0, x1, z1) = (p.x, p.z, p.x + p.w - 1, p.z + p.d - 1);
            let yf = p.y - 1;
            for z in z0..=z1 {
                for x in x0..=x1 {
                    if !w.inside(x, z) {
                        continue;
                    }
                    let ground = g.column(x, z).height;
                    for y in p.y..p.y + 3 {
                        w.set(x, y, z, 0);
                    }
                    let mut y = yf - 1;
                    while y >= ground.min(yf - 1) && y >= 0 {
                        w.set(x, y, z, voxel(Block::Dirt, 0));
                        y -= 1;
                    }
                    let border = x == x0 || x == x1 || z == z0 || z == z1;
                    if border {
                        w.set(x, yf, z, corner);
                    } else if x == (x0 + x1) / 2 {
                        w.set(x, yf, z, voxel(Block::Water, 0));
                    } else {
                        w.set(x, yf, z, voxel(Block::Farmland, 0));
                        let stage = 2 + rng.below(6) as u8;
                        w.set(x, p.y, z, voxel(Block::Wheat, stage));
                    }
                }
            }
        }
        PieceKind::LampPost => {
            foundation(g, w, p, floor);
            w.fill(p.x, p.y, p.z, p.x, p.y + 2, p.z, corner);
            w.set(p.x, p.y + 3, p.z, voxel(Block::Torch, 0));
        }
        PieceKind::Ruin => build_ruin(g, w, p, &mut rng),
    }
}

fn build_ruin(g: &Generator, w: &mut Writer, p: &Piece, rng: &mut Rng) {
    let cx = p.x + 8;
    let cz = p.z + 8;
    let bricks = voxel(Block::StoneBricks, 0);
    let cracked = voxel(Block::CrackedStoneBricks, 0);
    let mossy = voxel(Block::MossyCobblestone, 0);
    let cobble = voxel(Block::Cobblestone, 0);
    let pick = |rng: &mut Rng| match rng.below(10) {
        0..=5 => bricks,
        6..=7 => cracked,
        _ => mossy,
    };
    // pillars in a ring (heights decided up-front so every chunk agrees)
    let mut pillars = Vec::new();
    for i in 0..8 {
        let ang = i as f32 / 8.0 * std::f32::consts::TAU;
        let px = cx + (ang.cos() * 6.0).round() as i32;
        let pz = cz + (ang.sin() * 6.0).round() as i32;
        let height = if rng.chance(0.2) { 0 } else { rng.range(2, 7) };
        pillars.push((px, pz, height));
    }
    let base_y = p.y;
    for (px, pz, height) in &pillars {
        if !w.inside(*px, *pz) {
            continue;
        }
        let ground = g.column(*px, *pz).height;
        // pillar foundation down to the ground
        let mut y = base_y - 1;
        while y >= ground.min(base_y - 1) && y >= 0 {
            w.set(*px, y, *pz, cobble);
            y -= 1;
        }
        for dy in 0..*height {
            let v = pick(rng);
            w.set(*px, base_y + dy, *pz, v);
        }
        if *height >= 4 && rng.chance(0.5) {
            w.set(*px, base_y + height, *pz, voxel(Block::Torch, 0));
        }
    }
    // lintels between tall adjacent pillars
    for i in 0..8 {
        let a = pillars[i];
        let b = pillars[(i + 1) % 8];
        if a.2 >= 5 && b.2 >= 5 {
            let steps = (a.0 - b.0).abs().max((a.1 - b.1).abs()).max(1);
            for s in 0..=steps {
                let x = a.0 + (b.0 - a.0) * s / steps;
                let z = a.1 + (b.1 - a.1) * s / steps;
                w.set(x, base_y + 4, z, bricks);
            }
        }
    }
    // inner floor and altar
    for dz in -3..=3 {
        for dx in -3..=3 {
            if dx * dx + dz * dz <= 10 {
                let x = cx + dx;
                let z = cz + dz;
                if w.inside(x, z) {
                    let ground = g.column(x, z).height;
                    for y in base_y..base_y + 3 {
                        w.set(x, y, z, 0);
                    }
                    let mut y = base_y - 1;
                    while y >= ground.min(base_y - 1) && y >= 0 {
                        w.set(x, y, z, if y == base_y - 1 { if rng.chance(0.5) { mossy } else { cobble } } else { cobble });
                        y -= 1;
                    }
                }
            }
        }
    }
    w.fill(cx - 1, base_y, cz - 1, cx, base_y, cz, bricks);
    w.set(cx, base_y + 1, cz, voxel(Block::Chest, 0));
    let mut loot = dungeon_loot(rng);
    if rng.chance(0.5) {
        loot[13] = ItemStack::item(Item::GoldIngot, 3);
    }
    w.block_entity(cx, base_y + 1, cz, BlockEntity::Chest { items: loot });
    // rubble scattered around
    for _ in 0..10 {
        let x = cx + rng.range(-8, 9);
        let z = cz + rng.range(-8, 9);
        if w.inside(x, z) {
            let ground = g.column(x, z).height;
            if ground > SEA_LEVEL {
                w.set_if_air(x, ground + 1, z, if rng.chance(0.5) { cracked } else { mossy });
            }
        }
    }
}

fn dungeon(g: &Generator, w: &mut Writer) {
    let cx = w.chunk.cx;
    let cz = w.chunk.cz;
    let mut rng = Rng::at(g.seed, cx as i64, cz as i64, DUNGEON_SALT);
    if !rng.chance(1.0 / 9.0) {
        return;
    }
    let hw = if rng.chance(0.5) { 2 } else { 3 }; // half width
    let hd = if rng.chance(0.5) { 2 } else { 3 };
    let ox = cx * 16 + 8;
    let oz = cz * 16 + 8;
    let y0 = rng.range(8, 42);
    let surface = g.column(ox, oz).height;
    if surface < y0 + 10 {
        return;
    }
    let cobble = voxel(Block::Cobblestone, 0);
    let mossy = voxel(Block::MossyCobblestone, 0);
    let h = 4;
    for z in oz - hd - 1..=oz + hd + 1 {
        for x in ox - hw - 1..=ox + hw + 1 {
            for y in y0 - 1..=y0 + h {
                let wall = x == ox - hw - 1 || x == ox + hw + 1 || z == oz - hd - 1 || z == oz + hd + 1 || y == y0 - 1 || y == y0 + h;
                let v = if wall {
                    if rng.chance(0.3) { mossy } else { cobble }
                } else {
                    0
                };
                w.set(x, y, z, v);
            }
        }
    }
    let mob = if rng.chance(0.6) { 0u8 } else { 1u8 }; // zombie / skeleton
    w.set(ox, y0, oz, voxel(Block::Spawner, 0));
    w.block_entity(ox, y0, oz, BlockEntity::Spawner { mob, cooldown: 100 });
    let chests = 1 + rng.below(2);
    for _ in 0..chests {
        let (x, z) = match rng.below(4) {
            0 => (ox - hw, oz - hd),
            1 => (ox + hw, oz - hd),
            2 => (ox - hw, oz + hd),
            _ => (ox + hw, oz + hd),
        };
        w.set(x, y0, z, voxel(Block::Chest, 0));
        w.block_entity(x, y0, z, BlockEntity::Chest { items: dungeon_loot(&mut rng) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn villages_exist_somewhere() {
        let g = Generator::new(31337);
        let mut found = 0;
        for cz in -8..8 {
            for cx in -8..8 {
                if !village(&g, cx, cz).is_empty() {
                    found += 1;
                }
            }
        }
        assert!(found > 0, "no village in 256 cells");
    }

    #[test]
    fn structure_pieces_are_deterministic() {
        let g = Generator::new(5);
        let a: Vec<String> = village(&g, 1, 1).iter().map(|p| format!("{:?}", p)).collect();
        let b: Vec<String> = village(&g, 1, 1).iter().map(|p| format!("{:?}", p)).collect();
        assert_eq!(a, b);
    }
}
