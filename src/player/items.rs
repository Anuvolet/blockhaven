//! Item registry and item stacks. Block items share the block id (0..255); other items start at 256.

use crate::render::atlas::Tile;
use crate::world::block::{self, Block, Tool};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

pub type ItemId = u16;

macro_rules! items {
    ($($name:ident = $id:expr),* $(,)?) => {
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
        #[repr(u16)]
        pub enum Item { $($name = $id),* }
        impl Item {
            pub const ALL: &'static [Item] = &[$(Item::$name),*];
            pub fn from_id(id: u16) -> Option<Item> {
                match id { $($id => Some(Item::$name),)* _ => None }
            }
            #[inline]
            pub fn id(self) -> ItemId { self as u16 }
        }
    };
}

items! {
    Stick = 256, Coal = 257, IronIngot = 258, GoldIngot = 259, Diamond = 260, Redstone = 261,
    Wheat = 262, Bread = 263, PorkchopRaw = 264, PorkchopCooked = 265, BeefRaw = 266,
    BeefCooked = 267, ChickenRaw = 268, ChickenCooked = 269, Leather = 270, Feather = 271,
    Arrow = 272, Bone = 273, RottenFlesh = 274, Gunpowder = 275, String = 276, Apple = 277,
    Egg = 278, Bow = 279, ClayBall = 280, Brick = 281, Flint = 282,
    WoodPickaxe = 300, WoodAxe = 301, WoodShovel = 302, WoodSword = 303, WoodHoe = 304,
    StonePickaxe = 305, StoneAxe = 306, StoneShovel = 307, StoneSword = 308, StoneHoe = 309,
    IronPickaxe = 310, IronAxe = 311, IronShovel = 312, IronSword = 313, IronHoe = 314,
    GoldPickaxe = 315, GoldAxe = 316, GoldShovel = 317, GoldSword = 318, GoldHoe = 319,
    DiamondPickaxe = 320, DiamondAxe = 321, DiamondShovel = 322, DiamondSword = 323, DiamondHoe = 324,
    LeatherHelmet = 330, LeatherChest = 331, LeatherLegs = 332, LeatherBoots = 333,
    IronHelmet = 334, IronChest = 335, IronLegs = 336, IronBoots = 337,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ArmorSlot {
    Helmet = 0,
    Chest = 1,
    Legs = 2,
    Boots = 3,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ItemKind {
    Block(Block),
    Material,
    Tool { tool: Tool, tier: u8, speed: f32, durability: u16, damage: f32 },
    Food { hunger: u8, saturation: f32 },
    Armor { slot: ArmorSlot, defense: u8, durability: u16 },
}

#[derive(Clone, Copy, Debug)]
pub struct ItemProps {
    pub name: &'static str,
    pub tile: Tile,
    pub max_stack: u8,
    pub kind: ItemKind,
    /// Furnace fuel burn time in ticks (0 = not fuel).
    pub fuel: u32,
}

fn tool(name: &'static str, tile: Tile, t: Tool, tier: u8) -> ItemProps {
    let (speed, dur, dmg) = match tier {
        0 => (2.0, 59, 1.0),
        1 => (4.0, 131, 2.0),
        2 => (6.0, 250, 3.0),
        3 => (12.0, 32, 2.0),
        _ => (8.0, 1561, 4.0),
    };
    let dmg = if t == Tool::Sword { dmg + 3.0 } else { dmg };
    ItemProps { name, tile, max_stack: 1, kind: ItemKind::Tool { tool: t, tier: if tier == 3 { 0 } else { tier }, speed, durability: dur, damage: dmg }, fuel: if tier == 0 { 200 } else { 0 } }
}
fn food(name: &'static str, tile: Tile, hunger: u8, sat: f32) -> ItemProps {
    ItemProps { name, tile, max_stack: 64, kind: ItemKind::Food { hunger, saturation: sat }, fuel: 0 }
}
fn mat(name: &'static str, tile: Tile) -> ItemProps {
    ItemProps { name, tile, max_stack: 64, kind: ItemKind::Material, fuel: 0 }
}
fn armor(name: &'static str, tile: Tile, slot: ArmorSlot, defense: u8, durability: u16) -> ItemProps {
    ItemProps { name, tile, max_stack: 1, kind: ItemKind::Armor { slot, defense, durability }, fuel: 0 }
}

fn build() -> Vec<ItemProps> {
    use Item::*;
    let mut v = Vec::with_capacity(512);
    // block items
    for id in 0..256u16 {
        let b = Block::from_id(id as u8);
        let p = block::props(id as u8);
        let tile = match b {
            Block::Door => Tile::DoorItem,
            Block::Bed => Tile::BedItem,
            Block::Torch => Tile::TorchItem,
            Block::RedstoneTorchOn | Block::RedstoneTorchOff => Tile::RedstoneTorchOn,
            Block::Ladder => Tile::LadderItem,
            _ => block::face_tiles(b, 0)[5],
        };
        let fuel = match b {
            Block::OakPlanks | Block::BirchPlanks | Block::SprucePlanks => 300,
            Block::OakLog | Block::BirchLog | Block::SpruceLog => 300,
            Block::CraftingTable | Block::Chest | Block::Bookshelf => 300,
            Block::Ladder => 300,
            _ => 0,
        };
        v.push(ItemProps { name: if id == 0 { "" } else { p.name }, tile, max_stack: if matches!(b, Block::Bed | Block::Door) { 1 } else { 64 }, kind: ItemKind::Block(b), fuel });
    }
    let dummy = ItemProps { name: "", tile: Tile::White, max_stack: 64, kind: ItemKind::Material, fuel: 0 };
    v.resize(512, dummy);
    let set = |v: &mut Vec<ItemProps>, i: Item, p: ItemProps| v[i.id() as usize] = p;
    set(&mut v, Stick, ItemProps { fuel: 100, ..mat("stick", Tile::Stick) });
    set(&mut v, Coal, ItemProps { fuel: 1600, ..mat("coal", Tile::Coal) });
    set(&mut v, IronIngot, mat("iron ingot", Tile::IronIngot));
    set(&mut v, GoldIngot, mat("gold ingot", Tile::GoldIngot));
    set(&mut v, Diamond, mat("diamond", Tile::Diamond));
    set(&mut v, Redstone, mat("redstone", Tile::Redstone));
    set(&mut v, Wheat, mat("wheat", Tile::Wheat));
    set(&mut v, Bread, food("bread", Tile::Bread, 5, 6.0));
    set(&mut v, PorkchopRaw, food("raw porkchop", Tile::PorkchopRaw, 3, 1.8));
    set(&mut v, PorkchopCooked, food("cooked porkchop", Tile::PorkchopCooked, 8, 12.8));
    set(&mut v, BeefRaw, food("raw beef", Tile::BeefRaw, 3, 1.8));
    set(&mut v, BeefCooked, food("steak", Tile::BeefCooked, 8, 12.8));
    set(&mut v, ChickenRaw, food("raw chicken", Tile::ChickenRaw, 2, 1.2));
    set(&mut v, ChickenCooked, food("cooked chicken", Tile::ChickenCooked, 6, 7.2));
    set(&mut v, Leather, mat("leather", Tile::Leather));
    set(&mut v, Feather, mat("feather", Tile::Feather));
    set(&mut v, Arrow, mat("arrow", Tile::Arrow));
    set(&mut v, Bone, mat("bone", Tile::Bone));
    set(&mut v, RottenFlesh, food("rotten flesh", Tile::RottenFlesh, 4, 0.8));
    set(&mut v, Gunpowder, mat("gunpowder", Tile::Gunpowder));
    set(&mut v, String, mat("string", Tile::String));
    set(&mut v, Apple, food("apple", Tile::Apple, 4, 2.4));
    set(&mut v, Egg, ItemProps { max_stack: 16, ..mat("egg", Tile::Egg) });
    set(&mut v, Bow, ItemProps { max_stack: 1, ..mat("bow", Tile::Bow) });
    set(&mut v, ClayBall, mat("clay", Tile::ClayBall));
    set(&mut v, Brick, mat("brick", Tile::Brick));
    set(&mut v, Flint, mat("flint", Tile::Flint));
    let tiers: [(&str, u8, [Tile; 5]); 5] = [
        ("wooden", 0, [Tile::WoodPickaxe, Tile::WoodAxe, Tile::WoodShovel, Tile::WoodSword, Tile::WoodHoe]),
        ("stone", 1, [Tile::StonePickaxe, Tile::StoneAxe, Tile::StoneShovel, Tile::StoneSword, Tile::StoneHoe]),
        ("iron", 2, [Tile::IronPickaxe, Tile::IronAxe, Tile::IronShovel, Tile::IronSword, Tile::IronHoe]),
        ("golden", 3, [Tile::GoldPickaxe, Tile::GoldAxe, Tile::GoldShovel, Tile::GoldSword, Tile::GoldHoe]),
        ("diamond", 4, [Tile::DiamondPickaxe, Tile::DiamondAxe, Tile::DiamondShovel, Tile::DiamondSword, Tile::DiamondHoe]),
    ];
    let names = ["pickaxe", "axe", "shovel", "sword", "hoe"];
    let tools = [Tool::Pickaxe, Tool::Axe, Tool::Shovel, Tool::Sword, Tool::Hoe];
    for (ti, (tname, tier, tiles)) in tiers.iter().enumerate() {
        for k in 0..5 {
            let id = 300 + ti * 5 + k;
            let full: &'static str = Box::leak(format!("{} {}", tname, names[k]).into_boxed_str());
            v[id] = tool(full, tiles[k], tools[k], *tier);
        }
    }
    set(&mut v, LeatherHelmet, armor("leather cap", Tile::LeatherHelmet, ArmorSlot::Helmet, 1, 55));
    set(&mut v, LeatherChest, armor("leather tunic", Tile::LeatherChest, ArmorSlot::Chest, 3, 80));
    set(&mut v, LeatherLegs, armor("leather pants", Tile::LeatherLegs, ArmorSlot::Legs, 2, 75));
    set(&mut v, LeatherBoots, armor("leather boots", Tile::LeatherBoots, ArmorSlot::Boots, 1, 65));
    set(&mut v, IronHelmet, armor("iron helmet", Tile::IronHelmet, ArmorSlot::Helmet, 2, 165));
    set(&mut v, IronChest, armor("iron chestplate", Tile::IronChest, ArmorSlot::Chest, 6, 240));
    set(&mut v, IronLegs, armor("iron leggings", Tile::IronLegs, ArmorSlot::Legs, 5, 225));
    set(&mut v, IronBoots, armor("iron boots", Tile::IronBoots, ArmorSlot::Boots, 2, 195));
    v
}

static PROPS: OnceLock<Vec<ItemProps>> = OnceLock::new();

#[inline]
pub fn props(id: ItemId) -> &'static ItemProps {
    let t = PROPS.get_or_init(build);
    &t[(id as usize).min(t.len() - 1)]
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ItemStack {
    pub id: ItemId,
    pub count: u8,
    /// Damage taken (tools / armour).
    pub damage: u16,
}

impl ItemStack {
    pub const EMPTY: ItemStack = ItemStack { id: 0, count: 0, damage: 0 };
    pub fn new(id: ItemId, count: u8) -> ItemStack {
        ItemStack { id, count, damage: 0 }
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.count == 0 || self.id == 0
    }
    pub fn block(b: Block, count: u8) -> ItemStack {
        ItemStack::new(b.id() as u16, count)
    }
    pub fn item(i: Item, count: u8) -> ItemStack {
        ItemStack::new(i.id(), count)
    }
    pub fn props(&self) -> &'static ItemProps {
        props(self.id)
    }
    pub fn name(&self) -> &'static str {
        self.props().name
    }
    pub fn max_stack(&self) -> u8 {
        self.props().max_stack
    }
    pub fn as_block(&self) -> Option<Block> {
        if self.id < 256 && self.id != 0 {
            Some(Block::from_id(self.id as u8))
        } else {
            None
        }
    }
    pub fn as_item(&self) -> Option<Item> {
        Item::from_id(self.id)
    }
    pub fn can_merge(&self, other: &ItemStack) -> bool {
        self.id == other.id && self.damage == other.damage && self.max_stack() > 1
    }
    pub fn tool_info(&self) -> Option<(Tool, u8, f32, u16, f32)> {
        match self.props().kind {
            ItemKind::Tool { tool, tier, speed, durability, damage } => Some((tool, tier, speed, durability, damage)),
            _ => None,
        }
    }
    pub fn max_durability(&self) -> u16 {
        match self.props().kind {
            ItemKind::Tool { durability, .. } => durability,
            ItemKind::Armor { durability, .. } => durability,
            _ => 0,
        }
    }
    pub fn is_food(&self) -> bool {
        matches!(self.props().kind, ItemKind::Food { .. })
    }
    pub fn armor_slot(&self) -> Option<ArmorSlot> {
        match self.props().kind {
            ItemKind::Armor { slot, .. } => Some(slot),
            _ => None,
        }
    }
    pub fn attack_damage(&self) -> f32 {
        match self.props().kind {
            ItemKind::Tool { damage, .. } => damage,
            _ => 1.0,
        }
    }
}

/// The block a placed item stack becomes (None for non-placeable items).
pub fn placeable_block(stack: &ItemStack) -> Option<Block> {
    stack.as_block()
}

/// Which item stack a mined block drops (None = nothing).
pub fn block_drop(b: Block, meta: u8, rng: &mut crate::world::noise::Rng) -> Option<ItemStack> {
    let _ = meta;
    match b {
        Block::Air | Block::Water | Block::Lava | Block::Bedrock | Block::PistonHead | Block::Spawner => None,
        Block::Stone => Some(ItemStack::block(Block::Cobblestone, 1)),
        Block::Grass | Block::SnowyGrass | Block::Podzol | Block::Farmland => Some(ItemStack::block(Block::Dirt, 1)),
        Block::CoalOre => Some(ItemStack::item(Item::Coal, 1)),
        Block::DiamondOre => Some(ItemStack::item(Item::Diamond, 1)),
        Block::RedstoneOre => Some(ItemStack::item(Item::Redstone, 3 + rng.below(3) as u8)),
        Block::OakLeaves | Block::BirchLeaves | Block::SpruceLeaves => {
            if rng.chance(0.05) {
                Some(ItemStack::item(Item::Apple, 1))
            } else {
                None
            }
        }
        Block::Glass | Block::Ice => None,
        Block::TallGrass => {
            if rng.chance(0.15) {
                Some(ItemStack::item(Item::Wheat, 1))
            } else {
                None
            }
        }
        Block::DeadBush => Some(ItemStack::item(Item::Stick, 1 + rng.below(2) as u8)),
        Block::Snow => None,
        Block::Clay => Some(ItemStack::item(Item::ClayBall, 4)),
        Block::Wheat => {
            if meta >= 7 {
                Some(ItemStack::item(Item::Wheat, 1 + rng.below(3) as u8))
            } else {
                None
            }
        }
        Block::RedstoneDust => Some(ItemStack::item(Item::Redstone, 1)),
        Block::RedstoneTorchOff => Some(ItemStack::block(Block::RedstoneTorchOn, 1)),
        Block::RedstoneLampLit => Some(ItemStack::block(Block::RedstoneLamp, 1)),
        Block::FurnaceLit => Some(ItemStack::block(Block::Furnace, 1)),
        Block::Glowstone => Some(ItemStack::block(Block::Glowstone, 1)),
        Block::Bookshelf => Some(ItemStack::block(Block::Bookshelf, 1)),
        Block::MushroomStem | Block::RedMushroomBlock | Block::BrownMushroomBlock => {
            if rng.chance(0.3) {
                Some(ItemStack::block(if b == Block::RedMushroomBlock { Block::RedMushroom } else { Block::BrownMushroom }, 1))
            } else {
                None
            }
        }
        _ => Some(ItemStack::block(b, 1)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_props_are_consistent() {
        for it in Item::ALL {
            let p = props(it.id());
            assert!(!p.name.is_empty(), "item {:?} has no name", it);
            assert!(p.max_stack >= 1);
        }
        assert_eq!(props(Block::Stone.id() as u16).name, "stone");
        assert_eq!(ItemStack::item(Item::DiamondPickaxe, 1).tool_info().unwrap().1, 4);
        assert_eq!(ItemStack::item(Item::GoldPickaxe, 1).tool_info().unwrap().1, 0);
        assert!(ItemStack::item(Item::Bread, 1).is_food());
        assert_eq!(ItemStack::item(Item::IronHelmet, 1).armor_slot(), Some(ArmorSlot::Helmet));
    }
}
