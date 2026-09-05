//! Gameplay simulation: players, interactions, fixed ticks, entities, mobs, explosions.

use crate::app::{mob_sound, App, AUTOSAVE_SECS, TICK_DT};
use crate::audio::{material_sounds, Sound};
use crate::entity::{Arrow, PrimedTnt};
use crate::mobs::{spawn as mobspawn, MobCtx, MobEvent};
use crate::player::interact::{self, Ctx, Interaction};
use crate::player::physics::Aabb;
use crate::player::{OpenUi, PlayerEvent, PlayerInput};
use crate::redstone::RsEvent;
use crate::ui::screens;
use crate::world::block::{self, vox_block, vox_meta, voxel, Block};
use crate::world::chunk::BlockEntity;
use glam::Vec3;
use std::collections::HashSet;

impl App {
    /// One frame of gameplay (not called while paused / in the menu).
    pub(crate) fn update_playing(&mut self, dt: f32) {
        self.daytime.advance(dt as f64);
        if self.audio.is_some() {
            let p = &self.players[0];
            let (eye, right) = (p.eye(), p.right());
            let night = if self.daytime.is_night() { 1.0 } else { 0.0 };
            let alt = ((p.pos.y - 70.0) / 60.0).clamp(0.0, 1.0);
            let under = if p.head_in_water { 0.2 } else { 1.0 };
            let a = self.audio.as_mut().unwrap();
            a.begin_frame(eye, right);
            a.set_ambient((0.18 + 0.35 * alt) * under, 0.08 + 0.12 * night, if night > 0.5 { 82.0 } else { 110.0 });
        }
        // container screens
        let scale = self.gui_scale();
        for i in 0..self.players.len() {
            let ui_open = self.players[i].ui != OpenUi::None && self.players[i].ui != OpenUi::Dead;
            if !ui_open {
                continue;
            }
            let (sin, close) = self.screen_input(i, scale, dt);
            if close {
                screens::close(&mut self.players[i], &mut self.drops);
            } else {
                let (vw, vh) = self.viewport_size(i);
                let batch = crate::render::ui2d::UiBatch::new(vw, vh, scale);
                screens::update(&self.world, &mut self.players[i], &sin, &batch, &mut self.drops);
            }
        }
        // player inputs
        let raw: Vec<PlayerInput> = (0..self.players.len()).map(|i| if i == 0 { self.keyboard_input() } else { self.player2_input(dt) }).collect();
        let inputs: Vec<PlayerInput> = raw.into_iter().enumerate().map(|(i, pi)| self.gate_input(i, pi)).collect();
        let sens = self.settings.look_scale();
        let mut interactions = Vec::new();
        for i in 0..self.players.len() {
            let pin = inputs[i];
            if let Some((_, t)) = self.players[i].message.as_mut() {
                *t -= dt;
                if *t <= 0.0 {
                    self.players[i].message = None;
                }
            }
            // wait for the ground chunk before simulating
            let p = &self.players[i];
            let (cx, cz) = ((p.pos.x.floor() as i32) >> 4, (p.pos.z.floor() as i32) >> 4);
            if !self.world.has_chunk(cx, cz) {
                continue;
            }
            let events = self.players[i].update_physics(&self.world, &pin, dt, sens);
            let survival = self.players[i].tick_survival(dt);
            let ppos = self.players[i].pos;
            for ev in events.iter().chain(survival.iter()) {
                match ev {
                    PlayerEvent::Landed(d) => {
                        let dmg = (d - 3.0).floor();
                        if dmg > 0.0 {
                            self.players[i].damage(dmg);
                            self.sound_at(Sound::Fall, ppos, 1.0, 1.0);
                            self.sound_at(Sound::Hurt, ppos, 1.0, 1.0);
                        }
                    }
                    PlayerEvent::Step => {
                        let below = self.world.get_block(ppos.x.floor() as i32, (ppos.y - 0.1).floor() as i32, ppos.z.floor() as i32);
                        if below != Block::Air {
                            let (_, _, step) = material_sounds(below);
                            let pitch = 0.9 + self.rng.f32() * 0.2;
                            self.sound_at(step, ppos, 0.5, pitch);
                        }
                    }
                    PlayerEvent::Hurt => self.sound_at(Sound::Hurt, ppos, 1.0, 1.0),
                    PlayerEvent::EnteredWater => self.sound_at(Sound::Splash, ppos, 0.8, 1.0),
                    _ => {}
                }
            }
            if self.players[i].dead && self.players[i].hurt_timer > 0.45 {
                self.sound_at(Sound::Death, ppos, 1.0, 1.0);
            }
            let pin = self.melee_attack(i, pin);
            if self.players[i].dead {
                if pin.use_pressed || pin.jump_pressed {
                    let items = self.players[i].inventory.drain_all();
                    let pos = self.players[i].pos + Vec3::new(0.0, 0.5, 0.0);
                    for s in items {
                        crate::entity::spawn_drop(&mut self.drops, pos, s, &mut self.rng);
                    }
                    self.players[i].respawn();
                }
                continue;
            }
            let boxes: Vec<_> = self.players.iter().map(|p| p.aabb()).collect();
            let mut ctx = Ctx { world: &self.world, fluids: &mut self.fluids, drops: &mut self.drops, rng: &mut self.rng, player_boxes: &boxes };
            let acts = interact::update(&mut ctx, &mut self.players[i], &pin, dt);
            self.apply_interactions(i, &acts);
            interactions.extend(acts);
            if pin.inventory && self.players[i].ui == OpenUi::None {
                self.players[i].ui = OpenUi::Inventory;
                let (vw, vh) = self.viewport_size(i);
                self.players[i].cursor = (vw / scale * 0.5, vh / scale * 0.5);
            }
        }
        self.interactions = interactions;

        // fixed ticks
        self.tick_accum += dt;
        let mut guard = 0;
        while self.tick_accum >= TICK_DT && guard < 5 {
            self.tick_accum -= TICK_DT;
            guard += 1;
            self.tick();
        }
        self.update_entities(dt);
        self.update_mobs(dt);
        self.update_arrows(dt);

        // autosave
        if self.save.is_some() {
            self.autosave_timer += dt;
            if self.autosave_timer >= AUTOSAVE_SECS {
                self.autosave_timer = 0.0;
                self.save_world();
                self.players[0].say("World saved");
            }
        }
        self.end_pad_frame();
    }

    /// Melee hit on a mob along the look ray; returns the (possibly consumed) input.
    fn melee_attack(&mut self, i: usize, mut pin: PlayerInput) -> PlayerInput {
        if !(pin.attack_pressed && !self.players[i].dead && self.players[i].ui == OpenUi::None) {
            return pin;
        }
        let eye = self.players[i].eye();
        let dir = self.players[i].look_dir();
        let reach = self.players[i].reach();
        let mut best: Option<(usize, f32)> = None;
        for (mi, m) in self.mobs.iter().enumerate() {
            if m.dead {
                continue;
            }
            if let Some(t) = m.ray_hit(eye, dir, reach) {
                if best.map(|b| t < b.1).unwrap_or(true) {
                    best = Some((mi, t));
                }
            }
        }
        // the other player can be hit too (friendly fire, mild)
        let Some((mi, t)) = best else { return pin };
        let mut cache = crate::world::ChunkCache::new(&self.world);
        let blocked = crate::player::raycast::raycast(&mut cache, eye, dir, reach, false).map(|h| h.dist < t).unwrap_or(false);
        if blocked {
            return pin;
        }
        let dmg = self.players[i].inventory.held().attack_damage();
        let died = self.mobs[mi].damage(dmg, Some(eye));
        let mpos = self.mobs[mi].position();
        let kind = self.mobs[mi].kind;
        self.sound_at(mob_sound(kind), mpos, 0.8, 1.3);
        if died {
            let drops = self.mobs[mi].drops(&mut self.rng);
            for d in drops {
                crate::entity::spawn_drop(&mut self.drops, mpos + Vec3::new(0.0, 0.5, 0.0), d, &mut self.rng);
            }
        }
        if self.players[i].inventory.held().tool_info().is_some() {
            self.players[i].inventory.damage_held(1);
        }
        self.players[i].swing = 0.0;
        self.players[i].exhaustion += 0.1;
        self.players[i].breaking = None;
        pin.attack = false;
        pin.attack_pressed = false;
        pin
    }

    fn apply_interactions(&mut self, i: usize, acts: &[Interaction]) {
        let scale = self.gui_scale();
        for a in acts {
            match a {
                Interaction::OpenUi(ui) => {
                    self.players[i].ui = *ui;
                    let (vw, vh) = self.viewport_size(i);
                    self.players[i].cursor = (vw / scale * 0.5, vh / scale * 0.5);
                    if let OpenUi::Furnace(p) = ui {
                        self.furnaces.insert(*p);
                    }
                }
                Interaction::Placed { pos, block: Block::Furnace } => {
                    self.furnaces.insert(*pos);
                }
                Interaction::Sleep { .. } => {
                    let pos = self.players[i].pos;
                    self.players[i].bed_spawn = Some(pos);
                    if self.daytime.is_night() {
                        self.daytime.skip_to_morning();
                        self.players[i].say("Good morning! Respawn point set");
                    } else {
                        self.players[i].say("Respawn point set. You can only sleep at night");
                    }
                }
                Interaction::ShootArrow { origin, dir } => {
                    self.arrows.push(Arrow::new(*origin + *dir * 0.5, *dir * 30.0, 4.0, 0));
                }
                Interaction::Explode { pos } => {
                    self.world.set_block(pos.0, pos.1, pos.2, 0);
                    self.tnt.push(PrimedTnt::new(Vec3::new(pos.0 as f32 + 0.5, pos.1 as f32, pos.2 as f32 + 0.5)));
                }
                _ => {}
            }
            match a {
                Interaction::Broke { pos, .. } | Interaction::Placed { pos, .. } => self.redstone.mark(*pos),
                Interaction::Toggled { pos, block } => {
                    if *block == Block::Button {
                        self.redstone.press_button(*pos);
                    } else {
                        self.redstone.mark(*pos);
                    }
                }
                _ => {}
            }
            let (s, pos, gain, pitch) = match a {
                Interaction::Broke { pos, block } => (material_sounds(*block).0, *pos, 1.0, 1.0),
                Interaction::Hit { pos, block } => (material_sounds(*block).2, *pos, 0.6, 0.8),
                Interaction::Placed { pos, block } => (material_sounds(*block).1, *pos, 1.0, 1.0),
                Interaction::Toggled { pos, block } => (if *block == Block::Door { Sound::Door } else { Sound::Lever }, *pos, 1.0, 1.0),
                Interaction::OpenUi(OpenUi::Chest(p)) => (Sound::ChestOpen, *p, 1.0, 1.0),
                Interaction::Ate => {
                    self.sound(Sound::Eat, 1.0, 1.0);
                    continue;
                }
                Interaction::ShootArrow { .. } => {
                    self.sound(Sound::Bow, 1.0, 1.0);
                    continue;
                }
                Interaction::Explode { pos } => (Sound::Fuse, *pos, 1.0, 1.0),
                _ => continue,
            };
            let p = Vec3::new(pos.0 as f32 + 0.5, pos.1 as f32 + 0.5, pos.2 as f32 + 0.5);
            self.sound_at(s, p, gain, pitch);
        }
    }

    /// One 20 TPS world tick.
    fn tick(&mut self) {
        self.ticks += 1;
        self.fluids.step(&self.world);
        if self.ticks % 4 == 0 {
            self.random_ticks();
        }
        self.tick_furnaces();
        self.tick_redstone();
        let players: Vec<Vec3> = self.players.iter().filter(|p| !p.dead).map(|p| p.pos).collect();
        if self.ticks % 20 == 0 {
            let sun = self.daytime.sun_level();
            let day = !self.daytime.is_night();
            mobspawn::natural_spawn(&mut self.mobs, &self.world, &players, &mut self.rng, sun, day);
        }
        if self.ticks % 100 == 0 {
            mobspawn::despawn(&mut self.mobs, &self.world, &players);
        }
        let sun = self.daytime.sun_level();
        let list: Vec<(i32, i32, i32)> = self.spawners.iter().copied().collect();
        for p in list {
            if !mobspawn::tick_spawner(&self.world, p, &mut self.mobs, &players, &mut self.rng, sun) {
                self.spawners.remove(&p);
            }
        }
    }

    fn update_entities(&mut self, dt: f32) {
        for d in self.drops.iter_mut() {
            d.update(&self.world, dt);
        }
        let mut picked: Option<Vec3> = None;
        for i in 0..self.drops.len() {
            if self.drops[i].pickup_delay > 0.0 {
                continue;
            }
            let dp = self.drops[i].position();
            for p in self.players.iter_mut() {
                if p.dead {
                    continue;
                }
                if dp.distance(p.pos + Vec3::new(0.0, 0.8, 0.0)) < 1.6 {
                    let rem = p.inventory.add(self.drops[i].stack);
                    if rem.count != self.drops[i].stack.count {
                        picked = Some(dp);
                    }
                    self.drops[i].stack = rem;
                    if rem.is_empty() {
                        break;
                    }
                }
            }
        }
        if let Some(p) = picked {
            let pitch = 0.9 + self.rng.f32() * 0.3;
            self.sound_at(Sound::PickUp, p, 0.7, pitch);
        }
        self.drops.retain(|d| !d.stack.is_empty() && d.age < crate::entity::DROP_LIFETIME);
        let mut explosions = Vec::new();
        for t in self.tnt.iter_mut() {
            if t.update(&self.world, dt) {
                explosions.push(t.position());
            }
        }
        self.tnt.retain(|t| t.fuse > 0.0);
        for e in explosions {
            self.explode(e, 4.0);
            self.sound_at(Sound::Explode, e, 1.0, 1.0);
        }
    }

    fn update_arrows(&mut self, dt: f32) {
        let mut arrow_hits: Vec<Vec3> = Vec::new();
        for a in self.arrows.iter_mut() {
            let was_stuck = a.stuck;
            if a.update(&self.world, dt) && !was_stuck {
                arrow_hits.push(a.position());
            }
        }
        for p in arrow_hits {
            self.sound_at(Sound::ArrowHit, p, 0.8, 1.0);
        }
        let mut remove = Vec::new();
        let mut sounds = Vec::new();
        for ai in 0..self.arrows.len() {
            let a = &self.arrows[ai];
            if a.stuck || a.age < 0.05 {
                continue;
            }
            let p = a.position();
            let dmg = a.damage;
            let probe = Aabb::from_center(p - Vec3::new(0.0, 0.1, 0.0), 0.15, 0.2);
            let vel = a.velocity();
            if a.owner == 0 {
                for m in self.mobs.iter_mut() {
                    if !m.dead && m.aabb().intersects(&probe) {
                        let died = m.damage(dmg, Some(p - vel.normalize_or_zero()));
                        let mpos = m.position();
                        if died {
                            let drops = m.drops(&mut self.rng);
                            for d in drops {
                                crate::entity::spawn_drop(&mut self.drops, mpos + Vec3::new(0.0, 0.5, 0.0), d, &mut self.rng);
                            }
                        }
                        remove.push(ai);
                        sounds.push((mob_sound(m.kind), mpos));
                        break;
                    }
                }
            } else {
                for pl in self.players.iter_mut() {
                    if !pl.dead && pl.aabb().intersects(&probe) {
                        pl.damage(dmg);
                        pl.vel += vel.normalize_or_zero() * 3.0;
                        remove.push(ai);
                        sounds.push((Sound::Hurt, pl.pos));
                        break;
                    }
                }
            }
        }
        for (s, p) in sounds {
            self.sound_at(s, p, 0.8, 1.2);
        }
        remove.sort_unstable();
        remove.dedup();
        for ai in remove.into_iter().rev() {
            self.arrows.remove(ai);
        }
        self.arrows.retain(|a| a.age < 60.0 && a.position().y > -10.0);
    }

    fn update_mobs(&mut self, dt: f32) {
        let players: Vec<(Vec3, bool)> = self.players.iter().map(|p| (p.pos, p.dead)).collect();
        let sun = self.daytime.sun_level();
        let mut events = Vec::new();
        for m in self.mobs.iter_mut() {
            let p = m.position();
            if !self.world.is_loaded(p.x.floor() as i32, p.z.floor() as i32) {
                continue;
            }
            let mut ctx = MobCtx { world: &self.world, players: &players, rng: &mut self.rng, sun_level: sun };
            events.extend(m.update(&mut ctx, dt));
        }
        for e in events {
            match e {
                MobEvent::AttackPlayer { player, damage, from } => {
                    let mut snd = None;
                    if let Some(p) = self.players.get_mut(player) {
                        if p.damage(damage) {
                            snd = Some((Sound::Death, p.pos));
                        } else {
                            let push = (p.pos - from).normalize_or_zero();
                            p.vel += Vec3::new(push.x, 0.0, push.z) * 5.0 + Vec3::new(0.0, 3.5, 0.0);
                            snd = Some((Sound::Hurt, p.pos));
                        }
                    }
                    if let Some((s, pos)) = snd {
                        self.sound_at(s, pos, 1.0, 1.0);
                    }
                }
                MobEvent::ShootArrow { origin, dir } => {
                    self.arrows.push(Arrow::new(origin, dir * 22.0, 3.0, 1));
                    self.sound_at(Sound::Bow, origin, 0.8, 1.0);
                }
                MobEvent::Explode { pos, power } => {
                    self.explode(pos, power);
                    self.sound_at(Sound::Explode, pos, 1.0, 1.0);
                }
                MobEvent::Died { pos, drops, kind } => {
                    for d in drops {
                        crate::entity::spawn_drop(&mut self.drops, pos + Vec3::new(0.0, 0.5, 0.0), d, &mut self.rng);
                    }
                    self.sound_at(mob_sound(kind), pos, 0.9, 0.8);
                }
                MobEvent::Hurt { kind, pos } => self.sound_at(mob_sound(kind), pos, 0.6, 1.2),
                MobEvent::Ambient { kind, pos } => {
                    let pitch = 0.95 + self.rng.f32() * 0.1;
                    self.sound_at(mob_sound(kind), pos, 0.7, pitch);
                }
                MobEvent::FuseStart { pos } => self.sound_at(Sound::Fuse, pos, 1.0, 1.0),
                MobEvent::LayEgg { pos } => {
                    crate::entity::spawn_drop(&mut self.drops, pos, crate::player::items::ItemStack::item(crate::player::items::Item::Egg, 1), &mut self.rng);
                    self.sound_at(Sound::Egg, pos, 0.6, 1.0);
                }
            }
        }
        self.mobs.retain(|m| !(m.dead && m.death_timer > 1.2));
    }

    /// Pressure plates + redstone evaluation.
    fn tick_redstone(&mut self) {
        let mut feet: Vec<Vec3> = self.players.iter().filter(|p| !p.dead).map(|p| p.pos).collect();
        feet.extend(self.mobs.iter().filter(|m| !m.dead).map(|m| m.position()));
        feet.extend(self.drops.iter().map(|d| d.position()));
        let mut now_pressed: HashSet<(i32, i32, i32)> = HashSet::new();
        for f in &feet {
            for dy in [0.0f32, -0.05] {
                let p = (f.x.floor() as i32, (f.y + dy).floor() as i32, f.z.floor() as i32);
                if self.world.get_block(p.0, p.1, p.2) == Block::PressurePlate {
                    now_pressed.insert(p);
                }
            }
        }
        for p in now_pressed.iter() {
            if !self.redstone.pressed_plates.contains(p) {
                let v = self.world.get(p.0, p.1, p.2);
                self.world.set_block(p.0, p.1, p.2, voxel(Block::PressurePlate, vox_meta(v) | 1));
                self.redstone.mark(*p);
                self.sound_at(Sound::Click, Vec3::new(p.0 as f32 + 0.5, p.1 as f32, p.2 as f32 + 0.5), 0.6, 1.0);
            }
        }
        let released: Vec<(i32, i32, i32)> = self.redstone.pressed_plates.iter().filter(|p| !now_pressed.contains(p)).copied().collect();
        for p in released {
            let v = self.world.get(p.0, p.1, p.2);
            if vox_block(v) == Block::PressurePlate {
                self.world.set_block(p.0, p.1, p.2, voxel(Block::PressurePlate, vox_meta(v) & !1));
            }
            self.redstone.mark(p);
        }
        self.redstone.pressed_plates = now_pressed;
        let events = self.redstone.step(&self.world);
        for e in events {
            match e {
                RsEvent::PrimeTnt(p) => {
                    let c = Vec3::new(p.0 as f32 + 0.5, p.1 as f32, p.2 as f32 + 0.5);
                    self.tnt.push(PrimedTnt::new(c));
                    self.sound_at(Sound::Fuse, c, 1.0, 1.0);
                }
                RsEvent::Piston(p) => self.sound_at(Sound::Piston, Vec3::new(p.0 as f32 + 0.5, p.1 as f32 + 0.5, p.2 as f32 + 0.5), 0.8, 1.0),
                RsEvent::Door(p) => self.sound_at(Sound::Door, Vec3::new(p.0 as f32 + 0.5, p.1 as f32 + 0.5, p.2 as f32 + 0.5), 0.8, 1.0),
                RsEvent::Click(p) => self.sound_at(Sound::Lever, Vec3::new(p.0 as f32 + 0.5, p.1 as f32 + 0.5, p.2 as f32 + 0.5), 0.5, 0.8),
            }
        }
    }

    /// Advance every active furnace and keep its block's lit state in sync.
    fn tick_furnaces(&mut self) {
        let mut done = Vec::new();
        let list: Vec<(i32, i32, i32)> = self.furnaces.iter().copied().collect();
        for p in list {
            let v = self.world.get(p.0, p.1, p.2);
            let b = vox_block(v);
            if !matches!(b, Block::Furnace | Block::FurnaceLit) {
                done.push(p);
                continue;
            }
            let mut lit = false;
            let mut idle = false;
            let r = self.world.with_block_entity(p.0, p.1, p.2, |be| {
                if let BlockEntity::Furnace(f) = be {
                    lit = f.tick();
                    idle = f.burn_left == 0 && f.progress == 0 && f.input.map(|s| crate::player::furnace::smelt_result(s.id).is_none()).unwrap_or(true);
                }
            });
            if r.is_none() {
                done.push(p);
                continue;
            }
            let want = if lit { Block::FurnaceLit } else { Block::Furnace };
            if b != want {
                self.world.set_block(p.0, p.1, p.2, voxel(want, vox_meta(v)));
            }
            let open = self.players.iter().any(|pl| pl.ui == OpenUi::Furnace(p));
            if idle && !open {
                done.push(p);
            }
        }
        for p in done {
            self.furnaces.remove(&p);
        }
    }

    /// Crop growth near players.
    fn random_ticks(&mut self) {
        let positions: Vec<Vec3> = self.players.iter().map(|p| p.pos).collect();
        for pos in positions {
            for _ in 0..6 {
                let x = pos.x as i32 + self.rng.range(-24, 25);
                let z = pos.z as i32 + self.rng.range(-24, 25);
                let y = pos.y as i32 + self.rng.range(-8, 9);
                let v = self.world.get(x, y, z);
                let b = vox_block(v);
                let meta = vox_meta(v);
                if b == Block::Wheat && meta < 7 && self.rng.chance(0.3) {
                    self.world.set_block(x, y, z, voxel(b, meta + 1));
                }
            }
        }
    }

    /// Explosion: destroys blocks in a sphere, damages players and mobs.
    pub fn explode(&mut self, center: Vec3, power: f32) {
        let r = power.ceil() as i32;
        let (cx, cy, cz) = (center.x.floor() as i32, center.y.floor() as i32, center.z.floor() as i32);
        let boxes: Vec<_> = self.players.iter().map(|p| p.aabb()).collect();
        let mut chain = Vec::new();
        for dy in -r..=r {
            for dz in -r..=r {
                for dx in -r..=r {
                    let d2 = (dx * dx + dy * dy + dz * dz) as f32;
                    if d2 > power * power {
                        continue;
                    }
                    let (x, y, z) = (cx + dx, cy + dy, cz + dz);
                    let v = self.world.get(x, y, z);
                    if v == 0 {
                        continue;
                    }
                    let b = vox_block(v);
                    let p = block::props(b.id());
                    if p.hardness < 0.0 || b == Block::Obsidian || block::is_fluid(v) {
                        continue;
                    }
                    let resist = (p.hardness * 0.3).min(2.0);
                    if self.rng.f32() * (1.0 + resist) < 1.0 - (d2.sqrt() / power) * 0.6 {
                        if b == Block::Tnt {
                            chain.push(Vec3::new(x as f32 + 0.5, y as f32, z as f32 + 0.5));
                        }
                        let drop = self.rng.chance(0.3);
                        let mut ctx = Ctx { world: &self.world, fluids: &mut self.fluids, drops: &mut self.drops, rng: &mut self.rng, player_boxes: &boxes };
                        interact::destroy_block(&mut ctx, (x, y, z), drop);
                        self.redstone.mark((x, y, z));
                    }
                }
            }
        }
        for c in chain {
            self.world.set_block(c.x.floor() as i32, c.y.floor() as i32, c.z.floor() as i32, 0);
            let mut t = PrimedTnt::new(c);
            t.fuse = 0.3 + self.rng.f32() * 0.7;
            self.tnt.push(t);
        }
        for p in self.players.iter_mut() {
            let d = p.pos.distance(center);
            if d < power * 2.0 {
                let dmg = ((1.0 - d / (power * 2.0)) * power * 5.0).round();
                p.damage(dmg);
                let push = (p.pos - center).normalize_or_zero() * (1.0 - d / (power * 2.0)) * 12.0;
                p.vel += push + Vec3::new(0.0, 4.0, 0.0);
            }
        }
        let mut killed = Vec::new();
        for m in self.mobs.iter_mut() {
            let d = m.position().distance(center);
            if d < power * 2.0 && !m.dead {
                let dmg = ((1.0 - d / (power * 2.0)) * power * 6.0).round();
                if m.damage(dmg, Some(center)) {
                    killed.push((m.position(), m.drops(&mut self.rng)));
                }
            }
        }
        for (pos, drops) in killed {
            for d in drops {
                crate::entity::spawn_drop(&mut self.drops, pos, d, &mut self.rng);
            }
        }
    }
}
