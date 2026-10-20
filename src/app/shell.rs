#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Setup = 0,
    Home = 1,
    Settings = 2,
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
}

impl Default for ShellState {
    fn default() -> Self {
        Self {
            screen: Screen::Setup,
            active_module: "zalender".into(),
        }
    }
}
