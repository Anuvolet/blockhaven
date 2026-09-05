//! Sub-chunk meshing: greedy quads for cubes with smooth lighting + AO, custom geometry for
//! plants, fluids, torches, doors, etc. Runs on worker threads.

use crate::render::atlas::Tile;
use crate::world::block::{self, face_tiles, props, Block, Shape, TINT_NONE};
use crate::world::chunk::{CHUNK_HEIGHT, CHUNK_SIZE};
use crate::world::World;
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
pub struct ChunkVertex {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
    pub data: u32,
}

#[inline]
pub fn pack(tile: u16, normal: u8, tint: u8, ao: u8, sky: u8, blk: u8) -> u32 {
    (tile as u32 & 0xfff)
        | ((normal as u32 & 7) << 12)
        | ((tint as u32 & 7) << 15)
        | ((ao as u32 & 3) << 18)
        | ((sky as u32 & 15) << 20)
        | ((blk as u32 & 15) << 24)
}

pub struct MeshData {
    pub opaque: Vec<ChunkVertex>,
    pub translucent: Vec<ChunkVertex>,
}

impl MeshData {
    pub fn is_empty(&self) -> bool {
        self.opaque.is_empty() && self.translucent.is_empty()
    }
}

const P: usize = CHUNK_SIZE + 2;
const CS: i32 = CHUNK_SIZE as i32;

/// Padded 18^3 snapshot of voxels and light around a sub-chunk.
pub struct Pad {
    vox: Vec<u16>,
    lit: Vec<u8>,
}

impl Pad {
    #[inline]
    fn idx(x: i32, y: i32, z: i32) -> usize {
        (((y + 1) as usize) * P + (z + 1) as usize) * P + (x + 1) as usize
    }
    #[inline]
    pub fn get(&self, x: i32, y: i32, z: i32) -> u16 {
        self.vox[Self::idx(x, y, z)]
    }
    #[inline]
    pub fn light(&self, x: i32, y: i32, z: i32) -> (u8, u8) {
        let l = self.lit[Self::idx(x, y, z)];
        (l & 15, l >> 4)
    }
    #[inline]
    fn opaque(&self, x: i32, y: i32, z: i32) -> bool {
        props(block::vox_id(self.get(x, y, z))).opaque
    }
}

/// Gather the padded region for sub-chunk (cx, sy, cz). Returns None if the centre is missing.
pub fn gather(world: &World, cx: i32, cz: i32, sy: usize) -> Option<Pad> {
    let mut pad = Pad { vox: vec![0; P * P * P], lit: vec![0; P * P * P] };
    let y0 = sy as i32 * CS - 1;
    for dz in -1..=1 {
        for dx in -1..=1 {
            let Some(c) = world.get_chunk(cx + dx, cz + dz) else {
                if dx == 0 && dz == 0 {
                    return None;
                }
                // missing neighbour: treat as air with full sky light
                for y in -1..=CS {
                    for lz in 0..CS {
                        for lx in 0..CS {
                            let px = dx * CS + lx;
                            let pz = dz * CS + lz;
                            if (-1..=CS).contains(&px) && (-1..=CS).contains(&pz) {
                                pad.lit[Pad::idx(px, y, pz)] = 15;
                            }
                        }
                    }
                }
                continue;
            };
            let c = c.read().unwrap();
            let xr = if dx == -1 { 15..16 } else if dx == 1 { 0..1 } else { 0..16 };
            let zr = if dz == -1 { 15..16 } else if dz == 1 { 0..1 } else { 0..16 };
            for y in -1..=CS {
                let wy = y0 + y + 1;
                for lz in zr.clone() {
                    for lx in xr.clone() {
                        let px = dx * CS + lx as i32;
                        let pz = dz * CS + lz as i32;
                        let i = Pad::idx(px, y, pz);
                        if wy < 0 {
                            pad.vox[i] = block::voxel(Block::Bedrock, 0);
                            pad.lit[i] = 0;
                        } else if wy >= CHUNK_HEIGHT as i32 {
                            pad.vox[i] = 0;
                            pad.lit[i] = 15;
                        } else {
                            let wy = wy as usize;
                            pad.vox[i] = c.get(lx, wy, lz);
                            pad.lit[i] = c.sky(lx, wy, lz) | (c.block_light(lx, wy, lz) << 4);
                        }
                    }
                }
            }
        }
    }
    Some(pad)
}

pub fn mesh_subchunk(world: &World, cx: i32, cz: i32, sy: usize) -> MeshData {
    let Some(pad) = gather(world, cx, cz, sy) else {
        return MeshData { opaque: Vec::new(), translucent: Vec::new() };
    };
    let origin = [cx as f32 * 16.0, sy as f32 * 16.0, cz as f32 * 16.0];
    mesh_pad(&pad, origin)
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct FaceInfo {
    tile: u16,
    tint: u8,
    ao: [u8; 4],
    light: [u8; 4], // sky | blk<<4
}

/// Per-face definition: normal axis, sign, u axis, v axis, and the corner bits (u,v) in CCW order.
struct FaceDef {
    axis: usize,
    positive: bool,
    u_axis: usize,
    v_axis: usize,
    corners: [(u8, u8); 4],
}

const FACES: [FaceDef; 6] = [
    FaceDef { axis: 0, positive: false, u_axis: 2, v_axis: 1, corners: [(0, 0), (1, 0), (1, 1), (0, 1)] }, // -X
    FaceDef { axis: 0, positive: true, u_axis: 2, v_axis: 1, corners: [(0, 0), (0, 1), (1, 1), (1, 0)] },  // +X
    FaceDef { axis: 1, positive: false, u_axis: 0, v_axis: 2, corners: [(0, 0), (1, 0), (1, 1), (0, 1)] }, // -Y
    FaceDef { axis: 1, positive: true, u_axis: 0, v_axis: 2, corners: [(0, 0), (0, 1), (1, 1), (1, 0)] },  // +Y
    FaceDef { axis: 2, positive: false, u_axis: 0, v_axis: 1, corners: [(0, 0), (0, 1), (1, 1), (1, 0)] }, // -Z
    FaceDef { axis: 2, positive: true, u_axis: 0, v_axis: 1, corners: [(0, 0), (1, 0), (1, 1), (0, 1)] },  // +Z
];

#[inline]
fn axis_vec(a: usize) -> [i32; 3] {
    let mut v = [0; 3];
    v[a] = 1;
    v
}

/// Smooth light + AO for a vertex of face `f` at block (x,y,z) with corner bits (ub, vb).
fn vertex_ao_light(pad: &Pad, x: i32, y: i32, z: i32, f: &FaceDef, ub: u8, vb: u8) -> (u8, u8) {
    let n = {
        let mut v = [0; 3];
        v[f.axis] = if f.positive { 1 } else { -1 };
        v
    };
    let su = if ub == 1 { 1 } else { -1 };
    let sv = if vb == 1 { 1 } else { -1 };
    let u = axis_vec(f.u_axis);
    let v = axis_vec(f.v_axis);
    let base = [x + n[0], y + n[1], z + n[2]];
    let p1 = [base[0] + u[0] * su, base[1] + u[1] * su, base[2] + u[2] * su];
    let p2 = [base[0] + v[0] * sv, base[1] + v[1] * sv, base[2] + v[2] * sv];
    let p3 = [p1[0] + v[0] * sv, p1[1] + v[1] * sv, p1[2] + v[2] * sv];
    let s1 = pad.opaque(p1[0], p1[1], p1[2]);
    let s2 = pad.opaque(p2[0], p2[1], p2[2]);
    let c = pad.opaque(p3[0], p3[1], p3[2]);
    let ao = if s1 && s2 { 0 } else { 3 - (s1 as u8 + s2 as u8 + c as u8) };
    let mut sky = 0u32;
    let mut blk = 0u32;
    let mut cnt = 0u32;
    let (a, b) = pad.light(base[0], base[1], base[2]);
    sky += a as u32;
    blk += b as u32;
    cnt += 1;
    if !s1 {
        let (a, b) = pad.light(p1[0], p1[1], p1[2]);
        sky += a as u32;
        blk += b as u32;
        cnt += 1;
    }
    if !s2 {
        let (a, b) = pad.light(p2[0], p2[1], p2[2]);
        sky += a as u32;
        blk += b as u32;
        cnt += 1;
    }
    if !c && !(s1 && s2) {
        let (a, b) = pad.light(p3[0], p3[1], p3[2]);
        sky += a as u32;
        blk += b as u32;
        cnt += 1;
    }
    let sky = ((sky + cnt / 2) / cnt) as u8;
    let blk = ((blk + cnt / 2) / cnt) as u8;
    (ao, sky | (blk << 4))
}

#[inline]
fn face_tint(b: Block, face: usize, p: &block::BlockProps) -> u8 {
    match b {
        Block::Grass => {
            if face == 3 {
                p.tint
            } else {
                TINT_NONE
            }
        }
        _ => p.tint,
    }
}

/// Should the face of cube block `v` towards neighbour `nv` be drawn?
#[inline]
fn face_visible(v: u16, nv: u16) -> bool {
    let nb = block::vox_id(nv);
    let np = props(nb);
    if np.opaque {
        return false;
    }
    let b = block::vox_id(v);
    if b == nb {
        // same translucent block: hide internal faces
        let bb = Block::from_id(b);
        return !matches!(bb, Block::Glass | Block::Ice | Block::Water | Block::Lava);
    }
    true
}

pub struct Emitter<'a> {
    pub out: &'a mut Vec<ChunkVertex>,
    pub origin: [f32; 3],
}

impl<'a> Emitter<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn quad(&mut self, pos: [[f32; 3]; 4], uv: [[f32; 2]; 4], tile: u16, normal: u8, tint: u8, ao: [u8; 4], light: [u8; 4], flip: bool) {
        let mut order = [0usize, 1, 2, 3];
        if flip {
            order = [1, 2, 3, 0];
        }
        for k in order {
            self.out.push(ChunkVertex {
                pos: [pos[k][0] + self.origin[0], pos[k][1] + self.origin[1], pos[k][2] + self.origin[2]],
                uv: uv[k],
                data: pack(tile, normal, tint, ao[k], light[k] & 15, light[k] >> 4),
            });
        }
    }

    /// Axis-aligned box with per-face tiles; light is uniform. `uv_override` replaces the automatic
    /// uv rectangle of a face (u0,v0,u1,v1 in tile units).
    pub fn boxed(&mut self, min: [f32; 3], max: [f32; 3], tiles: [Tile; 6], light: u8, faces: [bool; 6], tint: u8, uv_override: Option<(usize, [f32; 4])>) {
        for (fi, f) in FACES.iter().enumerate() {
            if !faces[fi] {
                continue;
            }
            let mut pos = [[0f32; 3]; 4];
            let mut uv = [[0f32; 2]; 4];
            for (k, (ub, vb)) in f.corners.iter().enumerate() {
                let mut p = [0f32; 3];
                p[f.axis] = if f.positive { max[f.axis] } else { min[f.axis] };
                p[f.u_axis] = if *ub == 1 { max[f.u_axis] } else { min[f.u_axis] };
                p[f.v_axis] = if *vb == 1 { max[f.v_axis] } else { min[f.v_axis] };
                pos[k] = p;
                let (u0, v0, u1, v1) = match uv_override {
                    Some((face, r)) if face == fi => (r[0], r[1], r[2], r[3]),
                    _ => {
                        let (u0, u1) = (min[f.u_axis].fract_or_unit(), max[f.u_axis].fract_or_top(min[f.u_axis]));
                        let (v0, v1) = (min[f.v_axis].fract_or_unit(), max[f.v_axis].fract_or_top(min[f.v_axis]));
                        (u0, v0, u1, v1)
                    }
                };
                let u = if *ub == 1 { u1 } else { u0 };
                // side faces: v grows downward (texture top at block top)
                let v = if f.axis == 1 {
                    if *vb == 1 { v1 } else { v0 }
                } else if *vb == 1 {
                    1.0 - v1
                } else {
                    1.0 - v0
                };
                // mirror u on faces whose "right" runs along the negative axis
                let u = if fi == 1 || fi == 4 { (u0 + u1) - u } else { u };
                uv[k] = [u, v];
            }
            self.quad(pos, uv, tiles[fi].index(), fi as u8, tint, [3; 4], [light; 4], false);
        }
    }
}

trait Frac {
    fn fract_or_unit(self) -> f32;
    fn fract_or_top(self, lo: f32) -> f32;
}
impl Frac for f32 {
    fn fract_or_unit(self) -> f32 {
        self - self.floor()
    }
    fn fract_or_top(self, lo: f32) -> f32 {
        let base = lo.floor();
        self - base
    }
}

pub fn mesh_pad(pad: &Pad, origin: [f32; 3]) -> MeshData {
    let mut opaque: Vec<ChunkVertex> = Vec::new();
    let mut translucent: Vec<ChunkVertex> = Vec::new();

    // ---- greedy cubes ----
    let mut mask: Vec<Option<FaceInfo>> = vec![None; CHUNK_SIZE * CHUNK_SIZE];
    for (fi, f) in FACES.iter().enumerate() {
        let n = if f.positive { 1 } else { -1 };
        for slice in 0..CS {
            // build mask
            let mut any = false;
            for v in 0..CS {
                for u in 0..CS {
                    let mut p = [0i32; 3];
                    p[f.axis] = slice;
                    p[f.u_axis] = u;
                    p[f.v_axis] = v;
                    let vox = pad.get(p[0], p[1], p[2]);
                    let mut info = None;
                    if vox != 0 {
                        let b = Block::from_id(block::vox_id(vox));
                        let bp = props(b.id());
                        if bp.shape == Shape::Cube {
                            let mut q = p;
                            q[f.axis] += n;
                            let nv = pad.get(q[0], q[1], q[2]);
                            if face_visible(vox, nv) {
                                let tiles = face_tiles(b, block::vox_meta(vox));
                                let mut ao = [0u8; 4];
                                let mut light = [0u8; 4];
                                for (k, (ub, vb)) in f.corners.iter().enumerate() {
                                    let (a, l) = vertex_ao_light(pad, p[0], p[1], p[2], f, *ub, *vb);
                                    ao[k] = a;
                                    light[k] = l;
                                }
                                info = Some(FaceInfo { tile: tiles[fi].index(), tint: face_tint(b, fi, bp), ao, light });
                                any = true;
                            }
                        }
                    }
                    mask[(v * CS + u) as usize] = info;
                }
            }
            if !any {
                continue;
            }
            // greedy merge
            for v in 0..CS {
                let mut u = 0;
                while u < CS {
                    let Some(info) = mask[(v * CS + u) as usize] else {
                        u += 1;
                        continue;
                    };
                    let mut w = 1;
                    while u + w < CS && mask[(v * CS + u + w) as usize] == Some(info) {
                        w += 1;
                    }
                    let mut h = 1;
                    'outer: while v + h < CS {
                        for k in 0..w {
                            if mask[((v + h) * CS + u + k) as usize] != Some(info) {
                                break 'outer;
                            }
                        }
                        h += 1;
                    }
                    // emit
                    let mut p0 = [0f32; 3];
                    p0[f.axis] = slice as f32 + if f.positive { 1.0 } else { 0.0 };
                    p0[f.u_axis] = u as f32;
                    p0[f.v_axis] = v as f32;
                    let mut pos = [[0f32; 3]; 4];
                    let mut uv = [[0f32; 2]; 4];
                    for (k, (ub, vb)) in f.corners.iter().enumerate() {
                        let mut p = p0;
                        if *ub == 1 {
                            p[f.u_axis] += w as f32;
                        }
                        if *vb == 1 {
                            p[f.v_axis] += h as f32;
                        }
                        pos[k] = p;
                        let tu = if *ub == 1 { w as f32 } else { 0.0 };
                        let tv = if f.axis == 1 {
                            if *vb == 1 { h as f32 } else { 0.0 }
                        } else if *vb == 1 {
                            0.0
                        } else {
                            h as f32
                        };
                        let tu = if fi == 1 || fi == 4 { w as f32 - tu } else { tu };
                        uv[k] = [tu, tv];
                    }
                    let flip = info.ao[0] + info.ao[2] < info.ao[1] + info.ao[3];
                    let b = Block::from_id(block::vox_id(pad.get(
                        if f.axis == 0 { slice } else if f.u_axis == 0 { u } else { v },
                        if f.axis == 1 { slice } else if f.u_axis == 1 { u } else { v },
                        if f.axis == 2 { slice } else if f.u_axis == 2 { u } else { v },
                    )));
                    let target = if props(b.id()).translucent { &mut translucent } else { &mut opaque };
                    let mut em = Emitter { out: target, origin };
                    em.quad(pos, uv, info.tile, fi as u8, info.tint, info.ao, info.light, flip);
                    for hh in 0..h {
                        for ww in 0..w {
                            mask[((v + hh) * CS + u + ww) as usize] = None;
                        }
                    }
                    u += w;
                }
            }
        }
    }

    // ---- custom shapes ----
    for y in 0..CS {
        for z in 0..CS {
            for x in 0..CS {
                let vox = pad.get(x, y, z);
                if vox == 0 {
                    continue;
                }
                let b = Block::from_id(block::vox_id(vox));
                let bp = props(b.id());
                if bp.shape == Shape::Cube || bp.shape == Shape::None {
                    continue;
                }
                let (s, l) = pad.light(x, y, z);
                let light = s | (l << 4);
                let target = if bp.translucent { &mut translucent } else { &mut opaque };
                let mut em = Emitter { out: target, origin };
                custom_shape(&mut em, pad, x, y, z, b, block::vox_meta(vox), bp, light);
            }
        }
    }
    MeshData { opaque, translucent }
}

#[allow(clippy::too_many_arguments)]
fn custom_shape(em: &mut Emitter, pad: &Pad, x: i32, y: i32, z: i32, b: Block, meta: u8, bp: &block::BlockProps, light: u8) {
    let (fx, fy, fz) = (x as f32, y as f32, z as f32);
    let tiles = face_tiles(b, meta);
    let all = [true; 6];
    match bp.shape {
        Shape::Cross => {
            let t = tiles[0].index();
            let tint = bp.tint;
            let lo = [fx, fy, fz];
            let d = 0.146; // inset so the cross fits inside the block nicely
            let a0 = [lo[0] + d, lo[1], lo[2] + d];
            let a1 = [lo[0] + 1.0 - d, lo[1], lo[2] + 1.0 - d];
            let b0 = [lo[0] + 1.0 - d, lo[1], lo[2] + d];
            let b1 = [lo[0] + d, lo[1], lo[2] + 1.0 - d];
            for (p, q) in [(a0, a1), (b0, b1)] {
                let quad = [[p[0], p[1], p[2]], [q[0], q[1], q[2]], [q[0], q[1] + 1.0, q[2]], [p[0], p[1] + 1.0, p[2]]];
                let uv = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
                em.quad(quad, uv, t, 3, tint, [3; 4], [light; 4], false);
                let quad2 = [quad[3], quad[2], quad[1], quad[0]];
                let uv2 = [uv[3], uv[2], uv[1], uv[0]];
                em.quad(quad2, uv2, t, 3, tint, [3; 4], [light; 4], false);
            }
        }
        Shape::Fluid => {
            let level = meta & 7;
            let falling = meta & 8 != 0;
            let same = |v: u16| block::vox_id(v) == b.id();
            let h = if falling { 1.0 } else { fluid_height(level) };
            let above = pad.get(x, y + 1, z);
            let top_h = if same(above) { 1.0 } else { h };
            let mut faces = [false; 6];
            let dirs = [(-1, 0, 0), (1, 0, 0), (0, -1, 0), (0, 1, 0), (0, 0, -1), (0, 0, 1)];
            for (i, (dx, dy, dz)) in dirs.iter().enumerate() {
                let nv = pad.get(x + dx, y + dy, z + dz);
                if same(nv) {
                    continue;
                }
                if props(block::vox_id(nv)).opaque {
                    continue;
                }
                faces[i] = true;
            }
            let tint = bp.tint;
            em.boxed([fx, fy, fz], [fx + 1.0, fy + top_h, fz + 1.0], tiles, light, faces, tint, None);
        }
        Shape::Torch => {
            let t = tiles[0];
            // meta: 0 floor, 1 = attached to -X wall (torch leans from +X side)... we simply offset.
            let (ox, oz) = match meta & 7 {
                1 => (-0.3, 0.0),
                2 => (0.3, 0.0),
                3 => (0.0, -0.3),
                4 => (0.0, 0.3),
                _ => (0.0, 0.0),
            };
            let oy = if meta & 7 != 0 { 0.2 } else { 0.0 };
            let min = [fx + 7.0 / 16.0 + ox, fy + oy, fz + 7.0 / 16.0 + oz];
            let max = [fx + 9.0 / 16.0 + ox, fy + oy + 10.0 / 16.0, fz + 9.0 / 16.0 + oz];
            em.boxed(min, max, [t; 6], light, [true, true, false, false, true, true], 0, None);
            em.boxed(min, max, [t; 6], light, [false, false, false, true, false, false], 0, Some((3, [7.0 / 16.0, 1.0 / 16.0, 9.0 / 16.0, 3.0 / 16.0])));
        }
        Shape::Ladder => {
            let t = tiles[0].index();
            // meta facing: side of the block the ladder is attached to (0=-Z wall,1=+X,2=+Z,3=-X)
            let e = 1.0 / 16.0;
            let (p, q, n) = match meta & 3 {
                0 => ([fx, fy, fz + e], [fx + 1.0, fy, fz + e], 5u8),
                1 => ([fx + 1.0 - e, fy, fz + 1.0], [fx + 1.0 - e, fy, fz], 0u8),
                2 => ([fx + 1.0, fy, fz + 1.0 - e], [fx, fy, fz + 1.0 - e], 4u8),
                _ => ([fx + e, fy, fz], [fx + e, fy, fz + 1.0], 1u8),
            };
            let quad = [[p[0], p[1], p[2]], [q[0], q[1], q[2]], [q[0], q[1] + 1.0, q[2]], [p[0], p[1] + 1.0, p[2]]];
            let uv = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
            em.quad(quad, uv, t, n, 0, [3; 4], [light; 4], false);
            let quad2 = [quad[3], quad[2], quad[1], quad[0]];
            let uv2 = [uv[3], uv[2], uv[1], uv[0]];
            em.quad(quad2, uv2, t, n ^ 1, 0, [3; 4], [light; 4], false);
        }
        Shape::Door => {
            let facing = meta & 3;
            let open = meta & 4 != 0;
            let th = 3.0 / 16.0;
            // closed: slab against the wall opposite the facing direction's entry; open: rotated 90deg
            let side = if open { (facing + 1) & 3 } else { facing };
            let (min, max) = match side {
                0 => ([fx, fy, fz], [fx + 1.0, fy + 1.0, fz + th]),
                1 => ([fx + 1.0 - th, fy, fz], [fx + 1.0, fy + 1.0, fz + 1.0]),
                2 => ([fx, fy, fz + 1.0 - th], [fx + 1.0, fy + 1.0, fz + 1.0]),
                _ => ([fx, fy, fz], [fx + th, fy + 1.0, fz + 1.0]),
            };
            let t = tiles[0];
            let mut faces = all;
            faces[2] = meta & 8 == 0;
            faces[3] = meta & 8 != 0;
            let mut tl = [t; 6];
            tl[2] = Tile::OakPlanks;
            tl[3] = Tile::OakPlanks;
            em.boxed(min, max, tl, light, faces, 0, None);
        }
        Shape::Bed => {
            let facing = meta & 3;
            let head = meta & 4 != 0;
            let top = if head { Tile::BedTopHead } else { Tile::BedTopFoot };
            let mut tl = [Tile::BedSide; 6];
            tl[3] = top;
            tl[2] = Tile::OakPlanks;
            // end tile on the outer end
            let end_face = match (facing, head) {
                (0, true) => 4,
                (0, false) => 5,
                (1, true) => 1,
                (1, false) => 0,
                (2, true) => 5,
                (2, false) => 4,
                (3, true) => 0,
                _ => 1,
            };
            tl[end_face] = Tile::BedEnd;
            em.boxed([fx, fy, fz], [fx + 1.0, fy + 9.0 / 16.0, fz + 1.0], tl, light, all, 0, None);
        }
        Shape::Wire => {
            let t = if meta > 0 { Tile::RedstoneDustCross } else { Tile::RedstoneDust };
            let h = fy + 1.0 / 32.0;
            let quad = [[fx, h, fz], [fx, h, fz + 1.0], [fx + 1.0, h, fz + 1.0], [fx + 1.0, h, fz]];
            let uv = [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]];
            em.quad(quad, uv, t.index(), 3, 0, [3; 4], [light; 4], false);
        }
        Shape::Plate => {
            let d = 1.0 / 16.0;
            let h = if meta & 1 != 0 { 0.5 / 16.0 } else { 1.0 / 16.0 };
            em.boxed([fx + d, fy, fz + d], [fx + 1.0 - d, fy + h, fz + 1.0 - d], tiles, light, [true, true, false, true, true, true], 0, None);
        }
        Shape::Button => {
            // meta bits 0-2: attach face (0..5 = the face of the neighbour it sits on), bit 3: on
            let attach = (meta & 7).min(5);
            let on = meta & 8 != 0;
            let base = if b == Block::Lever { Tile::Cobblestone } else { Tile::Stone };
            let depth = if on && b == Block::Button { 1.0 / 16.0 } else { 2.0 / 16.0 };
            let (min, max) = match attach {
                0 => ([fx, fy + 6.0 / 16.0, fz + 5.0 / 16.0], [fx + depth, fy + 10.0 / 16.0, fz + 11.0 / 16.0]),
                1 => ([fx + 1.0 - depth, fy + 6.0 / 16.0, fz + 5.0 / 16.0], [fx + 1.0, fy + 10.0 / 16.0, fz + 11.0 / 16.0]),
                4 => ([fx + 5.0 / 16.0, fy + 6.0 / 16.0, fz], [fx + 11.0 / 16.0, fy + 10.0 / 16.0, fz + depth]),
                5 => ([fx + 5.0 / 16.0, fy + 6.0 / 16.0, fz + 1.0 - depth], [fx + 11.0 / 16.0, fy + 10.0 / 16.0, fz + 1.0]),
                3 => ([fx + 5.0 / 16.0, fy + 1.0 - depth, fz + 6.0 / 16.0], [fx + 11.0 / 16.0, fy + 1.0, fz + 10.0 / 16.0]),
                _ => ([fx + 5.0 / 16.0, fy, fz + 6.0 / 16.0], [fx + 11.0 / 16.0, fy + depth, fz + 10.0 / 16.0]),
            };
            em.boxed(min, max, [base; 6], light, all, 0, None);
            if b == Block::Lever {
                // the handle: a thin stick leaning one way or the other
                let c = [(min[0] + max[0]) * 0.5, (min[1] + max[1]) * 0.5, (min[2] + max[2]) * 0.5];
                let tilt = if on { 0.22 } else { -0.22 };
                let (smin, smax) = match attach {
                    2 | 3 => {
                        let dir = if attach == 2 { 1.0 } else { -1.0 };
                        ([c[0] - 1.0 / 32.0 + tilt, c[1].min(c[1] + dir * 0.5), c[2] - 1.0 / 32.0], [c[0] + 1.0 / 32.0 + tilt, c[1].max(c[1] + dir * 0.5), c[2] + 1.0 / 32.0])
                    }
                    0 | 1 => {
                        let dir = if attach == 1 { -1.0 } else { 1.0 };
                        ([c[0].min(c[0] + dir * 0.5), c[1] - 1.0 / 32.0 + tilt, c[2] - 1.0 / 32.0], [c[0].max(c[0] + dir * 0.5), c[1] + 1.0 / 32.0 + tilt, c[2] + 1.0 / 32.0])
                    }
                    _ => {
                        let dir = if attach == 5 { -1.0 } else { 1.0 };
                        ([c[0] - 1.0 / 32.0, c[1] - 1.0 / 32.0 + tilt, c[2].min(c[2] + dir * 0.5)], [c[0] + 1.0 / 32.0, c[1] + 1.0 / 32.0 + tilt, c[2].max(c[2] + dir * 0.5)])
                    }
                };
                let stick = if on { Tile::RedstoneTorchOn } else { Tile::Lever };
                em.boxed(smin, smax, [stick; 6], light, all, 0, None);
            }
        }
        Shape::Cactus => {
            let d = 1.0 / 16.0;
            let mut faces = all;
            faces[2] = !pad.opaque(x, y - 1, z) && block::vox_id(pad.get(x, y - 1, z)) != b.id();
            faces[3] = block::vox_id(pad.get(x, y + 1, z)) != b.id();
            em.boxed([fx + d, fy, fz + d], [fx + 1.0 - d, fy + 1.0, fz + 1.0 - d], tiles, light, faces, 0, None);
        }
        Shape::Farmland => {
            let mut faces = all;
            let dirs = [(-1, 0, 0), (1, 0, 0), (0, -1, 0), (0, 1, 0), (0, 0, -1), (0, 0, 1)];
            for (i, (dx, dy, dz)) in dirs.iter().enumerate() {
                if i != 3 && pad.opaque(x + dx, y + dy, z + dz) {
                    faces[i] = false;
                }
            }
            em.boxed([fx, fy, fz], [fx + 1.0, fy + 15.0 / 16.0, fz + 1.0], tiles, light, faces, 0, None);
        }
        Shape::PistonHead => {
            let dir = (meta & 7).min(5) as usize;
            let (nx, ny, nz) = block::face_offset(dir as u8);
            let th = 4.0 / 16.0;
            // plate on the far side (the side facing `dir`), arm going back to the piston body
            let mut pmin = [fx, fy, fz];
            let mut pmax = [fx + 1.0, fy + 1.0, fz + 1.0];
            let mut amin = [fx + 6.0 / 16.0, fy + 6.0 / 16.0, fz + 6.0 / 16.0];
            let mut amax = [fx + 10.0 / 16.0, fy + 10.0 / 16.0, fz + 10.0 / 16.0];
            let axis = dir / 2;
            let n = [nx, ny, nz][axis];
            if n > 0 {
                pmin[axis] = pmax[axis] - th;
                amin[axis] = [fx, fy, fz][axis];
                amax[axis] = pmin[axis];
            } else {
                pmax[axis] = pmin[axis] + th;
                amax[axis] = [fx, fy, fz][axis] + 1.0;
                amin[axis] = pmax[axis];
            }
            em.boxed(pmin, pmax, tiles, light, all, 0, None);
            em.boxed(amin, amax, [Tile::PistonSide; 6], light, all, 0, None);
        }
        Shape::Cube | Shape::None => {}
    }
}

/// Surface height of a fluid block for a spread level (0 = source).
pub fn fluid_height(level: u8) -> f32 {
    (14.0 - level as f32 * 1.75) / 16.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::block::voxel;
    use crate::world::chunk::Chunk;

    fn world_with(f: impl Fn(&mut Chunk)) -> std::sync::Arc<World> {
        let w = World::new(0);
        for cx in -1..=1 {
            for cz in -1..=1 {
                let mut c = Chunk::new(cx, cz);
                if cx == 0 && cz == 0 {
                    f(&mut c);
                }
                c.recompute_heightmap();
                crate::world::light::init_chunk_light(&mut c);
                w.insert_chunk(c);
            }
        }
        w
    }

    #[test]
    fn single_block_has_six_faces() {
        let w = world_with(|c| c.set(5, 5, 5, voxel(Block::Stone, 0)));
        let m = mesh_subchunk(&w, 0, 0, 0);
        assert_eq!(m.opaque.len(), 6 * 4);
        assert!(m.translucent.is_empty());
    }

    #[test]
    fn greedy_merges_a_flat_slab_into_few_quads() {
        let w = world_with(|c| {
            for z in 0..16 {
                for x in 0..16 {
                    c.set(x, 0, z, voxel(Block::Stone, 0));
                }
            }
        });
        let m = mesh_subchunk(&w, 0, 0, 0);
        // top merges into 1 quad, 4 sides merge into one quad each; the bottom faces bedrock
        // below the world and is culled = 5 quads
        assert_eq!(m.opaque.len(), 5 * 4);
    }

    #[test]
    fn hidden_faces_are_culled() {
        let w = world_with(|c| {
            c.set(5, 5, 5, voxel(Block::Stone, 0));
            c.set(6, 5, 5, voxel(Block::Stone, 0));
        });
        let m = mesh_subchunk(&w, 0, 0, 0);
        // 2 blocks: 12 faces minus 2 shared = 10 (top/bottom/sides merge some): count quads <= 10
        assert!(m.opaque.len() / 4 <= 10 && m.opaque.len() / 4 >= 6);
    }

    #[test]
    fn water_goes_to_translucent_pass() {
        let w = world_with(|c| c.set(5, 5, 5, voxel(Block::Water, 0)));
        let m = mesh_subchunk(&w, 0, 0, 0);
        assert!(m.opaque.is_empty());
        assert_eq!(m.translucent.len(), 6 * 4);
    }
}
