//! Keyboard / mouse state tracking (per frame).

use std::collections::HashSet;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

#[derive(Default)]
pub struct Input {
    pub down: HashSet<KeyCode>,
    pub just_pressed: HashSet<KeyCode>,
    pub just_released: HashSet<KeyCode>,
    pub mouse_down: [bool; 3],
    pub mouse_just_pressed: [bool; 3],
    pub mouse_just_released: [bool; 3],
    pub mouse_delta: (f32, f32),
    pub cursor: (f32, f32),
    pub scroll: f32,
    pub text: Vec<char>,
    pub focused: bool,
}

impl Input {
    pub fn new() -> Input {
        Input { focused: true, ..Default::default() }
    }

    pub fn handle_window_event(&mut self, ev: &WindowEvent) {
        match ev {
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    match event.state {
                        ElementState::Pressed => {
                            if !self.down.contains(&code) || event.repeat {
                                self.just_pressed.insert(code);
                            }
                            self.down.insert(code);
                        }
                        ElementState::Released => {
                            self.down.remove(&code);
                            self.just_released.insert(code);
                        }
                    }
                }
                if event.state == ElementState::Pressed {
                    if let Some(t) = &event.text {
                        for ch in t.chars() {
                            if !ch.is_control() {
                                self.text.push(ch);
                            }
                        }
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let idx = match button {
                    MouseButton::Left => 0,
                    MouseButton::Right => 1,
                    MouseButton::Middle => 2,
                    _ => return,
                };
                match state {
                    ElementState::Pressed => {
                        self.mouse_down[idx] = true;
                        self.mouse_just_pressed[idx] = true;
                    }
                    ElementState::Released => {
                        self.mouse_down[idx] = false;
                        self.mouse_just_released[idx] = true;
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = (position.x as f32, position.y as f32);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.scroll += match delta {
                    MouseScrollDelta::LineDelta(_, y) => *y,
                    MouseScrollDelta::PixelDelta(p) => (p.y as f32) / 40.0,
                };
            }
            WindowEvent::Focused(f) => {
                self.focused = *f;
                if !f {
                    self.down.clear();
                    self.mouse_down = [false; 3];
                }
            }
            _ => {}
        }
    }

    pub fn handle_mouse_motion(&mut self, dx: f64, dy: f64) {
        self.mouse_delta.0 += dx as f32;
        self.mouse_delta.1 += dy as f32;
    }

    #[inline]
    pub fn pressed(&self, k: KeyCode) -> bool {
        self.down.contains(&k)
    }
    #[inline]
    pub fn just(&self, k: KeyCode) -> bool {
        self.just_pressed.contains(&k)
    }

    /// Clear per-frame state. Call at the end of each frame.
    pub fn end_frame(&mut self) {
        self.just_pressed.clear();
        self.just_released.clear();
        self.mouse_just_pressed = [false; 3];
        self.mouse_just_released = [false; 3];
        self.mouse_delta = (0.0, 0.0);
        self.scroll = 0.0;
        self.text.clear();
    }
}
