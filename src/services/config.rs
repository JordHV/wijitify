use anyhow::{bail, Context, Result};
use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::storage::{self, Paths};

pub const SCHEMA_VERSION: u32 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
}

impl ThemeMode {
    pub fn is_dark(self) -> bool {
        matches!(self, Self::Dark)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Dark => "Dark",
            Self::Light => "Light",
        }
    }

    pub fn from_i32(v: i32) -> Self {
        if v == 0 {
            Self::Light
        } else {
            Self::Dark
        }
    }
}

/// Editable UI colors. Selecting Dark/Light resets these to mode defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemePalette {
    pub bg: String,
    pub surface: String,
    pub surface_2: String,
    pub surface_press: String,
    pub text: String,
    pub muted: String,
    pub accent: String,
    pub accent_press: String,
    pub danger: String,
    pub danger_press: String,
    pub ok: String,
    pub text_on_accent: String,
}

impl ThemePalette {
    pub fn for_mode(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Dark => Self {
                bg: "#0f1419".into(),
                surface: "#1a2332".into(),
                surface_2: "#243044".into(),
                surface_press: "#2a3850".into(),
                text: "#e7ecf3".into(),
                muted: "#8b9bb4".into(),
                accent: default_accent(),
                accent_press: "#2563eb".into(),
                danger: "#ef4444".into(),
                danger_press: "#dc2626".into(),
                ok: "#22c55e".into(),
                text_on_accent: "#ffffff".into(),
            },
            ThemeMode::Light => Self {
                bg: "#f4f6f9".into(),
                surface: "#ffffff".into(),
                surface_2: "#e8ecf2".into(),
                surface_press: "#d8dee8".into(),
                text: "#1a2332".into(),
                muted: "#5a6a80".into(),
                accent: default_accent(),
                accent_press: "#2563eb".into(),
                danger: "#ef4444".into(),
                danger_press: "#dc2626".into(),
                ok: "#16a34a".into(),
                text_on_accent: "#ffffff".into(),
            },
        }
    }

    pub fn labels() -> &'static [(&'static str, &'static str)] {
        &[
            ("bg", "Background"),
            ("surface", "Surface"),
            ("surface_2", "Surface 2"),
            ("surface_press", "Press"),
            ("text", "Text"),
            ("muted", "Muted text"),
            ("accent", "Accent"),
            ("accent_press", "Accent press"),
            ("danger", "Danger"),
            ("danger_press", "Danger press"),
            ("ok", "OK / success"),
            ("text_on_accent", "Text on accent"),
        ]
    }

    pub fn get(&self, key: &str) -> &str {
        match key {
            "bg" => &self.bg,
            "surface" => &self.surface,
            "surface_2" => &self.surface_2,
            "surface_press" => &self.surface_press,
            "text" => &self.text,
            "muted" => &self.muted,
            "accent" => &self.accent,
            "accent_press" => &self.accent_press,
            "danger" => &self.danger,
            "danger_press" => &self.danger_press,
            "ok" => &self.ok,
            "text_on_accent" => &self.text_on_accent,
            _ => &self.accent,
        }
    }

    pub fn set(&mut self, key: &str, hex: String) {
        match key {
            "bg" => self.bg = hex,
            "surface" => self.surface = hex,
            "surface_2" => self.surface_2 = hex,
            "surface_press" => self.surface_press = hex,
            "text" => self.text = hex,
            "muted" => self.muted = hex,
            "accent" => self.accent = hex,
            "accent_press" => self.accent_press = hex,
            "danger" => self.danger = hex,
            "danger_press" => self.danger_press = hex,
            "ok" => self.ok = hex,
            "text_on_accent" => self.text_on_accent = hex,
            _ => {}
        }
    }
}

impl Default for ThemePalette {
    fn default() -> Self {
        Self::for_mode(ThemeMode::Dark)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ViewUnit {
    Day,
    Week,
    #[default]
    Month,
    Year,
    Agenda,
}

impl ViewUnit {
    pub fn label(self) -> &'static str {
        match self {
            Self::Day => "Day",
            Self::Week => "Week",
            Self::Month => "Month",
            Self::Year => "Year",
            Self::Agenda => "Agenda",
        }
    }

    pub fn max_count(self) -> u32 {
        match self {
            Self::Day => 7,
            Self::Week => 6,
            Self::Month => 6,
            Self::Year => 2,
            Self::Agenda => 4,
        }
    }

    pub fn zoom_out(self) -> Self {
        match self {
            Self::Day => Self::Week,
            Self::Week => Self::Month,
            Self::Month => Self::Year,
            Self::Year => Self::Year,
            Self::Agenda => Self::Month,
        }
    }

    pub fn as_i32(self) -> i32 {
        match self {
            Self::Day => 0,
            Self::Week => 1,
            Self::Month => 2,
            Self::Year => 3,
            Self::Agenda => 4,
        }
    }

    pub fn from_i32(v: i32) -> Self {
        match v {
            0 => Self::Day,
            1 => Self::Week,
            3 => Self::Year,
            4 => Self::Agenda,
            _ => Self::Month,
        }
    }
}

/// Sunday = 0 … Saturday = 6 (chrono weekday numbering offset).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum WeekStart {
    #[default]
    Sunday,
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
}

impl WeekStart {
    pub fn as_chrono_num(self) -> u32 {
        match self {
            Self::Sunday => 6,
            Self::Monday => 0,
            Self::Tuesday => 1,
            Self::Wednesday => 2,
            Self::Thursday => 3,
            Self::Friday => 4,
            Self::Saturday => 5,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Sunday => "Sunday",
            Self::Monday => "Monday",
            Self::Tuesday => "Tuesday",
            Self::Wednesday => "Wednesday",
            Self::Thursday => "Thursday",
            Self::Friday => "Friday",
            Self::Saturday => "Saturday",
        }
    }

    pub fn from_i32(v: i32) -> Self {
        match v {
            1 => Self::Monday,
            2 => Self::Tuesday,
            3 => Self::Wednesday,
            4 => Self::Thursday,
            5 => Self::Friday,
            6 => Self::Saturday,
            _ => Self::Sunday,
        }
    }

    pub fn as_i32(self) -> i32 {
        match self {
            Self::Sunday => 0,
            Self::Monday => 1,
            Self::Tuesday => 2,
            Self::Wednesday => 3,
            Self::Thursday => 4,
            Self::Friday => 5,
            Self::Saturday => 6,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AnchorMode {
    #[default]
    CurrentFirst,
    Sunday,
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
}

impl AnchorMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::CurrentFirst => "Current period first",
            Self::Sunday => "Sunday first",
            Self::Monday => "Monday first",
            Self::Tuesday => "Tuesday first",
            Self::Wednesday => "Wednesday first",
            Self::Thursday => "Thursday first",
            Self::Friday => "Friday first",
            Self::Saturday => "Saturday first",
        }
    }

    pub fn as_i32(self) -> i32 {
        match self {
            Self::CurrentFirst => 0,
            Self::Sunday => 1,
            Self::Monday => 2,
            Self::Tuesday => 3,
            Self::Wednesday => 4,
            Self::Thursday => 5,
            Self::Friday => 6,
            Self::Saturday => 7,
        }
    }

    pub fn from_i32(v: i32) -> Self {
        match v {
            1 => Self::Sunday,
            2 => Self::Monday,
            3 => Self::Tuesday,
            4 => Self::Wednesday,
            5 => Self::Thursday,
            6 => Self::Friday,
            7 => Self::Saturday,
            _ => Self::CurrentFirst,
        }
    }
}

/// UI typeface for the whole app. Faces other than System are bundled
/// (`assets/fonts/ui`, SIL OFL) so the choice resolves on every device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum UiFontFamily {
    #[default]
    System,
    Sans,
    Serif,
    Mono,
    Display,
    Condensed,
    Hand,
    Bungee,
}

impl UiFontFamily {
    pub fn all() -> [Self; 8] {
        [
            Self::System,
            Self::Sans,
            Self::Serif,
            Self::Mono,
            Self::Display,
            Self::Condensed,
            Self::Hand,
            Self::Bungee,
        ]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::System => "System default",
            Self::Sans => "Nunito",
            Self::Serif => "Libre Baskerville",
            Self::Mono => "IBM Plex Mono",
            Self::Display => "Playfair Display",
            Self::Condensed => "Oswald",
            Self::Hand => "Caveat",
            Self::Bungee => "Bungee",
        }
    }

    /// Empty string keeps the platform default font.
    /// Other names match the bundled font's typographic family.
    pub fn family_name(self) -> &'static str {
        match self {
            Self::System => "",
            Self::Sans => "Nunito",
            Self::Serif => "Libre Baskerville",
            Self::Mono => "IBM Plex Mono",
            Self::Display => "Playfair Display",
            Self::Condensed => "Oswald",
            Self::Hand => "Caveat",
            Self::Bungee => "Bungee",
        }
    }

    pub fn as_i32(self) -> i32 {
        Self::all().iter().position(|f| *f == self).unwrap_or(0) as i32
    }

    pub fn from_i32(v: i32) -> Self {
        Self::all().get(v as usize).copied().unwrap_or_default()
    }
}

/// App font size (shell, settings, calendar chrome). Event chip text uses `Config::event_font_scale`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FontScale {
    Small,
    #[default]
    Medium,
    Large,
    XLarge,
}

impl FontScale {
    pub fn label(self) -> &'static str {
        match self {
            Self::Small => "Small",
            Self::Medium => "Medium",
            Self::Large => "Large",
            Self::XLarge => "Extra large",
        }
    }

    pub fn factor(self) -> f32 {
        match self {
            Self::Small => 0.875,
            Self::Medium => 1.0,
            Self::Large => 1.15,
            Self::XLarge => 1.3,
        }
    }

    pub fn as_i32(self) -> i32 {
        match self {
            Self::Small => 0,
            Self::Medium => 1,
            Self::Large => 2,
            Self::XLarge => 3,
        }
    }

    pub fn from_i32(v: i32) -> Self {
        match v {
            0 => Self::Small,
            2 => Self::Large,
            3 => Self::XLarge,
            _ => Self::Medium,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DatetimeFormat {
    #[default]
    DdMmYyAmPm,
    MmDdYyAmPm,
    YyyyMmDd24h,
    DdMmYyyy24h,
}

impl DatetimeFormat {
    pub fn label(self) -> &'static str {
        match self {
            Self::DdMmYyAmPm => "DD/MM/YY hh:mm AM/PM",
            Self::MmDdYyAmPm => "MM/DD/YY hh:mm AM/PM",
            Self::YyyyMmDd24h => "YYYY-MM-DD HH:mm",
            Self::DdMmYyyy24h => "DD/MM/YYYY HH:mm",
        }
    }

    pub fn chrono_pattern(self) -> &'static str {
        match self {
            Self::DdMmYyAmPm => "%d/%m/%y %I:%M %p",
            Self::MmDdYyAmPm => "%m/%d/%y %I:%M %p",
            Self::YyyyMmDd24h => "%Y-%m-%d %H:%M",
            Self::DdMmYyyy24h => "%d/%m/%Y %H:%M",
        }
    }

    pub fn as_i32(self) -> i32 {
        match self {
            Self::DdMmYyAmPm => 0,
            Self::MmDdYyAmPm => 1,
            Self::YyyyMmDd24h => 2,
            Self::DdMmYyyy24h => 3,
        }
    }

    pub fn from_i32(v: i32) -> Self {
        match v {
            1 => Self::MmDdYyAmPm,
            2 => Self::YyyyMmDd24h,
            3 => Self::DdMmYyyy24h,
            _ => Self::DdMmYyAmPm,
        }
    }
}

/// Legacy single-preset shape (schema ≤ 4). Kept for migration only.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyViewPreset {
    #[serde(default)]
    pub unit: ViewUnit,
    #[serde(default = "default_preset_count")]
    pub count: u32,
    #[serde(default)]
    pub anchor: AnchorMode,
}

fn default_preset_count() -> u32 {
    1
}

fn default_day_start_hour() -> u32 {
    6
}

fn default_day_end_hour() -> u32 {
    21
}

fn default_day_interval_minutes() -> u32 {
    60
}

/// Sunday = 0 … Saturday = 6. Missing config shows every day.
fn default_show_days() -> [bool; 7] {
    [true; 7]
}

/// Week-count default. See `ViewPreset::month_weeks`.
fn default_month_weeks() -> u32 {
    6
}

fn default_title_lines() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewPreset {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub unit: ViewUnit,
    #[serde(default = "default_preset_count")]
    pub count: u32,
    #[serde(default)]
    pub anchor: AnchorMode,
    /// First hour shown on day/week grids (0–23).
    #[serde(default = "default_day_start_hour")]
    pub day_start_hour: u32,
    /// Last hour label shown on day/week grids (0–23, inclusive).
    #[serde(default = "default_day_end_hour")]
    pub day_end_hour: u32,
    /// Slot step in minutes (15, 30, 60, or 120).
    #[serde(default = "default_day_interval_minutes")]
    pub day_interval_minutes: u32,
    /// Weekdays included in this preset. Index 0 = Sunday … 6 = Saturday.
    #[serde(default = "default_show_days")]
    pub show_days: [bool; 7],
    /// With `month_from_this_week`, this many rows start on the viewed week.
    /// Otherwise the grid runs from the week of the 1st through the viewed week,
    /// plus this many weeks minus one after that week.
    #[serde(default = "default_month_weeks")]
    pub month_weeks: u32,
    /// Month grid starts on the week of the viewed date instead of the 1st.
    #[serde(default)]
    pub month_from_this_week: bool,
    /// How many lines an event title may use (1–10) before the last line ends with ….
    #[serde(default = "default_title_lines")]
    pub title_lines: u32,
    #[serde(default = "default_true")]
    pub visible: bool,
    #[serde(default)]
    pub builtin: bool,
}

impl ViewPreset {
    pub fn builtin(unit: ViewUnit) -> Self {
        let (id, name) = match unit {
            ViewUnit::Day => ("preset-day", "Day"),
            ViewUnit::Week => ("preset-week", "Week"),
            ViewUnit::Month => ("preset-month", "Month"),
            ViewUnit::Year => ("preset-year", "Year"),
            ViewUnit::Agenda => ("preset-agenda", "Agenda"),
        };
        Self {
            id: id.into(),
            name: name.into(),
            unit,
            count: 1,
            anchor: AnchorMode::CurrentFirst,
            day_start_hour: default_day_start_hour(),
            day_end_hour: default_day_end_hour(),
            day_interval_minutes: default_day_interval_minutes(),
            show_days: default_show_days(),
            month_weeks: default_month_weeks(),
            month_from_this_week: false,
            title_lines: default_title_lines(),
            visible: true,
            builtin: true,
        }
    }

    pub fn builtins() -> Vec<Self> {
        vec![
            Self::builtin(ViewUnit::Day),
            Self::builtin(ViewUnit::Week),
            Self::builtin(ViewUnit::Month),
            Self::builtin(ViewUnit::Year),
            Self::builtin(ViewUnit::Agenda),
        ]
    }

    pub fn clamp_count(&mut self) {
        // Month span is `month_weeks`, not a stack of month grids.
        if self.unit == ViewUnit::Month {
            self.count = 1;
            return;
        }
        let max = self.unit.max_count();
        if self.count < 1 {
            self.count = 1;
        }
        if self.count > max {
            self.count = max;
        }
    }

    pub fn clamp_day_hours(&mut self) {
        self.day_start_hour = self.day_start_hour.min(23);
        self.day_end_hour = self.day_end_hour.min(23);
        if self.day_end_hour < self.day_start_hour {
            self.day_end_hour = self.day_start_hour;
        }
        self.day_interval_minutes = match self.day_interval_minutes {
            15 | 30 | 60 | 120 => self.day_interval_minutes,
            _ => 60,
        };
    }

    pub fn clamp_month_weeks(&mut self) {
        self.month_weeks = self.month_weeks.clamp(1, 6);
    }

    pub fn clamp_title_lines(&mut self) {
        self.title_lines = self.title_lines.clamp(1, 10);
    }

    /// At least one weekday stays visible.
    pub fn clamp_show_days(&mut self) {
        if self.show_days.iter().all(|on| !on) {
            self.show_days = default_show_days();
        }
    }

    pub fn shows_date(&self, day: NaiveDate) -> bool {
        self.show_days
            .get(day.weekday().num_days_from_sunday() as usize)
            .copied()
            .unwrap_or(true)
    }

    /// Slot label with `[start_mins, end_mins)` range from midnight.
    pub fn hour_slots(&self) -> Vec<(String, u32, u32)> {
        let start = self.day_start_hour.min(23);
        let end = self.day_end_hour.min(23).max(start);
        let step = match self.day_interval_minutes {
            15 | 30 | 60 | 120 => self.day_interval_minutes,
            _ => 60,
        };
        let mut mins = start * 60;
        let end_mins = end * 60;
        let mut out = Vec::new();
        while mins <= end_mins {
            let next = mins.saturating_add(step);
            out.push((format_clock_label(mins, step < 60), mins, next));
            mins = next;
            if out.len() > 96 {
                break;
            }
        }
        if out.is_empty() {
            let m = start * 60;
            out.push((format_clock_label(m, false), m, m.saturating_add(step)));
        }
        out
    }

    pub fn display_label(&self) -> String {
        if self.count > 1 {
            format!("{} {}", self.name, self.count)
        } else {
            self.name.clone()
        }
    }
}

fn format_clock_label(mins_from_midnight: u32, show_minutes: bool) -> String {
    let h24 = (mins_from_midnight / 60) % 24;
    let m = mins_from_midnight % 60;
    let (h12, suffix) = match h24 {
        0 => (12, "AM"),
        1..=11 => (h24, "AM"),
        12 => (12, "PM"),
        _ => (h24 - 12, "PM"),
    };
    if show_minutes || m != 0 {
        format!("{h12}:{m:02} {suffix}")
    } else {
        format!("{h12} {suffix}")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DayModalFields {
    #[serde(default = "default_true")]
    pub show_time: bool,
    #[serde(default = "default_true")]
    pub show_title: bool,
    #[serde(default = "default_true")]
    pub show_calendar: bool,
    #[serde(default = "default_true")]
    pub show_description: bool,
}

impl Default for DayModalFields {
    fn default() -> Self {
        Self {
            show_time: true,
            show_title: true,
            show_calendar: true,
            show_description: true,
        }
    }
}

/// Global calendar display toggles (dashboard grids).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayOptions {
    #[serde(default = "default_true")]
    pub dim_past: bool,
    #[serde(default = "default_true")]
    pub shade_weekends: bool,
}

impl Default for DisplayOptions {
    fn default() -> Self {
        Self {
            dim_past: true,
            shade_weekends: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CustomFieldType {
    #[default]
    Text,
    LongText,
    Bool,
    Choice,
}

impl CustomFieldType {
    pub fn label(self) -> &'static str {
        match self {
            Self::Text => "Short text",
            Self::LongText => "Long text",
            Self::Bool => "Yes / no",
            Self::Choice => "Choice",
        }
    }

    pub fn as_i32(self) -> i32 {
        match self {
            Self::Text => 0,
            Self::LongText => 1,
            Self::Bool => 2,
            Self::Choice => 3,
        }
    }

    pub fn from_i32(v: i32) -> Self {
        match v {
            1 => Self::LongText,
            2 => Self::Bool,
            3 => Self::Choice,
            _ => Self::Text,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomFieldDef {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub field_type: CustomFieldType,
    #[serde(default)]
    pub options: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub schema_version: u32,
    pub setup_complete: bool,
    #[serde(default)]
    pub theme: ThemeMode,
    #[serde(default = "default_accent")]
    pub accent: String,
    #[serde(default)]
    pub palette: ThemePalette,
    #[serde(default)]
    pub datetime_format: DatetimeFormat,
    #[serde(default)]
    pub font_family: UiFontFamily,
    #[serde(default)]
    pub font_scale: FontScale,
    /// Event title and time text on the calendar main view.
    #[serde(default)]
    pub event_font_scale: FontScale,
    #[serde(default)]
    pub modules: BTreeMap<String, ModuleConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ModuleConfig {
    Zalender(ZalenderConfig),
    Generic(GenericModuleConfig),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericModuleConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZalenderConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_tz")]
    pub timezone: String,
    #[serde(default = "default_refresh")]
    pub refresh_minutes: u32,
    #[serde(default = "default_next_feed_id")]
    pub next_feed_id: u32,
    #[serde(default)]
    pub feeds: Vec<FeedConfig>,
    /// Writable feed new events are created on. Only one calendar is the default.
    #[serde(default = "default_feed_id")]
    pub default_feed_id: String,
    #[serde(default)]
    pub week_start: WeekStart,
    #[serde(default)]
    pub presets: Vec<ViewPreset>,
    #[serde(default)]
    pub hot_preset_id: String,
    #[serde(default = "default_next_preset_id")]
    pub next_preset_id: u32,
    /// Schema ≤4 single preset; consumed by migrate → 5.
    #[serde(default, rename = "preset", skip_serializing)]
    pub legacy_preset: Option<LegacyViewPreset>,
    #[serde(default)]
    pub day_modal: DayModalFields,
    #[serde(default)]
    pub display: DisplayOptions,
    #[serde(default)]
    pub custom_fields: Vec<CustomFieldDef>,
    #[serde(default = "default_next_field_id")]
    pub next_field_id: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedConfig {
    #[serde(default)]
    pub id: String,
    pub name: String,
    pub url: String,
    #[serde(default = "default_color")]
    pub color: String,
    #[serde(default = "default_true")]
    pub visible: bool,
    #[serde(default)]
    pub writable: bool,
    /// Built-in on-device calendar (not ICS; cannot be removed or shared).
    #[serde(default)]
    pub local: bool,
}

pub const LOCAL_FEED_ID: &str = "feed-local";
pub const LOCAL_FEED_URL: &str = "local://";

impl FeedConfig {
    pub fn local_default() -> Self {
        Self {
            id: LOCAL_FEED_ID.into(),
            name: String::new(),
            url: LOCAL_FEED_URL.into(),
            color: "#22c55e".into(),
            visible: true,
            writable: true,
            local: true,
        }
    }
}

fn default_true() -> bool {
    true
}
fn default_tz() -> String {
    "America/New_York".into()
}
fn default_refresh() -> u32 {
    15
}
fn default_color() -> String {
    "#3b82f6".into()
}
fn default_accent() -> String {
    "#3b82f6".into()
}
fn default_next_feed_id() -> u32 {
    1
}
fn default_feed_id() -> String {
    LOCAL_FEED_ID.into()
}
fn default_next_field_id() -> u32 {
    1
}
fn default_next_preset_id() -> u32 {
    1
}
fn default_hot_preset_id() -> String {
    "preset-month".into()
}

impl Default for Config {
    fn default() -> Self {
        let mut modules = BTreeMap::new();
        modules.insert(
            "zalender".into(),
            ModuleConfig::Zalender(ZalenderConfig::default()),
        );
        Self {
            schema_version: SCHEMA_VERSION,
            setup_complete: false,
            theme: ThemeMode::Dark,
            accent: default_accent(),
            palette: ThemePalette::for_mode(ThemeMode::Dark),
            datetime_format: DatetimeFormat::default(),
            font_family: UiFontFamily::default(),
            font_scale: FontScale::default(),
            event_font_scale: FontScale::default(),
            modules,
        }
    }
}

impl Default for ZalenderConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            timezone: default_tz(),
            refresh_minutes: default_refresh(),
            next_feed_id: 1,
            feeds: vec![FeedConfig::local_default()],
            default_feed_id: LOCAL_FEED_ID.into(),
            week_start: WeekStart::default(),
            presets: ViewPreset::builtins(),
            hot_preset_id: default_hot_preset_id(),
            next_preset_id: 1,
            legacy_preset: None,
            day_modal: DayModalFields::default(),
            display: DisplayOptions::default(),
            custom_fields: Vec::new(),
            next_field_id: 1,
        }
    }
}

impl ZalenderConfig {
    pub fn alloc_feed_id(&mut self) -> String {
        let id = format!("feed-{}", self.next_feed_id);
        self.next_feed_id = self.next_feed_id.saturating_add(1);
        id
    }

    pub fn alloc_field_id(&mut self) -> String {
        let id = format!("field-{}", self.next_field_id);
        self.next_field_id = self.next_field_id.saturating_add(1);
        id
    }

    pub fn alloc_preset_id(&mut self) -> String {
        let id = format!("preset-{}", self.next_preset_id);
        self.next_preset_id = self.next_preset_id.saturating_add(1);
        id
    }

    pub fn ensure_feed_ids(&mut self) {
        for feed in &mut self.feeds {
            if feed.id.is_empty() {
                feed.id = format!("feed-{}", self.next_feed_id);
                self.next_feed_id = self.next_feed_id.saturating_add(1);
            }
        }
    }

    /// Always keep the built-in local calendar first; cannot be removed.
    pub fn ensure_local_feed(&mut self) {
        if let Some(f) = self
            .feeds
            .iter_mut()
            .find(|f| f.id == LOCAL_FEED_ID || f.local)
        {
            f.id = LOCAL_FEED_ID.into();
            f.url = LOCAL_FEED_URL.into();
            f.local = true;
            f.writable = true;
            return;
        }
        self.feeds.insert(0, FeedConfig::local_default());
    }

    pub fn local_feed(&self) -> Option<&FeedConfig> {
        self.feeds.iter().find(|f| f.local || f.id == LOCAL_FEED_ID)
    }

    pub fn local_feed_mut(&mut self) -> Option<&mut FeedConfig> {
        self.feeds
            .iter_mut()
            .find(|f| f.local || f.id == LOCAL_FEED_ID)
    }

    /// Keep `default_feed_id` on exactly one writable calendar.
    pub fn ensure_default_feed(&mut self) {
        let valid = self
            .feeds
            .iter()
            .any(|f| f.writable && f.id == self.default_feed_id);
        if valid {
            return;
        }
        if let Some(id) = self.feeds.iter().find(|f| f.writable).map(|f| f.id.clone()) {
            self.default_feed_id = id;
        }
    }

    pub fn default_feed(&self) -> Option<&FeedConfig> {
        self.feeds
            .iter()
            .find(|f| f.writable && f.id == self.default_feed_id)
            .or_else(|| self.feeds.iter().find(|f| f.writable))
    }

    pub fn ensure_presets(&mut self) {
        if self.presets.is_empty() {
            self.presets = ViewPreset::builtins();
        } else {
            for unit in [
                ViewUnit::Day,
                ViewUnit::Week,
                ViewUnit::Month,
                ViewUnit::Year,
                ViewUnit::Agenda,
            ] {
                let builtin = ViewPreset::builtin(unit);
                if !self.presets.iter().any(|p| p.id == builtin.id) {
                    self.presets.push(builtin);
                }
            }
        }
        for p in &mut self.presets {
            p.clamp_count();
            p.clamp_day_hours();
            p.clamp_show_days();
            p.clamp_month_weeks();
            p.clamp_title_lines();
            if p.name.trim().is_empty() {
                p.name = p.unit.label().into();
            }
        }
        if self.hot_preset_id.is_empty() || !self.presets.iter().any(|p| p.id == self.hot_preset_id)
        {
            self.hot_preset_id = self
                .presets
                .iter()
                .find(|p| p.id == "preset-month")
                .or_else(|| self.presets.first())
                .map(|p| p.id.clone())
                .unwrap_or_else(default_hot_preset_id);
        }
    }

    pub fn hot_preset(&self) -> ViewPreset {
        self.presets
            .iter()
            .find(|p| p.id == self.hot_preset_id)
            .cloned()
            .unwrap_or_else(|| ViewPreset::builtin(ViewUnit::Month))
    }
}

impl Config {
    pub fn load(paths: &Paths) -> Result<Self> {
        let path = paths.config_file();
        match storage::read_optional(&path)? {
            None => Ok(Self::default()),
            Some(bytes) => {
                let text = String::from_utf8(bytes).context("config is not utf-8")?;
                let mut cfg: Config = toml::from_str(&text).context("parse config.toml")?;
                cfg = migrate(cfg)?;
                Ok(cfg)
            }
        }
    }

    pub fn save(&self, paths: &Paths) -> Result<()> {
        let text = toml::to_string_pretty(self).context("serialize config")?;
        storage::write_atomic(&paths.config_file(), text.as_bytes())
    }

    pub fn zalender(&self) -> ZalenderConfig {
        match self.modules.get("zalender") {
            Some(ModuleConfig::Zalender(z)) => z.clone(),
            _ => ZalenderConfig::default(),
        }
    }

    pub fn set_zalender(&mut self, z: ZalenderConfig) {
        self.modules
            .insert("zalender".into(), ModuleConfig::Zalender(z));
    }

    pub fn module_enabled(&self, id: &str) -> bool {
        match self.modules.get(id) {
            Some(ModuleConfig::Zalender(z)) => z.enabled,
            Some(ModuleConfig::Generic(g)) => g.enabled,
            None => false,
        }
    }

    pub fn set_module_enabled(&mut self, id: &str, enabled: bool) {
        match self.modules.get_mut(id) {
            Some(ModuleConfig::Zalender(z)) => z.enabled = enabled,
            Some(ModuleConfig::Generic(g)) => g.enabled = enabled,
            None => {
                self.modules.insert(
                    id.into(),
                    ModuleConfig::Generic(GenericModuleConfig { enabled }),
                );
            }
        }
    }
}

fn migrate(mut cfg: Config) -> Result<Config> {
    if cfg.schema_version > SCHEMA_VERSION {
        bail!(
            "config schema {} is newer than supported {}",
            cfg.schema_version,
            SCHEMA_VERSION
        );
    }
    while cfg.schema_version < SCHEMA_VERSION {
        let next = cfg.schema_version + 1;
        if next == 2 {
            cfg.theme = ThemeMode::Dark;
        }
        if next == 3 {
            cfg.accent = default_accent();
            cfg.datetime_format = DatetimeFormat::default();
            let mut z = cfg.zalender();
            z.next_feed_id = 1;
            for feed in &mut z.feeds {
                if feed.id.is_empty() {
                    feed.id = format!("feed-{}", z.next_feed_id);
                    z.next_feed_id = z.next_feed_id.saturating_add(1);
                }
                // visible/writable default via serde when present; force for migration
                // (already defaulted if missing)
            }
            if z.next_feed_id == 1 && !z.feeds.is_empty() {
                // ids assigned above bump next_feed_id
            }
            z.week_start = WeekStart::default();
            z.legacy_preset = Some(LegacyViewPreset {
                unit: ViewUnit::Month,
                count: 1,
                anchor: AnchorMode::CurrentFirst,
            });
            z.presets = Vec::new();
            z.hot_preset_id.clear();
            z.day_modal = DayModalFields::default();
            z.custom_fields = Vec::new();
            z.next_field_id = 1;
            cfg.set_zalender(z);
        }
        if next == 4 {
            let mut palette = ThemePalette::for_mode(cfg.theme);
            if !cfg.accent.is_empty() {
                palette.accent = cfg.accent.clone();
            }
            cfg.palette = palette;
            cfg.accent = cfg.palette.accent.clone();
        }
        if next == 5 {
            let mut z = cfg.zalender();
            let legacy = z.legacy_preset.take();
            if z.presets.is_empty() {
                z.presets = ViewPreset::builtins();
                if let Some(old) = legacy {
                    if let Some(p) = z.presets.iter_mut().find(|p| p.unit == old.unit) {
                        p.count = old.count;
                        p.anchor = old.anchor;
                        p.clamp_count();
                        z.hot_preset_id = p.id.clone();
                    }
                }
            }
            z.ensure_presets();
            cfg.set_zalender(z);
        }
        if next == 6 {
            let mut z = cfg.zalender();
            z.display = DisplayOptions::default();
            z.ensure_presets();
            cfg.set_zalender(z);
        }
        if next == 7 {
            // font_family / font_scale default via serde on Config.
        }
        if next == 8 {
            // Keep the previous combined size until the user picks Event font size.
            cfg.event_font_scale = cfg.font_scale;
        }
        cfg.schema_version = next;
    }
    if !cfg.modules.contains_key("zalender") {
        cfg.set_zalender(ZalenderConfig::default());
    } else {
        let mut z = cfg.zalender();
        z.ensure_feed_ids();
        z.ensure_local_feed();
        z.ensure_default_feed();
        z.ensure_presets();
        cfg.set_zalender(z);
    }
    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_family_indices_match_menu_order() {
        for (i, family) in UiFontFamily::all().into_iter().enumerate() {
            assert_eq!(family.as_i32(), i as i32);
            assert_eq!(UiFontFamily::from_i32(i as i32), family);
            if family == UiFontFamily::System {
                assert!(family.family_name().is_empty());
            } else {
                assert_eq!(family.family_name(), family.label());
            }
        }
        assert_eq!(UiFontFamily::from_i32(99), UiFontFamily::System);
    }

    #[test]
    fn default_feed_is_exclusive() {
        let mut z = ZalenderConfig::default();
        let other = z.alloc_feed_id();
        z.feeds.push(FeedConfig {
            id: other.clone(),
            name: "Work".into(),
            url: "local://work".into(),
            color: "#3b82f6".into(),
            visible: true,
            writable: true,
            local: false,
        });
        z.default_feed_id = other.clone();
        z.ensure_default_feed();
        assert_eq!(z.default_feed_id, other);
        z.default_feed_id = "missing".into();
        z.ensure_default_feed();
        assert_eq!(z.default_feed_id, LOCAL_FEED_ID);
        assert_eq!(z.default_feed().map(|f| f.id.as_str()), Some(LOCAL_FEED_ID));
    }

    #[test]
    fn roundtrip_default() {
        let cfg = Config::default();
        let text = toml::to_string_pretty(&cfg).unwrap();
        let back: Config = toml::from_str(&text).unwrap();
        assert!(!back.setup_complete);
        assert!(back.module_enabled("zalender"));
        assert_eq!(back.theme, ThemeMode::Dark);
        assert_eq!(back.schema_version, SCHEMA_VERSION);
        assert_eq!(back.zalender().week_start, WeekStart::Sunday);
        assert_eq!(back.zalender().presets.len(), 5);
        assert_eq!(back.zalender().hot_preset().unit, ViewUnit::Month);
        assert!(back.zalender().display.dim_past);
        assert!(back.zalender().display.shade_weekends);
    }

    #[test]
    fn migrate_v1_to_current() {
        let mut cfg = Config {
            schema_version: 1,
            setup_complete: true,
            theme: ThemeMode::Dark,
            accent: String::new(),
            palette: ThemePalette::default(),
            datetime_format: DatetimeFormat::default(),
            font_family: UiFontFamily::default(),
            font_scale: FontScale::default(),
            event_font_scale: FontScale::default(),
            modules: BTreeMap::new(),
        };
        cfg = migrate(cfg).unwrap();
        assert_eq!(cfg.schema_version, SCHEMA_VERSION);
        assert_eq!(cfg.theme, ThemeMode::Dark);
        assert!(cfg.modules.contains_key("zalender"));
        assert_eq!(cfg.accent, "#3b82f6");
        assert_eq!(cfg.palette.bg, "#0f1419");
        let z = cfg.zalender();
        assert_eq!(z.presets.len(), 5);
        assert!(z.presets.iter().all(|p| p.builtin && p.visible));
        assert!(z.presets.iter().any(|p| p.id == "preset-agenda"));
        assert!(z.display.dim_past);
        assert!(z.display.shade_weekends);
    }

    #[test]
    fn migrate_v2_assigns_feed_ids() {
        let z = ZalenderConfig {
            feeds: vec![
                FeedConfig {
                    id: String::new(),
                    name: "Family".into(),
                    url: "https://example.com/a.ics".into(),
                    color: "#3b82f6".into(),
                    visible: true,
                    writable: false,
                    local: false,
                },
                FeedConfig {
                    id: String::new(),
                    name: "Work".into(),
                    url: "https://example.com/b.ics".into(),
                    color: "#ef4444".into(),
                    visible: true,
                    writable: false,
                    local: false,
                },
            ],
            ..ZalenderConfig::default()
        };
        let mut modules = BTreeMap::new();
        modules.insert("zalender".into(), ModuleConfig::Zalender(z));
        let mut cfg = Config {
            schema_version: 2,
            setup_complete: true,
            theme: ThemeMode::Dark,
            accent: String::new(),
            palette: ThemePalette::default(),
            datetime_format: DatetimeFormat::default(),
            font_family: UiFontFamily::default(),
            font_scale: FontScale::default(),
            event_font_scale: FontScale::default(),
            modules,
        };
        cfg = migrate(cfg).unwrap();
        assert_eq!(cfg.schema_version, SCHEMA_VERSION);
        let z = cfg.zalender();
        assert!(z.feeds[0].local);
        assert_eq!(z.feeds[0].id, LOCAL_FEED_ID);
        assert_eq!(z.feeds[1].id, "feed-1");
        assert_eq!(z.feeds[2].id, "feed-2");
        assert_eq!(z.next_feed_id, 3);
    }

    #[test]
    fn migrate_legacy_preset_into_builtins() {
        let z = ZalenderConfig {
            presets: Vec::new(),
            hot_preset_id: String::new(),
            legacy_preset: Some(LegacyViewPreset {
                unit: ViewUnit::Week,
                count: 2,
                anchor: AnchorMode::Monday,
            }),
            ..ZalenderConfig::default()
        };
        let mut modules = BTreeMap::new();
        modules.insert("zalender".into(), ModuleConfig::Zalender(z));
        let mut cfg = Config {
            schema_version: 4,
            setup_complete: true,
            theme: ThemeMode::Dark,
            accent: default_accent(),
            palette: ThemePalette::default(),
            datetime_format: DatetimeFormat::default(),
            font_family: UiFontFamily::default(),
            font_scale: FontScale::default(),
            event_font_scale: FontScale::default(),
            modules,
        };
        // Simulate pre-migrate deserialize: clear builtins that Default seeded.
        {
            let mut z = cfg.zalender();
            z.presets.clear();
            cfg.set_zalender(z);
        }
        cfg = migrate(cfg).unwrap();
        let z = cfg.zalender();
        assert_eq!(z.hot_preset_id, "preset-week");
        let week = z.presets.iter().find(|p| p.id == "preset-week").unwrap();
        assert_eq!(week.count, 2);
        assert_eq!(week.anchor, AnchorMode::Monday);
        assert!(z.presets.iter().any(|p| p.id == "preset-agenda"));
    }

    #[test]
    fn migrate_v5_adds_agenda_and_display() {
        let z = ZalenderConfig {
            presets: vec![
                ViewPreset::builtin(ViewUnit::Day),
                ViewPreset::builtin(ViewUnit::Week),
                ViewPreset::builtin(ViewUnit::Month),
                ViewPreset::builtin(ViewUnit::Year),
            ],
            ..ZalenderConfig::default()
        };
        let mut modules = BTreeMap::new();
        modules.insert("zalender".into(), ModuleConfig::Zalender(z));
        let mut cfg = Config {
            schema_version: 5,
            setup_complete: true,
            theme: ThemeMode::Dark,
            accent: default_accent(),
            palette: ThemePalette::default(),
            datetime_format: DatetimeFormat::default(),
            font_family: UiFontFamily::default(),
            font_scale: FontScale::default(),
            event_font_scale: FontScale::default(),
            modules,
        };
        cfg = migrate(cfg).unwrap();
        assert_eq!(cfg.schema_version, SCHEMA_VERSION);
        let z = cfg.zalender();
        assert!(z.presets.iter().any(|p| p.id == "preset-agenda"));
        assert!(z.display.dim_past);
        assert!(z.display.shade_weekends);
    }

    #[test]
    fn migrate_v7_copies_font_scale_onto_events() {
        let mut cfg = Config {
            schema_version: 7,
            font_scale: FontScale::Large,
            event_font_scale: FontScale::Medium,
            ..Config::default()
        };
        cfg = migrate(cfg).unwrap();
        assert_eq!(cfg.schema_version, SCHEMA_VERSION);
        assert_eq!(cfg.font_scale, FontScale::Large);
        assert_eq!(cfg.event_font_scale, FontScale::Large);
    }

    #[test]
    fn migrate_missing_zalender() {
        let mut cfg = Config {
            schema_version: 1,
            setup_complete: true,
            theme: ThemeMode::Dark,
            accent: default_accent(),
            palette: ThemePalette::default(),
            datetime_format: DatetimeFormat::default(),
            font_family: UiFontFamily::default(),
            font_scale: FontScale::default(),
            event_font_scale: FontScale::default(),
            modules: BTreeMap::new(),
        };
        cfg = migrate(cfg).unwrap();
        assert!(cfg.modules.contains_key("zalender"));
    }

    #[test]
    fn view_unit_caps() {
        assert_eq!(ViewUnit::Day.max_count(), 7);
        assert_eq!(ViewUnit::Week.max_count(), 6);
        assert_eq!(ViewUnit::Month.max_count(), 6);
        assert_eq!(ViewUnit::Year.max_count(), 2);
        assert_eq!(ViewUnit::Agenda.max_count(), 4);
        assert_eq!(ViewUnit::Day.zoom_out(), ViewUnit::Week);
        assert_eq!(ViewUnit::Year.zoom_out(), ViewUnit::Year);
        assert_eq!(ViewUnit::Agenda.zoom_out(), ViewUnit::Month);
        assert_eq!(ViewUnit::Agenda.as_i32(), 4);
        assert_eq!(ViewUnit::from_i32(4), ViewUnit::Agenda);
    }

    #[test]
    fn preset_show_days_default_on_and_keep_one() {
        let p: ViewPreset = serde_json::from_str(r#"{"id":"x","name":"X"}"#).unwrap();
        assert_eq!(p.show_days, [true; 7]);
        let mut hidden = p.clone();
        hidden.show_days = [false; 7];
        hidden.clamp_show_days();
        assert_eq!(hidden.show_days, [true; 7]);
        // 2026-10-18 is Sunday.
        let sunday = NaiveDate::from_ymd_opt(2026, 10, 18).unwrap();
        let monday = NaiveDate::from_ymd_opt(2026, 10, 19).unwrap();
        assert_eq!(p.month_weeks, 6);
        assert_eq!(p.title_lines, 1);
        let mut lines = p.clone();
        lines.title_lines = 0;
        lines.clamp_title_lines();
        assert_eq!(lines.title_lines, 1);
        lines.title_lines = 12;
        lines.clamp_title_lines();
        assert_eq!(lines.title_lines, 10);
        assert!(!p.month_from_this_week);
        let mut weekdays = p;
        weekdays.show_days[0] = false;
        assert!(!weekdays.shows_date(sunday));
        assert!(weekdays.shows_date(monday));
    }
}
