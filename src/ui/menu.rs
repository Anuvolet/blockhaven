//! Main menu, pause menu, world creation/selection and settings screens.

use crate::player::GameMode;
use crate::render::atlas::Tile;
use crate::render::ui2d::UiBatch;
use crate::settings::Settings;
use crate::ui::font;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Main,
    NewWorld,
    LoadWorld,
    Settings { from_pause: bool },
    Pause,
    Controls { from_pause: bool },
}

#[derive(Clone, Debug, PartialEq)]
pub enum MenuAction {
    None,
    NewWorld { name: String, seed: String, mode: GameMode, flat: bool },
    LoadWorld(String),
    DeleteWorld(String),
    Resume,
    SaveAndQuit,
    QuitApp,
    SettingsChanged,
    AddPlayer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Bid {
    NewWorld,
    LoadWorld,
    Settings,
    Controls,
    Quit,
    Create,
    Back,
    ToggleMode,
    ToggleFlat,
    Resume,
    SaveQuit,
    AddPlayer,
    Rd(i32),
    Fov(i32),
    Sens(i32),
    Vol(i32),
    Fullscreen,
    Vsync,
    World(usize),
    Play,
    Delete,
    FieldName,
    FieldSeed,
}

struct Button {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    label: String,
    id: Bid,
    enabled: bool,
}

pub struct MenuInput {
    pub mx: f32,
    pub my: f32,
    pub click: bool,
    pub text: Vec<char>,
    pub backspace: bool,
    pub enter: bool,
    pub escape: bool,
    pub tab: bool,
}

pub struct Menu {
    pub screen: Screen,
    pub name: String,
    pub seed: String,
    pub creative: bool,
    pub flat: bool,
    pub focus: usize,
    pub worlds: Vec<String>,
    pub selected_world: Option<usize>,
    pub in_game: bool,
    pub blink: f32,
    pub status: String,
}

impl Menu {
    pub fn new() -> Menu {
        Menu { screen: Screen::Main, name: "New World".to_string(), seed: String::new(), creative: false, flat: false, focus: 0, worlds: Vec::new(), selected_world: None, in_game: false, blink: 0.0, status: String::new() }
    }

    pub fn refresh_worlds(&mut self) {
        self.worlds = crate::save::list_worlds();
        if self.selected_world.map(|i| i >= self.worlds.len()).unwrap_or(false) {
            self.selected_world = None;
        }
    }

    fn buttons(&self, b: &UiBatch, settings: &Settings) -> Vec<Button> {
        let w = b.width;
        let h = b.height;
        let cx = w * 0.5;
        let mut v = Vec::new();
        let mut add = |x: f32, y: f32, bw: f32, label: String, id: Bid, enabled: bool| v.push(Button { x, y, w: bw, h: 20.0, label, id, enabled });
        match self.screen {
            Screen::Main => {
                let y = h * 0.42;
                add(cx - 100.0, y, 200.0, "New World".into(), Bid::NewWorld, true);
                add(cx - 100.0, y + 24.0, 200.0, "Load World".into(), Bid::LoadWorld, true);
                add(cx - 100.0, y + 48.0, 98.0, "Settings".into(), Bid::Settings, true);
                add(cx + 2.0, y + 48.0, 98.0, "Controls".into(), Bid::Controls, true);
                add(cx - 100.0, y + 78.0, 200.0, "Quit".into(), Bid::Quit, true);
            }
            Screen::NewWorld => {
                let y = h * 0.3;
                add(cx - 100.0, y + 12.0, 200.0, self.name.clone(), Bid::FieldName, true);
                add(cx - 100.0, y + 48.0, 200.0, self.seed.clone(), Bid::FieldSeed, true);
                add(cx - 100.0, y + 78.0, 200.0, format!("Game Mode: {}", if self.creative { "Creative" } else { "Survival" }), Bid::ToggleMode, true);
                add(cx - 100.0, y + 102.0, 200.0, format!("World Type: {}", if self.flat { "Flat" } else { "Default" }), Bid::ToggleFlat, true);
                add(cx - 100.0, y + 134.0, 98.0, "Create".into(), Bid::Create, !self.name.trim().is_empty());
                add(cx + 2.0, y + 134.0, 98.0, "Back".into(), Bid::Back, true);
            }
            Screen::LoadWorld => {
                let y = h * 0.2;
                for (i, name) in self.worlds.iter().enumerate().take(8) {
                    add(cx - 100.0, y + 14.0 + i as f32 * 22.0, 200.0, name.clone(), Bid::World(i), true);
                }
                let by = (y + 14.0 + 8.0 * 22.0).min(h - 30.0);
                add(cx - 100.0, by, 64.0, "Play".into(), Bid::Play, self.selected_world.is_some());
                add(cx - 32.0, by, 64.0, "Delete".into(), Bid::Delete, self.selected_world.is_some());
                add(cx + 36.0, by, 64.0, "Back".into(), Bid::Back, true);
            }
            Screen::Settings { .. } => {
                let y = h * 0.22;
                let rows: [(String, Bid, Bid); 4] = [
                    (format!("Render Distance: {}", settings.render_distance), Bid::Rd(-1), Bid::Rd(1)),
                    (format!("FOV: {:.0}", settings.fov), Bid::Fov(-5), Bid::Fov(5)),
                    (format!("Sensitivity: {:.0}%", settings.sensitivity * 100.0), Bid::Sens(-10), Bid::Sens(10)),
                    (format!("Volume: {:.0}%", settings.volume * 100.0), Bid::Vol(-10), Bid::Vol(10)),
                ];
                for (i, (label, dec, inc)) in rows.iter().enumerate() {
                    let ry = y + i as f32 * 24.0;
                    add(cx - 100.0, ry, 20.0, "<".into(), *dec, true);
                    add(cx - 78.0, ry, 156.0, label.clone(), Bid::Back, false);
                    add(cx + 80.0, ry, 20.0, ">".into(), *inc, true);
                }
                add(cx - 100.0, y + 96.0, 98.0, format!("Fullscreen: {}", if settings.fullscreen { "On" } else { "Off" }), Bid::Fullscreen, true);
                add(cx + 2.0, y + 96.0, 98.0, format!("VSync: {}", if settings.vsync { "On" } else { "Off" }), Bid::Vsync, true);
                add(cx - 100.0, y + 130.0, 200.0, "Done".into(), Bid::Back, true);
            }
            Screen::Pause => {
                let y = h * 0.3;
                add(cx - 100.0, y, 200.0, "Back to Game".into(), Bid::Resume, true);
                add(cx - 100.0, y + 24.0, 200.0, "Add Second Player (F2)".into(), Bid::AddPlayer, true);
                add(cx - 100.0, y + 48.0, 98.0, "Settings".into(), Bid::Settings, true);
                add(cx + 2.0, y + 48.0, 98.0, "Controls".into(), Bid::Controls, true);
                add(cx - 100.0, y + 78.0, 200.0, "Save and Quit to Title".into(), Bid::SaveQuit, true);
            }
            Screen::Controls { .. } => {
                add(cx - 100.0, h - 40.0, 200.0, "Back".into(), Bid::Back, true);
            }
        }
        v
    }

    pub fn update(&mut self, input: &MenuInput, b: &UiBatch, settings: &mut Settings, dt: f32) -> MenuAction {
        self.blink += dt;
        // text entry
        if self.screen == Screen::NewWorld {
            let field = if self.focus == 0 { &mut self.name } else { &mut self.seed };
            for c in &input.text {
                if field.chars().count() < 24 && (c.is_alphanumeric() || *c == ' ' || *c == '-' || *c == '_') {
                    field.push(*c);
                }
            }
            if input.backspace {
                field.pop();
            }
            if input.tab {
                self.focus = (self.focus + 1) % 2;
            }
            if input.enter && !self.name.trim().is_empty() {
                return MenuAction::NewWorld { name: self.name.trim().to_string(), seed: self.seed.clone(), mode: if self.creative { GameMode::Creative } else { GameMode::Survival }, flat: self.flat };
            }
        }
        if input.escape {
            return match self.screen {
                Screen::Main => MenuAction::None,
                Screen::Pause => MenuAction::Resume,
                Screen::Settings { from_pause } | Screen::Controls { from_pause } => {
                    self.screen = if from_pause { Screen::Pause } else { Screen::Main };
                    MenuAction::SettingsChanged
                }
                _ => {
                    self.screen = Screen::Main;
                    MenuAction::None
                }
            };
        }
        if !input.click {
            return MenuAction::None;
        }
        let buttons = self.buttons(b, settings);
        let hit = buttons.iter().find(|bt| bt.enabled && input.mx >= bt.x && input.mx < bt.x + bt.w && input.my >= bt.y && input.my < bt.y + bt.h);
        let Some(bt) = hit else { return MenuAction::None };
        match bt.id {
            Bid::NewWorld => {
                self.screen = Screen::NewWorld;
                self.focus = 0;
            }
            Bid::LoadWorld => {
                self.refresh_worlds();
                self.screen = Screen::LoadWorld;
            }
            Bid::Settings => self.screen = Screen::Settings { from_pause: self.screen == Screen::Pause },
            Bid::Controls => self.screen = Screen::Controls { from_pause: self.screen == Screen::Pause },
            Bid::Quit => return MenuAction::QuitApp,
            Bid::Create => {
                return MenuAction::NewWorld { name: self.name.trim().to_string(), seed: self.seed.clone(), mode: if self.creative { GameMode::Creative } else { GameMode::Survival }, flat: self.flat };
            }
            Bid::Back => {
                let from_pause = matches!(self.screen, Screen::Settings { from_pause: true } | Screen::Controls { from_pause: true });
                let was_settings = matches!(self.screen, Screen::Settings { .. });
                self.screen = if from_pause { Screen::Pause } else { Screen::Main };
                if was_settings {
                    return MenuAction::SettingsChanged;
                }
            }
            Bid::ToggleMode => self.creative = !self.creative,
            Bid::ToggleFlat => self.flat = !self.flat,
            Bid::Resume => return MenuAction::Resume,
            Bid::SaveQuit => return MenuAction::SaveAndQuit,
            Bid::AddPlayer => return MenuAction::AddPlayer,
            Bid::Rd(d) => {
                settings.render_distance = (settings.render_distance + d).clamp(2, 32);
                return MenuAction::SettingsChanged;
            }
            Bid::Fov(d) => {
                settings.fov = (settings.fov + d as f32).clamp(30.0, 110.0);
                return MenuAction::SettingsChanged;
            }
            Bid::Sens(d) => {
                settings.sensitivity = (settings.sensitivity + d as f32 / 100.0).clamp(0.1, 3.0);
                return MenuAction::SettingsChanged;
            }
            Bid::Vol(d) => {
                settings.volume = (settings.volume + d as f32 / 100.0).clamp(0.0, 1.0);
                return MenuAction::SettingsChanged;
            }
            Bid::Fullscreen => {
                settings.fullscreen = !settings.fullscreen;
                return MenuAction::SettingsChanged;
            }
            Bid::Vsync => {
                settings.vsync = !settings.vsync;
                return MenuAction::SettingsChanged;
            }
            Bid::World(i) => self.selected_world = Some(i),
            Bid::Play => {
                if let Some(i) = self.selected_world {
                    return MenuAction::LoadWorld(self.worlds[i].clone());
                }
            }
            Bid::Delete => {
                if let Some(i) = self.selected_world {
                    let name = self.worlds[i].clone();
                    self.selected_world = None;
                    return MenuAction::DeleteWorld(name);
                }
            }
            Bid::FieldName => self.focus = 0,
            Bid::FieldSeed => self.focus = 1,
        }
        MenuAction::None
    }

    pub fn draw(&self, b: &mut UiBatch, settings: &Settings, mx: f32, my: f32) {
        let w = b.width;
        let h = b.height;
        if self.in_game {
            b.rect(0.0, 0.0, w, h, [0.0, 0.0, 0.0, 0.6]);
        } else {
            // dirt background
            let n = 32.0;
            let mut y = 0.0;
            while y < h {
                let mut x = 0.0;
                while x < w {
                    b.tile(x, y, n, n, Tile::Dirt, [0.45, 0.45, 0.45, 1.0]);
                    x += n;
                }
                y += n;
            }
        }
        let cx = w * 0.5;
        match self.screen {
            Screen::Main => {
                b.text_centered(cx, h * 0.18, 4.0, "BLOCKHAVEN", [1.0, 1.0, 0.85, 1.0]);
                b.text_centered(cx, h * 0.18 + 36.0, 1.0, "a procedural voxel sandbox - every texture and sound is generated at startup", [0.85, 0.85, 0.85, 1.0]);
                b.text(2.0, h - 10.0, 1.0, "Blockhaven 0.1", [0.8, 0.8, 0.8, 1.0]);
            }
            Screen::NewWorld => {
                b.text_centered(cx, h * 0.3 - 20.0, 2.0, "Create New World", [1.0; 4]);
                b.text(cx - 100.0, h * 0.3 + 2.0, 1.0, "World Name", [0.8, 0.8, 0.8, 1.0]);
                b.text(cx - 100.0, h * 0.3 + 38.0, 1.0, "Seed (blank = random)", [0.8, 0.8, 0.8, 1.0]);
                b.text_centered(cx, h * 0.3 + 160.0, 1.0, "Tab switches field, Enter creates", [0.7, 0.7, 0.7, 1.0]);
            }
            Screen::LoadWorld => {
                b.text_centered(cx, h * 0.2 - 12.0, 2.0, "Select World", [1.0; 4]);
                if self.worlds.is_empty() {
                    b.text_centered(cx, h * 0.2 + 40.0, 1.0, "No saved worlds yet", [0.8, 0.8, 0.8, 1.0]);
                }
            }
            Screen::Settings { .. } => {
                b.text_centered(cx, h * 0.22 - 24.0, 2.0, "Settings", [1.0; 4]);
            }
            Screen::Pause => {
                b.text_centered(cx, h * 0.3 - 24.0, 2.0, "Game Paused", [1.0; 4]);
            }
            Screen::Controls { .. } => {
                b.text_centered(cx, 12.0, 2.0, "Controls", [1.0; 4]);
                let lines = [
                    "Player 1 (keyboard + mouse)",
                    "  WASD move  Space jump  Shift sneak  Ctrl sprint",
                    "  Mouse look  LMB break/attack  RMB place/use",
                    "  1-9 / wheel hotbar  E inventory  Q drop  Esc pause",
                    "  Double-tap Space toggles flight (creative)",
                    "  F2 add second player  F3 debug overlay  F11 fullscreen",
                    "",
                    "Player 2 (gamepad, or keyboard fallback)",
                    "  Left stick move  Right stick look  A jump  B sneak",
                    "  LS sprint  RT/LT break/place  Y inventory  X drop",
                    "  Bumpers change hotbar  Start pause",
                    "  Keyboard: Arrows move  IJKL look  Numpad0 jump",
                    "  Numpad1 sneak  Numpad2 sprint  Numpad4/6 break/place",
                    "  Numpad7/9 hotbar  Numpad5 inventory  Numpad3 drop",
                ];
                let mut y = 40.0;
                for l in lines {
                    b.text_shadow(cx - 150.0, y, 1.0, l, [0.9, 0.9, 0.9, 1.0]);
                    y += 11.0;
                }
            }
        }
        for bt in self.buttons(b, settings) {
            let hovered = bt.enabled && mx >= bt.x && mx < bt.x + bt.w && my >= bt.y && my < bt.y + bt.h;
            let is_field = matches!(bt.id, Bid::FieldName | Bid::FieldSeed);
            let selected_world = matches!(bt.id, Bid::World(i) if Some(i) == self.selected_world);
            if is_field {
                b.rect(bt.x, bt.y, bt.w, bt.h, [0.0, 0.0, 0.0, 1.0]);
                let focused = (bt.id == Bid::FieldName && self.focus == 0) || (bt.id == Bid::FieldSeed && self.focus == 1);
                b.rect_outline(bt.x - 1.0, bt.y - 1.0, bt.w + 2.0, bt.h + 2.0, 1.0, if focused { [1.0, 1.0, 1.0, 1.0] } else { [0.5, 0.5, 0.5, 1.0] });
                let cursor = if focused && (self.blink * 2.0) as i32 % 2 == 0 { "_" } else { "" };
                b.text(bt.x + 4.0, bt.y + 6.0, 1.0, &format!("{}{}", bt.label, cursor), [1.0; 4]);
            } else if selected_world {
                b.button(bt.x, bt.y, bt.w, bt.h, &bt.label, true, true);
                b.rect_outline(bt.x - 2.0, bt.y - 2.0, bt.w + 4.0, bt.h + 4.0, 1.0, [1.0, 1.0, 1.0, 1.0]);
            } else {
                b.button(bt.x, bt.y, bt.w, bt.h, &bt.label, hovered, bt.enabled);
            }
        }
        if !self.status.is_empty() {
            b.text_centered(cx, h - 20.0, 1.0, &self.status, [1.0, 0.6, 0.6, 1.0]);
        }
        let _ = font::ADVANCE;
    }
}

impl Default for Menu {
    fn default() -> Self {
        Menu::new()
    }
}
