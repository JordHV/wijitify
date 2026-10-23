/// Settings tab for the shell: version, exit, and installed modules.
/// Slint `SettingsOverlay` uses the same index for the Wijitify page.
pub const WIJITIFY_SETTINGS_PAGE: i32 = 9;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Setup = 0,
    Home = 1,
}

impl Screen {
    pub fn as_i32(self) -> i32 {
        self as i32
    }
}

#[derive(Debug, Clone)]
pub struct ShellState {
    pub screen: Screen,
    pub active_module: String,
    pub settings_open: bool,
    pub settings_page: i32,
}

impl Default for ShellState {
    fn default() -> Self {
        Self {
            screen: Screen::Setup,
            active_module: "zalender".into(),
            settings_open: false,
            settings_page: 0,
        }
    }
}
