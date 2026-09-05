// A few helpers are only used by later features or debug flags.

mod app;
mod audio;
mod daytime;
mod entity;
mod input;
mod mobs;
mod player;
mod redstone;
mod render;
mod save;
mod settings;
mod ui;
mod world;

use std::sync::Arc;
use winit::event::{DeviceEvent, Event, WindowEvent};
use winit::event_loop::EventLoop;
use winit::window::{CursorGrabMode, Fullscreen, WindowBuilder};

fn print_help() {
    println!("Blockhaven - procedural voxel sandbox");
    println!("  (no arguments)          start at the main menu");
    println!("  --world NAME            create/load a world directly (skips the menu)");
    println!("  --seed S --creative --flat   options for --world");
    println!("  --screenshot FILE.png   render into a file after the world loads and exit");
    println!("  --frames N --pos x,y,z --look yaw,pitch --time 0..1 --rd N --give ITEMID");
    println!("  --debug --open inventory|crafting --mobs pig,cow,... --split --nosave --find-biomes");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned());
    let flag = |name: &str| args.iter().any(|a| a == name);
    if flag("--help") || flag("-h") {
        print_help();
        return;
    }
    if flag("--find-biomes") {
        let seed = arg("--seed").map(|s| world::noise::seed_from_str(&s)).unwrap_or(1337);
        find_biomes(seed);
        return;
    }

    let event_loop = EventLoop::new().expect("event loop");
    let window = Arc::new(
        WindowBuilder::new()
            .with_title("Blockhaven")
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0))
            .build(&event_loop)
            .expect("window"),
    );
    let mut settings = settings::Settings::load();
    if let Some(rd) = arg("--rd").and_then(|s| s.parse::<i32>().ok()) {
        settings.render_distance = rd.clamp(2, 32);
    }
    let benchmark = flag("--benchmark");
    if benchmark {
        settings.vsync = false;
    }
    let gpu = render::gpu::Gpu::new(window.clone(), settings.vsync);
    println!("GPU: {} ({})", gpu.adapter_name, gpu.backend);
    if settings.fullscreen {
        window.set_fullscreen(Some(Fullscreen::Borderless(None)));
    }
    let mut app = app::App::new(gpu, settings);

    // --- direct world start (debug / screenshots) ---
    let screenshot = arg("--screenshot");
    let direct = arg("--world").or_else(|| if (screenshot.is_some() || flag("--pos")) && !flag("--menu-shot") { Some("debug".to_string()) } else { None });
    if let Some(name) = direct {
        let seed = arg("--seed").map(|s| world::noise::seed_from_str(&s)).unwrap_or(1337);
        let mode = if flag("--creative") { player::GameMode::Creative } else { player::GameMode::Survival };
        let persist = !flag("--nosave") && (screenshot.is_none() || flag("--world"));
        if persist && save::load_level(&name).is_some() {
            app.load_world(&name);
        } else {
            app.new_world(&name, seed, mode, flag("--flat"), persist);
        }
        if let Some(p) = arg("--pos") {
            let v: Vec<f32> = p.split(',').filter_map(|s| s.trim().parse().ok()).collect();
            if v.len() == 3 {
                app.players[0].pos = glam::Vec3::new(v[0], v[1], v[2]);
                app.players[0].flying = true;
                app.players[0].mode = player::GameMode::Creative;
            }
        }
        if let Some(id) = arg("--give").and_then(|s| s.parse::<u16>().ok()) {
            app.players[0].inventory.slots[0] = player::items::ItemStack::new(id, 32);
            app.players[0].inventory.slots[1] = player::items::ItemStack::item(player::items::Item::IronPickaxe, 1);
            app.players[0].inventory.slots[2] = player::items::ItemStack::item(player::items::Item::Bread, 7);
            app.players[0].inventory.slots[12] = player::items::ItemStack::block(world::block::Block::OakLog, 20);
            app.players[0].inventory.craft[0] = player::items::ItemStack::block(world::block::Block::OakPlanks, 2);
            app.players[0].inventory.craft[3] = player::items::ItemStack::block(world::block::Block::OakPlanks, 2);
        }
        if let Some(list) = arg("--mobs") {
            let base = app.players[0].pos;
            let fwd = app.players[0].forward_flat();
            let right = app.players[0].right();
            for (i, name) in list.split(',').enumerate() {
                let kind = match name.trim() {
                    "pig" => mobs::MobKind::Pig,
                    "cow" => mobs::MobKind::Cow,
                    "sheep" => mobs::MobKind::Sheep,
                    "chicken" => mobs::MobKind::Chicken,
                    "zombie" => mobs::MobKind::Zombie,
                    "skeleton" => mobs::MobKind::Skeleton,
                    _ => mobs::MobKind::Creeper,
                };
                let pos = base + fwd * 4.0 + right * (i as f32 - 3.0) * 1.6;
                let mut m = mobs::Mob::new(kind, pos + glam::Vec3::new(0.0, 0.5, 0.0), &mut app.rng);
                m.yaw = app.players[0].yaw + std::f32::consts::PI;
                m.head_yaw = m.yaw;
                app.mobs.push(m);
            }
        }
        if flag("--debug") {
            app.show_debug = true;
        }
        if flag("--split") {
            app.add_player();
        }
        match arg("--open").as_deref() {
            Some("inventory") => app.players[0].ui = player::OpenUi::Inventory,
            Some("crafting") => app.players[0].ui = player::OpenUi::CraftingTable,
            _ => {}
        }
        if let Some(p) = arg("--look") {
            let v: Vec<f32> = p.split(',').filter_map(|s| s.trim().parse().ok()).collect();
            if v.len() == 2 {
                app.players[0].yaw = v[0].to_radians();
                app.players[0].pitch = v[1].to_radians();
            }
        }
        if let Some(t) = arg("--time").and_then(|s| s.parse::<f64>().ok()) {
            app.daytime.time = t * daytime::DAY_LENGTH_SECS;
        }
    }
    let max_frames: u64 = arg("--frames").and_then(|s| s.parse().ok()).unwrap_or(600);
    let mut frame_count: u64 = 0;
    let mut bench_frames: u64 = 0;
    let mut bench_start: Option<std::time::Instant> = None;
    let mut bench_min_fps = f32::MAX;
    let mut bench_last = std::time::Instant::now();
    let mut idle_frames: u64 = 0;
    let mut title_timer = std::time::Instant::now();

    let set_grab = |window: &winit::window::Window, grab: bool| {
        if grab {
            let _ = window.set_cursor_grab(CursorGrabMode::Confined).or_else(|_| window.set_cursor_grab(CursorGrabMode::Locked));
            window.set_cursor_visible(false);
        } else {
            let _ = window.set_cursor_grab(CursorGrabMode::None);
            window.set_cursor_visible(true);
        }
    };

    event_loop
        .run(move |event, elwt| match event {
            Event::WindowEvent { event, .. } => {
                app.input.handle_window_event(&event);
                match event {
                    WindowEvent::CloseRequested => {
                        app.on_quit();
                        elwt.exit();
                    }
                    WindowEvent::Resized(size) => app.resize(size.width, size.height),
                    WindowEvent::MouseInput { state: winit::event::ElementState::Pressed, .. } => {
                        if !app.cursor_grabbed && app.wants_grab() {
                            app.cursor_grabbed = true;
                            set_grab(&window, true);
                        }
                    }
                    WindowEvent::Focused(false) => {
                        app.cursor_grabbed = false;
                        set_grab(&window, false);
                    }
                    WindowEvent::RedrawRequested => {
                        app.update();
                        if app.cursor_grabbed && !app.wants_grab() {
                            app.cursor_grabbed = false;
                            set_grab(&window, false);
                        }
                        if let Some(fs) = app.want_fullscreen.take() {
                            window.set_fullscreen(if fs { Some(Fullscreen::Borderless(None)) } else { None });
                        }
                        if app.quit {
                            app.on_quit();
                            elwt.exit();
                            return;
                        }
                        app.render();
                        frame_count += 1;
                        if benchmark {
                            let now = std::time::Instant::now();
                            let ft = (now - bench_last).as_secs_f32();
                            bench_last = now;
                            if app.is_idle() && frame_count > 60 {
                                if bench_start.is_none() {
                                    bench_start = Some(now);
                                    app.players[0].yaw += 0.0;
                                }
                                bench_frames += 1;
                                bench_min_fps = bench_min_fps.min(1.0 / ft.max(1e-4));
                                // rotate slowly so culling changes
                                app.players[0].yaw += 0.01;
                                if bench_frames >= max_frames {
                                    let secs = (now - bench_start.unwrap()).as_secs_f32();
                                    println!("BENCH rd={} frames={} avg_fps={:.1} min_fps={:.1} chunks={} subs_drawn={} quads={}", app.settings.render_distance, bench_frames, bench_frames as f32 / secs, bench_min_fps, app.world.chunk_count(), app.chunk_renderer.stats_drawn, app.chunk_renderer.stats_quads);
                                    elwt.exit();
                                }
                            }
                        }
                        if let Some(path) = &screenshot {
                            idle_frames = if app.is_idle() { idle_frames + 1 } else { 0 };
                            if (idle_frames > 40 && frame_count > 60) || frame_count >= max_frames {
                                app.screenshot(path);
                                println!("{}", app.stats);
                                app.on_quit();
                                elwt.exit();
                            }
                        }
                        if title_timer.elapsed().as_secs_f32() > 0.5 {
                            window.set_title(&app.stats);
                            title_timer = std::time::Instant::now();
                        }
                    }
                    _ => {}
                }
            }
            Event::DeviceEvent { event: DeviceEvent::MouseMotion { delta }, .. } => {
                app.input.handle_mouse_motion(delta.0, delta.1);
            }
            Event::AboutToWait => window.request_redraw(),
            _ => {}
        })
        .expect("event loop run");
}

/// Debug helper: print where each biome, the highest point, a cave and lava can be found.
fn find_biomes(seed: u64) {
    let g = world::gen::Generator::new(seed);
    let mut seen = std::collections::BTreeMap::new();
    for r in (0..3000).step_by(24) {
        for i in 0..16 {
            let ang = i as f64 / 16.0 * std::f64::consts::TAU;
            let x = (ang.cos() * r as f64) as i32;
            let z = (ang.sin() * r as f64) as i32;
            let c = g.column(x, z);
            seen.entry(c.biome.name()).or_insert((x, c.height, z));
        }
    }
    for (name, (x, h, z)) in seen {
        println!("{name}: x={x} y={h} z={z}");
    }
    let mut best = (0, 0, 0);
    for z in (-1500..1500).step_by(16) {
        for x in (-1500..1500).step_by(16) {
            let h = g.column(x, z).height;
            if h > best.1 {
                best = (x, h, z);
            }
        }
    }
    println!("highest: x={} y={} z={}", best.0, best.1, best.2);
    let mut cave: Option<(i32, i32, i32, usize)> = None;
    let mut lava: Option<(i32, i32, i32)> = None;
    for cz in -6..6 {
        for cx in -6..6 {
            let c = g.generate(cx, cz);
            for z in 0..16 {
                for x in 0..16 {
                    let mut run = 0usize;
                    for y in 5..48 {
                        let b = c.get_block(x, y, z);
                        if b == world::block::Block::Air {
                            run += 1;
                            if run >= 3 && cave.map(|c| c.3 < run).unwrap_or(true) {
                                cave = Some((cx * 16 + x as i32, y as i32 - run as i32 + 1, cz * 16 + z as i32, run));
                            }
                        } else {
                            run = 0;
                        }
                        if b == world::block::Block::Lava && lava.is_none() && c.get_block(x, y + 1, z) == world::block::Block::Air {
                            lava = Some((cx * 16 + x as i32, y as i32, cz * 16 + z as i32));
                        }
                    }
                }
            }
        }
    }
    if let Some((x, y, z, run)) = cave {
        println!("cave: x={x} y={y} z={z} (air run {run})");
    }
    if let Some((x, y, z)) = lava {
        println!("lava: x={x} y={y} z={z}");
    }
}
