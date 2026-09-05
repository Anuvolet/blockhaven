//! Procedural sound synthesis. Every effect is rendered to a mono f32 buffer at startup.

use crate::world::noise::Rng;

pub const RATE: u32 = 22050;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Sound {
    BreakStone,
    BreakWood,
    BreakDirt,
    BreakSand,
    BreakGrass,
    BreakGlass,
    PlaceStone,
    PlaceWood,
    PlaceDirt,
    PlaceSand,
    StepStone,
    StepWood,
    StepDirt,
    StepSand,
    StepGrass,
    Hurt,
    Death,
    Eat,
    PickUp,
    Splash,
    Click,
    Explode,
    Fuse,
    Bow,
    ArrowHit,
    Zombie,
    Skeleton,
    Pig,
    Cow,
    Sheep,
    Chicken,
    Egg,
    ChestOpen,
    Door,
    Lever,
    Piston,
    Fall,
    Craft,
    Burn,
    LevelUp,
}

pub const ALL: [Sound; 40] = [
    Sound::BreakStone,
    Sound::BreakWood,
    Sound::BreakDirt,
    Sound::BreakSand,
    Sound::BreakGrass,
    Sound::BreakGlass,
    Sound::PlaceStone,
    Sound::PlaceWood,
    Sound::PlaceDirt,
    Sound::PlaceSand,
    Sound::StepStone,
    Sound::StepWood,
    Sound::StepDirt,
    Sound::StepSand,
    Sound::StepGrass,
    Sound::Hurt,
    Sound::Death,
    Sound::Eat,
    Sound::PickUp,
    Sound::Splash,
    Sound::Click,
    Sound::Explode,
    Sound::Fuse,
    Sound::Bow,
    Sound::ArrowHit,
    Sound::Zombie,
    Sound::Skeleton,
    Sound::Pig,
    Sound::Cow,
    Sound::Sheep,
    Sound::Chicken,
    Sound::Egg,
    Sound::ChestOpen,
    Sound::Door,
    Sound::Lever,
    Sound::Piston,
    Sound::Fall,
    Sound::Craft,
    Sound::Burn,
    Sound::LevelUp,
];

struct Gen {
    rng: Rng,
    lp: f32,
    hp: f32,
}

impl Gen {
    fn new(seed: u64) -> Gen {
        Gen { rng: Rng::new(seed), lp: 0.0, hp: 0.0 }
    }
    fn noise(&mut self) -> f32 {
        self.rng.f32() * 2.0 - 1.0
    }
    /// One-pole low-pass, `cut` in Hz.
    fn lowpass(&mut self, x: f32, cut: f32) -> f32 {
        let a = (cut / RATE as f32 * std::f32::consts::TAU).clamp(0.0, 1.0);
        self.lp += (x - self.lp) * a;
        self.lp
    }
    fn highpass(&mut self, x: f32, cut: f32) -> f32 {
        let a = (cut / RATE as f32 * std::f32::consts::TAU).clamp(0.0, 1.0);
        self.hp += (x - self.hp) * a;
        x - self.hp
    }
}

fn env(t: f32, attack: f32, decay: f32) -> f32 {
    if t < attack {
        t / attack
    } else {
        (-(t - attack) / decay).exp()
    }
}

fn render(len: f32, mut f: impl FnMut(f32, usize) -> f32) -> Vec<f32> {
    let n = (len * RATE as f32) as usize;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / RATE as f32;
        out.push(f(t, i).clamp(-1.0, 1.0));
    }
    // short fade-out to avoid clicks
    let fade = (RATE as usize / 200).min(out.len());
    for k in 0..fade {
        let i = out.len() - 1 - k;
        out[i] *= k as f32 / fade as f32;
    }
    out
}

fn saw(ph: f32) -> f32 {
    2.0 * (ph - (ph + 0.5).floor())
}
fn square(ph: f32) -> f32 {
    if ph.fract() < 0.5 { 1.0 } else { -1.0 }
}
fn sine(ph: f32) -> f32 {
    (ph * std::f32::consts::TAU).sin()
}

/// Filtered noise burst used for block hits/steps.
fn burst(len: f32, cut: f32, decay: f32, seed: u64, gain: f32, tone: f32) -> Vec<f32> {
    let mut g = Gen::new(seed);
    let mut ph = 0.0f32;
    render(len, |t, _| {
        let n = g.noise();
        let s = g.lowpass(n, cut * (1.0 - t / len * 0.6));
        ph += tone / RATE as f32;
        let k = if tone > 0.0 { sine(ph) * 0.35 } else { 0.0 };
        (s * 1.6 + k) * env(t, 0.002, decay) * gain
    })
}

pub fn synthesize(sound: Sound) -> Vec<f32> {
    match sound {
        Sound::BreakStone => burst(0.22, 1800.0, 0.07, 1, 0.9, 0.0),
        Sound::BreakWood => burst(0.2, 900.0, 0.06, 2, 0.9, 180.0),
        Sound::BreakDirt => burst(0.18, 600.0, 0.07, 3, 0.8, 0.0),
        Sound::BreakSand => burst(0.25, 2600.0, 0.09, 4, 0.6, 0.0),
        Sound::BreakGrass => burst(0.16, 3000.0, 0.05, 5, 0.55, 0.0),
        Sound::BreakGlass => {
            let mut g = Gen::new(6);
            render(0.35, |t, _| {
                let n = g.noise();
                let s = g.highpass(n, 3000.0);
                let ring = sine(t * 5200.0) * 0.3 + sine(t * 7900.0) * 0.2;
                (s * 1.2 + ring) * env(t, 0.001, 0.09) * 0.8
            })
        }
        Sound::PlaceStone => burst(0.12, 1400.0, 0.04, 7, 0.8, 90.0),
        Sound::PlaceWood => burst(0.12, 800.0, 0.04, 8, 0.8, 160.0),
        Sound::PlaceDirt => burst(0.12, 500.0, 0.05, 9, 0.7, 0.0),
        Sound::PlaceSand => burst(0.14, 2200.0, 0.05, 10, 0.55, 0.0),
        Sound::StepStone => burst(0.08, 1600.0, 0.03, 11, 0.45, 0.0),
        Sound::StepWood => burst(0.09, 700.0, 0.03, 12, 0.45, 140.0),
        Sound::StepDirt => burst(0.09, 500.0, 0.035, 13, 0.4, 0.0),
        Sound::StepSand => burst(0.1, 2400.0, 0.04, 14, 0.35, 0.0),
        Sound::StepGrass => burst(0.09, 2800.0, 0.03, 15, 0.35, 0.0),
        Sound::Hurt => {
            let mut ph = 0.0f32;
            render(0.25, |t, _| {
                let f = 220.0 - t * 400.0;
                ph += f.max(60.0) / RATE as f32;
                (saw(ph) * 0.6 + square(ph * 0.5) * 0.2) * env(t, 0.005, 0.09) * 0.7
            })
        }
        Sound::Death => {
            let mut ph = 0.0f32;
            render(0.9, |t, _| {
                let f = 200.0 - t * 150.0;
                ph += f.max(40.0) / RATE as f32;
                (saw(ph) * 0.5 + sine(ph * 2.0) * 0.3) * env(t, 0.01, 0.4) * 0.7
            })
        }
        Sound::Eat => {
            let mut g = Gen::new(16);
            render(0.3, |t, _| {
                let n = g.noise();
                let s = g.lowpass(n, 1200.0);
                let gate = if (t * 14.0).fract() < 0.35 { 1.0 } else { 0.0 };
                s * gate * 0.6 * env(t, 0.005, 0.3)
            })
        }
        Sound::PickUp => {
            let mut ph = 0.0f32;
            render(0.18, |t, _| {
                let f = 500.0 + t * 3000.0;
                ph += f / RATE as f32;
                sine(ph) * env(t, 0.005, 0.06) * 0.5
            })
        }
        Sound::Splash => {
            let mut g = Gen::new(17);
            render(0.45, |t, _| {
                let n = g.noise();
                let s = g.lowpass(n, 900.0 + t * 1500.0);
                s * env(t, 0.02, 0.15) * 0.9
            })
        }
        Sound::Click => {
            let mut ph = 0.0f32;
            render(0.06, |t, _| {
                ph += 1100.0 / RATE as f32;
                square(ph) * env(t, 0.001, 0.02) * 0.35
            })
        }
        Sound::Explode => {
            let mut g = Gen::new(18);
            let mut ph = 0.0f32;
            render(1.6, |t, _| {
                let n = g.noise();
                let s = g.lowpass(n, 500.0 * (1.0 - t / 1.6) + 60.0);
                ph += (70.0 - t * 30.0).max(30.0) / RATE as f32;
                (s * 1.8 + sine(ph) * 0.5) * env(t, 0.01, 0.5) * 1.0
            })
        }
        Sound::Fuse => {
            let mut g = Gen::new(19);
            render(1.5, |t, _| {
                let n = g.noise();
                let s = g.highpass(n, 2500.0 + t * 3000.0);
                s * (0.3 + t * 0.4) * 0.9
            })
        }
        Sound::Bow => {
            let mut ph = 0.0f32;
            let mut g = Gen::new(20);
            render(0.35, |t, _| {
                let f = 260.0 - t * 300.0;
                ph += f.max(80.0) / RATE as f32;
                let raw = g.noise();
                let n = g.lowpass(raw, 3000.0);
                (sine(ph) * 0.6 + n * 0.3) * env(t, 0.003, 0.1) * 0.7
            })
        }
        Sound::ArrowHit => burst(0.1, 1200.0, 0.03, 21, 0.7, 320.0),
        Sound::Zombie => {
            let mut ph = 0.0f32;
            render(1.1, |t, _| {
                let vib = 1.0 + (t * 6.0).sin() * 0.04;
                let f = (95.0 + (t * 2.5).sin() * 20.0) * vib;
                ph += f / RATE as f32;
                (saw(ph) * 0.5 + square(ph * 0.5) * 0.25) * env(t, 0.15, 0.5) * 0.55
            })
        }
        Sound::Skeleton => {
            let mut g = Gen::new(22);
            render(0.6, |t, _| {
                let n = g.noise();
                let s = g.highpass(n, 1500.0);
                let gate = if (t * 22.0).fract() < 0.25 { 1.0 } else { 0.0 };
                s * gate * env(t, 0.01, 0.4) * 0.6
            })
        }
        Sound::Pig => {
            let mut ph = 0.0f32;
            render(0.45, |t, _| {
                let seg = if t < 0.2 { 1.0 } else { 0.8 };
                let f = 330.0 * seg - t * 120.0;
                ph += f.max(100.0) / RATE as f32;
                let gate = if t < 0.18 || (0.24..0.42).contains(&t) { 1.0 } else { 0.0 };
                (saw(ph) * 0.5 + sine(ph * 2.0) * 0.3) * gate * env(t, 0.02, 0.3) * 0.55
            })
        }
        Sound::Cow => {
            let mut ph = 0.0f32;
            render(0.9, |t, _| {
                let f = 130.0 + (t * 4.0).sin() * 8.0 - t * 25.0;
                ph += f / RATE as f32;
                (saw(ph) * 0.45 + sine(ph) * 0.4) * env(t, 0.08, 0.5) * 0.6
            })
        }
        Sound::Sheep => {
            let mut ph = 0.0f32;
            render(0.7, |t, _| {
                let vib = 1.0 + (t * 28.0).sin() * 0.08;
                let f = 250.0 * vib;
                ph += f / RATE as f32;
                (square(ph) * 0.3 + saw(ph) * 0.3) * env(t, 0.05, 0.35) * 0.5
            })
        }
        Sound::Chicken => {
            let mut ph = 0.0f32;
            render(0.35, |t, _| {
                let f = 900.0 + (t * 40.0).sin() * 200.0;
                ph += f / RATE as f32;
                let gate = if (t * 9.0).fract() < 0.5 { 1.0 } else { 0.0 };
                sine(ph) * gate * env(t, 0.01, 0.25) * 0.45
            })
        }
        Sound::Egg => burst(0.12, 2000.0, 0.03, 23, 0.4, 700.0),
        Sound::ChestOpen => {
            let mut g = Gen::new(24);
            let mut ph = 0.0f32;
            render(0.4, |t, _| {
                let raw = g.noise();
                let n = g.lowpass(raw, 600.0);
                ph += (180.0 + t * 200.0) / RATE as f32;
                (n * 0.8 + saw(ph) * 0.15) * env(t, 0.02, 0.2) * 0.6
            })
        }
        Sound::Door => {
            let mut g = Gen::new(25);
            let mut ph = 0.0f32;
            render(0.3, |t, _| {
                let raw = g.noise();
                let n = g.lowpass(raw, 900.0);
                ph += (320.0 - t * 300.0) / RATE as f32;
                (n * 0.7 + square(ph) * 0.15) * env(t, 0.01, 0.12) * 0.6
            })
        }
        Sound::Lever => {
            let mut ph = 0.0f32;
            render(0.12, |t, _| {
                ph += 600.0 / RATE as f32;
                square(ph) * env(t, 0.002, 0.03) * 0.4
            })
        }
        Sound::Piston => {
            let mut g = Gen::new(26);
            render(0.3, |t, _| {
                let n = g.noise();
                let hiss = g.highpass(n, 2000.0) * env(t, 0.01, 0.1);
                let thunk = if t > 0.15 { g.lowpass(n, 300.0) * env(t - 0.15, 0.002, 0.05) * 2.0 } else { 0.0 };
                (hiss * 0.5 + thunk) * 0.7
            })
        }
        Sound::Fall => burst(0.2, 700.0, 0.08, 27, 0.9, 60.0),
        Sound::Craft => burst(0.15, 1000.0, 0.05, 28, 0.6, 400.0),
        Sound::Burn => {
            let mut g = Gen::new(29);
            render(0.5, |t, _| {
                let n = g.noise();
                let s = g.lowpass(n, 1500.0);
                let crackle = if g.rng.chance(0.02) { 1.0 } else { 0.4 };
                s * crackle * env(t, 0.02, 0.3) * 0.5
            })
        }
        Sound::LevelUp => {
            let mut ph = 0.0f32;
            render(0.6, |t, _| {
                let step = (t * 8.0) as i32;
                let f = 440.0 * 1.25f32.powi(step.min(4));
                ph += f / RATE as f32;
                sine(ph) * env(t, 0.01, 0.4) * 0.4
            })
        }
    }
}

/// Continuous ambient generator: wind + a soft day/night pad.
pub struct Ambient {
    g: Gen,
    t: f64,
    pub wind: f32,
    pub pad: f32,
    pub pad_pitch: f32,
    ph1: f32,
    ph2: f32,
    ph3: f32,
}

impl Ambient {
    pub fn new() -> Ambient {
        Ambient { g: Gen::new(99), t: 0.0, wind: 0.3, pad: 0.2, pad_pitch: 110.0, ph1: 0.0, ph2: 0.0, ph3: 0.0 }
    }
    pub fn sample(&mut self) -> f32 {
        self.t += 1.0 / RATE as f64;
        let t = self.t as f32;
        let gust = 0.55 + 0.45 * (t * 0.23).sin() * (t * 0.071 + 1.0).sin();
        let n = self.g.noise();
        let wind = self.g.lowpass(n, 350.0 + 250.0 * gust) * gust * self.wind;
        self.ph1 += self.pad_pitch / RATE as f32;
        self.ph2 += self.pad_pitch * 1.005 / RATE as f32;
        self.ph3 += self.pad_pitch * 1.5 / RATE as f32;
        let pad = (sine(self.ph1) + sine(self.ph2) * 0.8 + sine(self.ph3) * 0.3) * 0.25 * self.pad * (0.7 + 0.3 * (t * 0.11).sin());
        (wind * 0.9 + pad).clamp(-1.0, 1.0)
    }
}

impl Default for Ambient {
    fn default() -> Self {
        Ambient::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_sound_renders_finite_audio() {
        for s in ALL {
            let buf = synthesize(s);
            assert!(buf.len() > 500, "{s:?} too short");
            assert!(buf.iter().all(|v| v.is_finite() && v.abs() <= 1.0), "{s:?} out of range");
            let peak = buf.iter().fold(0.0f32, |a, v| a.max(v.abs()));
            assert!(peak > 0.05, "{s:?} is silent");
        }
        let mut a = Ambient::new();
        for _ in 0..RATE {
            let v = a.sample();
            assert!(v.is_finite() && v.abs() <= 1.0);
        }
    }
}
