//! Furnace smelting state and recipes.

use crate::player::items::{props, Item, ItemStack};
use crate::world::block::Block;
use serde::{Deserialize, Serialize};

pub const SMELT_TICKS: u32 = 200;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FurnaceState {
    pub input: Option<ItemStack>,
    pub fuel: Option<ItemStack>,
    pub output: Option<ItemStack>,
    pub burn_left: u32,
    pub burn_total: u32,
    pub progress: u32,
}

/// What an item smelts into.
pub fn smelt_result(id: u16) -> Option<ItemStack> {
    if id < 256 {
        return match Block::from_id(id as u8) {
            Block::IronOre => Some(ItemStack::item(Item::IronIngot, 1)),
            Block::GoldOre => Some(ItemStack::item(Item::GoldIngot, 1)),
            Block::Sand => Some(ItemStack::block(Block::Glass, 1)),
            Block::Cobblestone => Some(ItemStack::block(Block::Stone, 1)),
            Block::OakLog | Block::BirchLog | Block::SpruceLog => Some(ItemStack::item(Item::Coal, 1)),
            Block::Clay => Some(ItemStack::block(Block::Bricks, 1)),
            Block::Cactus => None,
            _ => None,
        };
    }
    match Item::from_id(id)? {
        Item::PorkchopRaw => Some(ItemStack::item(Item::PorkchopCooked, 1)),
        Item::BeefRaw => Some(ItemStack::item(Item::BeefCooked, 1)),
        Item::ChickenRaw => Some(ItemStack::item(Item::ChickenCooked, 1)),
        Item::ClayBall => Some(ItemStack::item(Item::Brick, 1)),
        _ => None,
    }
}

pub fn fuel_ticks(id: u16) -> u32 {
    props(id).fuel
}

impl FurnaceState {
    fn can_smelt(&self) -> Option<ItemStack> {
        let input = self.input.filter(|s| !s.is_empty())?;
        let result = smelt_result(input.id)?;
        match self.output {
            None => Some(result),
            Some(o) if o.is_empty() => Some(result),
            Some(o) if o.id == result.id && (o.count as u32 + result.count as u32) <= o.max_stack() as u32 => Some(result),
            _ => None,
        }
    }

    /// Advance one tick. Returns true while burning (lit).
    pub fn tick(&mut self) -> bool {
        if self.burn_left > 0 {
            self.burn_left -= 1;
        }
        let smeltable = self.can_smelt();
        if self.burn_left == 0 && smeltable.is_some() {
            if let Some(f) = self.fuel.as_mut() {
                let t = fuel_ticks(f.id);
                if t > 0 && !f.is_empty() {
                    self.burn_left = t;
                    self.burn_total = t;
                    f.count -= 1;
                    if f.count == 0 {
                        self.fuel = None;
                    }
                }
            }
        }
        if self.burn_left > 0 {
            if let Some(result) = smeltable {
                self.progress += 1;
                if self.progress >= SMELT_TICKS {
                    self.progress = 0;
                    match self.output.as_mut() {
                        Some(o) if !o.is_empty() => o.count += result.count,
                        _ => self.output = Some(result),
                    }
                    if let Some(i) = self.input.as_mut() {
                        i.count -= 1;
                        if i.count == 0 {
                            self.input = None;
                        }
                    }
                }
            } else {
                self.progress = 0;
            }
        } else {
            self.progress = self.progress.saturating_sub(2);
        }
        self.burn_left > 0
    }

    pub fn progress_frac(&self) -> f32 {
        self.progress as f32 / SMELT_TICKS as f32
    }
    pub fn burn_frac(&self) -> f32 {
        if self.burn_total == 0 {
            0.0
        } else {
            self.burn_left as f32 / self.burn_total as f32
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iron_ore_smelts_into_an_ingot_with_coal() {
        let mut f = FurnaceState { input: Some(ItemStack::block(Block::IronOre, 2)), fuel: Some(ItemStack::item(Item::Coal, 1)), ..Default::default() };
        for _ in 0..SMELT_TICKS {
            f.tick();
        }
        assert_eq!(f.output, Some(ItemStack::item(Item::IronIngot, 1)));
        assert_eq!(f.input.map(|s| s.count), Some(1));
        assert!(f.fuel.is_none());
        assert!(f.burn_left > 0);
        for _ in 0..SMELT_TICKS {
            f.tick();
        }
        assert_eq!(f.output.map(|s| s.count), Some(2));
        assert!(f.input.is_none());
    }

    #[test]
    fn no_fuel_no_smelting() {
        let mut f = FurnaceState { input: Some(ItemStack::block(Block::Sand, 1)), ..Default::default() };
        for _ in 0..500 {
            assert!(!f.tick());
        }
        assert!(f.output.is_none());
    }
}
