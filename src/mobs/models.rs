//! Box models for mobs (and the second-player body) with a simple walk animation.

use crate::mobs::{Mob, MobKind};
use crate::render::atlas::Tile;
use crate::render::mesher::ChunkVertex;
use crate::render::overlay::box_quads;
use glam::Vec3;

#[derive(Clone, Copy)]
enum Swing {
    None,
    LegL,
    LegR,
    ArmL,
    ArmR,
    Head,
    ArmForward,
}

struct Part {
    /// centre in pixels, local space (x right, y up, z backward), origin at the feet
    center: [f32; 3],
    half: [f32; 3],
    tiles: [Tile; 6],
    swing: Swing,
    /// pivot y (pixels) for limb rotation
    pivot_y: f32,
}

fn t6(all: Tile) -> [Tile; 6] {
    [all; 6]
}
fn face(side: Tile, front: Tile) -> [Tile; 6] {
    // -X, +X, -Y, +Y, -Z(front), +Z
    [side, side, side, side, front, side]
}

fn parts(kind: MobKind, sheared: bool) -> Vec<Part> {
    use Tile as T;
    match kind {
        MobKind::Pig => vec![
            Part { center: [0.0, 10.0, 0.0], half: [5.0, 4.0, 8.0], tiles: t6(T::PigSkin), swing: Swing::None, pivot_y: 0.0 },
            Part { center: [0.0, 12.0, -12.0], half: [4.0, 4.0, 4.0], tiles: face(T::PigSkin, T::PigFace), swing: Swing::Head, pivot_y: 0.0 },
            Part { center: [-3.0, 3.0, -5.0], half: [2.0, 3.0, 2.0], tiles: t6(T::PigSkin), swing: Swing::LegL, pivot_y: 6.0 },
            Part { center: [3.0, 3.0, -5.0], half: [2.0, 3.0, 2.0], tiles: t6(T::PigSkin), swing: Swing::LegR, pivot_y: 6.0 },
            Part { center: [-3.0, 3.0, 5.0], half: [2.0, 3.0, 2.0], tiles: t6(T::PigSkin), swing: Swing::LegR, pivot_y: 6.0 },
            Part { center: [3.0, 3.0, 5.0], half: [2.0, 3.0, 2.0], tiles: t6(T::PigSkin), swing: Swing::LegL, pivot_y: 6.0 },
        ],
        MobKind::Cow => vec![
            Part { center: [0.0, 17.0, 0.0], half: [6.0, 5.0, 9.0], tiles: t6(T::CowSkin), swing: Swing::None, pivot_y: 0.0 },
            Part { center: [0.0, 11.0, 4.0], half: [2.0, 1.0, 3.0], tiles: t6(T::CowUdder), swing: Swing::None, pivot_y: 0.0 },
            Part { center: [0.0, 20.0, -12.0], half: [4.0, 4.0, 3.0], tiles: face(T::CowSkin, T::CowFace), swing: Swing::Head, pivot_y: 0.0 },
            Part { center: [-4.0, 6.0, -6.0], half: [2.0, 6.0, 2.0], tiles: t6(T::CowSkin), swing: Swing::LegL, pivot_y: 12.0 },
            Part { center: [4.0, 6.0, -6.0], half: [2.0, 6.0, 2.0], tiles: t6(T::CowSkin), swing: Swing::LegR, pivot_y: 12.0 },
            Part { center: [-4.0, 6.0, 6.0], half: [2.0, 6.0, 2.0], tiles: t6(T::CowSkin), swing: Swing::LegR, pivot_y: 12.0 },
            Part { center: [4.0, 6.0, 6.0], half: [2.0, 6.0, 2.0], tiles: t6(T::CowSkin), swing: Swing::LegL, pivot_y: 12.0 },
        ],
        MobKind::Sheep => {
            let body = if sheared { ([4.0, 4.0, 8.0], T::SheepFace) } else { ([5.0, 5.0, 9.0], T::SheepWool) };
            vec![
                Part { center: [0.0, 15.0, 0.0], half: body.0, tiles: t6(body.1), swing: Swing::None, pivot_y: 0.0 },
                Part { center: [0.0, 18.0, -11.0], half: [3.0, 3.0, 4.0], tiles: face(if sheared { T::SheepFace } else { T::SheepWool }, T::SheepFace), swing: Swing::Head, pivot_y: 0.0 },
                Part { center: [-3.0, 6.0, -5.0], half: [2.0, 6.0, 2.0], tiles: t6(T::SheepFace), swing: Swing::LegL, pivot_y: 12.0 },
                Part { center: [3.0, 6.0, -5.0], half: [2.0, 6.0, 2.0], tiles: t6(T::SheepFace), swing: Swing::LegR, pivot_y: 12.0 },
                Part { center: [-3.0, 6.0, 5.0], half: [2.0, 6.0, 2.0], tiles: t6(T::SheepFace), swing: Swing::LegR, pivot_y: 12.0 },
                Part { center: [3.0, 6.0, 5.0], half: [2.0, 6.0, 2.0], tiles: t6(T::SheepFace), swing: Swing::LegL, pivot_y: 12.0 },
            ]
        }
        MobKind::Chicken => vec![
            Part { center: [0.0, 9.0, 0.0], half: [3.0, 3.0, 4.0], tiles: t6(T::ChickenBody), swing: Swing::None, pivot_y: 0.0 },
            Part { center: [0.0, 15.0, -4.0], half: [2.0, 3.0, 2.0], tiles: face(T::ChickenBody, T::ChickenFace), swing: Swing::Head, pivot_y: 0.0 },
            Part { center: [-4.0, 10.0, 0.0], half: [1.0, 2.0, 3.0], tiles: t6(T::ChickenBody), swing: Swing::ArmL, pivot_y: 12.0 },
            Part { center: [4.0, 10.0, 0.0], half: [1.0, 2.0, 3.0], tiles: t6(T::ChickenBody), swing: Swing::ArmR, pivot_y: 12.0 },
            Part { center: [-2.0, 3.0, 0.0], half: [1.0, 3.0, 1.0], tiles: t6(T::ChickenLeg), swing: Swing::LegL, pivot_y: 6.0 },
            Part { center: [2.0, 3.0, 0.0], half: [1.0, 3.0, 1.0], tiles: t6(T::ChickenLeg), swing: Swing::LegR, pivot_y: 6.0 },
        ],
        MobKind::Zombie => vec![
            Part { center: [0.0, 18.0, 0.0], half: [4.0, 6.0, 2.0], tiles: t6(T::ZombieShirt), swing: Swing::None, pivot_y: 0.0 },
            Part { center: [0.0, 28.0, 0.0], half: [4.0, 4.0, 4.0], tiles: face(T::ZombieSkin, T::ZombieFace), swing: Swing::Head, pivot_y: 0.0 },
            Part { center: [-2.0, 6.0, 0.0], half: [2.0, 6.0, 2.0], tiles: t6(T::ZombiePants), swing: Swing::LegL, pivot_y: 12.0 },
            Part { center: [2.0, 6.0, 0.0], half: [2.0, 6.0, 2.0], tiles: t6(T::ZombiePants), swing: Swing::LegR, pivot_y: 12.0 },
            Part { center: [-6.0, 18.0, 0.0], half: [2.0, 6.0, 2.0], tiles: t6(T::ZombieSkin), swing: Swing::ArmForward, pivot_y: 22.0 },
            Part { center: [6.0, 18.0, 0.0], half: [2.0, 6.0, 2.0], tiles: t6(T::ZombieSkin), swing: Swing::ArmForward, pivot_y: 22.0 },
        ],
        MobKind::Skeleton => vec![
            Part { center: [0.0, 18.0, 0.0], half: [4.0, 6.0, 2.0], tiles: t6(T::SkeletonBody), swing: Swing::None, pivot_y: 0.0 },
            Part { center: [0.0, 28.0, 0.0], half: [4.0, 4.0, 4.0], tiles: face(T::SkeletonLimb, T::SkeletonFace), swing: Swing::Head, pivot_y: 0.0 },
            Part { center: [-2.0, 6.0, 0.0], half: [1.0, 6.0, 1.0], tiles: t6(T::SkeletonLimb), swing: Swing::LegL, pivot_y: 12.0 },
            Part { center: [2.0, 6.0, 0.0], half: [1.0, 6.0, 1.0], tiles: t6(T::SkeletonLimb), swing: Swing::LegR, pivot_y: 12.0 },
            Part { center: [-5.0, 18.0, 0.0], half: [1.0, 6.0, 1.0], tiles: t6(T::SkeletonLimb), swing: Swing::ArmForward, pivot_y: 22.0 },
            Part { center: [5.0, 18.0, 0.0], half: [1.0, 6.0, 1.0], tiles: t6(T::SkeletonLimb), swing: Swing::ArmForward, pivot_y: 22.0 },
        ],
        MobKind::Creeper => vec![
            Part { center: [0.0, 15.0, 0.0], half: [4.0, 6.0, 2.0], tiles: t6(T::CreeperBody), swing: Swing::None, pivot_y: 0.0 },
            Part { center: [0.0, 25.0, 0.0], half: [4.0, 4.0, 4.0], tiles: face(T::CreeperBody, T::CreeperFace), swing: Swing::Head, pivot_y: 0.0 },
            Part { center: [-2.0, 3.0, -2.0], half: [2.0, 3.0, 2.0], tiles: t6(T::CreeperBody), swing: Swing::LegL, pivot_y: 6.0 },
            Part { center: [2.0, 3.0, -2.0], half: [2.0, 3.0, 2.0], tiles: t6(T::CreeperBody), swing: Swing::LegR, pivot_y: 6.0 },
            Part { center: [-2.0, 3.0, 2.0], half: [2.0, 3.0, 2.0], tiles: t6(T::CreeperBody), swing: Swing::LegR, pivot_y: 6.0 },
            Part { center: [2.0, 3.0, 2.0], half: [2.0, 3.0, 2.0], tiles: t6(T::CreeperBody), swing: Swing::LegL, pivot_y: 6.0 },
        ],
    }
}

/// Humanoid player model (used for the other player in split-screen).
fn player_parts() -> Vec<Part> {
    use Tile as T;
    vec![
        Part { center: [0.0, 18.0, 0.0], half: [4.0, 6.0, 2.0], tiles: t6(T::PlayerShirt), swing: Swing::None, pivot_y: 0.0 },
        Part { center: [0.0, 28.0, 0.0], half: [4.0, 4.0, 4.0], tiles: face(T::PlayerSkin, T::PlayerFace), swing: Swing::Head, pivot_y: 0.0 },
        Part { center: [-2.0, 6.0, 0.0], half: [2.0, 6.0, 2.0], tiles: t6(T::PlayerPants), swing: Swing::LegL, pivot_y: 12.0 },
        Part { center: [2.0, 6.0, 0.0], half: [2.0, 6.0, 2.0], tiles: t6(T::PlayerPants), swing: Swing::LegR, pivot_y: 12.0 },
        Part { center: [-6.0, 18.0, 0.0], half: [2.0, 6.0, 2.0], tiles: t6(T::PlayerSkin), swing: Swing::ArmL, pivot_y: 22.0 },
        Part { center: [6.0, 18.0, 0.0], half: [2.0, 6.0, 2.0], tiles: t6(T::PlayerSkin), swing: Swing::ArmR, pivot_y: 22.0 },
    ]
}

/// Emit one model. `anim` is the walk phase, `swing_amp` the limb amplitude (0..1), `tint` a shader
/// tint index (0 none, 4 hurt red, 5 fire, 6 flash white). `pitch_over` tilts the whole body (death).
#[allow(clippy::too_many_arguments)]
fn emit(parts: &[Part], origin: Vec3, yaw: f32, head_yaw: f32, anim: f32, swing_amp: f32, light: (u8, u8), tint: u8, death_roll: f32, out: &mut Vec<ChunkVertex>) {
    let (sy, cy) = yaw.sin_cos();
    let xw = Vec3::new(cy, 0.0, -sy);
    let zw = Vec3::new(sy, 0.0, cy);
    // death: roll the body 90 degrees around its forward axis
    let (sr, cr) = death_roll.sin_cos();
    let xw_r = xw * cr + Vec3::Y * sr;
    let yw_r = Vec3::Y * cr - xw * sr;
    let s = 1.0 / 16.0;
    for p in parts {
        let swing = match p.swing {
            Swing::None | Swing::Head => 0.0,
            Swing::LegL | Swing::ArmR => anim.sin() * 0.8 * swing_amp,
            Swing::LegR | Swing::ArmL => -anim.sin() * 0.8 * swing_amp,
            Swing::ArmForward => -std::f32::consts::FRAC_PI_2 + (anim * 0.5).sin() * 0.12,
        };
        let (sa, ca) = swing.sin_cos();
        // local basis after limb pitch around X
        let ly = Vec3::new(0.0, ca, sa);
        let lz = Vec3::new(0.0, -sa, ca);
        let pivot = Vec3::new(p.center[0], p.pivot_y, p.center[2]);
        let rel = Vec3::new(0.0, p.center[1] - p.pivot_y, 0.0);
        let local_center = if matches!(p.swing, Swing::None | Swing::Head) { Vec3::from(p.center) } else { pivot + ly * rel.y };
        let (hy, hz) = if matches!(p.swing, Swing::Head) {
            let d = head_yaw - yaw;
            let (sd, cd) = d.sin_cos();
            // rotate head around Y relative to body
            let hx = Vec3::new(cd, 0.0, -sd);
            let hz = Vec3::new(sd, 0.0, cd);
            (hx, hz)
        } else {
            (Vec3::X, Vec3::Z)
        };
        // world basis: local X,Y,Z -> yaw-rotated (+ death roll)
        let to_world = |v: Vec3| xw_r * v.x + yw_r * v.y + zw * v.z;
        let (bx, by, bz) = if matches!(p.swing, Swing::Head) {
            (to_world(hy), yw_r, to_world(hz))
        } else {
            (xw_r, to_world(ly), to_world(lz))
        };
        let center = origin + to_world(local_center * s);
        let tiles = p.tiles.map(|t| t.index());
        let mut verts = Vec::with_capacity(24);
        box_quads(center, Vec3::from(p.half) * s, [bx, by, bz], tiles, light, [0, 1, 2, 3, 4, 5], &mut verts);
        if tint != 0 {
            for v in verts.iter_mut() {
                v.data = (v.data & !(7 << 15)) | ((tint as u32) << 15);
            }
        }
        out.extend(verts);
    }
}

pub fn mob_quads(mob: &Mob, light: (u8, u8), out: &mut Vec<ChunkVertex>) {
    let parts = parts(mob.kind, mob.sheared);
    let tint = if mob.hurt_timer > 0.0 {
        4
    } else if mob.kind == MobKind::Creeper && mob.ai.fuse > 0.0 && ((mob.ai.fuse * 10.0) as i32) % 2 == 0 {
        6
    } else if mob.burning > 0.0 {
        5
    } else {
        0
    };
    let roll = if mob.dead { (mob.death_timer * 4.0).min(1.0) * std::f32::consts::FRAC_PI_2 } else { 0.0 };
    let amp = if mob.dead { 0.0 } else { mob.anim_speed.min(1.0) };
    emit(&parts, mob.position(), mob.yaw, mob.head_yaw, mob.anim, amp, light, tint, roll, out);
}

/// Third-person player body (for the other split-screen player).
pub fn player_quads(pos: Vec3, yaw: f32, head_yaw: f32, anim: f32, amp: f32, light: (u8, u8), hurt: bool, out: &mut Vec<ChunkVertex>) {
    let parts = player_parts();
    emit(&parts, pos, yaw, head_yaw, anim, amp, light, if hurt { 4 } else { 0 }, 0.0, out);
}
