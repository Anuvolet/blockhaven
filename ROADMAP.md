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
- [ ] 8. Mobs (pig, cow, sheep, chicken, zombie, skeleton, creeper)
- [ ] 9. Redstone (dust, torch, lever, button, plate, lamp, pistons, doors, TNT)
- [ ] 10. Structures (villages, dungeons, ruins)
- [ ] 11. Audio (procedural synth via rodio)
- [ ] 12. Save / load (region files, autosave)
- [ ] 13. Menus, settings, HUD, F3 debug overlay, procedural font
- [ ] 14. Split-screen (F2, gamepad / keyboard fallback)
- [ ] 15. Optimization pass + polish

## Deferred

(nothing yet)
