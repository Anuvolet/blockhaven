# Blockhaven

A complete, playable voxel sandbox in the spirit of the classic block-building games, written from
scratch in Rust (wgpu + winit). **Zero external assets**: every block texture, item icon, mob skin,
UI glyph and sound effect is generated procedurally at startup.

![build](https://img.shields.io/badge/build-cargo%20run%20--release-blue)

## Building and running

Requirements: Windows 10/11 64-bit, a Rust toolchain (https://rustup.rs), and any GPU with
Vulkan or DirectX 12 (a GTX 1050 / Intel Iris class laptop is enough).

```
cargo run --release
```

or double-click / run `run.cmd`, which does the same thing. All dependencies are pure-Rust crates
fetched by cargo; no vcpkg, SDKs, DLLs or asset downloads are needed.

Toolchain notes:

- The default `x86_64-pc-windows-msvc` toolchain needs the Visual Studio Build Tools with the
  Windows SDK (rustup tells you how to install them). This is the standard Rust-on-Windows setup.
- If you use the `x86_64-pc-windows-gnu` toolchain instead, put a full MinGW-w64 `bin` directory
  on `PATH` (rustup's bundled linker lacks `as.exe`, which some Windows crates need). The project
  was developed and verified with the GNU toolchain plus a portable
  [winlibs](https://github.com/brechtsanders/winlibs_mingw) install; see `DECISIONS.md`.

Run `cargo test --release` for the unit tests (noise determinism, lighting, crafting, redstone,
fluids, physics, raycasting, chunk serialization round trip, font, sound synthesis).

Useful flags (see `--help`): `--world NAME`, `--seed S`, `--creative`, `--flat`, `--rd N`,
`--split`, `--benchmark`, `--screenshot out.png`, `--find-biomes`.

## Controls

### Player 1 (keyboard + mouse)

| Action | Key |
|---|---|
| Move / jump / sneak / sprint | W A S D / Space / Left Shift / Left Ctrl |
| Look | Mouse |
| Break block, attack | Left mouse (hold to mine) |
| Place block, use (doors, levers, chests, eat, bow) | Right mouse |
| Pick block (creative) | Middle mouse |
| Hotbar | 1 – 9, mouse wheel |
| Inventory / crafting | E (Esc or E closes) |
| Drop item | Q |
| Toggle flight (creative) | Double-tap Space |
| Pause menu | Esc |
| Add second player | F2 |
| Debug overlay | F3 |
| Fullscreen | F11 |
| Render distance +/- | ] / [ |

Inventory screens: left click picks up / places a whole stack, right click takes half or places one,
Shift+click quick-moves between inventory and container. Clicking outside the panel throws the
held stack.

### Player 2 (gamepad, XInput)

Left stick move, right stick look, A jump, B sneak, click left stick to sprint, RT break / attack,
LT place / use, Y inventory, X drop, LB / RB hotbar, click right stick pick block, Start pause. In
container screens the left stick moves a cursor, A = left click, B = right click, Y closes.

### Player 2 keyboard fallback (no gamepad connected)

Arrow keys move, I J K L look, Numpad 0 jump, Numpad 1 sneak, Numpad 2 sprint, Numpad 4 break,
Numpad 6 place / use, Numpad 7 / 9 hotbar, Numpad 5 inventory, Numpad 3 drop, Numpad 8 pause.
In container screens I J K L move the cursor, Numpad 4 = left click, Numpad 6 = right click.

## Features

- Infinite world in X/Z, 256 high, 16×256×16 column chunks made of 16³ sub-chunks.
- Seeded layered-noise terrain: continents, hills, ridged mountains up to y≈210, rivers, oceans,
  beaches; biomes (plains, forest, desert, snowy taiga, swamp, mountains) from temperature and
  humidity with biome-specific surfaces, trees (oak, birch, spruce, cactus, giant mushrooms) and
  plants; cheese + worm caves with lava lakes, bedrock, ores by depth (coal, iron, gold, diamond,
  redstone).
- Structures: villages (well, huts, houses with beds/chests/furnaces/crafting tables, wheat farms,
  lamp posts; desert and snowy variants), underground dungeons with a mob spawner and loot chests,
  and "ancient watch ring" ruins with a loot altar.
- Finite fluids: water spreads 7 blocks, lava 3, water + lava makes obsidian / cobblestone.
- Rendering: greedy meshing with smooth lighting and ambient occlusion, one draw call per
  sub-chunk, worker-pool generation and meshing, frustum culling, distance fog, transparent pass
  for water/glass/ice, alpha-tested leaves, 16-level sun + block light with per-voxel propagation,
  a 20-minute day/night cycle with sun, moon, stars and coloured sunsets, block break cracks,
  selection outline, first-person arm / held item, view bobbing.
- Procedural 16×16 texture array (≈220 tiles) with mipmaps, generated at startup.
- Player: AABB physics, swimming, ladders, sneaking edge protection, sprinting FOV, fall damage,
  drowning, lava, hunger with saturation and natural regeneration, death screen and respawn at
  spawn or bed.
- Inventory: 36 slots + hotbar + 4 armour slots, drag-and-drop, stacking, item drops that float
  and get picked up, 2×2 and 3×3 crafting with ~90 recipes (planks, sticks, all tool tiers,
  crafting table, furnace, chest, torches, beds, doors, ladders, bread, armour, every redstone
  component, TNT, blocks…), furnaces with fuel (iron, gold, glass from sand, cooked food, stone,
  charcoal), chests with persistent contents, beds that set spawn and skip the night, doors.
- Mobs: pig, cow, sheep, chicken (wander, flee, drop food/leather/wool/feathers/eggs); zombie
  (chases, melee), skeleton (keeps distance, shoots arrows), creeper (fuse + explosion). Light-
  and distance-based spawning, dungeon spawners, daylight burning, box models with walk animation.
- Redstone at 20 TPS: dust with 15-level decay, torches (inverters), levers, buttons, pressure
  plates, lamps, pistons and sticky pistons (push up to 12 blocks), powered doors, TNT.
- Split-screen: F2 adds a second local player with their own viewport, camera, HUD, inventory and
  physics; gamepad via XInput with keyboard fallback.
- Procedural audio (rodio): per-material break/place/step, damage, eating, pickup, splash,
  explosions, bow/arrows, mob voices, doors/levers/pistons, UI clicks, ambient wind and a day/night
  pad, all synthesized in code.
- Saving: `saves/<world>/level.bin` + region files (32×32 chunks, RLE + zlib); only modified
  chunks are stored, everything else regenerates from the seed. Autosave every 2 minutes and on
  quit.
- UI: main menu (new world with name + seed + mode + type, load/delete world, settings, controls),
  pause menu, settings (render distance, FOV, sensitivity, volume, fullscreen, vsync), HUD, F3
  overlay, procedural 5×7 bitmap font.

## Performance

`blockhaven --benchmark --rd 12 --frames 600 --pos 0,80,0` (vsync off) on a Radeon RX 480:
383 fps average, 187 fps minimum with 729 chunks loaded and ~290k quads on screen, so the 60 fps
target at render distance 12 holds with a large margin on the reference GTX 1050 / Iris class.

## Known issues

- Fluid surfaces use one height per block (no per-corner slopes) and flowing water has no
  current that pushes entities.
- Mob pathfinding is greedy steering with auto-jump; mobs can get stuck behind two-block walls.
- Wheat seeds are the wheat item itself (drops from tall grass); there is no hoe use — farmland
  comes from villages.
- Sticky pistons are crafted from a piston + string (no slime).
- Chests do not animate; beds skip the night for everyone as soon as one player sleeps.
- The GNU toolchain needs an external MinGW `bin` on `PATH` (see above); MSVC does not.

## Architecture

```
src/
  main.rs            window, event loop, CLI flags
  app/               App state: world lifecycle, menus, save/load (mod.rs), input mapping,
                     simulation (sim.rs), chunk streaming (stream.rs), rendering (render.rs)
  world/             block registry, chunk/sub-chunk storage, World (Arc<RwLock<Chunk>> map),
                     noise + PRNG, lighting BFS, fluids, worker pool, gen/ (terrain, biomes,
                     caves, ores, decorations, structures)
  render/            wgpu device, procedural atlas + texgen, greedy mesher, chunk renderer,
                     sky, overlays (outline/crack/hand), 2D UI batch renderer, screenshot capture
  player/            player state + physics, raycast, inventory, items, crafting, furnace,
                     block interaction
  mobs/              mob state, AI, box models, spawning
  entity/            item drops, arrows, primed TNT
  redstone/          power evaluation, pistons, doors, TNT priming
  audio/             rodio mixer + synthesizer
  ui/                font, HUD, container screens, menus
  save/              level data, chunk (de)serialization, region files
```

Threading: N-1 worker threads take `Generate` and `Mesh` jobs from a crossbeam channel. Chunks live
behind `Arc<RwLock<Chunk>>` so workers read neighbours directly and resolve cross-chunk light seams
themselves (light BFS is monotonic, so concurrent runs converge). The main thread only inserts
generated chunks, uploads vertex buffers, and runs gameplay. Sub-chunk meshes carry the version they
were built from; edits bump versions, so stale meshes are automatically rebuilt.

See `DECISIONS.md` for the language choice and the detailed architecture decisions, and
`ROADMAP.md` for the milestone history.
