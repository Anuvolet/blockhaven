pub mod font;
pub mod hud;
pub mod menu;
pub mod screens;

/// GUI scale (gui units = pixels / scale) for a window height.
pub fn gui_scale(height: u32) -> f32 {
    ((height as f32 / 300.0).floor()).clamp(1.0, 4.0)
}
