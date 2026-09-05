# Decisions

## Language: Rust (wgpu + winit)

Weighed against C++ (OpenGL 4.x + GLFW):

1. **One-command build on Windows.** `cargo run --release` pulls and builds every
   dependency. No vcpkg, no CMake, no manual GLFW/GLEW/glad download. C++ would
   require either vcpkg bootstrapping or vendoring binaries.
2. **Ecosystem.** winit (windowing/input), wgpu (DX12/Vulkan without GL driver
   quirks), rodio/cpal (WASAPI audio), gilrs (XInput gamepads), flate2 + serde
   (saves) are all pure-Rust crates that compile on Windows without any C
   toolchain configuration.
3. **Compile-time safety.** The project has a worker pool doing generation,
   lighting and meshing concurrently with the main thread. `Send`/`Sync` and
   the borrow checker turn the data races I would inevitably write in C++ into
   compile errors.
4. **My own reliability.** I write correct wgpu/winit code more consistently
   than raw OpenGL state-machine code; wgpu validation errors are explicit
   rather than silent black screens.
5. **Testing.** `cargo test` gives unit tests for noise, lighting, crafting and
   serialization with no framework setup.

### Toolchain note (this machine)

This development machine has VS2019 Build Tools but **no Windows 10 SDK**, so the
MSVC Rust target cannot link. The project was therefore built and verified with
the `x86_64-pc-windows-gnu` toolchain, which ships its own self-contained MinGW
linker inside rustup (`rust-mingw` component) and needs nothing else. All
dependencies are pure Rust, so the project builds identically on the default
`x86_64-pc-windows-msvc` toolchain on a machine that has the SDK. See README.

## Architecture

### Crates
| Concern | Crate | Why |
|---|---|---|
| Window/input | winit 0.29 | stable API, raw mouse motion, cursor grab |
| GPU | wgpu 0.19 | DX12 on Windows, Vulkan fallback, WGSL shaders |
| Math | glam | SIMD vec/mat, no surprises |
| Threads | crossbeam-channel + std threads | MPMC job queue for the worker pool |
| Audio | rodio 0.17 | mixer + custom `Source`s for synthesized sounds |
| Gamepad | gilrs 0.10 | XInput on Windows |
| Compression | flate2 (miniz_oxide backend) | pure Rust, no zlib |
| Serialization | serde + bincode 1 | player/chest/entity state |
| Noise / RNG | own implementation | deterministic, testable, no dependency drift |

No ECS crate. Entities (mobs, item drops, arrows) are plain structs in typed
`Vec`s; the count is small (hundreds) so an ECS adds no value and costs clarity.

### World / chunk layout
- Column chunks `16 x 256 x 16`, keyed by `(cx, cz)`, containing 16 sub-chunks
  of `16 x 16 x 16`.
- A voxel is `u16`: low 8 bits block id, high 8 bits metadata (fluid level,
  facing, door open, redstone power, crop stage, piston state ...).
- Per voxel one light byte: low nibble sky light, high nibble block light.
- A sub-chunk that is entirely air stores no block array (`Option<Box<..>>`).
- Chunks live in `HashMap<ChunkPos, Arc<RwLock<Chunk>>>` behind an `Arc<World>`
  so worker threads read neighbours directly instead of the main thread
  snapshotting padded regions.
- Block entities (chest inventory, furnace state, spawner) live in a per-chunk
  `HashMap<local_pos, BlockEntity>`.

### Threading model
- N-1 worker threads (min 2) share one crossbeam job channel.
- Jobs: `Generate(cx,cz)` → produces a fully populated chunk (terrain, caves,
  ores, decorations, structures, intra-chunk light) and sends it back.
- Jobs: `Mesh(cx, cz, sub_y)` → reads the chunk + neighbours under read locks
  into a padded 18³ buffer, builds opaque + translucent quad lists, sends back
  CPU vertex data. Main thread only uploads to the GPU.
- Cross-chunk light seams are propagated on the main thread with a per-frame
  budget when a chunk gains a new neighbour (BFS is monotonic, order-independent).
- Deterministic decoration: a chunk's trees/structures are a pure function of
  (seed, cx, cz). Chunk C also evaluates the decoration lists of its 8
  neighbours and writes the parts that fall inside C, so no cross-chunk writes
  are ever needed during generation.

### Mesh format
- Greedy meshing per face direction; quads merge only when block texture, light
  (sky, block) and the 4 AO values match, so smooth lighting is preserved.
- Vertex = world-space `f32x3` position, `f32x2` uv (may exceed 1 for merged
  quads; the atlas is a 2D **texture array** so UVs tile), one packed `u32`:
  texture layer (12 bits), sky light (4), block light (4), AO (2), normal (3),
  tint index (3).
- One shared `u16` index buffer (quad pattern) for every sub-chunk; each
  sub-chunk has its own vertex buffer, one draw call for opaque, one for
  translucent (water, glass). Leaves use alpha cut-out in the opaque pass.
- Mob models and item drops use the same pipeline and texture array: every box
  face maps to a 16×16 atlas tile, so a single texture array serves the whole
  3D scene.

### Save format
- `saves/<world>/level.bin`: bincode `LevelData` (seed, name, time of day,
  spawn, players, version).
- `saves/<world>/region/r.<rx>.<rz>.bin`: 32×32 chunks per region. Header of
  1024 `(u32 offset, u32 len)` entries, then zlib-compressed chunk blobs. Each
  blob is RLE encoded sub-chunks + light + block entities. Only chunks that
  were modified by the player/simulation are written; untouched chunks are
  regenerated from the seed.
- Autosave every 120 s and on quit; region files are rewritten atomically
  (write temp, rename).

### Simulation timing
- Render loop runs uncapped (vsync).
- World ticks at fixed 20 TPS via an accumulator: redstone, fluids, random
  ticks (crops), furnaces, mob AI decisions, spawning.
- Player and mob physics integrate per frame with dt clamped to 50 ms.

### Rendering pipeline per viewport
sky (fullscreen triangle) → opaque chunks → entities → translucent chunks →
block outline / crack overlay → held item → 2D HUD/UI.

## Decisions made during development

- **Light seams on workers.** Cross-chunk light propagation runs inside the `Mesh` job (before
  meshing) instead of on the main thread. Light "add" BFS only ever raises values, so concurrent
  runs from two workers converge; any sub-chunk they touch gets its version bumped and is remeshed.
- **Torch and glowstone light is warm-tinted in the shader** (block light mixes toward orange,
  night sky light toward blue) rather than storing colour per voxel.
- **Mob and item textures live in the same 16×16 array texture as blocks**, so mobs, drops, the
  first-person hand, cracks and the UI all use a single atlas and two pipelines.
- **Menus overlay a live demo world**: the main menu renders a random world with an orbiting
  camera behind it; starting or loading a world swaps the world/generator/pool in place.
- **Save format only stores modified chunks**; untouched chunks regenerate from the seed. Region
  files are rewritten atomically (temp + rename).
- **Split-screen renders each viewport with its own encoder/submit** so the single uniform buffer
  can be rewritten between views; the first viewport clears the attachments, the second loads them.
- **Simplifications recorded** (see README "Known issues"): one surface height per fluid block,
  greedy mob steering instead of A*, wheat item doubles as seeds, sticky piston = piston + string,
  glass is smelted from sand (also craftable from ice) rather than a crafting recipe.
- **Windows GNU toolchain**: rustup's self-contained MinGW ships `dlltool.exe` without `as.exe`,
  which `windows-sys` (raw-dylib) needs, so a full MinGW `bin` directory must be on `PATH`. A
  portable winlibs zip extracted into the user profile was enough; MSVC users need nothing extra.
- **`#![allow(dead_code)]` was used during early milestones** and removed at the end; the final
  build has zero warnings.
