//! Day/night cycle: 20 real minutes per day.

use glam::Vec3;

pub const DAY_LENGTH_SECS: f64 = 1200.0;

#[derive(Clone, Debug)]
pub struct DayCycle {
    /// Seconds since world creation.
    pub time: f64,
}

impl DayCycle {
    pub fn new() -> DayCycle {
        // start shortly after sunrise
        DayCycle { time: DAY_LENGTH_SECS * 0.05 }
    }
    pub fn advance(&mut self, dt: f64) {
        self.time += dt;
    }
    /// 0 = sunrise, 0.25 = noon, 0.5 = sunset, 0.75 = midnight.
    pub fn time_of_day(&self) -> f32 {
        ((self.time / DAY_LENGTH_SECS).rem_euclid(1.0)) as f32
    }
    pub fn day_number(&self) -> u32 {
        (self.time / DAY_LENGTH_SECS) as u32
    }
    pub fn sun_dir(&self) -> Vec3 {
        let a = self.time_of_day() * std::f32::consts::TAU;
        Vec3::new(a.cos(), a.sin(), 0.22).normalize()
    }
    /// Sun elevation in [-1, 1].
    pub fn elevation(&self) -> f32 {
        self.sun_dir().y
    }
    /// Sky light multiplier in [0.24, 1].
    pub fn sun_level(&self) -> f32 {
        let e = self.elevation();
        let t = smoothstep(-0.12, 0.22, e);
        0.24 + 0.76 * t
    }
    pub fn is_night(&self) -> bool {
        self.elevation() < -0.05
    }
    /// Skip to the next sunrise (used by beds).
    pub fn skip_to_morning(&mut self) {
        let day = (self.time / DAY_LENGTH_SECS).floor();
        self.time = (day + 1.0) * DAY_LENGTH_SECS + DAY_LENGTH_SECS * 0.02;
    }
    /// Returns (zenith, horizon) sky colours.
    pub fn sky_colors(&self) -> ([f32; 3], [f32; 3]) {
        let e = self.elevation();
        let day_z = [0.33, 0.55, 0.95];
        let day_h = [0.68, 0.81, 0.95];
        let night_z = [0.012, 0.015, 0.045];
        let night_h = [0.045, 0.055, 0.11];
        let dusk_h = [0.95, 0.52, 0.28];
        let dusk_z = [0.22, 0.28, 0.55];
        let day_t = smoothstep(-0.12, 0.22, e);
        let dusk_t = (1.0 - (e / 0.18).abs()).clamp(0.0, 1.0);
        let mut z = lerp3(night_z, day_z, day_t);
        let mut h = lerp3(night_h, day_h, day_t);
        z = lerp3(z, dusk_z, dusk_t * 0.6);
        h = lerp3(h, dusk_h, dusk_t * 0.85);
        (z, h)
    }
    pub fn fog_color(&self) -> [f32; 3] {
        self.sky_colors().1
    }
    /// Time formatted as HH:MM for the debug overlay (06:00 = sunrise).
    pub fn clock(&self) -> String {
        let t = (self.time_of_day() + 0.25).rem_euclid(1.0); // shift so 0 = 06:00
        let mins = (t * 24.0 * 60.0) as u32;
        format!("{:02}:{:02}", mins / 60, mins % 60)
    }
}

impl Default for DayCycle {
    fn default() -> Self {
        DayCycle::new()
    }
}

pub fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}
