//! Player inventory: 9 hotbar + 27 main slots, 4 armour slots, crafting grid, cursor stack.

use crate::player::items::{ArmorSlot, ItemKind, ItemStack};
use serde::{Deserialize, Serialize};

pub const HOTBAR: usize = 9;
pub const MAIN: usize = 36;
pub const ARMOR: usize = 4;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Inventory {
    pub slots: Vec<ItemStack>,
    pub armor: Vec<ItemStack>,
    pub selected: usize,
    /// 3x3 crafting grid (only the top-left 2x2 is used in the player inventory screen).
    pub craft: Vec<ItemStack>,
    /// Stack picked up by the mouse in UI.
    pub cursor: ItemStack,
}

impl Default for Inventory {
    fn default() -> Self {
        Inventory::new()
    }
}

impl Inventory {
    pub fn new() -> Inventory {
        Inventory { slots: vec![ItemStack::EMPTY; MAIN], armor: vec![ItemStack::EMPTY; ARMOR], selected: 0, craft: vec![ItemStack::EMPTY; 9], cursor: ItemStack::EMPTY }
    }

    pub fn held(&self) -> ItemStack {
        self.slots[self.selected]
    }
    pub fn held_mut(&mut self) -> &mut ItemStack {
        &mut self.slots[self.selected]
    }

    /// Add a stack, merging into existing stacks first. Returns what did not fit.
    pub fn add(&mut self, mut stack: ItemStack) -> ItemStack {
        if stack.is_empty() {
            return ItemStack::EMPTY;
        }
        let max = stack.max_stack();
        // merge
        for s in self.slots.iter_mut() {
            if !s.is_empty() && s.can_merge(&stack) && s.count < max {
                let n = (max - s.count).min(stack.count);
                s.count += n;
                stack.count -= n;
                if stack.count == 0 {
                    return ItemStack::EMPTY;
                }
            }
        }
        // empty slots (hotbar first)
        for s in self.slots.iter_mut() {
            if s.is_empty() {
                let n = stack.count.min(max);
                *s = ItemStack { id: stack.id, count: n, damage: stack.damage };
                stack.count -= n;
                if stack.count == 0 {
                    return ItemStack::EMPTY;
                }
            }
        }
        stack
    }

    pub fn count(&self, id: u16) -> u32 {
        self.slots.iter().filter(|s| !s.is_empty() && s.id == id).map(|s| s.count as u32).sum()
    }

    pub fn has(&self, id: u16, n: u32) -> bool {
        self.count(id) >= n
    }

    /// Remove `n` items of `id` from anywhere. Returns false (and removes nothing) if not enough.
    pub fn take(&mut self, id: u16, n: u32) -> bool {
        if !self.has(id, n) {
            return false;
        }
        let mut left = n;
        for s in self.slots.iter_mut() {
            if left == 0 {
                break;
            }
            if !s.is_empty() && s.id == id {
                let k = (s.count as u32).min(left) as u8;
                s.count -= k;
                left -= k as u32;
                if s.count == 0 {
                    *s = ItemStack::EMPTY;
                }
            }
        }
        true
    }

    /// Consume `n` from the selected stack.
    pub fn consume_selected(&mut self, n: u8) -> bool {
        let s = &mut self.slots[self.selected];
        if s.is_empty() || s.count < n {
            return false;
        }
        s.count -= n;
        if s.count == 0 {
            *s = ItemStack::EMPTY;
        }
        true
    }

    /// Apply durability damage to the held tool. Returns true if it broke.
    pub fn damage_held(&mut self, amount: u16) -> bool {
        let s = &mut self.slots[self.selected];
        let max = s.max_durability();
        if s.is_empty() || max == 0 {
            return false;
        }
        s.damage = s.damage.saturating_add(amount);
        if s.damage >= max {
            *s = ItemStack::EMPTY;
            return true;
        }
        false
    }

    pub fn armor_defense(&self) -> u32 {
        self.armor
            .iter()
            .filter(|s| !s.is_empty())
            .map(|s| match s.props().kind {
                ItemKind::Armor { defense, .. } => defense as u32,
                _ => 0,
            })
            .sum()
    }

    /// Damage all worn armour pieces; broken pieces vanish.
    pub fn damage_armor(&mut self, amount: u16) {
        for s in self.armor.iter_mut() {
            if s.is_empty() {
                continue;
            }
            let max = s.max_durability();
            s.damage = s.damage.saturating_add(amount);
            if max > 0 && s.damage >= max {
                *s = ItemStack::EMPTY;
            }
        }
    }

    pub fn armor_slot_index(slot: ArmorSlot) -> usize {
        slot as usize
    }

    /// Take everything out (death).
    pub fn drain_all(&mut self) -> Vec<ItemStack> {
        let mut out = Vec::new();
        for s in self.slots.iter_mut().chain(self.armor.iter_mut()).chain(self.craft.iter_mut()) {
            if !s.is_empty() {
                out.push(*s);
                *s = ItemStack::EMPTY;
            }
        }
        if !self.cursor.is_empty() {
            out.push(self.cursor);
            self.cursor = ItemStack::EMPTY;
        }
        out
    }

    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(|s| s.is_empty()) && self.armor.iter().all(|s| s.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::items::Item;
    use crate::world::block::Block;

    #[test]
    fn add_merges_and_overflows() {
        let mut inv = Inventory::new();
        assert!(inv.add(ItemStack::block(Block::Stone, 40)).is_empty());
        assert!(inv.add(ItemStack::block(Block::Stone, 40)).is_empty());
        assert_eq!(inv.slots[0].count, 64);
        assert_eq!(inv.slots[1].count, 16);
        assert_eq!(inv.count(Block::Stone.id() as u16), 80);
        assert!(inv.take(Block::Stone.id() as u16, 70));
        assert_eq!(inv.count(Block::Stone.id() as u16), 10);
        assert!(!inv.take(Block::Stone.id() as u16, 11));
        // fill everything
        for _ in 0..40 {
            inv.add(ItemStack::item(Item::Diamond, 64));
        }
        let rem = inv.add(ItemStack::item(Item::Coal, 5));
        assert_eq!(rem.count, 5);
    }

    #[test]
    fn tools_break_at_zero_durability() {
        let mut inv = Inventory::new();
        inv.slots[0] = ItemStack::item(Item::WoodPickaxe, 1);
        for _ in 0..58 {
            assert!(!inv.damage_held(1));
        }
        assert!(inv.damage_held(1));
        assert!(inv.slots[0].is_empty());
    }
}
