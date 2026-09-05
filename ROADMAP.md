# Roadmap — "Blockhaven" (working title)

Milestones are implemented in order. Each milestone is only checked off after a
release build with zero warnings, a smoke run, and a commit.

- [x] 0. Project skeleton: `cargo run --release` opens a window (wgpu + winit), git initialized
- [x] 1. Window, input, camera, flat test world rendered
- [x] 2. Chunk system + meshing + threading + frustum culling
- [x] 3. Terrain generation (noise, biomes, caves, ores, trees, fluids)
- [x] 4. Player physics, block break/place, procedural textures
- [x] 5. Lighting (sun + block light, AO) + day/night cycle
- [x] 6. Inventory, items, crafting, furnace, chests, item drops
- [x] 7. Health / hunger / damage / respawn
- [x] 8. Mobs (pig, cow, sheep, chicken, zombie, skeleton, creeper)
- [x] 9. Redstone (dust, torch, lever, button, plate, lamp, pistons, doors, TNT)
- [x] 10. Structures (villages, dungeons, ruins)
- [x] 11. Audio (procedural synth via rodio)
- [x] 12. Save / load (region files, autosave)
- [x] 13. Menus, settings, HUD, F3 debug overlay, procedural font
- [x] 14. Split-screen (F2, gamepad / keyboard fallback)
- [x] 15. Optimization pass + polish (benchmark: 383 fps avg / 187 min at render distance 12 on an RX 480, zero warnings, dead code removed)

## Deferred

- A* pathfinding for mobs (greedy steering + auto-jump is implemented; the bonus A* is not).
- Per-corner sloped fluid surfaces and water currents pushing entities.
- Hoe tilling / farmland creation by the player (farmland only generates in villages).
- Slabs, stairs and other partial blocks beyond doors, beds, plates, cactus and farmland.
- Chest lid animation, sheep shearing (sheep drop wool on death only).
