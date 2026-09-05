//! User settings, persisted to `settings.bin` next to the saves folder.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settings {
    pub render_distance: i32,
    pub fov: f32,
    pub sensitivity: f32,
    pub volume: f32,
    pub fullscreen: bool,
    pub vsync: bool,
    pub smooth_lighting: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { render_distance: 12, fov: 70.0, sensitivity: 1.0, volume: 0.8, fullscreen: false, vsync: true, smooth_lighting: true }
    }
}

pub const SETTINGS_FILE: &str = "settings.bin";

impl Settings {
    pub fn load() -> Settings {
        match std::fs::read(SETTINGS_FILE) {
            Ok(bytes) => bincode::deserialize(&bytes).unwrap_or_default(),
            Err(_) => Settings::default(),
        }
    }
    pub fn save(&self) {
        if let Ok(bytes) = bincode::serialize(self) {
            let _ = std::fs::write(SETTINGS_FILE, bytes);
        }
    }
    /// Radians per mouse pixel.
    pub fn look_scale(&self) -> f32 {
        0.0022 * self.sensitivity
    }
}
