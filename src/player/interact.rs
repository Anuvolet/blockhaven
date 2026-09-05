//! Block breaking / placing / using, item dropping, eating.

use crate::entity::{spawn_drop, throw_drop, ItemDrop};
use crate::player::items::{self, block_drop, ItemKind, ItemStack};
use crate::player::physics::{self, Aabb};
use crate::player::raycast::{raycast, RayHit};
use crate::player::{GameMode, OpenUi, Player, PlayerInput};
use crate::world::block::{self, facing_from_yaw, props, voxel, Block, Shape, Tool};
use crate::world::fluid::FluidSim;
use crate::world::noise::Rng;
use crate::world::{ChunkCache, World};
use glam::Vec3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Interaction {
    Broke { pos: (i32, i32, i32), block: Block },
    Hit { pos: (i32, i32, i32), block: Block },
    Placed { pos: (i32, i32, i32), block: Block },
    Toggled { pos: (i32, i32, i32), block: Block },
    OpenUi(OpenUi),
    Ate,
    Sleep { pos: (i32, i32, i32) },
    ShootArrow { origin: Vec3, dir: Vec3 },
    Explode { pos: (i32, i32, i32) },
}

pub struct Ctx<'a> {
    pub world: &'a World,
    pub fluids: &'a mut FluidSim,
    pub drops: &'a mut Vec<ItemDrop>,
    pub rng: &'a mut Rng,
    pub player_boxes: &'a [Aabb],
}

/// Seconds to break `block` with the held stack.
pub fn break_time(b: Block, held: &ItemStack) -> (f32, bool) {
    let p = props(b.id());
    if p.hardness < 0.0 {
        return (f32::INFINITY, false);
    }
    if p.hardness == 0.0 {
        return (0.05, true);
    }
    let tool = held.tool_info();
    let (right_tool, tier, speed) = match tool {
        Some((t, tier, speed, _, _)) if t == p.tool => (true, tier, speed),
        Some((Tool::Sword, _, _, _, _)) if matches!(b, Block::OakLeaves | Block::BirchLeaves | Block::SpruceLeaves | Block::TallGrass) => (true, 0, 1.5),
        _ => (false, 0, 1.0),
    };
    let needs_tool = p.tool != Tool::None && p.tool != Tool::Hoe && matches!(p.tool, Tool::Pickaxe) || p.min_tier > 0;
    let can_harvest = !needs_tool || (right_tool && tier >= p.min_tier);
    let mult = if right_tool { speed } else { 1.0 };
    let base = if can_harvest { p.hardness * 1.5 } else { p.hardness * 5.0 };
    ((base / mult).max(0.05), can_harvest)
}

fn target_from_hit(hit: &RayHit) -> (i32, i32, i32) {
    (hit.pos.0 + hit.normal.0, hit.pos.1 + hit.normal.1, hit.pos.2 + hit.normal.2)
}

/// Blocks that need support: returns true if `v` at `pos` can stay given its neighbours.
pub fn is_supported(cache: &mut ChunkCache, pos: (i32, i32, i32), v: u16) -> bool {
    let b = block::vox_block(v);
    let meta = block::vox_meta(v);
    let p = props(b.id());
    let (x, y, z) = pos;
    let solid_below = cache.is_solid(x, y - 1, z);
    match p.shape {
        Shape::Cross => {
            let below = cache.get_block(x, y - 1, z);
            if b == Block::Wheat {
                below == Block::Farmland
            } else if b == Block::Cactus {
                matches!(below, Block::Sand | Block::Cactus)
            } else {
                matches!(below, Block::Grass | Block::Dirt | Block::Podzol | Block::SnowyGrass | Block::Farmland | Block::Sand | Block::Gravel | Block::Stone | Block::MushroomStem | Block::Cobblestone | Block::Clay) || solid_below
            }
        }
        Shape::Cactus => matches!(cache.get_block(x, y - 1, z), Block::Sand | Block::Cactus),
        Shape::Wire | Shape::Plate | Shape::Bed => solid_below,
        Shape::Torch => match meta & 7 {
            0 => solid_below,
            1 => cache.is_solid(x + 1, y, z),
            2 => cache.is_solid(x - 1, y, z),
            3 => cache.is_solid(x, y, z + 1),
            4 => cache.is_solid(x, y, z - 1),
            _ => solid_below,
        },
        Shape::Ladder => match meta & 3 {
            0 => cache.is_solid(x, y, z - 1),
            1 => cache.is_solid(x + 1, y, z),
            2 => cache.is_solid(x, y, z + 1),
            _ => cache.is_solid(x - 1, y, z),
        },
        Shape::Button => {
            let (dx, dy, dz) = block::face_offset(meta & 7);
            // attach face f means the block it sits on is in direction -normal(f)... we store the
            // face of *this* block that touches the support, so the support is at pos + face dir
            cache.is_solid(x + dx, y + dy, z + dz)
        }
        Shape::Door => {
            if meta & 8 != 0 {
                cache.get_block(x, y - 1, z) == Block::Door
            } else {
                solid_below
            }
        }
        _ => true,
    }
}

/// Meta for the torch shape given the face normal the player clicked.
fn torch_meta(normal: (i32, i32, i32)) -> Option<u8> {
    match normal {
        (0, 1, 0) => Some(0),
        (1, 0, 0) => Some(2),
        (-1, 0, 0) => Some(1),
        (0, 0, 1) => Some(4),
        (0, 0, -1) => Some(3),
        _ => None,
    }
}

fn ladder_meta(normal: (i32, i32, i32)) -> Option<u8> {
    match normal {
        (0, 0, 1) => Some(0),
        (-1, 0, 0) => Some(1),
        (0, 0, -1) => Some(2),
        (1, 0, 0) => Some(3),
        _ => None,
    }
}

/// Button / lever attach face: the face of the new block that touches the clicked block.
fn attach_face(normal: (i32, i32, i32)) -> u8 {
    match normal {
        (1, 0, 0) => 0,
        (-1, 0, 0) => 1,
        (0, 1, 0) => 2,
        (0, -1, 0) => 3,
        (0, 0, 1) => 4,
        _ => 5,
    }
}

fn place_block(ctx: &mut Ctx, player: &Player, hit: &RayHit, b: Block) -> Option<Vec<((i32, i32, i32), u16)>> {
    let world = ctx.world;
    let clicked = world.get(hit.pos.0, hit.pos.1, hit.pos.2);
    let clicked_props = props(block::vox_id(clicked));
    // clicking replaceable vegetation replaces it in place
    let (tx, ty, tz) = if clicked_props.replaceable && !block::is_fluid(clicked) { hit.pos } else { target_from_hit(hit) };
    if !(0..256).contains(&ty) {
        return None;
    }
    let cur = world.get(tx, ty, tz);
    if !props(block::vox_id(cur)).replaceable {
        return None;
    }
    let facing = facing_from_yaw(player.yaw);
    let mut writes: Vec<((i32, i32, i32), u16)> = Vec::new();
    let mut cache = ChunkCache::new(world);
    match b {
        Block::Torch | Block::RedstoneTorchOn => {
            let m = torch_meta(hit.normal)?;
            writes.push(((tx, ty, tz), voxel(b, m)));
        }
        Block::Ladder => {
            let m = ladder_meta(hit.normal)?;
            writes.push(((tx, ty, tz), voxel(b, m)));
        }
        Block::Door => {
            let above = world.get(tx, ty + 1, tz);
            if !props(block::vox_id(above)).replaceable || ty + 1 >= 256 {
                return None;
            }
            if !cache.is_solid(tx, ty - 1, tz) {
                return None;
            }
            writes.push(((tx, ty, tz), voxel(b, facing)));
            writes.push(((tx, ty + 1, tz), voxel(b, facing | 8)));
        }
        Block::Bed => {
            let (dx, dz) = block::facing_offset(facing);
            let (hx, hz) = (tx + dx, tz + dz);
            let head = world.get(hx, ty, hz);
            if !props(block::vox_id(head)).replaceable {
                return None;
            }
            if !cache.is_solid(tx, ty - 1, tz) || !cache.is_solid(hx, ty - 1, hz) {
                return None;
            }
            writes.push(((tx, ty, tz), voxel(b, facing)));
            writes.push(((hx, ty, hz), voxel(b, facing | 4)));
        }
        Block::Wheat => {
            if cache.get_block(tx, ty - 1, tz) != Block::Farmland {
                return None;
            }
            writes.push(((tx, ty, tz), voxel(b, 0)));
        }
        Block::Button | Block::Lever => {
            let f = attach_face(hit.normal);
            writes.push(((tx, ty, tz), voxel(b, f)));
        }
        Block::Furnace | Block::Chest => {
            writes.push(((tx, ty, tz), voxel(b, (facing + 2) & 3)));
        }
        Block::Piston | Block::StickyPiston => {
            // face the piston toward the player: direction from block to player
            let d = player.eye() - Vec3::new(tx as f32 + 0.5, ty as f32 + 0.5, tz as f32 + 0.5);
            let dir = if d.y.abs() > d.x.abs().max(d.z.abs()) {
                if d.y > 0.0 { 3 } else { 2 }
            } else if d.x.abs() > d.z.abs() {
                if d.x > 0.0 { 1 } else { 0 }
            } else if d.z > 0.0 {
                5
            } else {
                4
            };
            writes.push(((tx, ty, tz), voxel(b, dir)));
        }
        Block::OakLog | Block::BirchLog | Block::SpruceLog => {
            let axis = match hit.normal {
                (1, 0, 0) | (-1, 0, 0) => 1,
                (0, 0, 1) | (0, 0, -1) => 2,
                _ => 0,
            };
            writes.push(((tx, ty, tz), voxel(b, axis)));
        }
        _ => {
            let v = voxel(b, 0);
            // support checks for plants etc.
            if !is_supported(&mut cache, (tx, ty, tz), v) {
                return None;
            }
            writes.push(((tx, ty, tz), v));
        }
    }
    // don't place solid blocks inside players
    if props(b.id()).solid {
        for (p, v) in &writes {
            if let Some(bb) = physics::block_aabb(*v, p.0, p.1, p.2) {
                for pb in ctx.player_boxes {
                    if bb.intersects(pb) {
                        return None;
                    }
                }
            }
        }
    }
    Some(writes)
}

/// Break the block at `pos` (drops, block-entity contents), returns the old voxel.
pub fn destroy_block(ctx: &mut Ctx, pos: (i32, i32, i32), drop_items: bool) -> Option<u16> {
    let world = ctx.world;
    let (x, y, z) = pos;
    let old = world.get(x, y, z);
    if old == 0 {
        return None;
    }
    let b = block::vox_block(old);
    let meta = block::vox_meta(old);
    let center = Vec3::new(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5);
    // block entity contents
    if let Some(be) = world.block_entity(x, y, z) {
        match be {
            crate::world::chunk::BlockEntity::Chest { items } => {
                for s in items {
                    if !s.is_empty() {
                        spawn_drop(ctx.drops, center, s, ctx.rng);
                    }
                }
            }
            crate::world::chunk::BlockEntity::Furnace(f) => {
                for s in [f.input, f.fuel, f.output].into_iter().flatten() {
                    if !s.is_empty() {
                        spawn_drop(ctx.drops, center, s, ctx.rng);
                    }
                }
            }
            _ => {}
        }
        world.set_block_entity(x, y, z, None);
    }
    world.set_block(x, y, z, 0);
    ctx.fluids.touch(world, x, y, z);
    if drop_items {
        if let Some(d) = block_drop(b, meta, ctx.rng) {
            spawn_drop(ctx.drops, center, d, ctx.rng);
        }
    }
    // doors: remove the other half
    if b == Block::Door {
        let oy = if meta & 8 != 0 { y - 1 } else { y + 1 };
        if world.get_block(x, oy, z) == Block::Door {
            world.set_block(x, oy, z, 0);
        }
    }
    if b == Block::Bed {
        let facing = meta & 3;
        let (dx, dz) = block::facing_offset(facing);
        let (ox, oz) = if meta & 4 != 0 { (x - dx, z - dz) } else { (x + dx, z + dz) };
        if world.get_block(ox, y, oz) == Block::Bed {
            world.set_block(ox, y, oz, 0);
        }
    }
    Some(old)
}

/// After a block changed at `pos`, pop neighbours that lost their support. Returns broken positions.
pub fn cascade_support(ctx: &mut Ctx, pos: (i32, i32, i32)) -> Vec<(i32, i32, i32)> {
    let mut broken = Vec::new();
    let mut stack = vec![pos];
    let mut guard = 0;
    while let Some((x, y, z)) = stack.pop() {
        guard += 1;
        if guard > 256 {
            break;
        }
        for (dx, dy, dz) in [(-1, 0, 0), (1, 0, 0), (0, -1, 0), (0, 1, 0), (0, 0, -1), (0, 0, 1)] {
            let n = (x + dx, y + dy, z + dz);
            let v = ctx.world.get(n.0, n.1, n.2);
            if v == 0 {
                continue;
            }
            let mut cache = ChunkCache::new(ctx.world);
            if !is_supported(&mut cache, n, v) {
                if destroy_block(ctx, n, true).is_some() {
                    broken.push(n);
                    stack.push(n);
                }
            }
        }
    }
    broken
}

/// Per-frame interaction update for one player.
pub fn update(ctx: &mut Ctx, player: &mut Player, input: &PlayerInput, dt: f32) -> Vec<Interaction> {
    let mut out = Vec::new();
    if player.dead || player.ui != OpenUi::None {
        player.breaking = None;
        return out;
    }
    // hotbar
    if let Some(h) = input.hotbar {
        player.inventory.selected = h.min(8);
        player.hotbar_changed_at = player.time;
    }
    if input.scroll != 0 {
        let n = 9i32;
        player.inventory.selected = ((player.inventory.selected as i32 - input.scroll).rem_euclid(n)) as usize;
        player.hotbar_changed_at = player.time;
    }
    // drop
    if input.drop {
        let held = player.inventory.held();
        if !held.is_empty() {
            let mut one = held;
            one.count = 1;
            player.inventory.consume_selected(1);
            throw_drop(ctx.drops, player.eye() + player.look_dir() * 0.3, player.look_dir(), one);
            player.swing = 0.0;
        }
    }
    let eye = player.eye();
    let dir = player.look_dir();
    let hit = {
        let mut cache = ChunkCache::new(ctx.world);
        raycast(&mut cache, eye, dir, player.reach(), false)
    };

    // eating
    let held = player.inventory.held();
    if input.use_held && held.is_food() && player.hunger < 20.0 && player.mode == GameMode::Survival {
        player.eating += dt;
        if player.eating >= 1.6 {
            player.eating = 0.0;
            if let ItemKind::Food { hunger, saturation } = held.props().kind {
                player.eat(hunger, saturation);
            }
            player.inventory.consume_selected(1);
            out.push(Interaction::Ate);
        }
        player.swing = 0.6;
        player.breaking = None;
        return out;
    } else {
        player.eating = 0.0;
    }

    // breaking
    if input.attack {
        if let Some(h) = hit {
            let v = ctx.world.get(h.pos.0, h.pos.1, h.pos.2);
            let b = block::vox_block(v);
            let (time, harvest) = break_time(b, &held);
            if player.mode == GameMode::Creative {
                if player.attack_cooldown <= 0.0 && props(b.id()).hardness >= 0.0 {
                    if destroy_block(ctx, h.pos, false).is_some() {
                        cascade_support(ctx, h.pos);
                        out.push(Interaction::Broke { pos: h.pos, block: b });
                    }
                    player.attack_cooldown = 0.25;
                }
                player.swing = 0.0;
            } else if time.is_finite() {
                let progress = match player.breaking {
                    Some((p, prog)) if p == h.pos => prog + dt / time,
                    _ => dt / time,
                };
                if progress >= 1.0 {
                    if destroy_block(ctx, h.pos, harvest).is_some() {
                        cascade_support(ctx, h.pos);
                        out.push(Interaction::Broke { pos: h.pos, block: b });
                        if held.tool_info().is_some() {
                            player.inventory.damage_held(1);
                        }
                        player.exhaustion += 0.005;
                    }
                    player.breaking = None;
                    player.attack_cooldown = 0.3;
                } else {
                    player.breaking = Some((h.pos, progress));
                    if player.attack_cooldown <= 0.0 {
                        out.push(Interaction::Hit { pos: h.pos, block: b });
                        player.attack_cooldown = 0.25;
                    }
                }
                player.swing = 0.0;
            }
        } else {
            player.breaking = None;
            if input.attack_pressed {
                player.swing = 0.0;
            }
        }
    } else {
        player.breaking = None;
    }

    // using
    if input.use_pressed && player.place_cooldown <= 0.0 {
        player.place_cooldown = 0.2;
        if let Some(h) = hit {
            let v = ctx.world.get(h.pos.0, h.pos.1, h.pos.2);
            let b = block::vox_block(v);
            let meta = block::vox_meta(v);
            if !player.sneaking {
                match b {
                    Block::CraftingTable => {
                        out.push(Interaction::OpenUi(OpenUi::CraftingTable));
                        return out;
                    }
                    Block::Chest => {
                        out.push(Interaction::OpenUi(OpenUi::Chest(h.pos)));
                        return out;
                    }
                    Block::Furnace | Block::FurnaceLit => {
                        out.push(Interaction::OpenUi(OpenUi::Furnace(h.pos)));
                        return out;
                    }
                    Block::Door => {
                        let open = meta ^ 4;
                        ctx.world.set_block(h.pos.0, h.pos.1, h.pos.2, voxel(b, open));
                        let oy = if meta & 8 != 0 { h.pos.1 - 1 } else { h.pos.1 + 1 };
                        let ov = ctx.world.get(h.pos.0, oy, h.pos.2);
                        if block::vox_block(ov) == Block::Door {
                            ctx.world.set_block(h.pos.0, oy, h.pos.2, voxel(b, block::vox_meta(ov) ^ 4));
                        }
                        out.push(Interaction::Toggled { pos: h.pos, block: b });
                        player.swing = 0.0;
                        return out;
                    }
                    Block::Lever => {
                        ctx.world.set_block(h.pos.0, h.pos.1, h.pos.2, voxel(b, meta ^ 8));
                        out.push(Interaction::Toggled { pos: h.pos, block: b });
                        player.swing = 0.0;
                        return out;
                    }
                    Block::Button => {
                        if meta & 8 == 0 {
                            ctx.world.set_block(h.pos.0, h.pos.1, h.pos.2, voxel(b, meta | 8));
                            out.push(Interaction::Toggled { pos: h.pos, block: b });
                        }
                        player.swing = 0.0;
                        return out;
                    }
                    Block::Bed => {
                        out.push(Interaction::Sleep { pos: h.pos });
                        return out;
                    }
                    Block::Tnt => {
                        if held.as_item() == Some(items::Item::Flint) || held.as_block() == Some(Block::Torch) {
                            out.push(Interaction::Explode { pos: h.pos });
                            return out;
                        }
                    }
                    _ => {}
                }
            }
            // place a block
            if let Some(pb) = items::placeable_block(&held) {
                if let Some(writes) = place_block(ctx, player, &h, pb) {
                    for (p, v) in &writes {
                        ctx.world.set_block(p.0, p.1, p.2, *v);
                        ctx.fluids.touch(ctx.world, p.0, p.1, p.2);
                    }
                    if player.mode == GameMode::Survival {
                        player.inventory.consume_selected(1);
                    }
                    out.push(Interaction::Placed { pos: writes[0].0, block: pb });
                    player.swing = 0.0;
                }
            }
        }
        // bow
        if held.as_item() == Some(items::Item::Bow) {
            let has_arrow = player.mode == GameMode::Creative || player.inventory.has(items::Item::Arrow.id(), 1);
            if has_arrow {
                if player.mode == GameMode::Survival {
                    player.inventory.take(items::Item::Arrow.id(), 1);
                    player.inventory.damage_held(1);
                }
                out.push(Interaction::ShootArrow { origin: eye, dir });
                player.swing = 0.0;
                player.place_cooldown = 0.5;
            }
        }
    }

    // pick block (creative)
    if input.pick_block && player.mode == GameMode::Creative {
        if let Some(h) = hit {
            let b = ctx.world.get_block(h.pos.0, h.pos.1, h.pos.2);
            if b != Block::Air {
                let item = match b {
                    Block::FurnaceLit => Block::Furnace,
                    Block::RedstoneLampLit => Block::RedstoneLamp,
                    Block::RedstoneTorchOff => Block::RedstoneTorchOn,
                    Block::PistonHead => Block::Piston,
                    _ => b,
                };
                let sel = player.inventory.selected;
                player.inventory.slots[sel] = ItemStack::block(item, 1);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::chunk::Chunk;
    use crate::world::World;

    fn flat_world() -> std::sync::Arc<World> {
        let w = World::new(0);
        for cz in -1..=1 {
            for cx in -1..=1 {
                let mut c = Chunk::new(cx, cz);
                for z in 0..16 {
                    for x in 0..16 {
                        for y in 0..10 {
                            c.set(x, y, z, voxel(if y == 9 { Block::Grass } else { Block::Dirt }, 0));
                        }
                    }
                }
                c.recompute_heightmap();
                crate::world::light::init_chunk_light(&mut c);
                w.insert_chunk(c);
            }
        }
        w
    }

    #[test]
    fn survival_player_breaks_and_places_blocks() {
        let world = flat_world();
        let mut fluids = FluidSim::new();
        let mut drops = Vec::new();
        let mut rng = Rng::new(1);
        let mut player = Player::new(0, "t", Vec3::new(8.5, 10.0, 8.5), GameMode::Survival);
        player.pitch = -1.2; // look down
        let boxes = vec![player.aabb()];
        let mut input = PlayerInput { attack: true, ..Default::default() };
        let mut broke = None;
        for _ in 0..200 {
            let mut ctx = Ctx { world: &world, fluids: &mut fluids, drops: &mut drops, rng: &mut rng, player_boxes: &boxes };
            for a in update(&mut ctx, &mut player, &input, 0.05) {
                if let Interaction::Broke { pos, block } = a {
                    broke = Some((pos, block));
                }
            }
            if broke.is_some() {
                break;
            }
        }
        let (pos, block) = broke.expect("block should break within 10 seconds");
        assert_eq!(block, Block::Grass);
        assert_eq!(world.get_block(pos.0, pos.1, pos.2), Block::Air);
        assert_eq!(drops.len(), 1);
        assert_eq!(drops[0].stack.as_block(), Some(Block::Dirt));
        // now place a cobblestone back into the hole
        player.inventory.slots[0] = ItemStack::block(Block::Cobblestone, 3);
        input = PlayerInput { use_pressed: true, ..Default::default() };
        player.place_cooldown = 0.0;
        let mut ctx = Ctx { world: &world, fluids: &mut fluids, drops: &mut drops, rng: &mut rng, player_boxes: &boxes };
        let acts = update(&mut ctx, &mut player, &input, 0.05);
        assert!(acts.iter().any(|a| matches!(a, Interaction::Placed { block: Block::Cobblestone, .. })));
        assert_eq!(world.get_block(pos.0, pos.1, pos.2), Block::Cobblestone);
        assert_eq!(player.inventory.slots[0].count, 2);
        // torches attach to walls and pop when the wall goes away
        player.inventory.slots[0] = ItemStack::block(Block::Torch, 5);
        player.pitch = 0.0;
        player.yaw = 0.0; // looking toward -Z
        player.pos = Vec3::new(8.5, 10.0, 8.5);
        world.set_block(8, 10, 6, voxel(Block::Stone, 0));
        world.set_block(8, 11, 6, voxel(Block::Stone, 0));
        player.place_cooldown = 0.0;
        let mut ctx = Ctx { world: &world, fluids: &mut fluids, drops: &mut drops, rng: &mut rng, player_boxes: &boxes };
        let acts = update(&mut ctx, &mut player, &input, 0.05);
        assert!(acts.iter().any(|a| matches!(a, Interaction::Placed { block: Block::Torch, .. })), "{acts:?}");
        assert_eq!(world.get_block(8, 11, 7), Block::Torch);
        assert_eq!(world.light_at(8, 11, 7).1, 14);
        let mut ctx = Ctx { world: &world, fluids: &mut fluids, drops: &mut drops, rng: &mut rng, player_boxes: &boxes };
        destroy_block(&mut ctx, (8, 11, 6), true);
        let popped = cascade_support(&mut ctx, (8, 11, 6));
        assert!(popped.contains(&(8, 11, 7)));
        assert_eq!(world.get_block(8, 11, 7), Block::Air);
        assert_eq!(world.light_at(8, 11, 7).1, 0);
    }

    #[test]
    fn break_times_respect_tools() {
        let hand = ItemStack::EMPTY;
        let (t_hand, harvest_hand) = break_time(Block::Stone, &hand);
        assert!(!harvest_hand);
        let pick = ItemStack::item(items::Item::WoodPickaxe, 1);
        let (t_pick, harvest_pick) = break_time(Block::Stone, &pick);
        assert!(harvest_pick);
        assert!(t_pick < t_hand);
        let (_, diamond_with_wood) = break_time(Block::DiamondOre, &pick);
        assert!(!diamond_with_wood);
        let iron = ItemStack::item(items::Item::IronPickaxe, 1);
        assert!(break_time(Block::DiamondOre, &iron).1);
        assert!(break_time(Block::Bedrock, &iron).0.is_infinite());
    }
}
