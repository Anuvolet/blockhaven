# Roadmap — "Blockhaven" (working title)

Milestones are implemented in order. Each milestone is only checked off after a
release build with zero warnings, a smoke run, and a commit.

- [ ] 0. Project skeleton: `cargo run --release` opens a window (wgpu + winit), git initialized
- [ ] 1. Window, input, camera, flat test world rendered
- [ ] 2. Chunk system + meshing + threading + frustum culling
- [ ] 3. Terrain generation (noise, biomes, caves, ores, trees, fluids)
- [ ] 4. Player physics, block break/place, procedural textures
- [ ] 5. Lighting (sun + block light, AO) + day/night cycle
- [ ] 6. Inventory, items, crafting, furnace, chests, item drops
- [ ] 7. Health / hunger / damage / respawn
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
