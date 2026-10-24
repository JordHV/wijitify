//! Long-press / right-click text edit menu for touch tablets.
//!
//! Slint's ContextMenuArea long-press is Android-only. On Wayland a finger hold
//! never becomes a right-click, so we watch winit pointer events and open the
//! shell menu when a press stays still long enough over a focused TextInput.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use slint::{ComponentHandle, Timer, TimerMode};
use slint::winit_030::{winit, EventResult, WinitWindowAccessor};

use super::bridge::AppWindow;

const HOLD_MS: u64 = 500;
const SLOP_PX: f64 = 12.0;

#[derive(Default)]
struct HoldState {
    /// Physical coords where the press began.
    origin: Option<(f64, f64)>,
    started: Option<Instant>,
    /// Latest cursor / touch position (physical).
    cursor: (f64, f64),
    armed: bool,
}

pub fn install(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    let hold = Rc::new(RefCell::new(HoldState::default()));
    let timer = Rc::new(RefCell::new(Timer::default()));

    let open_menu = Rc::new({
        let ui_weak = ui_weak.clone();
        let hold = hold.clone();
        let timer = timer.clone();
        move || {
            timer.borrow().stop();
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            if !ui.get_text_edit_focused() {
                hold.borrow_mut().armed = false;
                return;
            }
            let (px, py) = {
                let mut s = hold.borrow_mut();
                s.armed = false;
                s.origin = None;
                s.started = None;
                s.cursor
            };
            let scale = ui.window().scale_factor() as f64;
            let mut lx = px / scale;
            let mut ly = py / scale;
            let w = f64::from(ui.window().size().width) / scale;
            let h = f64::from(ui.window().size().height) / scale;
            // Keep the 188×208 menu on-screen (matches app.slint).
            lx = lx.clamp(0.0, (w - 188.0).max(0.0));
            ly = ly.clamp(0.0, (h - 220.0).max(0.0));
            ui.set_text_edit_menu_x(lx as f32);
            ui.set_text_edit_menu_y(ly as f32);
            ui.set_text_edit_menu_open(true);
        }
    });

    let cancel_hold = Rc::new({
        let hold = hold.clone();
        let timer = timer.clone();
        move || {
            timer.borrow().stop();
            let mut s = hold.borrow_mut();
            s.armed = false;
            s.origin = None;
            s.started = None;
        }
    });

    let start_hold = Rc::new({
        let hold = hold.clone();
        let timer = timer.clone();
        let open_menu = open_menu.clone();
        move |x: f64, y: f64| {
            {
                let mut s = hold.borrow_mut();
                s.origin = Some((x, y));
                s.cursor = (x, y);
                s.started = Some(Instant::now());
                s.armed = true;
            }
            let hold = hold.clone();
            let open_menu = open_menu.clone();
            timer.borrow().start(
                TimerMode::SingleShot,
                Duration::from_millis(HOLD_MS),
                move || {
                    let ok = {
                        let s = hold.borrow();
                        s.armed
                            && s.started
                                .is_some_and(|t| t.elapsed() >= Duration::from_millis(HOLD_MS))
                    };
                    if ok {
                        open_menu();
                    }
                },
            );
        }
    });

    let move_hold = Rc::new({
        let hold = hold.clone();
        let cancel_hold = cancel_hold.clone();
        move |x: f64, y: f64| {
            let mut s = hold.borrow_mut();
            s.cursor = (x, y);
            if let Some((ox, oy)) = s.origin {
                let dx = x - ox;
                let dy = y - oy;
                if dx * dx + dy * dy > SLOP_PX * SLOP_PX {
                    drop(s);
                    cancel_hold();
                }
            }
        }
    });

    ui.window().on_winit_window_event(move |_slint_win, event| {
        use winit::event::{ElementState, MouseButton, TouchPhase, WindowEvent};

        match event {
            WindowEvent::CursorMoved { position, .. } => {
                move_hold(position.x, position.y);
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                let (x, y) = hold.borrow().cursor;
                start_hold(x, y);
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Right,
                ..
            } => {
                cancel_hold();
                open_menu();
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                cancel_hold();
            }
            WindowEvent::Touch(touch) => match touch.phase {
                TouchPhase::Started => {
                    start_hold(touch.location.x, touch.location.y);
                }
                TouchPhase::Moved => {
                    move_hold(touch.location.x, touch.location.y);
                }
                TouchPhase::Ended | TouchPhase::Cancelled => {
                    cancel_hold();
                }
            },
            _ => {}
        }
        EventResult::Propagate
    });
}
