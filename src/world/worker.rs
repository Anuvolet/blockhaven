//! Worker pool for chunk generation and meshing. The main thread only uploads results.

use crate::render::mesher::{self, MeshData};
use crate::world::chunk::Chunk;
use crate::world::gen::Generator;
use crate::world::World;
use crate::save::SaveManager;
use crossbeam_channel::{unbounded, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

pub enum Job {
    Generate { cx: i32, cz: i32 },
    Mesh { cx: i32, cz: i32, sy: usize, version: u32 },
    Shutdown,
}

pub enum JobResult {
    Generated { chunk: Chunk },
    Meshed { cx: i32, cz: i32, sy: usize, version: u32, mesh: MeshData },
}

pub struct WorkerPool {
    tx: Sender<Job>,
    rx: Receiver<JobResult>,
    handles: Vec<thread::JoinHandle<()>>,
    pub in_flight: usize,
}

impl WorkerPool {
    pub fn new(world: Arc<World>, generator: Arc<Generator>, save: Option<Arc<Mutex<SaveManager>>>, threads: usize) -> WorkerPool {
        let (tx, job_rx) = unbounded::<Job>();
        let (res_tx, rx) = unbounded::<JobResult>();
        let mut handles = Vec::new();
        for i in 0..threads.max(1) {
            let job_rx = job_rx.clone();
            let res_tx = res_tx.clone();
            let world = world.clone();
            let generator = generator.clone();
            let save = save.clone();
            handles.push(
                thread::Builder::new()
                    .name(format!("worker-{i}"))
                    .stack_size(8 << 20)
                    .spawn(move || worker_main(job_rx, res_tx, world, generator, save))
                    .expect("spawn worker"),
            );
        }
        WorkerPool { tx, rx, handles, in_flight: 0 }
    }

    pub fn submit(&mut self, job: Job) {
        self.in_flight += 1;
        let _ = self.tx.send(job);
    }

    pub fn poll(&mut self) -> Vec<JobResult> {
        let mut out = Vec::new();
        while let Ok(r) = self.rx.try_recv() {
            self.in_flight = self.in_flight.saturating_sub(1);
            out.push(r);
        }
        out
    }

    pub fn shutdown(&mut self) {
        for _ in 0..self.handles.len() {
            let _ = self.tx.send(Job::Shutdown);
        }
        for h in self.handles.drain(..) {
            let _ = h.join();
        }
    }
}

impl Drop for WorkerPool {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn worker_main(rx: Receiver<Job>, tx: Sender<JobResult>, world: Arc<World>, generator: Arc<Generator>, save: Option<Arc<Mutex<SaveManager>>>) {
    while let Ok(job) = rx.recv() {
        match job {
            Job::Shutdown => break,
            Job::Generate { cx, cz } => {
                let saved = save.as_ref().and_then(|s| s.lock().ok().and_then(|mut s| s.load_chunk(cx, cz)));
                let chunk = match saved {
                    Some(c) => c,
                    None => {
                        let mut chunk = generator.generate(cx, cz);
                        crate::world::light::init_chunk_light(&mut chunk);
                        chunk
                    }
                };
                if tx.send(JobResult::Generated { chunk }).is_err() {
                    break;
                }
            }
            Job::Mesh { cx, cz, sy, version } => {
                // resolve light seams with neighbours first (idempotent, monotonic)
                crate::world::light::propagate_seams(&world, cx, cz);
                let mesh = mesher::mesh_subchunk(&world, cx, cz, sy);
                if tx.send(JobResult::Meshed { cx, cz, sy, version, mesh }).is_err() {
                    break;
                }
            }
        }
    }
}
