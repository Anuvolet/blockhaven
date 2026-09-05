//! Procedural 16x16 tile painter for blocks. Item / mob / UI tiles live in `texgen_extra`.

use crate::render::atlas::{Tile, TILE};

pub type Rgba = [u8; 4];

pub struct Canvas {
    pub px: Vec<Rgba>,
}

impl Canvas {
    pub fn new() -> Canvas {
        Canvas { px: vec![[0, 0, 0, 0]; TILE * TILE] }
    }
    pub fn fill(&mut self, c: Rgba) {
        for p in self.px.iter_mut() {
            *p = c;
        }
    }
    #[inline]
    pub fn set(&mut self, x: i32, y: i32, c: Rgba) {
        if (0..TILE as i32).contains(&x) && (0..TILE as i32).contains(&y) {
            self.px[y as usize * TILE + x as usize] = c;
        }
    }
    #[inline]
    pub fn get(&self, x: i32, y: i32) -> Rgba {
        let x = x.rem_euclid(TILE as i32) as usize;
        let y = y.rem_euclid(TILE as i32) as usize;
        self.px[y * TILE + x]
    }
    pub fn rect(&mut self, x0: i32, y0: i32, w: i32, h: i32, c: Rgba) {
        for y in y0..y0 + h {
            for x in x0..x0 + w {
                self.set(x, y, c);
            }
        }
    }
    /// Per-pixel brightness jitter of an RGB base. `var` = max +/- per channel.
    pub fn noise_fill(&mut self, base: [u8; 3], var: i32, seed: u32) {
        for y in 0..TILE as i32 {
            for x in 0..TILE as i32 {
                let n = hash01(x, y, seed) * 2.0 - 1.0;
                self.set(x, y, shade3(base, (n * var as f32) as i32, 255));
            }
        }
    }
    /// Smooth blotchy noise between two colours (value noise, period `cells`).
    pub fn blotch_fill(&mut self, a: [u8; 3], b: [u8; 3], cells: i32, seed: u32) {
        for y in 0..TILE as i32 {
            for x in 0..TILE as i32 {
                let t = value_noise(x as f32, y as f32, cells, seed);
                self.set(x, y, lerp3(a, b, t));
            }
        }
    }
    pub fn multiply(&mut self, f: f32) {
        for p in self.px.iter_mut() {
            for c in p.iter_mut().take(3) {
                *c = (*c as f32 * f).clamp(0.0, 255.0) as u8;
            }
        }
    }
    pub fn stamp(&mut self, rows: &[&str], pal: &[(char, Rgba)], ox: i32, oy: i32) {
        for (y, row) in rows.iter().enumerate() {
            for (x, ch) in row.chars().enumerate() {
                if ch == '.' || ch == ' ' {
                    continue;
                }
                if let Some((_, c)) = pal.iter().find(|(k, _)| *k == ch) {
                    self.set(ox + x as i32, oy + y as i32, *c);
                }
            }
        }
    }
    pub fn to_bytes(&self) -> Vec<u8> {
        self.px.iter().flat_map(|p| p.iter().copied()).collect()
    }
}

pub fn rgb(r: u8, g: u8, b: u8) -> Rgba {
    [r, g, b, 255]
}
pub fn shade3(base: [u8; 3], d: i32, a: u8) -> Rgba {
    [
        (base[0] as i32 + d).clamp(0, 255) as u8,
        (base[1] as i32 + d).clamp(0, 255) as u8,
        (base[2] as i32 + d).clamp(0, 255) as u8,
        a,
    ]
}
pub fn shade(c: Rgba, d: i32) -> Rgba {
    shade3([c[0], c[1], c[2]], d, c[3])
}
pub fn lerp3(a: [u8; 3], b: [u8; 3], t: f32) -> Rgba {
    let t = t.clamp(0.0, 1.0);
    [
        (a[0] as f32 + (b[0] as f32 - a[0] as f32) * t) as u8,
        (a[1] as f32 + (b[1] as f32 - a[1] as f32) * t) as u8,
        (a[2] as f32 + (b[2] as f32 - a[2] as f32) * t) as u8,
        255,
    ]
}

#[inline]
pub fn hash_u32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846ca68b);
    x ^= x >> 16;
    x
}
#[inline]
pub fn hash01(x: i32, y: i32, seed: u32) -> f32 {
    let h = hash_u32((x as u32).wrapping_mul(0x9E3779B1) ^ (y as u32).wrapping_mul(0x85EBCA77) ^ seed.wrapping_mul(0xC2B2AE3D));
    (h & 0xffffff) as f32 / 16777216.0
}
/// Tileable value noise on a 16x16 canvas with `cells` cells per side.
pub fn value_noise(x: f32, y: f32, cells: i32, seed: u32) -> f32 {
    let s = cells as f32 / TILE as f32;
    let fx = x * s;
    let fy = y * s;
    let x0 = fx.floor() as i32;
    let y0 = fy.floor() as i32;
    let tx = fx - x0 as f32;
    let ty = fy - y0 as f32;
    let sx = tx * tx * (3.0 - 2.0 * tx);
    let sy = ty * ty * (3.0 - 2.0 * ty);
    let g = |i: i32, j: i32| hash01(i.rem_euclid(cells), j.rem_euclid(cells), seed);
    let a = g(x0, y0);
    let b = g(x0 + 1, y0);
    let c = g(x0, y0 + 1);
    let d = g(x0 + 1, y0 + 1);
    let top = a + (b - a) * sx;
    let bot = c + (d - c) * sx;
    top + (bot - top) * sy
}

/// Voronoi-style lumps (cobblestone, gravel). Returns per-pixel (edge distance 0..1, cell id).
fn cells(c: &mut Canvas, n: usize, base: [u8; 3], var: i32, edge_dark: i32, seed: u32) {
    let pts: Vec<(f32, f32, i32)> = (0..n)
        .map(|i| {
            (
                hash01(i as i32, 0, seed) * 16.0,
                hash01(i as i32, 1, seed) * 16.0,
                (hash01(i as i32, 2, seed) * var as f32 * 2.0) as i32 - var,
            )
        })
        .collect();
    for y in 0..16 {
        for x in 0..16 {
            let mut d1 = 1e9f32;
            let mut d2 = 1e9f32;
            let mut best = 0;
            for (i, p) in pts.iter().enumerate() {
                // toroidal distance for tileability
                let mut dx = (x as f32 + 0.5 - p.0).abs();
                let mut dy = (y as f32 + 0.5 - p.1).abs();
                if dx > 8.0 {
                    dx = 16.0 - dx;
                }
                if dy > 8.0 {
                    dy = 16.0 - dy;
                }
                let d = dx * dx + dy * dy;
                if d < d1 {
                    d2 = d1;
                    d1 = d;
                    best = i;
                } else if d < d2 {
                    d2 = d;
                }
            }
            let edge = (d2.sqrt() - d1.sqrt()).clamp(0.0, 2.0) / 2.0;
            let dark = if edge < 0.35 { edge_dark } else { 0 };
            let jitter = (hash01(x, y, seed ^ 77) * 10.0) as i32 - 5;
            c.set(x, y, shade3(base, pts[best].2 + dark + jitter, 255));
        }
    }
}

fn ore(c: &mut Canvas, ore: [u8; 3], seed: u32) {
    c.noise_fill([125, 125, 125], 12, 1);
    // 4-6 small clusters
    let n = 5;
    for i in 0..n {
        let cx = (hash01(i, 10, seed) * 16.0) as i32;
        let cy = (hash01(i, 11, seed) * 16.0) as i32;
        let pattern = [(0, 0), (1, 0), (0, 1), (1, 1), (2, 0), (0, 2), (-1, 1), (1, -1)];
        let count = 4 + (hash01(i, 12, seed) * 4.0) as usize;
        for (k, (dx, dy)) in pattern.iter().enumerate().take(count) {
            let d = if k % 3 == 0 { 15 } else if k % 3 == 1 { -15 } else { 0 };
            c.set((cx + dx).rem_euclid(16), (cy + dy).rem_euclid(16), shade3(ore, d, 255));
        }
    }
}

fn planks(c: &mut Canvas, base: [u8; 3], seed: u32) {
    for y in 0..16 {
        for x in 0..16 {
            let board = y / 4;
            let grain = value_noise(x as f32 * 0.5 + board as f32 * 7.0, y as f32 * 3.0, 4, seed) - 0.5;
            let mut col = shade3(base, (grain * 40.0) as i32, 255);
            if y % 4 == 3 {
                col = shade(col, -45);
            }
            // board end joints
            let joint = (board * 7 + 3) % 16;
            if x == joint && y % 4 != 3 {
                col = shade(col, -35);
            }
            c.set(x, y, col);
        }
    }
}

fn log_side(c: &mut Canvas, base: [u8; 3], stripe: i32, seed: u32) {
    for y in 0..16 {
        for x in 0..16 {
            let n = value_noise(x as f32, y as f32 * 0.25, 8, seed) - 0.5;
            let s = if (x + (y / 5)) % 3 == 0 { stripe } else { 0 };
            c.set(x, y, shade3(base, (n * 50.0) as i32 + s, 255));
        }
    }
}

fn log_top(c: &mut Canvas, bark: [u8; 3], wood: [u8; 3], seed: u32) {
    for y in 0..16 {
        for x in 0..16 {
            let dx = x as f32 - 7.5;
            let dy = y as f32 - 7.5;
            let r = (dx * dx + dy * dy).sqrt();
            let col = if r > 6.6 {
                shade3(bark, ((hash01(x, y, seed) - 0.5) * 30.0) as i32, 255)
            } else {
                let ring = ((r * 1.6).sin() * 0.5 + 0.5) * 30.0;
                shade3(wood, ring as i32 - 10 + ((hash01(x, y, seed) - 0.5) * 12.0) as i32, 255)
            };
            c.set(x, y, col);
        }
    }
}

fn leaves(c: &mut Canvas, base: [u8; 3], seed: u32, holes: f32) {
    for y in 0..16 {
        for x in 0..16 {
            let n = value_noise(x as f32, y as f32, 8, seed);
            let h = hash01(x, y, seed ^ 5);
            if h < holes {
                c.set(x, y, [0, 0, 0, 0]);
            } else {
                c.set(x, y, shade3(base, ((n - 0.5) * 70.0) as i32, 255));
            }
        }
    }
}

fn bricks(c: &mut Canvas, brick: [u8; 3], mortar: [u8; 3], bw: i32, bh: i32, seed: u32) {
    for y in 0..16 {
        for x in 0..16 {
            let row = y / bh;
            let off = if row % 2 == 0 { 0 } else { bw / 2 };
            let bx = (x + off) % bw;
            let by = y % bh;
            if bx == 0 || by == 0 {
                c.set(x, y, shade3(mortar, ((hash01(x, y, seed) - 0.5) * 16.0) as i32, 255));
            } else {
                let id = (x + off) / bw + row * 31;
                let v = (hash01(id, row, seed) - 0.5) * 30.0;
                c.set(x, y, shade3(brick, v as i32 + ((hash01(x, y, seed ^ 3) - 0.5) * 10.0) as i32, 255));
            }
        }
    }
}

fn metal_block(c: &mut Canvas, base: [u8; 3]) {
    c.noise_fill(base, 4, 9);
    for i in 0..16 {
        c.set(i, 0, shade3(base, 35, 255));
        c.set(0, i, shade3(base, 35, 255));
        c.set(i, 15, shade3(base, -45, 255));
        c.set(15, i, shade3(base, -45, 255));
        c.set(i, 1, shade3(base, 18, 255));
        c.set(1, i, shade3(base, 18, 255));
    }
}

fn grass_side(c: &mut Canvas, top: [u8; 3], seed: u32) {
    c.noise_fill([134, 96, 67], 18, 2);
    for x in 0..16 {
        let depth = 2 + (hash01(x, 0, seed) * 3.0) as i32;
        for y in 0..depth {
            let n = (hash01(x, y, seed) - 0.5) * 30.0;
            c.set(x, y, shade3(top, n as i32, 255));
        }
    }
}

fn cross_plant(c: &mut Canvas, rows: &[&str], pal: &[(char, Rgba)]) {
    c.stamp(rows, pal, 0, 0);
}

pub fn tile_pixels(t: Tile) -> Vec<u8> {
    let mut c = Canvas::new();
    paint(&mut c, t);
    c.to_bytes()
}

pub fn paint(c: &mut Canvas, t: Tile) {
    use Tile::*;
    let g = |c: &mut Canvas, base: [u8; 3], v: i32, s: u32| c.noise_fill(base, v, s);
    match t {
        Stone => {
            c.blotch_fill([118, 118, 118], [138, 138, 138], 4, 11);
            for y in 0..16 {
                for x in 0..16 {
                    let h = hash01(x, y, 12);
                    if h < 0.12 {
                        let p = c.get(x, y);
                        c.set(x, y, shade(p, -18));
                    }
                }
            }
        }
        Dirt => {
            g(c, [134, 96, 67], 18, 2);
            for y in 0..16 {
                for x in 0..16 {
                    if hash01(x, y, 21) < 0.1 {
                        c.set(x, y, shade(c.get(x, y), -25));
                    }
                }
            }
        }
        GrassTop => {
            // grayscale, tinted in shader
            c.blotch_fill([150, 150, 150], [190, 190, 190], 5, 31);
            for y in 0..16 {
                for x in 0..16 {
                    if hash01(x, y, 32) < 0.2 {
                        c.set(x, y, shade(c.get(x, y), -22));
                    }
                }
            }
        }
        GrassSide => grass_side(c, [95, 159, 53], 41),
        SnowGrassSide => grass_side(c, [235, 240, 245], 42),
        Cobblestone => cells(c, 9, [122, 122, 122], 14, -38, 51),
        MossyCobblestone => {
            cells(c, 9, [110, 118, 100], 14, -38, 51);
            for y in 0..16 {
                for x in 0..16 {
                    if value_noise(x as f32, y as f32, 4, 52) > 0.6 {
                        c.set(x, y, lerp3([80, 120, 50], [100, 140, 60], hash01(x, y, 53)));
                    }
                }
            }
        }
        OakPlanks => planks(c, [162, 130, 78], 61),
        BirchPlanks => planks(c, [196, 180, 128], 62),
        SprucePlanks => planks(c, [114, 84, 48], 63),
        Bedrock => c.blotch_fill([40, 40, 40], [110, 110, 110], 8, 71),
        Water => {
            for y in 0..16 {
                for x in 0..16 {
                    let n = value_noise(x as f32, y as f32, 4, 81);
                    let col = lerp3([170, 190, 230], [220, 235, 255], n);
                    c.set(x, y, [col[0], col[1], col[2], 170]);
                }
            }
        }
        Lava => {
            for y in 0..16 {
                for x in 0..16 {
                    let n = value_noise(x as f32, y as f32, 4, 91);
                    let n2 = value_noise(x as f32 + 5.0, y as f32 + 3.0, 8, 92);
                    let col = if n > 0.55 {
                        lerp3([255, 200, 40], [255, 240, 120], n2)
                    } else {
                        lerp3([160, 30, 10], [230, 90, 20], n * 1.8)
                    };
                    c.set(x, y, col);
                }
            }
        }
        Sand => g(c, [219, 207, 163], 10, 101),
        Gravel => cells(c, 20, [128, 124, 122], 25, -25, 111),
        CoalOre => ore(c, [40, 40, 40], 121),
        IronOre => ore(c, [216, 175, 147], 122),
        GoldOre => ore(c, [250, 220, 60], 123),
        DiamondOre => ore(c, [90, 230, 230], 124),
        RedstoneOre => ore(c, [220, 30, 30], 125),
        OakLogSide => log_side(c, [104, 82, 50], -18, 131),
        OakLogTop => log_top(c, [104, 82, 50], [185, 150, 95], 131),
        BirchLogSide => {
            log_side(c, [214, 212, 200], -6, 132);
            for i in 0..7 {
                let x = (hash01(i, 0, 133) * 16.0) as i32;
                let y = (hash01(i, 1, 133) * 16.0) as i32;
                let w = 1 + (hash01(i, 2, 133) * 3.0) as i32;
                c.rect(x, y, w, 1, rgb(50, 45, 40));
            }
        }
        BirchLogTop => log_top(c, [214, 212, 200], [200, 185, 140], 132),
        SpruceLogSide => log_side(c, [60, 40, 22], -14, 134),
        SpruceLogTop => log_top(c, [60, 40, 22], [140, 105, 60], 134),
        OakLeaves => leaves(c, [150, 150, 150], 141, 0.18), // grayscale, tinted
        BirchLeaves => leaves(c, [110, 165, 70], 142, 0.2),
        SpruceLeaves => leaves(c, [50, 90, 55], 143, 0.15),
        Glass => {
            c.fill([220, 240, 255, 40]);
            for i in 0..16 {
                c.set(i, 0, [255, 255, 255, 230]);
                c.set(0, i, [255, 255, 255, 230]);
                c.set(i, 15, [200, 220, 230, 230]);
                c.set(15, i, [200, 220, 230, 230]);
            }
            for i in 2..7 {
                c.set(13 - i, i, [255, 255, 255, 150]);
                c.set(12 - i, i, [255, 255, 255, 150]);
            }
        }
        SandstoneTop => g(c, [214, 200, 150], 6, 151),
        SandstoneSide => {
            g(c, [214, 200, 150], 6, 152);
            for y in [3, 7, 11, 15] {
                for x in 0..16 {
                    c.set(x, y, shade(c.get(x, y), -20));
                }
            }
        }
        Snow => g(c, [242, 246, 250], 6, 161),
        Ice => {
            c.blotch_fill([150, 190, 240], [190, 220, 250], 4, 171);
            for p in c.px.iter_mut() {
                p[3] = 210;
            }
            for i in 0..8 {
                c.set(3 + i, 2 + i, [230, 245, 255, 240]);
                c.set(10 - i / 2, 9 + i / 2, [230, 245, 255, 240]);
            }
        }
        CactusSide => {
            g(c, [60, 130, 40], 10, 181);
            for y in 0..16 {
                for x in [0, 4, 8, 12] {
                    c.set(x, y, shade(c.get(x, y), -30));
                }
                for x in [2, 6, 10, 14] {
                    if y % 4 == 1 {
                        c.set(x, y, rgb(200, 210, 150));
                    }
                }
            }
        }
        CactusTop => {
            g(c, [60, 130, 40], 8, 182);
            c.rect(4, 4, 8, 8, rgb(100, 170, 70));
        }
        Clay => cells(c, 10, [158, 164, 176], 10, -14, 191),
        TallGrass => cross_plant(
            c,
            &[
                "................",
                "....#...........",
                "...##......#....",
                "...##.....##....",
                "..##......##.#..",
                "..##..#..###.#..",
                ".###..#..##..#..",
                ".###.##..##.##..",
                ".###.##.###.##..",
                ".##.###.##.###..",
                "..#####.##.##...",
                "..#####.#####...",
                "...##########...",
                "....########....",
                "....#######.....",
                ".....#####......",
            ],
            &[('#', rgb(160, 160, 160))],
        ),
        DeadBush => cross_plant(
            c,
            &[
                "................",
                ".......#........",
                "..#....#...#....",
                "..#...#....#....",
                "...#..#...#.....",
                "...#.#....#.....",
                "....##...#......",
                "..#..#..#..#....",
                "...#.#.#..#.....",
                "....###..#......",
                ".....##.#.......",
                ".....###........",
                "......#.........",
                "......#.........",
                "......#.........",
                ".....###........",
            ],
            &[('#', rgb(120, 85, 40))],
        ),
        Dandelion => cross_plant(
            c,
            &[
                "................",
                "................",
                "......yy........",
                ".....yYYy.......",
                ".....yYYy.......",
                "......yy........",
                ".......s........",
                ".......s........",
                "......gs........",
                ".....g.s........",
                "......gs.g......",
                ".......sg.......",
                ".......s........",
                "......gsg.......",
                ".....g.s.g......",
                ".......s........",
            ],
            &[('y', rgb(230, 200, 30)), ('Y', rgb(255, 240, 90)), ('s', rgb(60, 130, 40)), ('g', rgb(70, 150, 45))],
        ),
        Poppy => cross_plant(
            c,
            &[
                "................",
                "................",
                ".....rrr........",
                "....rRRRr.......",
                "....rRkRr.......",
                "....rRRRr.......",
                ".....rrr........",
                ".......s........",
                "......gs........",
                ".....g.s........",
                "......gs.g......",
                ".......sg.......",
                ".......s........",
                "......gsg.......",
                ".....g.s.g......",
                ".......s........",
            ],
            &[('r', rgb(180, 30, 30)), ('R', rgb(230, 50, 40)), ('k', rgb(30, 20, 20)), ('s', rgb(60, 130, 40)), ('g', rgb(70, 150, 45))],
        ),
        BrownMushroom => cross_plant(
            c,
            &[
                "................",
                "................",
                "................",
                "................",
                "................",
                ".....cccc.......",
                "....cCCCCc......",
                "...cCCCCCCc.....",
                "...cccccccc.....",
                ".....ssss.......",
                ".....sSSs.......",
                ".....sSSs.......",
                ".....sSSs.......",
                ".....sSSs.......",
                ".....ssss.......",
                "................",
            ],
            &[('c', rgb(120, 85, 60)), ('C', rgb(160, 120, 85)), ('s', rgb(170, 150, 120)), ('S', rgb(200, 185, 150))],
        ),
        RedMushroom => cross_plant(
            c,
            &[
                "................",
                "................",
                "................",
                "................,",
                "....rrrrrr......",
                "...rrWWrrrr.....",
                "..rrrWWrrWWr....",
                "..rrrrrrrWWr....",
                "..rWWrrrrrrr....",
                "..rWWrrrrrrr....",
                ".....ssss.......",
                ".....sSSs.......",
                ".....sSSs.......",
                ".....sSSs.......",
                ".....ssss.......",
                "................",
            ],
            &[('r', rgb(200, 40, 40)), ('W', rgb(240, 240, 240)), ('s', rgb(170, 150, 120)), ('S', rgb(200, 185, 150))],
        ),
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
