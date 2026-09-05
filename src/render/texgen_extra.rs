//! Procedural tiles for items, mobs, effects and UI.

use crate::render::atlas::Tile;
use crate::render::texgen::{hash01, rgb, shade, shade3, value_noise, Canvas, Rgba};

const PICKAXE: [&str; 16] = [
    "................",
    ".......hhhhhh...",
    ".....hhhhhhhhh..",
    "....hhh....shhh.",
    "...hh.....s..hh.",
    "...h.....s....h.",
    "........s.......",
    ".......s........",
    "......s.........",
    ".....s..........",
    "....s...........",
    "...s............",
    "..s.............",
    ".s..............",
    "................",
    "................",
];
const AXE: [&str; 16] = [
    "................",
    "......hhh.......",
    ".....hhhhhh.....",
    "....hhhhhhhh....",
    "....hhhhhhhh....",
    ".....hhhs.hh....",
    "......hs........",
    ".......s........",
    "......s.........",
    ".....s..........",
    "....s...........",
    "...s............",
    "..s.............",
    ".s..............",
    "................",
    "................",
];
const SHOVEL: [&str; 16] = [
    "................",
    "..........hhh...",
    ".........hhhhh..",
    ".........hhhhh..",
    "..........hhh...",
    ".........s......",
    "........s.......",
    ".......s........",
    "......s.........",
    ".....s..........",
    "....s...........",
    "...s............",
    "..s.............",
    ".s..............",
    "................",
    "................",
];
const SWORD: [&str; 16] = [
    "................",
    "............hh..",
    "...........hhh..",
    "..........hhh...",
    ".........hhh....",
    "........hhh.....",
    ".......hhh......",
    "..d...hhh.......",
    "...d.hhh........",
    "....dd..........",
    "...dsd..........",
    "..d..s..........",
    ".....s..........",
    "....s...........",
    "................",
    "................",
];
const HOE: [&str; 16] = [
    "................",
    "......hhhhhh....",
    ".....hhhhhhhh...",
    "....hh....shh...",
    ".........s......",
    "........s.......",
    ".......s........",
    "......s.........",
    ".....s..........",
    "....s...........",
    "...s............",
    "..s.............",
    ".s..............",
    "................",
    "................",
    "................",
];

fn tool(c: &mut Canvas, rows: &[&str; 16], head: Rgba) {
    let pal = [('h', head), ('s', rgb(120, 90, 50)), ('d', rgb(80, 60, 35))];
    c.stamp(rows, &pal, 0, 0);
    // highlight / shadow the head for depth
    for y in 0..16 {
        for x in 0..16 {
            if c.get(x, y) == head {
                if x > 0 && c.get(x - 1, y)[3] == 0 || y > 0 && c.get(x, y - 1)[3] == 0 {
                    c.set(x, y, shade(head, 30));
                } else if x < 15 && c.get(x + 1, y)[3] == 0 || y < 15 && c.get(x, y + 1)[3] == 0 {
                    c.set(x, y, shade(head, -35));
                }
            }
        }
    }
}

fn ingot(c: &mut Canvas, col: Rgba) {
    c.stamp(
        &[
            "................",
            "................",
            "................",
            "................",
            "......hhhhhh....",
            ".....hhhhhhhh...",
            "....hhhhhhhhdd..",
            "...hhhhhhhhhdd..",
            "...hhhhhhhhddd..",
            "...dddddddddd...",
            "...ddddddddd....",
            "................",
            "................",
            "................",
            "................",
            "................",
        ],
        &[('h', col), ('d', shade(col, -50))],
        0,
        0,
    );
}

fn blob(c: &mut Canvas, col: Rgba, seed: u32) {
    for y in 0..16 {
        for x in 0..16 {
            let dx = x as f32 - 7.5;
            let dy = y as f32 - 8.0;
            let r = (dx * dx + dy * dy * 1.3).sqrt();
            if r < 4.5 + hash01(x, y, seed) * 1.5 {
                let d = ((hash01(x, y, seed ^ 1) - 0.5) * 40.0) as i32 - if r > 4.0 { 30 } else { 0 };
                c.set(x, y, shade(col, d));
            }
        }
    }
}

fn meat(c: &mut Canvas, outer: Rgba, inner: Rgba) {
    c.stamp(
        &[
            "................",
            "................",
            "................",
            "....oooooo......",
            "...oooiiooo.....",
            "..ooiiiiiioo....",
            "..ooiiiiiiioo...",
            "..ooiiiiiiioo...",
            "..ooiiiiiiioo...",
            "...ooiiiiiioo...",
            "....ooiiiooo....",
            ".....ooooo......",
            "................",
            "................",
            "................",
            "................",
        ],
        &[('o', outer), ('i', inner)],
        0,
        0,
    );
}

fn armor(c: &mut Canvas, t: Tile, col: Rgba) {
    let dark = shade(col, -50);
    let pal = [('h', col), ('d', dark)];
    let rows: [&str; 16] = match t {
        Tile::LeatherHelmet | Tile::IronHelmet => [
            "................", "................", "................", "................", ".....hhhhhh.....",
            "....hhhhhhhh....", "...hhhhhhhhhh...", "...hhhhhhhhhh...", "...hhhhhhhhhh...", "...hhd....dhh...",
            "...hhd....dhh...", "...ddd....ddd...", "................", "................", "................",
            "................",
        ],
        Tile::LeatherChest | Tile::IronChest => [
            "................", "................", "...hhh....hhh...", "...hhhh..hhhh...", "...hhhhhhhhhh...",
            "...hhhhhhhhhh...", "...hhhhhhhhhh...", "...dhhhhhhhhd...", "....hhhhhhhh....", "....hhhhhhhh....",
            "....hhhhhhhh....", "....hhhhhhhh....", "....dddddddd....", "................", "................",
            "................",
        ],
        Tile::LeatherLegs | Tile::IronLegs => [
            "................", "................", "....hhhhhhhh....", "....hhhhhhhh....", "....hhhhhhhh....",
            "....hhhd.dhhh...", "....hhh...hhh...", "....hhh...hhh...", "....hhh...hhh...", "....hhh...hhh...",
            "....hhh...hhh...", "....hhh...hhh...", "....ddd...ddd...", "................", "................",
            "................",
        ],
        _ => [
            "................", "................", "................", "................", "................",
            "....hhh...hhh...", "....hhh...hhh...", "....hhh...hhh...", "....hhh...hhh...", "...hhhh..hhhh...",
            "..hhhhh.hhhhh...", "..hhhhh.hhhhh...", "..ddddd.ddddd...", "................", "................",
            "................",
        ],
    };
    c.stamp(&rows, &pal, 0, 0);
}

fn face_base(c: &mut Canvas, skin: [u8; 3], seed: u32) {
    c.noise_fill(skin, 8, seed);
}

fn eyes(c: &mut Canvas, y: i32, white: bool, col: Rgba) {
    if white {
        c.set(3, y, rgb(255, 255, 255));
        c.set(12, y, rgb(255, 255, 255));
        c.set(4, y, col);
        c.set(11, y, col);
    } else {
        c.rect(3, y, 2, 2, col);
        c.rect(11, y, 2, 2, col);
    }
}

fn heart(c: &mut Canvas, fill: Rgba, half: bool, empty: bool) {
    let rows = [
        "................",
        "................",
        "................",
        "....##....##....",
        "...####..####...",
        "..############..",
        "..############..",
        "..############..",
        "...##########...",
        "....########....",
        ".....######.....",
        "......####......",
        ".......##.......",
        "................",
        "................",
        "................",
    ];
    // outline
    let dark = rgb(40, 10, 10);
    for (y, row) in rows.iter().enumerate() {
        for (x, ch) in row.chars().enumerate() {
            if ch == '#' {
                let fill_here = if empty { rgb(60, 20, 20) } else if half && x >= 8 { rgb(60, 20, 20) } else { fill };
                let x = x as i32;
                let y = y as i32;
                c.set(x, y, fill_here);
                for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                    let nx = x + dx;
                    let ny = y + dy;
                    if (0..16).contains(&nx) && (0..16).contains(&ny) {
                        let r = rows[ny as usize].as_bytes()[nx as usize];
                        if r != b'#' {
                            c.set(nx, ny, dark);
                        }
                    }
                }
            }
        }
    }
    if !empty && !half {
        c.set(5, 5, shade(fill, 60));
        c.set(6, 5, shade(fill, 60));
    }
}

fn drumstick(c: &mut Canvas, half: bool, empty: bool) {
    let meat = if empty { rgb(70, 45, 30) } else { rgb(180, 100, 50) };
    let bone = if empty { rgb(90, 90, 90) } else { rgb(240, 240, 220) };
    let rows = [
        "................",
        "................",
        "......mmm.......",
        ".....mmmmm......",
        "....mmmmmmm.....",
        "....mmmmmmm.....",
        "....mmmmmmm.....",
        ".....mmmmm......",
        "......mmb.......",
        ".......bb.......",
        "........bb......",
        ".........bb.....",
        "........bbbb....",
        ".........bb.....",
        "................",
        "................",
    ];
    for (y, row) in rows.iter().enumerate() {
        for (x, ch) in row.chars().enumerate() {
            let col = match ch {
                'm' => meat,
                'b' => bone,
                _ => continue,
            };
            let col = if half && x >= 8 { rgb(70, 45, 30) } else { col };
            c.set(x as i32, y as i32, col);
        }
    }
}

pub fn paint(c: &mut Canvas, t: Tile) {
    use Tile::*;
    let wood = rgb(160, 125, 70);
    let stone = rgb(130, 130, 130);
    let iron = rgb(215, 215, 215);
    let gold = rgb(250, 215, 60);
    let diamond = rgb(100, 230, 225);
    match t {
        Stick => c.stamp(
            &["................", "................", "...........ss...", "..........ss....", ".........ss.....", "........ss......", ".......ss.......", "......ss........", ".....ss.........", "....ss..........", "...ss...........", "..ss............", "................", "................", "................", "................"],
            &[('s', rgb(130, 95, 50))],
            0,
            0,
        ),
        Coal => blob(c, rgb(40, 40, 40), 1),
        IronIngot => ingot(c, iron),
        GoldIngot => ingot(c, gold),
        Diamond => c.stamp(
            &["................", "................", "................", ".....dddddd.....", "....dDDDDDDd....", "...dDDDDDDDDd...", "...dDDDDDDDDd...", "....dDDDDDDd....", ".....dDDDDd.....", "......dDDd......", ".......dd.......", "................", "................", "................", "................", "................"],
            &[('d', rgb(60, 170, 170)), ('D', diamond)],
            0,
            0,
        ),
        Redstone => {
            for y in 0..16 {
                for x in 0..16 {
                    let dx = x as f32 - 7.5;
                    let dy = y as f32 - 9.0;
                    if dx * dx * 0.4 + dy * dy < 12.0 && hash01(x, y, 3) < 0.8 {
                        c.set(x, y, shade(rgb(200, 30, 30), ((hash01(x, y, 4) - 0.5) * 60.0) as i32));
                    }
                }
            }
        }
        Wheat => {
            for x in [4, 7, 10] {
                for y in 2..15 {
                    c.set(x, y, if y < 8 { rgb(210, 180, 60) } else { rgb(150, 160, 60) });
                    if y < 8 {
                        c.set(x + 1, y, rgb(230, 200, 80));
                    }
                }
            }
        }
        Bread => c.stamp(
            &["................", "................", "................", "................", "................", "....bbbbbbbb....", "...bBBBBBBBBb...", "..bBBBBBBBBBBb..", "..bBBBBBBBBBBb..", "..bbbbbbbbbbbb..", "...dddddddddd...", "................", "................", "................", "................", "................"],
            &[('b', rgb(190, 130, 60)), ('B', rgb(220, 170, 90)), ('d', rgb(140, 95, 45))],
            0,
            0,
        ),
        PorkchopRaw => meat(c, rgb(240, 170, 170), rgb(250, 200, 200)),
        PorkchopCooked => meat(c, rgb(150, 90, 60), rgb(200, 140, 100)),
        BeefRaw => meat(c, rgb(190, 60, 60), rgb(230, 100, 100)),
        BeefCooked => meat(c, rgb(110, 60, 40), rgb(160, 100, 70)),
        ChickenRaw => meat(c, rgb(240, 200, 190), rgb(250, 225, 215)),
        ChickenCooked => meat(c, rgb(190, 120, 60), rgb(220, 160, 90)),
        Leather => c.stamp(
            &["................", "................", "................", "...LLL....LLL...", "..LLLLLLLLLLLL..", "..LLLLLLLLLLLL..", "..LLLLLLLLLLLL..", "...LLLLLLLLLL...", "...LLLLLLLLLL...", "..LLLLLLLLLLLL..", "..LLLLLLLLLLLL..", "...LLL....LLL...", "................", "................", "................", "................"],
            &[('L', rgb(160, 100, 50))],
            0,
            0,
        ),
        Feather => c.stamp(
            &["................", "..........ww....", ".........wwww...", "........wwwww...", ".......wwwwww...", "......wwwwww....", ".....wwwwww.....", "....wwwwww......", "...wwwwww.......", "...wwwww........", "..wwsw..........", "..sss...........", ".ss.............", "................", "................", "................"],
            &[('w', rgb(240, 240, 240)), ('s', rgb(200, 200, 200))],
            0,
            0,
        ),
        Arrow => c.stamp(
            &["................", "............hh..", "...........hhh..", "..........hhh...", ".........hss....", "........ss......", ".......ss.......", "......ss........", ".....ss.........", "....ss..........", "...fs...........", "..ff............", ".fff............", "................", "................", "................"],
            &[('h', rgb(200, 200, 200)), ('s', rgb(130, 95, 50)), ('f', rgb(230, 230, 230))],
            0,
            0,
        ),
        Bone => c.stamp(
            &["................", "................", "..........bb....", ".........bbbb...", "........bbbbb...", ".......bbbbb....", "......bbb.......", ".....bbb........", "....bbb.........", "...bbbbb........", "..bbbbb.........", "..bbbb..........", "...bb...........", "................", "................", "................"],
            &[('b', rgb(230, 230, 215))],
            0,
            0,
        ),
        RottenFlesh => meat(c, rgb(120, 90, 60), rgb(110, 140, 70)),
        Gunpowder => {
            for y in 0..16 {
                for x in 0..16 {
                    let dx = x as f32 - 7.5;
                    let dy = y as f32 - 9.0;
                    if dx * dx * 0.4 + dy * dy < 12.0 && hash01(x, y, 5) < 0.85 {
                        c.set(x, y, shade(rgb(70, 70, 70), ((hash01(x, y, 6) - 0.5) * 60.0) as i32));
                    }
                }
            }
        }
        String => {
            for i in 0..12 {
                c.set(2 + i, 4 + ((i as f32 * 0.9).sin() * 3.0) as i32 + 4, rgb(240, 240, 240));
            }
            for i in 0..12 {
                c.set(2 + i, 5 + ((i as f32 * 0.9 + 1.0).sin() * 3.0) as i32 + 4, rgb(220, 220, 220));
            }
        }
        Apple => {
            blob(c, rgb(200, 30, 30), 7);
            c.rect(7, 2, 2, 2, rgb(100, 70, 40));
            c.set(9, 2, rgb(80, 150, 50));
        }
        Egg => {
            for y in 0..16 {
                for x in 0..16 {
                    let dx = (x as f32 - 7.5) / 3.5;
                    let dy = (y as f32 - 8.0) / 4.5;
                    if dx * dx + dy * dy < 1.0 {
                        c.set(x, y, if dx < -0.3 && dy < -0.3 { rgb(255, 250, 240) } else { rgb(235, 225, 200) });
                    }
                }
            }
        }
        Bow => c.stamp(
            &["................", "......sss.......", ".....s...t......", "....s.....t.....", "...s.......t....", "...s.......t....", "..s.........t...", "..s.........t...", "..s.........t...", "...s.......t....", "...s.......t....", "....s.....t.....", ".....s...t......", "......sss.......", "................", "................"],
            &[('s', rgb(130, 95, 50)), ('t', rgb(230, 230, 230))],
            0,
            0,
        ),
        ClayBall => blob(c, rgb(160, 165, 178), 8),
        Brick => ingot(c, rgb(150, 80, 65)),
        Flint => blob(c, rgb(60, 60, 65), 9),
        WoodPickaxe => tool(c, &PICKAXE, wood),
        WoodAxe => tool(c, &AXE, wood),
        WoodShovel => tool(c, &SHOVEL, wood),
        WoodSword => tool(c, &SWORD, wood),
        WoodHoe => tool(c, &HOE, wood),
        StonePickaxe => tool(c, &PICKAXE, stone),
        StoneAxe => tool(c, &AXE, stone),
        StoneShovel => tool(c, &SHOVEL, stone),
        StoneSword => tool(c, &SWORD, stone),
        StoneHoe => tool(c, &HOE, stone),
        IronPickaxe => tool(c, &PICKAXE, iron),
        IronAxe => tool(c, &AXE, iron),
        IronShovel => tool(c, &SHOVEL, iron),
        IronSword => tool(c, &SWORD, iron),
        IronHoe => tool(c, &HOE, iron),
        GoldPickaxe => tool(c, &PICKAXE, gold),
        GoldAxe => tool(c, &AXE, gold),
        GoldShovel => tool(c, &SHOVEL, gold),
        GoldSword => tool(c, &SWORD, gold),
        GoldHoe => tool(c, &HOE, gold),
        DiamondPickaxe => tool(c, &PICKAXE, diamond),
        DiamondAxe => tool(c, &AXE, diamond),
        DiamondShovel => tool(c, &SHOVEL, diamond),
        DiamondSword => tool(c, &SWORD, diamond),
        DiamondHoe => tool(c, &HOE, diamond),
        LeatherHelmet | LeatherChest | LeatherLegs | LeatherBoots => armor(c, t, rgb(160, 100, 50)),
        IronHelmet | IronChest | IronLegs | IronBoots => armor(c, t, iron),
        DoorItem => {
            c.rect(4, 1, 8, 14, rgb(150, 118, 70));
            c.rect(4, 1, 8, 1, rgb(90, 65, 35));
            c.rect(4, 14, 8, 1, rgb(90, 65, 35));
            c.rect(4, 1, 1, 14, rgb(90, 65, 35));
            c.rect(11, 1, 1, 14, rgb(90, 65, 35));
            c.rect(6, 3, 4, 3, [200, 230, 255, 200]);
            c.set(10, 8, rgb(200, 200, 200));
        }
        BedItem => {
            c.rect(1, 6, 14, 5, rgb(180, 30, 30));
            c.rect(1, 6, 4, 3, rgb(240, 240, 240));
            c.rect(1, 11, 2, 3, rgb(120, 85, 50));
            c.rect(13, 11, 2, 3, rgb(120, 85, 50));
            c.rect(1, 10, 14, 1, rgb(140, 20, 20));
        }
        TorchItem => crate::render::texgen::paint(c, Torch),
        LadderItem => crate::render::texgen::paint(c, Ladder),
        // --- mobs ---
        PigSkin => face_base(c, [240, 160, 160], 401),
        PigFace => {
            face_base(c, [240, 160, 160], 401);
            eyes(c, 5, true, rgb(30, 30, 40));
            c.rect(5, 9, 6, 4, rgb(220, 120, 130));
            c.set(6, 10, rgb(150, 70, 80));
            c.set(9, 10, rgb(150, 70, 80));
        }
        CowSkin => {
            for y in 0..16 {
                for x in 0..16 {
                    let n = value_noise(x as f32, y as f32, 3, 411);
                    let base = if n > 0.55 { [235, 235, 235] } else { [95, 65, 45] };
                    c.set(x, y, shade3(base, ((hash01(x, y, 412) - 0.5) * 20.0) as i32, 255));
                }
            }
        }
        CowFace => {
            face_base(c, [95, 65, 45], 413);
            eyes(c, 5, true, rgb(30, 30, 40));
            c.rect(4, 10, 8, 5, rgb(200, 180, 170));
            c.rect(5, 12, 2, 1, rgb(120, 90, 80));
            c.rect(9, 12, 2, 1, rgb(120, 90, 80));
            c.rect(0, 0, 3, 2, rgb(170, 170, 170));
            c.rect(13, 0, 3, 2, rgb(170, 170, 170));
        }
        CowUdder => c.noise_fill([230, 190, 190], 8, 414),
        SheepWool => c.blotch_fill([225, 225, 225], [250, 250, 250], 8, 421),
        SheepFace => {
            face_base(c, [230, 220, 210], 422);
            eyes(c, 6, true, rgb(30, 30, 40));
            c.rect(6, 11, 4, 3, rgb(200, 180, 170));
        }
        ChickenBody => c.noise_fill([240, 240, 240], 8, 431),
        ChickenFace => {
            face_base(c, [240, 240, 240], 431);
            eyes(c, 5, false, rgb(30, 30, 40));
            c.rect(6, 8, 4, 3, rgb(240, 180, 40));
            c.rect(7, 11, 2, 3, rgb(200, 40, 40));
        }
        ChickenLeg => c.noise_fill([235, 180, 50], 8, 432),
        ZombieFace => {
            face_base(c, [70, 130, 60], 441);
            eyes(c, 6, false, rgb(20, 20, 20));
            c.rect(6, 11, 4, 1, rgb(40, 70, 35));
            c.rect(6, 12, 4, 1, rgb(40, 70, 35));
        }
        ZombieShirt => c.noise_fill([50, 140, 140], 10, 442),
        ZombiePants => c.noise_fill([55, 60, 130], 10, 443),
        ZombieSkin => face_base(c, [70, 130, 60], 444),
        SkeletonFace => {
            face_base(c, [200, 200, 200], 451);
            eyes(c, 6, false, rgb(30, 30, 30));
            c.rect(7, 9, 2, 2, rgb(80, 80, 80));
            c.rect(5, 12, 6, 1, rgb(90, 90, 90));
        }
        SkeletonBody => {
            face_base(c, [190, 190, 190], 452);
            for y in [3, 6, 9, 12] {
                c.rect(2, y, 12, 1, rgb(110, 110, 110));
            }
            c.rect(7, 2, 2, 12, rgb(120, 120, 120));
        }
        SkeletonLimb => {
            face_base(c, [190, 190, 190], 453);
            c.rect(0, 0, 16, 1, rgb(120, 120, 120));
            c.rect(0, 15, 16, 1, rgb(120, 120, 120));
        }
        CreeperFace => {
            c.blotch_fill([60, 130, 50], [110, 200, 90], 4, 461);
            c.stamp(
                &["................", "................", "................", "................", "..kkkk....kkkk..", "..kkkk....kkkk..", "..kkkk....kkkk..", "......kkkk......", ".....kkkkkk.....", "....kkkkkkkk....", "....kkkkkkkk....", "....kkk..kkk....", "....kkk..kkk....", "................", "................", "................"],
                &[('k', rgb(20, 25, 20))],
                0,
                0,
            );
        }
        CreeperBody => c.blotch_fill([60, 130, 50], [110, 200, 90], 4, 462),
        PlayerFace => {
            face_base(c, [235, 190, 150], 471);
            c.rect(0, 0, 16, 4, rgb(80, 55, 35));
            c.rect(0, 4, 2, 4, rgb(80, 55, 35));
            c.rect(14, 4, 2, 4, rgb(80, 55, 35));
            eyes(c, 7, true, rgb(60, 80, 160));
            c.rect(6, 12, 4, 1, rgb(180, 120, 100));
        }
        PlayerSkin => face_base(c, [235, 190, 150], 472),
        PlayerShirt => c.noise_fill([70, 170, 190], 10, 473),
        PlayerPants => c.noise_fill([60, 80, 170], 10, 474),
        ArrowSide => {
            c.rect(0, 7, 12, 2, rgb(130, 95, 50));
            c.rect(12, 6, 4, 4, rgb(200, 200, 200));
            c.rect(0, 5, 3, 6, rgb(230, 230, 230));
        }
        TntPrimed => c.fill([255, 255, 255, 255]),
        // --- effects / ui ---
        Crack0 | Crack1 | Crack2 | Crack3 | Crack4 | Crack5 | Crack6 | Crack7 | Crack8 | Crack9 => {
            let stage = (t.index() - Crack0.index()) as f32 / 9.0;
            // a fixed set of crack segments; reveal progressively
            let segs: [(i32, i32, i32, i32); 14] = [
                (7, 7, 3, 2), (7, 7, 11, 4), (7, 7, 9, 12), (7, 7, 4, 12), (3, 2, 1, 0), (11, 4, 14, 1),
                (9, 12, 12, 15), (4, 12, 2, 15), (7, 7, 13, 8), (13, 8, 15, 9), (7, 7, 1, 8), (3, 2, 5, 0),
                (9, 12, 7, 15), (11, 4, 10, 1),
            ];
            let n = ((segs.len() as f32) * (0.25 + 0.75 * stage)) as usize;
            for (x0, y0, x1, y1) in segs.iter().take(n) {
                let steps = (x1 - x0).abs().max((y1 - y0).abs()).max(1);
                for s in 0..=steps {
                    let x = x0 + (x1 - x0) * s / steps;
                    let y = y0 + (y1 - y0) * s / steps;
                    c.set(x, y, [0, 0, 0, 140 + (stage * 100.0) as u8]);
                }
            }
        }
        HeartFull => heart(c, rgb(220, 30, 30), false, false),
        HeartHalf => heart(c, rgb(220, 30, 30), true, false),
        HeartEmpty => heart(c, rgb(220, 30, 30), false, true),
        FoodFull => drumstick(c, false, false),
        FoodHalf => drumstick(c, true, false),
        FoodEmpty => drumstick(c, false, true),
        ArmorFull | ArmorHalf | ArmorEmpty => {
            let col = if t == ArmorEmpty { rgb(80, 80, 80) } else { rgb(220, 220, 220) };
            c.stamp(
                &["................", "................", "...aaaaaaaaaa...", "...aaaaaaaaaa...", "...aaaaaaaaaa...", "...aaaaaaaaaa...", "...aaaaaaaaaa...", "....aaaaaaaa....", ".....aaaaaa.....", "......aaaa......", ".......aa.......", "................", "................", "................", "................", "................"],
                &[('a', col)],
                0,
                0,
            );
            if t == ArmorHalf {
                for y in 0..16 {
                    for x in 8..16 {
                        if c.get(x, y)[3] > 0 {
                            c.set(x, y, rgb(80, 80, 80));
                        }
                    }
                }
            }
        }
        Slot => {
            c.fill([140, 140, 140, 200]);
            for i in 0..16 {
                c.set(i, 0, [60, 60, 60, 255]);
                c.set(0, i, [60, 60, 60, 255]);
                c.set(i, 15, [200, 200, 200, 255]);
                c.set(15, i, [200, 200, 200, 255]);
            }
        }
        SlotSelected => {
            c.fill([0, 0, 0, 0]);
            for i in 0..16 {
                for k in 0..2 {
                    c.set(i, k, rgb(255, 255, 255));
                    c.set(k, i, rgb(255, 255, 255));
                    c.set(i, 15 - k, rgb(255, 255, 255));
                    c.set(15 - k, i, rgb(255, 255, 255));
                }
            }
        }
        Sun => {
            for y in 0..16 {
                for x in 0..16 {
                    let dx = x as f32 - 7.5;
                    let dy = y as f32 - 7.5;
                    let r = (dx * dx + dy * dy).sqrt();
                    if r < 5.5 {
                        c.set(x, y, rgb(255, 250, 200));
                    } else if r < 7.5 {
                        c.set(x, y, [255, 230, 120, (255.0 * (1.0 - (r - 5.5) / 2.0)) as u8]);
                    }
                }
            }
        }
        Moon => {
            for y in 0..16 {
                for x in 0..16 {
                    let dx = x as f32 - 7.5;
                    let dy = y as f32 - 7.5;
                    let r = (dx * dx + dy * dy).sqrt();
                    if r < 5.5 {
                        let crater = value_noise(x as f32, y as f32, 4, 481);
                        c.set(x, y, if crater > 0.6 { rgb(190, 190, 200) } else { rgb(230, 230, 235) });
                    }
                }
            }
        }
        Bubble => {
            for y in 0..16 {
                for x in 0..16 {
                    let dx = x as f32 - 7.5;
                    let dy = y as f32 - 7.5;
                    let r = (dx * dx + dy * dy).sqrt();
                    if r < 5.0 && r > 3.5 {
                        c.set(x, y, rgb(230, 240, 255));
                    }
                }
            }
            c.set(5, 5, rgb(255, 255, 255));
        }
        White => c.fill([255, 255, 255, 255]),
        Crosshair => {
            c.rect(7, 2, 2, 12, [255, 255, 255, 220]);
            c.rect(2, 7, 12, 2, [255, 255, 255, 220]);
        }
        ArrowUp => c.stamp(
            &["................", "................", ".......aa.......", "......aaaa......", ".....aaaaaa.....", "....aaaaaaaa....", "...aaaaaaaaaa...", "......aaaa......", "......aaaa......", "......aaaa......", "......aaaa......", "......aaaa......", "......aaaa......", "................", "................", "................"],
            &[('a', rgb(255, 255, 255))],
            0,
            0,
        ),
        Checkmark => c.stamp(
            &["................", "................", "................", "............cc..", "...........cc...", "..........cc....", ".........cc.....", "..cc....cc......", "...cc..cc.......", "....cccc........", ".....cc.........", "................", "................", "................", "................", "................"],
            &[('c', rgb(80, 220, 80))],
            0,
            0,
        ),
        _ => c.fill([255, 0, 255, 255]),
    }
}
