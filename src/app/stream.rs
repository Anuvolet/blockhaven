//! Chunk streaming: generation / mesh job scheduling, result upload, unloading with saving.

use crate::app::App;
use crate::world::chunk::BlockEntity;
use crate::world::worker::{Job, JobResult};

impl App {
    fn anchors(&self) -> Vec<(i32, i32)> {
        self.players.iter().map(|p| ((p.pos.x.floor() as i32) >> 4, (p.pos.z.floor() as i32) >> 4)).collect()
    }

    pub(crate) fn stream_chunks(&mut self) {
        let anchors = self.anchors();
        let rd = self.settings.render_distance;
        let moved = anchors[0] != self.last_player_chunk;
        self.last_player_chunk = anchors[0];
        let dist = |cx: i32, cz: i32| anchors.iter().map(|(ax, az)| (cx - ax).abs().max((cz - az).abs())).min().unwrap_or(0);

        // unload (and save) far chunks
        if self.frame % 60 == 0 {
            let far = rd + 3;
            for (cx, cz) in self.world.chunk_keys() {
                if dist(cx, cz) > far {
                    if self.pending_mesh.keys().any(|k| k.0 == cx && k.2 == cz) {
                        continue;
                    }
                    if let Some(c) = self.world.remove_chunk(cx, cz) {
                        if let Some(save) = &self.save {
                            let c = c.read().unwrap();
                            if c.dirty_save {
                                if let Ok(mut sm) = save.lock() {
                                    sm.store_chunk(&c);
                                }
                            }
                        }
                    }
                    self.chunk_renderer.remove_column(cx, cz);
                }
            }
        }

        let max_gen_inflight = self.threads * 3;
        if (moved || self.frame % 15 == 0) && self.pending_gen.len() < max_gen_inflight {
            let mut wanted: Vec<(i32, (i32, i32))> = Vec::new();
            let gr = rd + 1;
            for (ax, az) in &anchors {
                for dz in -gr..=gr {
                    for dx in -gr..=gr {
                        let key = (ax + dx, az + dz);
                        if self.pending_gen.contains(&key) || self.world.has_chunk(key.0, key.1) {
                            continue;
                        }
                        wanted.push((dx * dx + dz * dz, key));
                    }
                }
            }
            wanted.sort_by_key(|w| w.0);
            wanted.dedup_by_key(|w| w.1);
            for (_, key) in wanted.into_iter().take(max_gen_inflight - self.pending_gen.len()) {
                self.pending_gen.insert(key);
                self.pool.submit(Job::Generate { cx: key.0, cz: key.1 });
            }
        }

        let max_mesh_inflight = self.threads * 4;
        if self.frame % 3 == 0 && self.pending_mesh.len() < max_mesh_inflight {
            let mut wanted: Vec<(i32, (i32, i32, i32), u32)> = Vec::new();
            let chunks = self.world.chunks.read().unwrap();
            let py = (self.players[0].pos.y as i32) >> 4;
            for (cx, cz) in chunks.keys() {
                let d = dist(*cx, *cz);
                if d > rd {
                    continue;
                }
                let (cx, cz) = (*cx, *cz);
                let mut ok = true;
                'n: for nz in -1..=1 {
                    for nx in -1..=1 {
                        if (nx != 0 || nz != 0) && !chunks.contains_key(&(cx + nx, cz + nz)) {
                            ok = false;
                            break 'n;
                        }
                    }
                }
                if !ok {
                    continue;
                }
                let c = chunks.get(&(cx, cz)).unwrap().read().unwrap();
                for (sy, sub) in c.subs.iter().enumerate() {
                    let key = (cx, sy as i32, cz);
                    if let Some(v) = self.pending_mesh.get(&key) {
                        if *v == sub.version {
                            continue;
                        }
                    }
                    if self.chunk_renderer.mesh_version(cx, sy, cz) == Some(sub.version) {
                        continue;
                    }
                    let dy = (sy as i32) - py;
                    wanted.push((d * d + dy * dy / 2, key, sub.version));
                }
            }
            drop(chunks);
            wanted.sort_by_key(|w| w.0);
            let budget = max_mesh_inflight - self.pending_mesh.len();
            for (_, key, version) in wanted.into_iter().take(budget) {
                self.pending_mesh.insert(key, version);
                self.pool.submit(Job::Mesh { cx: key.0, cz: key.2, sy: key.1 as usize, version });
            }
        }
    }

    pub(crate) fn collect_results(&mut self) {
        let rd = self.settings.render_distance;
        let anchors = self.anchors();
        for r in self.pool.poll() {
            match r {
                JobResult::Generated { chunk } => {
                    let key = (chunk.cx, chunk.cz);
                    self.pending_gen.remove(&key);
                    let near = anchors.iter().any(|(ax, az)| (key.0 - ax).abs() <= rd + 3 && (key.1 - az).abs() <= rd + 3);
                    if !near {
                        continue;
                    }
                    for (k, be) in chunk.block_entities.iter() {
                        if let BlockEntity::Spawner { .. } = be {
                            self.spawners.insert((chunk.cx * 16 + k.0 as i32, k.1 as i32, chunk.cz * 16 + k.2 as i32));
                        }
                    }
                    self.world.insert_chunk(chunk);
                }
                JobResult::Meshed { cx, cz, sy, version, mesh } => {
                    let key = (cx, sy as i32, cz);
                    if self.pending_mesh.get(&key) == Some(&version) {
                        self.pending_mesh.remove(&key);
                    }
                    if !self.world.has_chunk(cx, cz) {
                        continue;
                    }
                    self.chunk_renderer.upload(&self.gpu, cx, sy, cz, version, mesh);
                }
            }
        }
    }
}
