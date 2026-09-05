//! Container screens: player inventory (2x2 craft), crafting table (3x3), chest, furnace.
//! Drag-and-drop with left/right/shift clicks.

use crate::entity::{throw_drop, ItemDrop};
use crate::player::crafting;
use crate::player::furnace::{fuel_ticks, FurnaceState};
use crate::player::items::{ArmorSlot, ItemStack};
use crate::player::{OpenUi, Player};
use crate::render::atlas::Tile;
use crate::render::ui2d::UiBatch;
use crate::world::block::Block;
use crate::world::chunk::BlockEntity;
use crate::world::World;

pub const SLOT: f32 = 18.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotKind {
    Main(usize),
    Armor(usize),
    Craft(usize),
    CraftResult,
    Container(usize),
    FurnaceIn,
    FurnaceFuel,
    FurnaceOut,
}

#[derive(Clone, Copy, Debug)]
pub struct SlotDef {
    pub x: f32,
    pub y: f32,
    pub kind: SlotKind,
}

pub struct ScreenInput {
    pub mx: f32,
    pub my: f32,
    pub left: bool,
    pub right: bool,
    pub shift: bool,
}

fn inventory_slots(out: &mut Vec<SlotDef>, y_main: f32) {
    for i in 9..36 {
        let r = (i - 9) / 9;
        let c = (i - 9) % 9;
        out.push(SlotDef { x: 8.0 + c as f32 * SLOT, y: y_main + r as f32 * SLOT, kind: SlotKind::Main(i) });
    }
    for i in 0..9 {
        out.push(SlotDef { x: 8.0 + i as f32 * SLOT, y: y_main + 58.0, kind: SlotKind::Main(i) });
    }
}

/// Panel size and slot layout (relative to the panel's top-left).
pub fn layout(ui: OpenUi) -> (f32, f32, Vec<SlotDef>) {
    let mut s = Vec::new();
    match ui {
        OpenUi::Inventory => {
            for i in 0..4 {
                s.push(SlotDef { x: 8.0, y: 8.0 + i as f32 * SLOT, kind: SlotKind::Armor(i) });
            }
            // 2x2 grid uses craft indices 0,1,3,4
            for (k, idx) in [0usize, 1, 3, 4].iter().enumerate() {
                s.push(SlotDef { x: 98.0 + (k % 2) as f32 * SLOT, y: 18.0 + (k / 2) as f32 * SLOT, kind: SlotKind::Craft(*idx) });
            }
            s.push(SlotDef { x: 154.0, y: 28.0, kind: SlotKind::CraftResult });
            inventory_slots(&mut s, 84.0);
            (176.0, 166.0, s)
        }
        OpenUi::CraftingTable => {
            for i in 0..9 {
                s.push(SlotDef { x: 30.0 + (i % 3) as f32 * SLOT, y: 17.0 + (i / 3) as f32 * SLOT, kind: SlotKind::Craft(i) });
            }
            s.push(SlotDef { x: 124.0, y: 35.0, kind: SlotKind::CraftResult });
            inventory_slots(&mut s, 84.0);
            (176.0, 166.0, s)
        }
        OpenUi::Chest(_) => {
            for i in 0..27 {
                s.push(SlotDef { x: 8.0 + (i % 9) as f32 * SLOT, y: 18.0 + (i / 9) as f32 * SLOT, kind: SlotKind::Container(i) });
            }
            inventory_slots(&mut s, 86.0);
            (176.0, 168.0, s)
        }
        OpenUi::Furnace(_) => {
            s.push(SlotDef { x: 56.0, y: 17.0, kind: SlotKind::FurnaceIn });
            s.push(SlotDef { x: 56.0, y: 53.0, kind: SlotKind::FurnaceFuel });
            s.push(SlotDef { x: 116.0, y: 35.0, kind: SlotKind::FurnaceOut });
            inventory_slots(&mut s, 84.0);
            (176.0, 166.0, s)
        }
        _ => (0.0, 0.0, s),
    }
}

pub fn panel_origin(b: &UiBatch, pw: f32, ph: f32) -> (f32, f32) {
    (((b.width - pw) * 0.5).floor(), ((b.height - ph) * 0.5).floor())
}

fn container_items(world: &World, ui: OpenUi) -> Vec<ItemStack> {
    match ui {
        OpenUi::Chest(p) => match world.block_entity(p.0, p.1, p.2) {
            Some(BlockEntity::Chest { items }) => {
                let mut v = items;
                v.resize(27, ItemStack::EMPTY);
                v
            }
            _ => vec![ItemStack::EMPTY; 27],
        },
        _ => Vec::new(),
    }
}

fn furnace_state(world: &World, ui: OpenUi) -> FurnaceState {
    if let OpenUi::Furnace(p) = ui {
        if let Some(BlockEntity::Furnace(f)) = world.block_entity(p.0, p.1, p.2) {
            return f;
        }
    }
    FurnaceState::default()
}

fn craft_grid(player: &Player, ui: OpenUi) -> (Vec<ItemStack>, usize) {
    match ui {
        OpenUi::Inventory => (vec![player.inventory.craft[0], player.inventory.craft[1], player.inventory.craft[3], player.inventory.craft[4]], 2),
        _ => (player.inventory.craft.clone(), 3),
    }
}

pub fn craft_result(player: &Player, ui: OpenUi) -> Option<ItemStack> {
    let (grid, gw) = craft_grid(player, ui);
    crafting::find_match(&grid, gw)
}

fn get_slot(world: &World, player: &Player, ui: OpenUi, k: SlotKind) -> ItemStack {
    match k {
        SlotKind::Main(i) => player.inventory.slots[i],
        SlotKind::Armor(i) => player.inventory.armor[i],
        SlotKind::Craft(i) => player.inventory.craft[i],
        SlotKind::CraftResult => craft_result(player, ui).unwrap_or(ItemStack::EMPTY),
        SlotKind::Container(i) => container_items(world, ui).get(i).copied().unwrap_or(ItemStack::EMPTY),
        SlotKind::FurnaceIn => furnace_state(world, ui).input.unwrap_or(ItemStack::EMPTY),
        SlotKind::FurnaceFuel => furnace_state(world, ui).fuel.unwrap_or(ItemStack::EMPTY),
        SlotKind::FurnaceOut => furnace_state(world, ui).output.unwrap_or(ItemStack::EMPTY),
    }
}

fn set_slot(world: &World, player: &mut Player, ui: OpenUi, k: SlotKind, v: ItemStack) {
    let v = if v.is_empty() { ItemStack::EMPTY } else { v };
    match k {
        SlotKind::Main(i) => player.inventory.slots[i] = v,
        SlotKind::Armor(i) => player.inventory.armor[i] = v,
        SlotKind::Craft(i) => player.inventory.craft[i] = v,
        SlotKind::CraftResult => {}
        SlotKind::Container(i) => {
            if let OpenUi::Chest(p) = ui {
                let changed = world.with_block_entity(p.0, p.1, p.2, |be| {
                    if let BlockEntity::Chest { items } = be {
                        items.resize(27, ItemStack::EMPTY);
                        items[i] = v;
                    }
                });
                if changed.is_none() {
                    let mut items = vec![ItemStack::EMPTY; 27];
                    items[i] = v;
                    world.set_block_entity(p.0, p.1, p.2, Some(BlockEntity::Chest { items }));
                }
            }
        }
        SlotKind::FurnaceIn | SlotKind::FurnaceFuel | SlotKind::FurnaceOut => {
            if let OpenUi::Furnace(p) = ui {
                let opt = if v.is_empty() { None } else { Some(v) };
                let r = world.with_block_entity(p.0, p.1, p.2, |be| {
                    if let BlockEntity::Furnace(f) = be {
                        match k {
                            SlotKind::FurnaceIn => f.input = opt,
                            SlotKind::FurnaceFuel => f.fuel = opt,
                            _ => f.output = opt,
                        }
                    }
                });
                if r.is_none() {
                    let mut f = FurnaceState::default();
                    match k {
                        SlotKind::FurnaceIn => f.input = opt,
                        SlotKind::FurnaceFuel => f.fuel = opt,
                        _ => f.output = opt,
                    }
                    world.set_block_entity(p.0, p.1, p.2, Some(BlockEntity::Furnace(f)));
                }
            }
        }
    }
}

/// Can `stack` be put into this slot?
fn accepts(k: SlotKind, stack: &ItemStack) -> bool {
    match k {
        SlotKind::CraftResult | SlotKind::FurnaceOut => false,
        SlotKind::Armor(i) => stack.armor_slot().map(|s| s as usize == i).unwrap_or(false),
        SlotKind::FurnaceFuel => fuel_ticks(stack.id) > 0,
        _ => true,
    }
}

fn take_craft_result(world: &World, player: &mut Player, ui: OpenUi) -> Option<ItemStack> {
    let result = craft_result(player, ui)?;
    let _ = world;
    match ui {
        OpenUi::Inventory => {
            let mut g = vec![player.inventory.craft[0], player.inventory.craft[1], player.inventory.craft[3], player.inventory.craft[4]];
            crafting::consume(&mut g);
            player.inventory.craft[0] = g[0];
            player.inventory.craft[1] = g[1];
            player.inventory.craft[3] = g[2];
            player.inventory.craft[4] = g[3];
        }
        _ => crafting::consume(&mut player.inventory.craft),
    }
    Some(result)
}

/// Quick-move (shift-click) a slot's content to the "other" section.
fn quick_move(world: &World, player: &mut Player, ui: OpenUi, k: SlotKind) {
    let stack = get_slot(world, player, ui, k);
    if stack.is_empty() {
        return;
    }
    match k {
        SlotKind::CraftResult => {
            for _ in 0..64 {
                let Some(r) = craft_result(player, ui) else { break };
                if player.inventory.add(r).is_empty() {
                    take_craft_result(world, player, ui);
                } else {
                    break;
                }
            }
        }
        SlotKind::Main(i) => {
            // armour auto-equip
            if let Some(slot) = stack.armor_slot() {
                let ai = slot as usize;
                if player.inventory.armor[ai].is_empty() {
                    player.inventory.armor[ai] = stack;
                    player.inventory.slots[i] = ItemStack::EMPTY;
                    return;
                }
            }
            match ui {
                OpenUi::Chest(_) => {
                    let mut items = container_items(world, ui);
                    let rem = merge_into(&mut items, stack);
                    for (j, it) in items.iter().enumerate() {
                        set_slot(world, player, ui, SlotKind::Container(j), *it);
                    }
                    player.inventory.slots[i] = rem;
                }
                OpenUi::Furnace(_) => {
                    let target = if fuel_ticks(stack.id) > 0 && crate::player::furnace::smelt_result(stack.id).is_none() { SlotKind::FurnaceFuel } else { SlotKind::FurnaceIn };
                    let cur = get_slot(world, player, ui, target);
                    if cur.is_empty() {
                        set_slot(world, player, ui, target, stack);
                        player.inventory.slots[i] = ItemStack::EMPTY;
                    } else if cur.can_merge(&stack) {
                        let n = (cur.max_stack() - cur.count).min(stack.count);
                        let mut c = cur;
                        c.count += n;
                        set_slot(world, player, ui, target, c);
                        let mut s = stack;
                        s.count -= n;
                        player.inventory.slots[i] = if s.count == 0 { ItemStack::EMPTY } else { s };
                    }
                }
                _ => {
                    // hotbar <-> main swap
                    let range = if i < 9 { 9..36 } else { 0..9 };
                    let mut rest = stack;
                    for j in range.clone() {
                        let s = &mut player.inventory.slots[j];
                        if !s.is_empty() && s.can_merge(&rest) && s.count < s.max_stack() {
                            let n = (s.max_stack() - s.count).min(rest.count);
                            s.count += n;
                            rest.count -= n;
                            if rest.count == 0 {
                                break;
                            }
                        }
                    }
                    if rest.count > 0 {
                        for j in range {
                            if player.inventory.slots[j].is_empty() {
                                player.inventory.slots[j] = rest;
                                rest = ItemStack::EMPTY;
                                break;
                            }
                        }
                    }
                    player.inventory.slots[i] = if rest.count == 0 { ItemStack::EMPTY } else { rest };
                }
            }
        }
        _ => {
            let rem = player.inventory.add(stack);
            set_slot(world, player, ui, k, rem);
        }
    }
}

fn merge_into(items: &mut [ItemStack], mut stack: ItemStack) -> ItemStack {
    let max = stack.max_stack();
    for s in items.iter_mut() {
        if !s.is_empty() && s.can_merge(&stack) && s.count < max {
            let n = (max - s.count).min(stack.count);
            s.count += n;
            stack.count -= n;
            if stack.count == 0 {
                return ItemStack::EMPTY;
            }
        }
    }
    for s in items.iter_mut() {
        if s.is_empty() {
            *s = stack;
            return ItemStack::EMPTY;
        }
    }
    stack
}

/// Handle one frame of screen input. Returns true if the screen should close (click outside etc.).
pub fn update(world: &World, player: &mut Player, input: &ScreenInput, b: &UiBatch, drops: &mut Vec<ItemDrop>) {
    let ui = player.ui;
    let (pw, ph, slots) = layout(ui);
    let (ox, oy) = panel_origin(b, pw, ph);
    if !(input.left || input.right) {
        return;
    }
    let hovered = slots.iter().find(|s| input.mx >= ox + s.x && input.mx < ox + s.x + SLOT && input.my >= oy + s.y && input.my < oy + s.y + SLOT).copied();
    let Some(slot) = hovered else {
        // click outside the panel with an item in hand: throw it
        let inside = input.mx >= ox && input.mx < ox + pw && input.my >= oy && input.my < oy + ph;
        if !inside && !player.inventory.cursor.is_empty() {
            let dir = player.look_dir();
            if input.left {
                throw_drop(drops, player.eye() + dir * 0.3, dir, player.inventory.cursor);
                player.inventory.cursor = ItemStack::EMPTY;
            } else {
                let mut one = player.inventory.cursor;
                one.count = 1;
                throw_drop(drops, player.eye() + dir * 0.3, dir, one);
                player.inventory.cursor.count -= 1;
                if player.inventory.cursor.count == 0 {
                    player.inventory.cursor = ItemStack::EMPTY;
                }
            }
        }
        return;
    };
    let k = slot.kind;
    let cursor = player.inventory.cursor;
    if input.shift && input.left {
        quick_move(world, player, ui, k);
        return;
    }
    if k == SlotKind::CraftResult {
        let Some(r) = craft_result(player, ui) else { return };
        if cursor.is_empty() {
            if let Some(taken) = take_craft_result(world, player, ui) {
                player.inventory.cursor = taken;
            }
        } else if cursor.can_merge(&r) && cursor.count as u32 + r.count as u32 <= cursor.max_stack() as u32 {
            if take_craft_result(world, player, ui).is_some() {
                player.inventory.cursor.count += r.count;
            }
        }
        return;
    }
    let in_slot = get_slot(world, player, ui, k);
    if input.left {
        if cursor.is_empty() {
            if !in_slot.is_empty() {
                player.inventory.cursor = in_slot;
                set_slot(world, player, ui, k, ItemStack::EMPTY);
            }
        } else if in_slot.is_empty() {
            if accepts(k, &cursor) {
                set_slot(world, player, ui, k, cursor);
                player.inventory.cursor = ItemStack::EMPTY;
            }
        } else if in_slot.can_merge(&cursor) {
            let n = (in_slot.max_stack() - in_slot.count).min(cursor.count);
            let mut s = in_slot;
            s.count += n;
            set_slot(world, player, ui, k, s);
            player.inventory.cursor.count -= n;
            if player.inventory.cursor.count == 0 {
                player.inventory.cursor = ItemStack::EMPTY;
            }
        } else if accepts(k, &cursor) {
            set_slot(world, player, ui, k, cursor);
            player.inventory.cursor = in_slot;
        }
    } else if input.right {
        if cursor.is_empty() {
            if !in_slot.is_empty() {
                let half = (in_slot.count + 1) / 2;
                let mut taken = in_slot;
                taken.count = half;
                let mut left = in_slot;
                left.count -= half;
                player.inventory.cursor = taken;
                set_slot(world, player, ui, k, left);
            }
        } else if accepts(k, &cursor) {
            if in_slot.is_empty() {
                let mut one = cursor;
                one.count = 1;
                set_slot(world, player, ui, k, one);
                player.inventory.cursor.count -= 1;
            } else if in_slot.can_merge(&cursor) && in_slot.count < in_slot.max_stack() {
                let mut s = in_slot;
                s.count += 1;
                set_slot(world, player, ui, k, s);
                player.inventory.cursor.count -= 1;
            }
            if player.inventory.cursor.count == 0 {
                player.inventory.cursor = ItemStack::EMPTY;
            }
        }
    }
}

/// Called when the screen closes: craft grid + cursor go back to the inventory (or drop).
pub fn close(player: &mut Player, drops: &mut Vec<ItemDrop>) {
    let mut leftovers = Vec::new();
    for s in player.inventory.craft.iter_mut() {
        if !s.is_empty() {
            leftovers.push(*s);
            *s = ItemStack::EMPTY;
        }
    }
    if !player.inventory.cursor.is_empty() {
        leftovers.push(player.inventory.cursor);
        player.inventory.cursor = ItemStack::EMPTY;
    }
    for s in leftovers {
        let rem = player.inventory.add(s);
        if !rem.is_empty() {
            throw_drop(drops, player.eye(), player.look_dir(), rem);
        }
    }
    player.ui = OpenUi::None;
}

pub fn draw(b: &mut UiBatch, world: &World, player: &Player, mx: f32, my: f32) {
    let ui = player.ui;
    let (pw, ph, slots) = layout(ui);
    if slots.is_empty() {
        return;
    }
    b.rect(0.0, 0.0, b.width, b.height, [0.0, 0.0, 0.0, 0.5]);
    let (ox, oy) = panel_origin(b, pw, ph);
    b.panel(ox, oy, pw, ph);
    let title = match ui {
        OpenUi::Inventory => "Inventory",
        OpenUi::CraftingTable => "Crafting",
        OpenUi::Chest(_) => "Chest",
        OpenUi::Furnace(_) => "Furnace",
        _ => "",
    };
    if ui != OpenUi::Inventory {
        b.text(ox + 8.0, oy + 6.0, 1.0, title, [0.25, 0.25, 0.25, 1.0]);
    }
    let inv_title_y = match ui {
        OpenUi::Chest(_) => 75.0,
        _ => 73.0,
    };
    b.text(ox + 8.0, oy + inv_title_y, 1.0, "Inventory", [0.25, 0.25, 0.25, 1.0]);
    // decorations
    match ui {
        OpenUi::Inventory => {
            b.text(ox + 98.0, oy + 8.0, 1.0, "Craft", [0.25, 0.25, 0.25, 1.0]);
            b.tile(ox + 136.0, oy + 30.0, 14.0, 14.0, Tile::ArrowRight, [0.3, 0.3, 0.3, 1.0]);
            // player preview: face + shirt
            b.rect(ox + 30.0, oy + 8.0, 50.0, 62.0, [0.1, 0.1, 0.1, 1.0]);
            b.tile(ox + 45.0, oy + 11.0, 20.0, 20.0, Tile::PlayerFace, [1.0; 4]);
            b.tile(ox + 45.0, oy + 31.0, 20.0, 26.0, Tile::PlayerShirt, [1.0; 4]);
            b.tile(ox + 45.0, oy + 57.0, 20.0, 11.0, Tile::PlayerPants, [1.0; 4]);
        }
        OpenUi::CraftingTable => {
            b.tile(ox + 92.0, oy + 36.0, 16.0, 16.0, Tile::ArrowRight, [0.3, 0.3, 0.3, 1.0]);
        }
        OpenUi::Furnace(_) => {
            let f = furnace_state(world, ui);
            // flame
            let burn = f.burn_frac();
            b.tile_part(ox + 57.0, oy + 37.0 + 14.0 * (1.0 - burn), 14.0, 14.0 * burn, Tile::FurnaceFrontLit, [0.25, 0.5 + 0.5 * (1.0 - burn), 0.75, 1.0], [1.0, 0.4, 0.4, 0.4]);
            b.rect_outline(ox + 56.0, oy + 36.0, 16.0, 16.0, 1.0, [0.35, 0.35, 0.35, 1.0]);
            // progress arrow
            let prog = f.progress_frac();
            b.rect(ox + 79.0, oy + 40.0, 24.0, 6.0, [0.45, 0.45, 0.45, 1.0]);
            b.rect(ox + 79.0, oy + 40.0, 24.0 * prog, 6.0, [1.0, 1.0, 1.0, 1.0]);
        }
        _ => {}
    }
    let mut hovered: Option<ItemStack> = None;
    for s in &slots {
        let x = ox + s.x;
        let y = oy + s.y;
        b.slot(x, y, SLOT);
        let stack = get_slot(world, player, ui, s.kind);
        if stack.is_empty() {
            let ghost = match s.kind {
                SlotKind::Armor(0) => Some(Tile::IronHelmet),
                SlotKind::Armor(1) => Some(Tile::IronChest),
                SlotKind::Armor(2) => Some(Tile::IronLegs),
                SlotKind::Armor(3) => Some(Tile::IronBoots),
                _ => None,
            };
            if let Some(t) = ghost {
                b.tile(x + 1.0, y + 1.0, 16.0, 16.0, t, [0.4, 0.4, 0.4, 0.5]);
            }
        }
        b.item(x + 1.0, y + 1.0, 16.0, &stack);
        let over = mx >= x && mx < x + SLOT && my >= y && my < y + SLOT;
        if over {
            b.rect(x + 1.0, y + 1.0, SLOT - 2.0, SLOT - 2.0, [1.0, 1.0, 1.0, 0.35]);
            if !stack.is_empty() {
                hovered = Some(stack);
            }
        }
    }
    // cursor item
    let cur = player.inventory.cursor;
    if !cur.is_empty() {
        b.item(mx - 8.0, my - 8.0, 16.0, &cur);
    } else if let Some(h) = hovered {
        let name = h.name();
        let tw = crate::ui::font::text_width(name, 1.0);
        let tx = (mx + 10.0).min(b.width - tw - 6.0);
        let ty = my - 14.0;
        b.rect(tx - 3.0, ty - 3.0, tw + 6.0, 13.0, [0.05, 0.0, 0.1, 0.92]);
        b.rect_outline(tx - 3.0, ty - 3.0, tw + 6.0, 13.0, 1.0, [0.35, 0.2, 0.6, 1.0]);
        b.text(tx, ty, 1.0, name, [1.0; 4]);
    }
    let _ = ArmorSlot::Helmet;
    let _ = Block::Air;
}
