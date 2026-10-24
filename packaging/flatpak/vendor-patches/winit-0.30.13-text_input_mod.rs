//! Wayland text-input-v3 handling (winit 0.30.13 + Wijitify OSK backspace patch).
//!
//! GNOME's on-screen keyboard deletes via `delete_surrounding_text` once text-input
//! is enabled. Stock winit 0.30.13 never sends surrounding text and drops those
//! events, so OSK Backspace is a no-op. We report a one-char placeholder so GNOME
//! uses the delete path, then synthesize a Backspace key for Slint's TextInput.

use std::ops::Deref;

use sctk::globals::GlobalData;
use sctk::reexports::client::{Connection, Proxy, QueueHandle};

use sctk::reexports::client::globals::{BindError, GlobalList};
use sctk::reexports::client::protocol::wl_surface::WlSurface;
use sctk::reexports::client::{delegate_dispatch, Dispatch};
use sctk::reexports::protocols::wp::text_input::zv3::client::zwp_text_input_manager_v3::ZwpTextInputManagerV3;
use sctk::reexports::protocols::wp::text_input::zv3::client::zwp_text_input_v3::{
    ContentHint, ContentPurpose, Event as TextInputEvent, ZwpTextInputV3,
};

use crate::event::{ElementState, Ime, KeyEvent, WindowEvent};
use crate::keyboard::{Key, KeyCode, KeyLocation, NamedKey, PhysicalKey};
use crate::platform_impl::wayland;
use crate::platform_impl::wayland::state::WinitState;
use crate::platform_impl::wayland::DeviceId;
use crate::platform_impl::KeyEventExtra;
use crate::window::ImePurpose;

pub struct TextInputState {
    text_input_manager: ZwpTextInputManagerV3,
}

impl TextInputState {
    pub fn new(
        globals: &GlobalList,
        queue_handle: &QueueHandle<WinitState>,
    ) -> Result<Self, BindError> {
        let text_input_manager = globals.bind(queue_handle, 1..=1, GlobalData)?;
        Ok(Self { text_input_manager })
    }
}

impl Deref for TextInputState {
    type Target = ZwpTextInputManagerV3;

    fn deref(&self) -> &Self::Target {
        &self.text_input_manager
    }
}

impl Dispatch<ZwpTextInputManagerV3, GlobalData, WinitState> for TextInputState {
    fn event(
        _state: &mut WinitState,
        _proxy: &ZwpTextInputManagerV3,
        _event: <ZwpTextInputManagerV3 as Proxy>::Event,
        _data: &GlobalData,
        _conn: &Connection,
        _qhandle: &QueueHandle<WinitState>,
    ) {
    }
}

impl Dispatch<ZwpTextInputV3, TextInputData, WinitState> for TextInputState {
    fn event(
        state: &mut WinitState,
        text_input: &ZwpTextInputV3,
        event: <ZwpTextInputV3 as Proxy>::Event,
        data: &TextInputData,
        _conn: &Connection,
        _qhandle: &QueueHandle<WinitState>,
    ) {
        let windows = state.windows.get_mut();
        let mut text_input_data = data.inner.lock().unwrap();
        match event {
            TextInputEvent::Enter { surface } => {
                let window_id = wayland::make_wid(&surface);
                text_input_data.surface = Some(surface);

                let mut window = match windows.get(&window_id) {
                    Some(window) => window.lock().unwrap(),
                    None => return,
                };

                if window.ime_allowed() {
                    text_input.enable();
                    text_input.set_content_type_by_purpose(window.ime_purpose());
                    // Placeholder surrounding text: GNOME OSK only uses
                    // delete_surrounding_text when the cursor is not at offset 0.
                    text_input.set_surrounding_text("x".into(), 1, 1);
                    text_input.commit();
                    state.events_sink.push_window_event(WindowEvent::Ime(Ime::Enabled), window_id);
                }

                window.text_input_entered(text_input);
            },
            TextInputEvent::Leave { surface } => {
                text_input_data.surface = None;

                // Always issue a disable.
                text_input.disable();
                text_input.commit();

                let window_id = wayland::make_wid(&surface);

                // XXX this check is essential, because `leave` could have a
                // reference to nil surface...
                let mut window = match windows.get(&window_id) {
                    Some(window) => window.lock().unwrap(),
                    None => return,
                };

                window.text_input_left(text_input);

                state.events_sink.push_window_event(WindowEvent::Ime(Ime::Disabled), window_id);
            },
            TextInputEvent::PreeditString { text, cursor_begin, cursor_end } => {
                let text = text.unwrap_or_default();
                let cursor_begin = usize::try_from(cursor_begin)
                    .ok()
                    .and_then(|idx| text.is_char_boundary(idx).then_some(idx));
                let cursor_end = usize::try_from(cursor_end)
                    .ok()
                    .and_then(|idx| text.is_char_boundary(idx).then_some(idx));

                text_input_data.pending_preedit = Some(Preedit { text, cursor_begin, cursor_end })
            },
            TextInputEvent::CommitString { text } => {
                text_input_data.pending_preedit = None;
                text_input_data.pending_commit = text;
            },
            TextInputEvent::DeleteSurroundingText { before_length, after_length } => {
                // GNOME OSK Backspace. Stock winit ignored this entirely.
                text_input_data.pending_delete = before_length > 0 || after_length > 0;
            },
            TextInputEvent::Done { .. } => {
                let window_id = match text_input_data.surface.as_ref() {
                    Some(surface) => wayland::make_wid(surface),
                    None => return,
                };

                // Clear preedit, unless all we'll be doing next is sending a new preedit.
                if text_input_data.pending_commit.is_some()
                    || text_input_data.pending_preedit.is_none()
                {
                    state.events_sink.push_window_event(
                        WindowEvent::Ime(Ime::Preedit(String::new(), None)),
                        window_id,
                    );
                }

                // Send `Commit`.
                if let Some(text) = text_input_data.pending_commit.take() {
                    state
                        .events_sink
                        .push_window_event(WindowEvent::Ime(Ime::Commit(text)), window_id);
                }

                // Send preedit.
                if let Some(preedit) = text_input_data.pending_preedit.take() {
                    let cursor_range =
                        preedit.cursor_begin.map(|b| (b, preedit.cursor_end.unwrap_or(b)));

                    state.events_sink.push_window_event(
                        WindowEvent::Ime(Ime::Preedit(preedit.text, cursor_range)),
                        window_id,
                    );
                }

                // One Backspace per delete request (Slint deletes one char / selection).
                if text_input_data.pending_delete {
                    text_input_data.pending_delete = false;
                    let device_id =
                        crate::event::DeviceId(crate::platform_impl::DeviceId::Wayland(DeviceId));
                    for key_state in [ElementState::Pressed, ElementState::Released] {
                        let event = KeyEvent {
                            physical_key: PhysicalKey::Code(KeyCode::Backspace),
                            logical_key: Key::Named(NamedKey::Backspace),
                            text: None,
                            location: KeyLocation::Standard,
                            state: key_state,
                            repeat: false,
                            platform_specific: KeyEventExtra {
                                text_with_all_modifiers: None,
                                key_without_modifiers: Key::Named(NamedKey::Backspace),
                            },
                        };
                        state.events_sink.push_window_event(
                            WindowEvent::KeyboardInput {
                                device_id,
                                event,
                                is_synthetic: false,
                            },
                            window_id,
                        );
                    }
                    // Refresh so hold-to-delete keeps receiving surrounding-text-set.
                    text_input.set_surrounding_text("x".into(), 1, 1);
                    text_input.commit();
                }
            },
            _ => {},
        }
    }
}

pub trait ZwpTextInputV3Ext {
    fn set_content_type_by_purpose(&self, purpose: ImePurpose);
}

impl ZwpTextInputV3Ext for ZwpTextInputV3 {
    fn set_content_type_by_purpose(&self, purpose: ImePurpose) {
        let (hint, purpose) = match purpose {
            ImePurpose::Normal => (ContentHint::None, ContentPurpose::Normal),
            ImePurpose::Password => (ContentHint::SensitiveData, ContentPurpose::Password),
            ImePurpose::Terminal => (ContentHint::None, ContentPurpose::Terminal),
        };
        self.set_content_type(hint, purpose);
    }
}

/// The Data associated with the text input.
#[derive(Default)]
pub struct TextInputData {
    inner: std::sync::Mutex<TextInputDataInner>,
}

#[derive(Default)]
pub struct TextInputDataInner {
    /// The `WlSurface` we're performing input to.
    surface: Option<WlSurface>,

    /// The commit to submit on `done`.
    pending_commit: Option<String>,

    /// The preedit to submit on `done`.
    pending_preedit: Option<Preedit>,

    /// OSK requested delete_surrounding_text (apply on `done` as Backspace).
    pending_delete: bool,
}

/// The state of the preedit.
struct Preedit {
    text: String,
    cursor_begin: Option<usize>,
    cursor_end: Option<usize>,
}

delegate_dispatch!(WinitState: [ZwpTextInputManagerV3: GlobalData] => TextInputState);
delegate_dispatch!(WinitState: [ZwpTextInputV3: TextInputData] => TextInputState);
