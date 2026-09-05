//! Maps keyboard/mouse (player 1), gamepad or keyboard fallback (player 2) to `PlayerInput`,
//! plus menu and container-screen input.

use crate::app::App;
use crate::player::{OpenUi, PlayerInput};
use crate::ui::menu::MenuInput;
use crate::ui::screens::ScreenInput;
use gilrs::{Axis, Button};
use winit::keyboard::KeyCode;

const DEADZONE: f32 = 0.2;
const STICK_LOOK_SPEED: f32 = 1500.0; // "pixels" per second at full deflection
const KEY_LOOK_SPEED: f32 = 700.0;
const CURSOR_SPEED: f32 = 320.0; // gui units per second

fn dz(v: f32) -> f32 {
    if v.abs() < DEADZONE {
        0.0
    } else {
        (v - DEADZONE * v.signum()) / (1.0 - DEADZONE)
    }
}

impl App {
    /// Drain gamepad events and remember button edges.
    pub(crate) fn poll_gamepad(&mut self) {
        if let Some(g) = self.gilrs.as_mut() {
            while g.next_event().is_some() {}
        }
    }

    pub(crate) fn gamepad_connected(&self) -> bool {
        self.gilrs.as_ref().map(|g| g.gamepads().next().is_some()).unwrap_or(false)
    }

    fn pad_pressed(&self, b: Button) -> bool {
        self.gilrs.as_ref().and_then(|g| g.gamepads().next().map(|(_, gp)| gp.is_pressed(b))).unwrap_or(false)
    }

    fn pad_axis(&self, a: Axis) -> f32 {
        self.gilrs.as_ref().and_then(|g| g.gamepads().next().map(|(_, gp)| dz(gp.value(a)))).unwrap_or(0.0)
    }

    /// Edge-triggered button press (uses `pad_prev`, updated in `end_pad_frame`).
    pub(crate) fn pad_just(&self, b: Button) -> bool {
        self.pad_pressed(b) && !self.pad_prev.contains(&b)
    }

    pub(crate) fn end_pad_frame(&mut self) {
        let buttons = [Button::South, Button::East, Button::North, Button::West, Button::LeftTrigger, Button::RightTrigger, Button::LeftTrigger2, Button::RightTrigger2, Button::Start, Button::Select, Button::LeftThumb, Button::RightThumb];
        self.pad_prev.clear();
        for b in buttons {
            if self.pad_pressed(b) {
                self.pad_prev.insert(b);
            }
        }
    }

    /// Keyboard + mouse mapping for player 1.
    pub(crate) fn keyboard_input(&self) -> PlayerInput {
        let inp = &self.input;
        let k = |c: KeyCode| inp.pressed(c);
        let mut pi = PlayerInput::default();
        if !self.cursor_grabbed {
            return pi;
        }
        pi.forward = (k(KeyCode::KeyW) as i32 - k(KeyCode::KeyS) as i32) as f32;
        pi.strafe = (k(KeyCode::KeyD) as i32 - k(KeyCode::KeyA) as i32) as f32;
        pi.jump = k(KeyCode::Space);
        pi.jump_pressed = inp.just(KeyCode::Space);
        pi.sneak = k(KeyCode::ShiftLeft);
        pi.sprint = k(KeyCode::ControlLeft);
        pi.look_dx = inp.mouse_delta.0;
        pi.look_dy = inp.mouse_delta.1;
        pi.attack = inp.mouse_down[0];
        pi.attack_pressed = inp.mouse_just_pressed[0];
        pi.use_pressed = inp.mouse_just_pressed[1] || (inp.mouse_down[1] && self.frame % 12 == 0);
        pi.use_held = inp.mouse_down[1];
        pi.pick_block = inp.mouse_just_pressed[2];
        pi.scroll = inp.scroll.round() as i32;
        pi.inventory = inp.just(KeyCode::KeyE);
        pi.drop = inp.just(KeyCode::KeyQ);
        let digits = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6, KeyCode::Digit7, KeyCode::Digit8, KeyCode::Digit9];
        for (i, d) in digits.iter().enumerate() {
            if inp.just(*d) {
                pi.hotbar = Some(i);
            }
        }
        pi
    }

    /// Gamepad mapping (XInput via gilrs), falling back to arrows/IJKL/numpad.
    pub(crate) fn player2_input(&self, dt: f32) -> PlayerInput {
        let mut pi = PlayerInput::default();
        if self.gamepad_connected() {
            pi.forward = self.pad_axis(Axis::LeftStickY);
            pi.strafe = self.pad_axis(Axis::LeftStickX);
            pi.look_dx = self.pad_axis(Axis::RightStickX) * STICK_LOOK_SPEED * dt;
            pi.look_dy = -self.pad_axis(Axis::RightStickY) * STICK_LOOK_SPEED * dt;
            pi.jump = self.pad_pressed(Button::South);
            pi.jump_pressed = self.pad_just(Button::South);
            pi.sneak = self.pad_pressed(Button::East);
            pi.sprint = self.pad_pressed(Button::LeftThumb);
            pi.attack = self.pad_pressed(Button::RightTrigger2);
            pi.attack_pressed = self.pad_just(Button::RightTrigger2);
            pi.use_held = self.pad_pressed(Button::LeftTrigger2);
            pi.use_pressed = self.pad_just(Button::LeftTrigger2) || (pi.use_held && self.frame % 12 == 0);
            pi.inventory = self.pad_just(Button::North);
            pi.drop = self.pad_just(Button::West);
            pi.scroll = self.pad_just(Button::RightTrigger) as i32 * -1 + self.pad_just(Button::LeftTrigger) as i32;
            pi.pick_block = self.pad_just(Button::RightThumb);
        } else {
            let inp = &self.input;
            let k = |c: KeyCode| inp.pressed(c);
            pi.forward = (k(KeyCode::ArrowUp) as i32 - k(KeyCode::ArrowDown) as i32) as f32;
            pi.strafe = (k(KeyCode::ArrowRight) as i32 - k(KeyCode::ArrowLeft) as i32) as f32;
            pi.look_dx = (k(KeyCode::KeyL) as i32 - k(KeyCode::KeyJ) as i32) as f32 * KEY_LOOK_SPEED * dt;
            pi.look_dy = (k(KeyCode::KeyK) as i32 - k(KeyCode::KeyI) as i32) as f32 * KEY_LOOK_SPEED * dt;
            pi.jump = k(KeyCode::Numpad0);
            pi.jump_pressed = inp.just(KeyCode::Numpad0);
            pi.sneak = k(KeyCode::Numpad1);
            pi.sprint = k(KeyCode::Numpad2);
            pi.attack = k(KeyCode::Numpad4);
            pi.attack_pressed = inp.just(KeyCode::Numpad4);
            pi.use_held = k(KeyCode::Numpad6);
            pi.use_pressed = inp.just(KeyCode::Numpad6) || (pi.use_held && self.frame % 12 == 0);
            pi.inventory = inp.just(KeyCode::Numpad5);
            pi.drop = inp.just(KeyCode::Numpad3);
            pi.scroll = inp.just(KeyCode::Numpad9) as i32 * -1 + inp.just(KeyCode::Numpad7) as i32;
        }
        pi
    }

    /// Container-screen input for a player. Player 1 uses the mouse; player 2 a virtual cursor.
    pub(crate) fn screen_input(&mut self, idx: usize, scale: f32, dt: f32) -> (ScreenInput, bool) {
        if idx == 0 {
            let close = self.input.just(KeyCode::Escape) || self.input.just(KeyCode::KeyE);
            self.players[0].cursor = (self.input.cursor.0 / scale, self.input.cursor.1 / scale);
            return (
                ScreenInput {
                    mx: self.players[0].cursor.0,
                    my: self.players[0].cursor.1,
                    left: self.input.mouse_just_pressed[0],
                    right: self.input.mouse_just_pressed[1],
                    shift: self.input.pressed(KeyCode::ShiftLeft),
                },
                close,
            );
        }
        let (dx, dy, left, right, shift, close) = if self.gamepad_connected() {
            (
                self.pad_axis(Axis::LeftStickX),
                -self.pad_axis(Axis::LeftStickY),
                self.pad_just(Button::South) || self.pad_just(Button::RightTrigger2),
                self.pad_just(Button::East) || self.pad_just(Button::LeftTrigger2),
                self.pad_pressed(Button::LeftThumb),
                self.pad_just(Button::North) || self.pad_just(Button::Start),
            )
        } else {
            let inp = &self.input;
            let k = |c: KeyCode| inp.pressed(c);
            (
                (k(KeyCode::KeyL) as i32 - k(KeyCode::KeyJ) as i32) as f32,
                (k(KeyCode::KeyK) as i32 - k(KeyCode::KeyI) as i32) as f32,
                inp.just(KeyCode::Numpad4),
                inp.just(KeyCode::Numpad6),
                k(KeyCode::Numpad1),
                inp.just(KeyCode::Numpad5),
            )
        };
        let p = &mut self.players[idx];
        p.cursor.0 += dx * CURSOR_SPEED * dt;
        p.cursor.1 += dy * CURSOR_SPEED * dt;
        (ScreenInput { mx: p.cursor.0, my: p.cursor.1, left, right, shift }, close)
    }

    pub(crate) fn menu_input(&self, scale: f32) -> MenuInput {
        let inp = &self.input;
        MenuInput {
            mx: inp.cursor.0 / scale,
            my: inp.cursor.1 / scale,
            click: inp.mouse_just_pressed[0],
            text: inp.text.clone(),
            backspace: inp.just(KeyCode::Backspace),
            enter: inp.just(KeyCode::Enter) || inp.just(KeyCode::NumpadEnter),
            escape: inp.just(KeyCode::Escape) || self.pad_just(Button::Start) || self.pad_just(Button::East),
            tab: inp.just(KeyCode::Tab),
        }
    }

    /// Zero out gameplay input while a UI is open (keeps respawn keys when dead).
    pub(crate) fn gate_input(&self, idx: usize, mut pi: PlayerInput) -> PlayerInput {
        let p = &self.players[idx];
        if p.ui != OpenUi::None {
            let dead = p.dead;
            let jump = pi.jump_pressed;
            let use_p = pi.use_pressed;
            pi = PlayerInput::default();
            if dead {
                pi.jump_pressed = jump;
                pi.use_pressed = use_p;
            }
        }
        pi
    }
}
