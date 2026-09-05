//! In-game HUD: hotbar, hearts, hunger, armour, air, crosshair, overlays, debug text.

use crate::player::{GameMode, OpenUi, Player};
use crate::render::atlas::Tile;
use crate::render::ui2d::UiBatch;
use crate::ui::font;

pub const HOTBAR_W: f32 = 182.0;

pub fn draw_hud(b: &mut UiBatch, player: &Player, debug: Option<&[String]>, show_crosshair: bool) {
    let w = b.width;
    let h = b.height;
    // overlays first (under everything)
    if player.head_in_water {
        b.rect(0.0, 0.0, w, h, [0.05, 0.15, 0.45, 0.28]);
    }
    if player.in_lava {
        b.rect(0.0, 0.0, w, h, [0.9, 0.3, 0.05, 0.6]);
    }
    if player.hurt_timer > 0.0 {
        b.rect(0.0, 0.0, w, h, [0.8, 0.0, 0.0, 0.35 * (player.hurt_timer / 0.5)]);
    }
    if player.dead {
        b.rect(0.0, 0.0, w, h, [0.4, 0.0, 0.0, 0.55]);
        b.text_centered(w * 0.5, h * 0.4, 3.0, "You died!", [1.0, 1.0, 1.0, 1.0]);
        b.text_centered(w * 0.5, h * 0.4 + 32.0, 1.0, "Click or press Jump to respawn", [1.0, 1.0, 1.0, 1.0]);
        return;
    }
    if show_crosshair && player.ui == OpenUi::None {
        b.tile(w * 0.5 - 4.5, h * 0.5 - 4.5, 9.0, 9.0, Tile::Crosshair, [1.0, 1.0, 1.0, 0.9]);
    }
    // hotbar
    let x0 = (w - HOTBAR_W) * 0.5;
    let y0 = h - 22.0;
    for i in 0..9 {
        let x = x0 + i as f32 * 20.0;
        b.tile(x, y0, 20.0, 20.0, Tile::Slot, [1.0, 1.0, 1.0, 0.9]);
    }
    let sel = player.inventory.selected as f32;
    b.tile(x0 + sel * 20.0 - 1.0, y0 - 1.0, 22.0, 22.0, Tile::SlotSelected, [1.0; 4]);
    for i in 0..9 {
        let x = x0 + i as f32 * 20.0;
        b.item(x + 2.0, y0 + 2.0, 16.0, &player.inventory.slots[i]);
    }
    if let Some((msg, t)) = &player.message {
        let a = (*t * 2.0).clamp(0.0, 1.0);
        b.text_centered(w * 0.5, y0 - 46.0, 1.0, msg, [1.0, 1.0, 0.7, a]);
    }
    // held item name
    let since = player.time - player.hotbar_changed_at;
    if since < 2.0 {
        let held = player.inventory.held();
        if !held.is_empty() {
            let a = ((2.0 - since) * 2.0).clamp(0.0, 1.0);
            b.text_centered(w * 0.5, y0 - 34.0, 1.0, held.name(), [1.0, 1.0, 1.0, a]);
        }
    }
    if player.mode == GameMode::Survival {
        // hearts
        let hy = y0 - 10.0;
        for i in 0..10 {
            let x = x0 + 1.0 + i as f32 * 8.0;
            let hp = player.health;
            let tile = if hp >= (i * 2 + 2) as f32 {
                Tile::HeartFull
            } else if hp >= (i * 2 + 1) as f32 {
                Tile::HeartHalf
            } else {
                Tile::HeartEmpty
            };
            let bounce = if hp <= 4.0 && (player.time * 8.0 + i as f32).sin() > 0.7 { -1.0 } else { 0.0 };
            b.tile(x, hy + bounce, 9.0, 9.0, tile, [1.0; 4]);
        }
        // hunger (right aligned)
        for i in 0..10 {
            let x = x0 + HOTBAR_W - 10.0 - i as f32 * 8.0;
            let f = player.hunger;
            let tile = if f >= (i * 2 + 2) as f32 {
                Tile::FoodFull
            } else if f >= (i * 2 + 1) as f32 {
                Tile::FoodHalf
            } else {
                Tile::FoodEmpty
            };
            b.tile(x, hy, 9.0, 9.0, tile, [1.0; 4]);
        }
        // armour
        let def = player.inventory.armor_defense();
        if def > 0 {
            for i in 0..10 {
                let x = x0 + 1.0 + i as f32 * 8.0;
                let tile = if def >= (i * 2 + 2) as u32 {
                    Tile::ArmorFull
                } else if def >= (i * 2 + 1) as u32 {
                    Tile::ArmorHalf
                } else {
                    Tile::ArmorEmpty
                };
                b.tile(x, hy - 10.0, 9.0, 9.0, tile, [1.0; 4]);
            }
        }
        // air
        if player.head_in_water || player.air < 10.0 {
            let bubbles = (player.air / 10.0 * 10.0).ceil() as i32;
            for i in 0..bubbles.max(0) {
                let x = x0 + HOTBAR_W - 10.0 - i as f32 * 8.0;
                b.tile(x, hy - 10.0, 9.0, 9.0, Tile::Bubble, [1.0; 4]);
            }
        }
    }
    // debug overlay
    if let Some(lines) = debug {
        let mut y = 2.0;
        for l in lines {
            let tw = font::text_width(l, 1.0);
            b.rect(1.0, y - 1.0, tw + 2.0, 9.0, [0.0, 0.0, 0.0, 0.45]);
            b.text(2.0, y, 1.0, l, [1.0; 4]);
            y += font::LINE_HEIGHT;
        }
    }
}

/// Screen-space label used for split-screen ("P1"/"P2") and hints.
pub fn draw_label(b: &mut UiBatch, text: &str) {
    b.text_shadow(3.0, b.height - 32.0, 1.0, text, [1.0, 1.0, 1.0, 0.8]);
}
