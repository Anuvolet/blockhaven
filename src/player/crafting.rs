//! Crafting recipes (shaped + shapeless) and grid matching.

use crate::player::items::{Item, ItemStack};
use crate::world::block::Block;
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ingredient {
    Id(u16),
    Planks,
    Log,
    Cobble,
    Wool,
}

impl Ingredient {
    pub fn matches(&self, s: &ItemStack) -> bool {
        if s.is_empty() {
            return false;
        }
        match self {
            Ingredient::Id(id) => s.id == *id,
            Ingredient::Planks => matches!(s.as_block(), Some(Block::OakPlanks | Block::BirchPlanks | Block::SprucePlanks)),
            Ingredient::Log => matches!(s.as_block(), Some(Block::OakLog | Block::BirchLog | Block::SpruceLog)),
            Ingredient::Cobble => matches!(s.as_block(), Some(Block::Cobblestone | Block::MossyCobblestone)),
            Ingredient::Wool => s.as_block() == Some(Block::Wool),
        }
    }
    /// Representative item for displaying the recipe.
    pub fn example(&self) -> ItemStack {
        match self {
            Ingredient::Id(id) => ItemStack::new(*id, 1),
            Ingredient::Planks => ItemStack::block(Block::OakPlanks, 1),
            Ingredient::Log => ItemStack::block(Block::OakLog, 1),
            Ingredient::Cobble => ItemStack::block(Block::Cobblestone, 1),
            Ingredient::Wool => ItemStack::block(Block::Wool, 1),
        }
    }
}

#[derive(Clone, Debug)]
pub enum RecipeKind {
    Shaped { w: usize, h: usize, cells: Vec<Option<Ingredient>> },
    Shapeless(Vec<Ingredient>),
}

#[derive(Clone, Debug)]
pub struct Recipe {
    pub result: ItemStack,
    pub kind: RecipeKind,
}

fn b(x: Block) -> Ingredient {
    Ingredient::Id(x.id() as u16)
}
fn i(x: Item) -> Ingredient {
    Ingredient::Id(x.id())
}

/// Build a shaped recipe from pattern rows. Keys map characters to ingredients; '.' is empty.
fn shaped(result: ItemStack, rows: &[&str], keys: &[(char, Ingredient)]) -> Recipe {
    let h = rows.len();
    let w = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let mut cells = Vec::with_capacity(w * h);
    for r in rows {
        for x in 0..w {
            let ch = r.chars().nth(x).unwrap_or('.');
            cells.push(if ch == '.' { None } else { keys.iter().find(|(k, _)| *k == ch).map(|(_, v)| *v) });
        }
    }
    Recipe { result, kind: RecipeKind::Shaped { w, h, cells } }
}

fn shapeless(result: ItemStack, items: &[Ingredient]) -> Recipe {
    Recipe { result, kind: RecipeKind::Shapeless(items.to_vec()) }
}

fn build() -> Vec<Recipe> {
    let mut v = Vec::new();
    let p = Ingredient::Planks;
    let s = i(Item::Stick);
    let c = Ingredient::Cobble;
    // planks (per wood) and sticks
    for (log, plank) in [(Block::OakLog, Block::OakPlanks), (Block::BirchLog, Block::BirchPlanks), (Block::SpruceLog, Block::SprucePlanks)] {
        v.push(shapeless(ItemStack::block(plank, 4), &[b(log)]));
    }
    v.push(shaped(ItemStack::item(Item::Stick, 4), &["P", "P"], &[('P', p)]));
    v.push(shaped(ItemStack::block(Block::CraftingTable, 1), &["PP", "PP"], &[('P', p)]));
    v.push(shaped(ItemStack::block(Block::Furnace, 1), &["CCC", "C.C", "CCC"], &[('C', c)]));
    v.push(shaped(ItemStack::block(Block::Chest, 1), &["PPP", "P.P", "PPP"], &[('P', p)]));
    v.push(shaped(ItemStack::block(Block::Torch, 4), &["C", "S"], &[('C', i(Item::Coal)), ('S', s)]));
    v.push(shaped(ItemStack::block(Block::Bed, 1), &["WWW", "PPP"], &[('W', Ingredient::Wool), ('P', p)]));
    v.push(shaped(ItemStack::block(Block::Door, 3), &["PP", "PP", "PP"], &[('P', p)]));
    v.push(shaped(ItemStack::block(Block::Ladder, 3), &["S.S", "SSS", "S.S"], &[('S', s)]));
    v.push(shaped(ItemStack::item(Item::Bread, 1), &["WWW"], &[('W', i(Item::Wheat))]));
    v.push(shaped(ItemStack::block(Block::Glass, 1), &["G"], &[('G', b(Block::Ice))]));
    // tools
    let mats: [(Ingredient, Item, Item, Item, Item, Item); 5] = [
        (p, Item::WoodPickaxe, Item::WoodAxe, Item::WoodShovel, Item::WoodSword, Item::WoodHoe),
        (c, Item::StonePickaxe, Item::StoneAxe, Item::StoneShovel, Item::StoneSword, Item::StoneHoe),
        (i(Item::IronIngot), Item::IronPickaxe, Item::IronAxe, Item::IronShovel, Item::IronSword, Item::IronHoe),
        (i(Item::GoldIngot), Item::GoldPickaxe, Item::GoldAxe, Item::GoldShovel, Item::GoldSword, Item::GoldHoe),
        (i(Item::Diamond), Item::DiamondPickaxe, Item::DiamondAxe, Item::DiamondShovel, Item::DiamondSword, Item::DiamondHoe),
    ];
    for (m, pick, axe, shovel, sword, hoe) in mats {
        let k = [('M', m), ('S', s)];
        v.push(shaped(ItemStack::item(pick, 1), &["MMM", ".S.", ".S."], &k));
        v.push(shaped(ItemStack::item(axe, 1), &["MM", "MS", ".S"], &k));
        v.push(shaped(ItemStack::item(shovel, 1), &["M", "S", "S"], &k));
        v.push(shaped(ItemStack::item(sword, 1), &["M", "M", "S"], &k));
        v.push(shaped(ItemStack::item(hoe, 1), &["MM", ".S", ".S"], &k));
    }
    // redstone components
    let r = i(Item::Redstone);
    v.push(shaped(ItemStack::block(Block::RedstoneTorchOn, 1), &["R", "S"], &[('R', r), ('S', s)]));
    v.push(shaped(ItemStack::block(Block::Lever, 1), &["S", "C"], &[('S', s), ('C', c)]));
    v.push(shaped(ItemStack::block(Block::Button, 1), &["T"], &[('T', b(Block::Stone))]));
    v.push(shaped(ItemStack::block(Block::PressurePlate, 1), &["PP"], &[('P', p)]));
    v.push(shaped(ItemStack::block(Block::RedstoneLamp, 1), &[".R.", "RGR", ".R."], &[('R', r), ('G', b(Block::Glowstone))]));
    v.push(shaped(ItemStack::block(Block::Piston, 1), &["PPP", "CIC", "CRC"], &[('P', p), ('C', c), ('I', i(Item::IronIngot)), ('R', r)]));
    v.push(shapeless(ItemStack::block(Block::StickyPiston, 1), &[b(Block::Piston), i(Item::String)]));
    v.push(shaped(ItemStack::block(Block::Tnt, 1), &["GSG", "SGS", "GSG"], &[('G', i(Item::Gunpowder)), ('S', b(Block::Sand))]));
    v.push(shaped(ItemStack::block(Block::RedstoneDust, 1), &["R"], &[('R', b(Block::RedstoneOre))]));
    // misc
    v.push(shaped(ItemStack::block(Block::Wool, 1), &["SS", "SS"], &[('S', i(Item::String))]));
    v.push(shaped(ItemStack::item(Item::Bow, 1), &[".ST", "S.T", ".ST"], &[('S', s), ('T', i(Item::String))]));
    v.push(shaped(ItemStack::item(Item::Arrow, 4), &["F", "S", "E"], &[('F', i(Item::Flint)), ('S', s), ('E', i(Item::Feather))]));
    v.push(shaped(ItemStack::block(Block::Sandstone, 1), &["SS", "SS"], &[('S', b(Block::Sand))]));
    v.push(shaped(ItemStack::block(Block::Clay, 1), &["CC", "CC"], &[('C', i(Item::ClayBall))]));
    v.push(shaped(ItemStack::block(Block::Bricks, 1), &["BB", "BB"], &[('B', i(Item::Brick))]));
    v.push(shaped(ItemStack::block(Block::StoneBricks, 4), &["SS", "SS"], &[('S', b(Block::Stone))]));
    v.push(shaped(ItemStack::block(Block::HayBale, 1), &["WWW", "WWW", "WWW"], &[('W', i(Item::Wheat))]));
    v.push(shapeless(ItemStack::item(Item::Wheat, 9), &[b(Block::HayBale)]));
    v.push(shaped(ItemStack::block(Block::Bookshelf, 1), &["PPP", "WWW", "PPP"], &[('P', p), ('W', i(Item::Wheat))]));
    v.push(shaped(ItemStack::block(Block::Glowstone, 1), &["RR", "RR"], &[('R', i(Item::Redstone))]));
    for (ingot, block) in [(Item::IronIngot, Block::IronBlock), (Item::GoldIngot, Block::GoldBlock), (Item::Diamond, Block::DiamondBlock)] {
        v.push(shaped(ItemStack::block(block, 1), &["III", "III", "III"], &[('I', i(ingot))]));
        v.push(shapeless(ItemStack::item(ingot, 9), &[b(block)]));
    }
    // armour
    for (m, helmet, chest, legs, boots) in [
        (i(Item::Leather), Item::LeatherHelmet, Item::LeatherChest, Item::LeatherLegs, Item::LeatherBoots),
        (i(Item::IronIngot), Item::IronHelmet, Item::IronChest, Item::IronLegs, Item::IronBoots),
    ] {
        let k = [('M', m)];
        v.push(shaped(ItemStack::item(helmet, 1), &["MMM", "M.M"], &k));
        v.push(shaped(ItemStack::item(chest, 1), &["M.M", "MMM", "MMM"], &k));
        v.push(shaped(ItemStack::item(legs, 1), &["MMM", "M.M", "M.M"], &k));
        v.push(shaped(ItemStack::item(boots, 1), &["M.M", "M.M"], &k));
    }
    v
}

static RECIPES: OnceLock<Vec<Recipe>> = OnceLock::new();

pub fn recipes() -> &'static Vec<Recipe> {
    RECIPES.get_or_init(build)
}

/// Match a grid (`gw` x `gw`, row-major, `gw` = 2 or 3) against every recipe.
pub fn find_match(grid: &[ItemStack], gw: usize) -> Option<ItemStack> {
    // bounding box of non-empty cells
    let mut minx = usize::MAX;
    let mut miny = usize::MAX;
    let mut maxx = 0;
    let mut maxy = 0;
    let mut count = 0;
    for y in 0..gw {
        for x in 0..gw {
            if !grid[y * gw + x].is_empty() {
                minx = minx.min(x);
                miny = miny.min(y);
                maxx = maxx.max(x);
                maxy = maxy.max(y);
                count += 1;
            }
        }
    }
    if count == 0 {
        return None;
    }
    let bw = maxx - minx + 1;
    let bh = maxy - miny + 1;
    for r in recipes() {
        match &r.kind {
            RecipeKind::Shaped { w, h, cells } => {
                if *w != bw || *h != bh {
                    continue;
                }
                for mirror in [false, true] {
                    let mut ok = true;
                    'cells: for y in 0..*h {
                        for x in 0..*w {
                            let gx = if mirror { minx + (w - 1 - x) } else { minx + x };
                            let gy = miny + y;
                            let cell = &grid[gy * gw + gx];
                            match cells[y * w + x] {
                                None => {
                                    if !cell.is_empty() {
                                        ok = false;
                                        break 'cells;
                                    }
                                }
                                Some(ing) => {
                                    if !ing.matches(cell) {
                                        ok = false;
                                        break 'cells;
                                    }
                                }
                            }
                        }
                    }
                    if ok {
                        return Some(r.result);
                    }
                }
            }
            RecipeKind::Shapeless(items) => {
                if items.len() != count {
                    continue;
                }
                let mut used = vec![false; items.len()];
                let mut ok = true;
                for cell in grid.iter().filter(|c| !c.is_empty()) {
                    let mut found = false;
                    for (k, ing) in items.iter().enumerate() {
                        if !used[k] && ing.matches(cell) {
                            used[k] = true;
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        ok = false;
                        break;
                    }
                }
                if ok {
                    return Some(r.result);
                }
            }
        }
    }
    None
}

/// Remove one item from every occupied grid cell (after taking a crafting result).
pub fn consume(grid: &mut [ItemStack]) {
    for c in grid.iter_mut() {
        if !c.is_empty() {
            c.count -= 1;
            if c.count == 0 {
                *c = ItemStack::EMPTY;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid3(cells: &[(usize, ItemStack)]) -> Vec<ItemStack> {
        let mut g = vec![ItemStack::EMPTY; 9];
        for (i, s) in cells {
            g[*i] = *s;
        }
        g
    }

    #[test]
    fn logs_make_planks_anywhere_in_a_2x2() {
        let mut g = vec![ItemStack::EMPTY; 4];
        g[3] = ItemStack::block(Block::BirchLog, 1);
        assert_eq!(find_match(&g, 2), Some(ItemStack::block(Block::BirchPlanks, 4)));
    }

    #[test]
    fn shaped_pickaxe_and_mirrored_axe() {
        let planks = ItemStack::block(Block::OakPlanks, 1);
        let stick = ItemStack::item(Item::Stick, 1);
        let g = grid3(&[(0, planks), (1, planks), (2, planks), (4, stick), (7, stick)]);
        assert_eq!(find_match(&g, 3), Some(ItemStack::item(Item::WoodPickaxe, 1)));
        // axe: "MM","MS",".S" -> mirrored "MM","SM","S."
        let g = grid3(&[(1, planks), (2, planks), (4, stick), (5, planks), (7, stick)]);
        assert_eq!(find_match(&g, 3), Some(ItemStack::item(Item::WoodAxe, 1)));
        // stone tools use cobblestone
        let cobble = ItemStack::block(Block::Cobblestone, 1);
        let g = grid3(&[(1, cobble), (4, stick), (7, stick)]);
        assert_eq!(find_match(&g, 3), Some(ItemStack::item(Item::StoneShovel, 1)));
    }

    #[test]
    fn two_by_two_cannot_hold_three_wide_recipes() {
        let wheat = ItemStack::item(Item::Wheat, 1);
        let mut g = vec![ItemStack::EMPTY; 4];
        g[0] = wheat;
        g[1] = wheat;
        assert_eq!(find_match(&g, 2), None);
        let g = grid3(&[(3, wheat), (4, wheat), (5, wheat)]);
        assert_eq!(find_match(&g, 3), Some(ItemStack::item(Item::Bread, 1)));
    }

    #[test]
    fn consuming_decrements_every_cell() {
        let mut g = grid3(&[(0, ItemStack::block(Block::OakPlanks, 3)), (3, ItemStack::block(Block::OakPlanks, 1))]);
        consume(&mut g);
        assert_eq!(g[0].count, 2);
        assert!(g[3].is_empty());
    }

    #[test]
    fn every_required_recipe_exists() {
        let results: Vec<u16> = recipes().iter().map(|r| r.result.id).collect();
        for needed in [
            Block::OakPlanks.id() as u16,
            Block::CraftingTable.id() as u16,
            Block::Furnace.id() as u16,
            Block::Chest.id() as u16,
            Block::Torch.id() as u16,
            Block::Bed.id() as u16,
            Block::Door.id() as u16,
            Block::Ladder.id() as u16,
            Block::RedstoneTorchOn.id() as u16,
            Block::Lever.id() as u16,
            Block::Button.id() as u16,
            Block::PressurePlate.id() as u16,
            Block::RedstoneLamp.id() as u16,
            Block::Piston.id() as u16,
            Block::StickyPiston.id() as u16,
            Block::Tnt.id() as u16,
            Item::Stick.id(),
            Item::Bread.id(),
            Item::DiamondPickaxe.id(),
            Item::GoldSword.id(),
            Item::IronShovel.id(),
            Item::StoneAxe.id(),
        ] {
            assert!(results.contains(&needed), "missing recipe for item id {needed}");
        }
    }
}
