//! Procedural block tiles, part 2 (utility, redstone, decoration blocks).

use crate::render::atlas::Tile;
use crate::render::texgen::{bricks, cells, hash01, metal_block, planks, rgb, shade, shade3, Canvas};
use crate::render::texgen::grass_side;

pub fn paint(c: &mut Canvas, t: Tile) {
    use Tile::*;
    let g = |c: &mut Canvas, base: [u8; 3], v: i32, s: u32| c.noise_fill(base, v, s);
    match t {
        CraftingTableTop => {
            planks(c, [162, 130, 78], 61);
            c.rect(2, 2, 12, 12, rgb(150, 118, 70));
            c.rect(3, 3, 4, 4, rgb(120, 90, 55));
            c.rect(9, 3, 4, 4, rgb(120, 90, 55));
            c.rect(3, 9, 4, 4, rgb(120, 90, 55));
            c.rect(9, 9, 4, 4, rgb(120, 90, 55));
            c.rect(7, 2, 2, 12, rgb(100, 75, 45));
            c.rect(2, 7, 12, 2, rgb(100, 75, 45));
        }
        CraftingTableSide => {
            planks(c, [162, 130, 78], 61);
            c.rect(0, 0, 16, 2, rgb(120, 90, 55));
            c.stamp(
                &["..........", "..........", "..sss.....", "...s..hh..", "...s..hh..", "...s...h..", "...s...h..", "...s...h.."],
                &[('s', rgb(140, 140, 140)), ('h', rgb(90, 90, 90))],
                3,
                3,
            );
        }
        CraftingTableFront => {
            planks(c, [162, 130, 78], 61);
            c.rect(0, 0, 16, 2, rgb(120, 90, 55));
            c.stamp(
                &["hh.......", "hhh.s.....", ".hhss.....", "..hs......", "..s.......", ".s........", "s........."],
                &[('s', rgb(140, 140, 140)), ('h', rgb(160, 160, 160))],
                4,
                4,
            );
        }
        FurnaceSide => cells(c, 9, [110, 110, 110], 12, -35, 201),
        FurnaceTop => cells(c, 9, [100, 100, 100], 12, -35, 202),
        FurnaceFront | FurnaceFrontLit => {
            cells(c, 9, [110, 110, 110], 12, -35, 201);
            c.rect(4, 6, 8, 8, rgb(30, 30, 30));
            c.rect(4, 5, 8, 1, rgb(60, 60, 60));
            if t == FurnaceFrontLit {
                c.stamp(
                    &["........", ".o.oo.o.", ".oYoYYo.", "oYYYYYYo", "oYWYWYYo", "oYYWWYYo"],
                    &[('o', rgb(230, 100, 20)), ('Y', rgb(250, 190, 40)), ('W', rgb(255, 240, 150))],
                    4,
                    8,
                );
            }
        }
        ChestSide | ChestFront | ChestTop => {
            planks(c, [140, 105, 55], 211);
            for i in 0..16 {
                c.set(i, 0, rgb(70, 50, 25));
                c.set(i, 15, rgb(70, 50, 25));
                c.set(0, i, rgb(70, 50, 25));
                c.set(15, i, rgb(70, 50, 25));
                c.set(i, 6, rgb(80, 60, 30));
            }
            if t == ChestFront {
                c.rect(6, 5, 4, 4, rgb(60, 60, 60));
                c.rect(7, 6, 2, 2, rgb(120, 120, 120));
            }
        }
        Torch => {
            c.stamp(
                &["........", "..YY....", ".oYYo...", ".ooOo...", "..bb....", "..bb....", "..bb....", "..bb....", "..bb....", "..bb....", "..bb....", "..bb....", "..bb....", "..bb....", "..bb....", "..bb...."],
                &[('Y', rgb(255, 240, 120)), ('O', rgb(255, 200, 40)), ('o', rgb(230, 120, 20)), ('b', rgb(120, 90, 50))],
                5,
                0,
            );
        }
        RedstoneTorchOn | RedstoneTorchOff => {
            let head = if t == RedstoneTorchOn { rgb(255, 60, 50) } else { rgb(90, 20, 15) };
            let head2 = if t == RedstoneTorchOn { rgb(255, 150, 130) } else { rgb(120, 30, 20) };
            c.stamp(
                &["........", "..RR....", ".RrrR...", ".RRRR...", "..bb....", "..bb....", "..bb....", "..bb....", "..bb....", "..bb....", "..bb....", "..bb....", "..bb....", "..bb....", "..bb....", "..bb...."],
                &[('R', head), ('r', head2), ('b', rgb(120, 90, 50))],
                5,
                0,
            );
        }
        Ladder => {
            for y in 0..16 {
                c.set(2, y, rgb(140, 105, 55));
                c.set(3, y, rgb(160, 125, 70));
                c.set(12, y, rgb(140, 105, 55));
                c.set(13, y, rgb(160, 125, 70));
                if y % 4 == 1 || y % 4 == 2 {
                    for x in 4..12 {
                        c.set(x, y, if y % 4 == 1 { rgb(170, 135, 80) } else { rgb(130, 100, 55) });
                    }
                }
            }
        }
        DoorLower | DoorUpper => {
            planks(c, [150, 118, 70], 221);
            for i in 0..16 {
                c.set(0, i, rgb(90, 65, 35));
                c.set(15, i, rgb(90, 65, 35));
            }
            if t == DoorUpper {
                c.rect(0, 0, 16, 1, rgb(90, 65, 35));
                c.rect(3, 3, 10, 6, rgb(90, 65, 35));
                c.rect(4, 4, 8, 4, [200, 230, 255, 120]);
                c.rect(7, 4, 2, 4, rgb(90, 65, 35));
                c.rect(3, 11, 10, 4, rgb(120, 92, 52));
            } else {
                c.rect(0, 15, 16, 1, rgb(90, 65, 35));
                c.rect(3, 1, 10, 5, rgb(120, 92, 52));
                c.rect(3, 9, 10, 5, rgb(120, 92, 52));
                c.rect(12, 6, 2, 2, rgb(200, 200, 200));
            }
        }
        BedTopHead => {
            c.noise_fill([180, 30, 30], 8, 231);
            c.rect(2, 1, 12, 5, rgb(240, 240, 240));
            c.rect(3, 2, 10, 3, rgb(255, 255, 255));
            c.rect(0, 0, 16, 1, rgb(120, 85, 50));
        }
        BedTopFoot => {
            c.noise_fill([180, 30, 30], 8, 232);
            c.rect(0, 15, 16, 1, rgb(120, 85, 50));
            c.rect(0, 12, 16, 1, rgb(150, 25, 25));
        }
        BedSide => {
            planks(c, [140, 105, 55], 233);
            c.rect(0, 0, 16, 7, rgb(180, 30, 30));
            c.rect(0, 6, 16, 1, rgb(150, 25, 25));
            c.rect(0, 7, 16, 1, rgb(230, 230, 230));
        }
        BedEnd => {
            planks(c, [140, 105, 55], 234);
            c.rect(0, 0, 16, 7, rgb(180, 30, 30));
            c.rect(0, 7, 16, 1, rgb(230, 230, 230));
        }
        RedstoneDust | RedstoneDustCross => {
            let col = if t == RedstoneDust { rgb(90, 15, 10) } else { rgb(240, 40, 30) };
            let col2 = shade(col, 20);
            for i in 0..16 {
                c.set(i, 7, col);
                c.set(i, 8, col2);
                c.set(7, i, col);
                c.set(8, i, col2);
            }
            c.rect(6, 6, 4, 4, col);
        }
        Lever => {
            cells(c, 9, [110, 110, 110], 12, -35, 201);
            c.rect(6, 8, 4, 6, rgb(60, 60, 60));
            c.rect(7, 1, 2, 8, rgb(130, 100, 55));
            c.rect(7, 0, 2, 2, rgb(200, 30, 30));
        }
        RedstoneLampOff => {
            c.blotch_fill([90, 60, 35], [120, 80, 45], 4, 241);
            for i in 0..16 {
                c.set(i, 0, rgb(60, 40, 25));
                c.set(0, i, rgb(60, 40, 25));
                c.set(i, 15, rgb(60, 40, 25));
                c.set(15, i, rgb(60, 40, 25));
            }
        }
        RedstoneLampOn => {
            c.blotch_fill([230, 190, 90], [255, 230, 140], 4, 241);
            for i in 0..16 {
                c.set(i, 0, rgb(150, 110, 50));
                c.set(0, i, rgb(150, 110, 50));
                c.set(i, 15, rgb(150, 110, 50));
                c.set(15, i, rgb(150, 110, 50));
            }
        }
        PistonSide => {
            cells(c, 9, [110, 110, 110], 12, -35, 251);
            for y in 0..4 {
                for x in 0..16 {
                    c.set(x, y, shade3([162, 130, 78], ((hash01(x, y, 252) - 0.5) * 30.0) as i32, 255));
                }
            }
            c.rect(0, 4, 16, 1, rgb(80, 60, 35));
        }
        PistonTop => planks(c, [162, 130, 78], 253),
        PistonTopSticky => {
            planks(c, [162, 130, 78], 253);
            c.blotch_fill([110, 170, 90], [140, 200, 110], 4, 254);
            for i in 0..16 {
                c.set(i, 0, rgb(120, 90, 55));
                c.set(0, i, rgb(120, 90, 55));
                c.set(i, 15, rgb(120, 90, 55));
                c.set(15, i, rgb(120, 90, 55));
            }
        }
        PistonBottom => cells(c, 9, [100, 100, 100], 12, -35, 255),
        PistonInner => {
            cells(c, 9, [100, 100, 100], 12, -35, 255);
            c.rect(6, 6, 4, 4, rgb(80, 60, 35));
        }
        TntSide => {
            c.noise_fill([200, 50, 40], 10, 261);
            c.rect(0, 5, 16, 6, rgb(230, 230, 230));
            c.stamp(
                &["###.#.#.###", ".#..##.#.#.", ".#..#.##.#.", ".#..#.#..#."],
                &[('#', rgb(30, 30, 30))],
                3,
                6,
            );
            c.rect(0, 0, 16, 1, rgb(150, 30, 25));
            c.rect(0, 15, 16, 1, rgb(150, 30, 25));
        }
        TntTop => {
            c.noise_fill([200, 50, 40], 10, 262);
            c.rect(3, 3, 10, 10, rgb(120, 120, 120));
            c.rect(6, 6, 4, 4, rgb(60, 60, 60));
        }
        TntBottom => {
            c.noise_fill([200, 50, 40], 10, 263);
            c.rect(3, 3, 10, 10, rgb(120, 120, 120));
        }
        Glowstone => {
            c.blotch_fill([190, 140, 60], [255, 230, 140], 4, 271);
            for y in 0..16 {
                for x in 0..16 {
                    if hash01(x, y, 272) < 0.15 {
                        c.set(x, y, rgb(255, 250, 200));
                    }
                }
            }
        }
        Spawner => {
            c.fill([20, 20, 25, 200]);
            for i in 0..16 {
                if i % 3 == 0 {
                    for j in 0..16 {
                        c.set(i, j, rgb(45, 45, 55));
                        c.set(j, i, rgb(45, 45, 55));
                    }
                }
            }
        }
        Obsidian => c.blotch_fill([20, 15, 35], [60, 45, 85], 4, 281),
        Wool => c.blotch_fill([225, 225, 225], [245, 245, 245], 8, 291),
        StoneBricks => bricks(c, [125, 125, 125], [95, 95, 95], 8, 8, 301),
        CrackedStoneBricks => {
            bricks(c, [125, 125, 125], [95, 95, 95], 8, 8, 301);
            for i in 0..14 {
                let x = 2 + i;
                let y = (i * 7 / 8 + ((i % 3) as i32)).min(15);
                c.set(x, y, rgb(70, 70, 70));
            }
        }
        IronBlock => metal_block(c, [220, 220, 220]),
        GoldBlock => metal_block(c, [250, 215, 60]),
        DiamondBlock => metal_block(c, [100, 230, 225]),
        MushroomStem => {
            g(c, [205, 195, 175], 8, 311);
            for x in [1, 5, 9, 13] {
                for y in 0..16 {
                    c.set(x, y, shade(c.get(x, y), -25));
                }
            }
        }
        MushroomRed => {
            g(c, [200, 40, 40], 10, 321);
            for i in 0..4 {
                let x = 2 + (i % 2) * 8 + (i / 2) * 2;
                let y = 2 + (i / 2) * 7;
                c.rect(x, y, 3, 3, rgb(240, 240, 240));
            }
        }
        MushroomBrown => g(c, [140, 100, 70], 12, 331),
        Farmland => {
            g(c, [95, 65, 40], 12, 341);
            for y in [2, 6, 10, 14] {
                for x in 0..16 {
                    c.set(x, y, shade(c.get(x, y), -30));
                    c.set(x, y - 1, shade(c.get(x, y - 1), 15));
                }
            }
        }
        Wheat0 | Wheat1 | Wheat2 | Wheat3 | Wheat4 => {
            let stage = t.index() - Wheat0.index();
            let height = 4 + stage as i32 * 3;
            let col = match stage {
                0 | 1 => rgb(80, 160, 50),
                2 => rgb(120, 170, 50),
                3 => rgb(170, 170, 60),
                _ => rgb(210, 180, 60),
            };
            for x in [1, 4, 7, 10, 13] {
                for y in (16 - height)..16 {
                    c.set(x, y, col);
                    if stage >= 3 && y < 16 - height + 4 {
                        c.set(x + 1, y, shade(col, 20));
                        c.set(x - 1, y, shade(col, 20));
                    }
                }
            }
        }
        PodzolTop => {
            g(c, [120, 85, 45], 14, 351);
            for y in 0..16 {
                for x in 0..16 {
                    if hash01(x, y, 352) < 0.2 {
                        c.set(x, y, rgb(70, 50, 30));
                    }
                }
            }
        }
        PodzolSide => grass_side(c, [120, 85, 45], 353),
        Bricks => bricks(c, [150, 80, 65], [175, 170, 160], 8, 4, 361),
        Bookshelf => {
            planks(c, [162, 130, 78], 61);
            let cols = [rgb(180, 40, 40), rgb(40, 80, 180), rgb(40, 140, 60), rgb(200, 160, 40), rgb(120, 60, 140)];
            for shelf in 0..2 {
                let y0 = 1 + shelf * 8;
                let mut x = 1;
                let mut i = 0;
                while x < 15 {
                    let w = 1 + ((hash01(x, shelf, 371) * 2.0) as i32);
                    c.rect(x, y0, w, 6, cols[i % cols.len()]);
                    c.rect(x, y0 + 1, w, 1, shade(cols[i % cols.len()], 30));
                    x += w + (if hash01(x, shelf, 372) < 0.2 { 1 } else { 0 });
                    i += 1;
                }
                c.rect(0, y0 + 6, 16, 1, rgb(100, 75, 45));
            }
        }
        HaySide => {
            g(c, [200, 160, 50], 14, 381);
            for y in [3, 7, 11, 15] {
                for x in 0..16 {
                    c.set(x, y, shade(c.get(x, y), -35));
                }
            }
        }
        HayTop => {
            g(c, [200, 160, 50], 14, 382);
            for i in 0..4 {
                c.rect(1 + i * 4, 1, 3, 3, rgb(160, 120, 30));
                c.rect(1 + i * 4, 6, 3, 3, rgb(160, 120, 30));
                c.rect(1 + i * 4, 11, 3, 3, rgb(160, 120, 30));
            }
        }
        _ => crate::render::texgen_extra::paint(c, t),
    }
}
