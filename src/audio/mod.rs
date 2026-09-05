//! Audio output via rodio: pre-rendered procedural effects, positional attenuation, ambient loop.

pub mod synth;

use glam::Vec3;
use rodio::buffer::SamplesBuffer;
use rodio::{OutputStream, OutputStreamHandle, Sink, Source};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
pub use synth::Sound;

pub const MAX_DISTANCE: f32 = 20.0;

struct AmbientSource {
    state: Arc<Mutex<synth::Ambient>>,
}

impl Iterator for AmbientSource {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        Some(self.state.lock().map(|mut a| a.sample()).unwrap_or(0.0))
    }
}

impl Source for AmbientSource {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        1
    }
    fn sample_rate(&self) -> u32 {
        synth::RATE
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

pub struct Audio {
    _stream: OutputStream,
    handle: OutputStreamHandle,
    sounds: HashMap<Sound, Arc<Vec<f32>>>,
    ambient: Arc<Mutex<synth::Ambient>>,
    ambient_sink: Sink,
    pub volume: f32,
    pub listener: Vec3,
    pub listener_right: Vec3,
    played_this_frame: u32,
}

impl Audio {
    /// Returns None when no output device is available (the game keeps running silently).
    pub fn new(volume: f32) -> Option<Audio> {
        let (stream, handle) = OutputStream::try_default().ok()?;
        let mut sounds = HashMap::new();
        for s in synth::ALL {
            sounds.insert(s, Arc::new(synth::synthesize(s)));
        }
        let ambient = Arc::new(Mutex::new(synth::Ambient::new()));
        let ambient_sink = Sink::try_new(&handle).ok()?;
        ambient_sink.set_volume(volume * 0.5);
        ambient_sink.append(AmbientSource { state: ambient.clone() });
        Some(Audio { _stream: stream, handle, sounds, ambient, ambient_sink, volume, listener: Vec3::ZERO, listener_right: Vec3::X, played_this_frame: 0 })
    }

    pub fn set_volume(&mut self, v: f32) {
        self.volume = v.clamp(0.0, 1.0);
        self.ambient_sink.set_volume(self.volume * 0.5);
    }

    pub fn begin_frame(&mut self, listener: Vec3, right: Vec3) {
        self.listener = listener;
        self.listener_right = right;
        self.played_this_frame = 0;
    }

    /// Ambient mood: wind strength and pad by time of day / altitude.
    pub fn set_ambient(&mut self, wind: f32, pad: f32, pad_pitch: f32) {
        if let Ok(mut a) = self.ambient.lock() {
            a.wind = wind;
            a.pad = pad;
            a.pad_pitch = pad_pitch;
        }
    }

    /// Non-positional (UI / player-local) sound.
    pub fn play(&mut self, s: Sound, gain: f32, pitch: f32) {
        self.emit(s, gain, pitch, 0.0);
    }

    /// Positional sound with distance attenuation and stereo pan.
    pub fn play_at(&mut self, s: Sound, pos: Vec3, gain: f32, pitch: f32) {
        let d = pos.distance(self.listener);
        if d > MAX_DISTANCE {
            return;
        }
        let att = (1.0 - d / MAX_DISTANCE).powf(1.5);
        let to = (pos - self.listener).normalize_or_zero();
        let pan = to.dot(self.listener_right).clamp(-1.0, 1.0) * (d / MAX_DISTANCE).min(1.0);
        self.emit(s, gain * att, pitch, pan);
    }

    fn emit(&mut self, s: Sound, gain: f32, pitch: f32, pan: f32) {
        if self.played_this_frame > 12 || gain * self.volume < 0.01 {
            return;
        }
        self.played_this_frame += 1;
        let Some(buf) = self.sounds.get(&s) else { return };
        let g = gain * self.volume;
        let l = g * (1.0 - pan.max(0.0));
        let r = g * (1.0 + pan.min(0.0));
        let mut data = Vec::with_capacity(buf.len() * 2);
        for v in buf.iter() {
            data.push(v * l);
            data.push(v * r);
        }
        let rate = (synth::RATE as f32 * pitch.clamp(0.5, 2.0)) as u32;
        let src = SamplesBuffer::new(2, rate, data);
        let _ = self.handle.play_raw(src.convert_samples());
    }
}

/// Block material -> break / place / step sounds.
pub fn material_sounds(b: crate::world::block::Block) -> (Sound, Sound, Sound) {
    use crate::world::block::{props, Block, Tool};
    let p = props(b.id());
    match b {
        Block::Glass | Block::Ice => (Sound::BreakGlass, Sound::PlaceStone, Sound::StepStone),
        Block::Sand | Block::Gravel | Block::Snow | Block::Clay => (Sound::BreakSand, Sound::PlaceSand, Sound::StepSand),
        Block::Grass | Block::SnowyGrass | Block::Podzol | Block::Dirt | Block::Farmland => (Sound::BreakDirt, Sound::PlaceDirt, Sound::StepGrass),
        Block::TallGrass | Block::DeadBush | Block::Dandelion | Block::Poppy | Block::Wheat | Block::BrownMushroom | Block::RedMushroom | Block::OakLeaves | Block::BirchLeaves | Block::SpruceLeaves => (Sound::BreakGrass, Sound::PlaceDirt, Sound::StepGrass),
        Block::Wool | Block::Bed | Block::HayBale => (Sound::BreakGrass, Sound::PlaceDirt, Sound::StepGrass),
        _ => match p.tool {
            Tool::Axe => (Sound::BreakWood, Sound::PlaceWood, Sound::StepWood),
            Tool::Shovel => (Sound::BreakDirt, Sound::PlaceDirt, Sound::StepDirt),
            _ => (Sound::BreakStone, Sound::PlaceStone, Sound::StepStone),
        },
    }
}
