use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{Datelike, Local, NaiveDate, NaiveTime, TimeZone, Timelike, Utc};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel, Weak};

use crate::app::color;
use crate::app::setup::SetupDraft;
use crate::app::shell::{Screen, ShellState, WIJITIFY_SETTINGS_PAGE};
use crate::modules::zalender::{
    self,
    local_events::{self, LocalEventStore, LOCAL_FEED_ID},
    model as zal_model,
    overrides::{self, EventOverride, OverrideKey, OverrideStore},
    recur::{self, RepeatDraft},
    view as zal_view, CalendarSnapshot,
};
use crate::modules::{registry, Module, ModuleContext};
use crate::services::config::{
    AnchorMode, Config, CustomFieldDef, CustomFieldType, DatetimeFormat, DisplayOptions,
    FeedConfig, FontScale, ThemeMode, ThemePalette, UiFontFamily, ViewPreset, ViewUnit, WeekStart,
    ZalenderConfig,
};
use crate::services::storage::Paths;
use crate::services::tasks::TaskRuntime;
use crate::services::update::{self, status, UpdateCmd, UpdateState};

#[allow(clippy::unwrap_used)]
mod generated {
    slint::include_modules!();
}
pub use generated::*;

const HELP_TEXT: &str = "\
Dashboard\n\
The top bar shows the clock, period title with arrows, Today, the view pill, refresh, and Settings (gear). The left sidebar has two mini months and calendar toggles.\n\n\
Views\n\
Day and Week show an all-day row plus timed events with a red now-line on today. Month and Year use grids (title-only chips). Agenda is a chronological list. Settings → Preset views manages Day/Week/Month/Year/Agenda auto presets plus any you create. Toggle Show to put a preset in the dashboard pill. Default (one at a time) is what the dashboard Default button restores — today in that view — and the startup view. Each preset has a checkbox per weekday; unchecked days are left out of that view. Month presets set a week count. With Start on this week, 1 is that week and 2 adds the next. Without it, the grid starts on the week of the 1st and runs through the week you are viewing; week count 1 stops there, and 2 also shows the next week. Title lines is how many lines an event title uses before it ends with …; the event box grows to fit.\n\n\
Navigation\n\
Arrows move by one unit of the current grain (Agenda steps by its week count). A month that starts on this week steps by its week count; otherwise arrows move one month. Tapping the period title zooms out one level. The view pill switches among visible presets.\n\n\
Display\n\
Settings → Global: font, app font size, event font size, date/time format, week start, dim past days, and shade weekends. App font size covers the shell and calendar labels. Event font size covers event titles and times on the calendar. Settings → Calendar display toggles day-popup fields.\n\n\
Calendars\n\
Sidebar checkboxes toggle visibility without refetching. On editable calendars, Default picks the one calendar new events are created on. Manage feeds under Settings → Calendars.\n\n\
Events\n\
Day and Week: tap an empty hour to create an event, or tap the all-day row for an all-day event. Month, Year, and Agenda: tap a day for the day modal. Customize stores color, icon, and custom fields locally.\n\n\
About\n\
What Wijitify is, and the public GitHub repository for source, issues, and releases.\n\n\
Wijitify\n\
Installed version, check for updates / update to a new version, and return to the desktop. Module on/off and run-setup-again live here too. This is the only settings tab when every module is off.\n";

pub struct AppCtx {
    pub paths: Paths,
    pub config: Config,
    pub runtime: Arc<TaskRuntime>,
    pub shell: ShellState,
    pub setup: SetupDraft,
    pub snapshot: Option<CalendarSnapshot>,
    pub overrides: OverrideStore,
    pub view_unit: ViewUnit,
    pub view_count: u32,
    pub active_preset_id: String,
    pub settings_preset_index: usize,
    pub settings_preset_name: String,
    pub anchor: NaiveDate,
    pub selected_day: NaiveDate,
    pub day_modal_open: bool,
    pub event_detail_open: bool,
    pub customize_open: bool,
    pub edit_placeholder_open: bool,
    pub name_local_open: bool,
    pub create_event_open: bool,
    pub detail_index: Option<usize>,
    pub detail_uid: Option<String>,
    pub customize_color: String,
    pub customize_icon: String,
    pub customize_occurrence: bool,
    pub customize_fields: HashMap<String, String>,
    pub name_local_draft: String,
    pub create_title: String,
    pub create_location: String,
    pub create_description: String,
    pub create_date_key: String,
    pub create_start_mins: i32,
    pub create_end_mins: i32,
    pub create_all_day: bool,
    pub create_repeat: bool,
    pub create_repeat_open: bool,
    pub create_repeat_interval: i32,
    /// 0 day, 1 week, 2 month, 3 quarter, 4 year.
    pub create_repeat_unit: i32,
    /// 0 never, 1 on a date, 2 after a count.
    pub create_repeat_end: i32,
    pub create_repeat_until: String,
    pub create_repeat_count: i32,
    /// Sunday = 0 … Saturday = 6.
    pub create_weekdays: [bool; 7],
    /// Month and quarter: false = day of month, true = nth weekday.
    pub create_by_weekday: bool,
    pub settings_new_feed_name: String,
    pub settings_new_feed_url: String,
    pub settings_new_feed_color: String,
    pub settings_feed_banner: String,
    pub settings_feed_banner_kind: i32,
    pub new_field_name: String,
    pub new_field_type: CustomFieldType,
    pub new_field_options: Vec<String>,
    pub refreshing: bool,
    pub status: String,
    pub status_kind: i32,
    pub theme_banner: String,
    pub theme_banner_kind: i32,
    pub perf_enabled: bool,
    pub refresh_gen: u64,
    pub update_tx: Option<tokio::sync::mpsc::Sender<UpdateCmd>>,
    pub local_events: LocalEventStore,
    modules: Vec<Box<dyn Module>>,
}

impl AppCtx {
    pub fn new(paths: Paths, config: Config, runtime: Arc<TaskRuntime>) -> Self {
        let today = Local::now().date_naive();
        let metas = registry::all_metas();
        let module_ids: Vec<(&str, &str, &str)> = metas
            .iter()
            .map(|m| (m.id, m.name, m.description))
            .collect();
        let setup = SetupDraft::from_config(&config, &module_ids);
        let modules_on = metas.iter().any(|m| config.module_enabled(m.id));
        let shell = ShellState {
            screen: if config.setup_complete {
                Screen::Home
            } else {
                Screen::Setup
            },
            active_module: if config.module_enabled("zalender") {
                "zalender".into()
            } else {
                String::new()
            },
            settings_open: false,
            settings_page: if modules_on {
                0
            } else {
                WIJITIFY_SETTINGS_PAGE
            },
        };
        let mut config = config;
        {
            let mut z = config.zalender();
            z.ensure_feed_ids();
            z.ensure_local_feed();
            z.ensure_default_feed();
            z.ensure_presets();
            config.set_zalender(z);
        }
        let z = config.zalender();
        let preset = z.hot_preset();
        let ovr_path = overrides::overrides_path(&paths.config_dir);
        let overrides = OverrideStore::load(&ovr_path).unwrap_or_default();
        let local_events = LocalEventStore::load(&paths.config_dir)
            .unwrap_or_else(|_| LocalEventStore::empty(&paths.config_dir));
        let needs_name = config.setup_complete
            && z.local_feed()
                .map(|f| f.name.trim().is_empty())
                .unwrap_or(true);
        Self {
            paths,
            config,
            runtime,
            shell,
            setup,
            snapshot: None,
            overrides,
            view_unit: preset.unit,
            view_count: preset.count.max(1),
            active_preset_id: preset.id.clone(),
            settings_preset_index: z
                .presets
                .iter()
                .position(|p| p.id == z.hot_preset_id)
                .unwrap_or(0),
            settings_preset_name: String::new(),
            anchor: today,
            selected_day: today,
            day_modal_open: false,
            event_detail_open: false,
            customize_open: false,
            edit_placeholder_open: false,
            name_local_open: needs_name,
            create_event_open: false,
            detail_index: None,
            detail_uid: None,
            customize_color: String::new(),
            customize_icon: String::new(),
            customize_occurrence: false,
            customize_fields: HashMap::new(),
            name_local_draft: String::new(),
            create_title: String::new(),
            create_location: String::new(),
            create_description: String::new(),
            create_date_key: today.format("%Y-%m-%d").to_string(),
            create_start_mins: 9 * 60,
            create_end_mins: 10 * 60,
            create_all_day: false,
            create_repeat: false,
            create_repeat_open: false,
            create_repeat_interval: 1,
            create_repeat_unit: 0,
            create_repeat_end: 0,
            create_repeat_until: today.format("%Y-%m-%d").to_string(),
            create_repeat_count: 10,
            create_weekdays: [false; 7],
            create_by_weekday: false,
            settings_new_feed_name: String::new(),
            settings_new_feed_url: String::new(),
            settings_new_feed_color: "#3b82f6".into(),
            settings_feed_banner: String::new(),
            settings_feed_banner_kind: 0,
            new_field_name: String::new(),
            new_field_type: CustomFieldType::Text,
            new_field_options: Vec::new(),
            refreshing: false,
            status: String::new(),
            status_kind: 0,
            theme_banner: String::new(),
            theme_banner_kind: 0,
            perf_enabled: std::env::var_os("WIJITIFY_PERF").is_some(),
            refresh_gen: 0,
            update_tx: None,
            local_events,
            modules: Vec::new(),
        }
    }

    pub fn sync_modules(&mut self) {
        self.stop_modules();
        let ids: Vec<String> = registry::all_metas()
            .iter()
            .filter(|m| self.config.module_enabled(m.id))
            .map(|m| m.id.to_string())
            .collect();
        self.modules = registry::create_enabled(&ids);
        for m in &mut self.modules {
            let id = m.meta().id;
            let started = self
                .paths
                .module_cache(id)
                .and_then(|cache_dir| m.start(ModuleContext { cache_dir }));
            if let Err(e) = started {
                tracing::warn!("module {id} failed to start: {e:#}");
            }
        }
    }

    pub fn stop_modules(&mut self) {
        for m in &mut self.modules {
            m.stop();
        }
        self.modules.clear();
    }

    fn schedule_config_save(&self) {
        let paths = self.paths.clone();
        let cfg = self.config.clone();
        self.runtime.handle.spawn(async move {
            if let Err(e) = cfg.save(&paths) {
                tracing::warn!("config save failed: {e:#}");
            }
        });
    }

    fn schedule_overrides_save(&self) {
        let path = overrides::overrides_path(&self.paths.config_dir);
        let store = self.overrides.clone();
        self.runtime.handle.spawn(async move {
            // Brief debounce so rapid edits coalesce naturally via latest spawn.
            tokio::time::sleep(Duration::from_millis(300)).await;
            if let Err(e) = store.save(&path) {
                tracing::warn!("overrides save failed: {e:#}");
            }
        });
    }
}

pub fn bind(ui: &AppWindow, ctx: std::rc::Rc<std::cell::RefCell<AppCtx>>) {
    {
        let c = ctx.borrow();
        apply_theme(ui, &c.config);
    }
    start_updater(ui, &ctx);
    apply_all(ui, &ctx.borrow());

    bind_updates(ui, &ctx);
    bind_setup(ui, &ctx);
    bind_settings(ui, &ctx);
    bind_zalender(ui, &ctx);

    {
        let mut c = ctx.borrow_mut();
        if let Ok(dir) = c.paths.module_cache("zalender") {
            if let Ok(Some(snap)) = zalender::load_from_cache(&dir) {
                c.status = format!(
                    "Loaded {} events · last updated {}",
                    snap.events.len(),
                    snap.generated_at.format("%Y-%m-%d %H:%M UTC")
                );
                c.status_kind = 1;
                c.snapshot = Some(snap);
            }
        }
        merge_local_into_snapshot(&mut c);
    }
    apply_zalender(ui, &ctx.borrow());
    if ctx.borrow().config.setup_complete {
        ctx.borrow_mut().sync_modules();
        maybe_start_refresh(ui, &ctx);
    }

    start_clock(ui.as_weak(), ctx.clone());
    if ctx.borrow().perf_enabled {
        install_perf(ui);
    }
}

fn bind_updates(ui: &AppWindow, ctx: &std::rc::Rc<std::cell::RefCell<AppCtx>>) {
    {
        let ctx = ctx.clone();
        ui.on_check_updates(move || {
            let t0 = Instant::now();
            send_update_cmd(&ctx, UpdateCmd::CheckNow);
            log_cb("check_updates", t0);
        });
    }
    {
        let ctx = ctx.clone();
        ui.on_start_update(move || {
            let t0 = Instant::now();
            send_update_cmd(&ctx, UpdateCmd::Install);
            log_cb("start_update", t0);
        });
    }
    {
        let ctx = ctx.clone();
        ui.on_restart_app(move || {
            let t0 = Instant::now();
            send_update_cmd(&ctx, UpdateCmd::Restart);
            log_cb("restart_app", t0);
        });
    }
}

fn bind_setup(ui: &AppWindow, ctx: &std::rc::Rc<std::cell::RefCell<AppCtx>>) {
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_setup_toggle_module(move |i| {
            let t0 = Instant::now();
            ctx.borrow_mut().setup.toggle(i as usize);
            if let Some(ui) = ui_weak.upgrade() {
                apply_setup(&ui, &ctx.borrow());
            }
            log_cb("setup_toggle_module", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_setup_add_feed(move || {
            let t0 = Instant::now();
            {
                let mut c = ctx.borrow_mut();
                if let Some(ui) = ui_weak.upgrade() {
                    sync_feed_fields_from_ui(&ui, &mut c.setup);
                }
                let _ = c.setup.add_feed();
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_setup(&ui, &ctx.borrow());
            }
            log_cb("setup_add_feed", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_setup_remove_feed(move |i| {
            let t0 = Instant::now();
            ctx.borrow_mut().setup.remove_feed(i as usize);
            if let Some(ui) = ui_weak.upgrade() {
                apply_setup(&ui, &ctx.borrow());
            }
            log_cb("setup_remove_feed", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_setup_next(move || {
            let t0 = Instant::now();
            let mut c = ctx.borrow_mut();
            c.setup.zalender.timezone = ui_weak
                .upgrade()
                .map(|u| u.get_zalender_timezone().to_string())
                .unwrap_or_else(|| c.setup.zalender.timezone.clone());
            if c.setup.zalender_selected() {
                c.setup.step = 1;
            } else {
                finish_setup_inner(&mut c);
            }
            drop(c);
            if let Some(ui) = ui_weak.upgrade() {
                apply_all(&ui, &ctx.borrow());
                maybe_start_refresh(&ui, &ctx);
            }
            log_cb("setup_next", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_setup_back(move || {
            let t0 = Instant::now();
            ctx.borrow_mut().setup.step = 0;
            if let Some(ui) = ui_weak.upgrade() {
                apply_setup(&ui, &ctx.borrow());
            }
            log_cb("setup_back", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_setup_finish(move || {
            let t0 = Instant::now();
            let finish_ok = {
                let mut c = ctx.borrow_mut();
                if let Some(ui) = ui_weak.upgrade() {
                    c.setup.zalender.timezone = ui.get_zalender_timezone().to_string();
                    sync_feed_fields_from_ui(&ui, &mut c.setup);
                }
                if !c.setup.new_feed_url.trim().is_empty() {
                    let _ = c.setup.add_feed();
                }
                if c.setup.zalender_selected() && c.setup.zalender.feeds.is_empty() {
                    c.setup.banner_message =
                        "Add at least one calendar feed, or go Back and disable Zalender.".into();
                    c.setup.banner_kind = 3;
                    false
                } else {
                    finish_setup_inner(&mut c);
                    c.status = "Setup saved.".into();
                    c.status_kind = 1;
                    true
                }
            };
            if let Some(ui) = ui_weak.upgrade() {
                if finish_ok {
                    apply_all(&ui, &ctx.borrow());
                    maybe_start_refresh(&ui, &ctx);
                } else {
                    apply_setup(&ui, &ctx.borrow());
                }
            }
            log_cb("setup_finish", t0);
        });
    }
}

fn bind_settings(ui: &AppWindow, ctx: &std::rc::Rc<std::cell::RefCell<AppCtx>>) {
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_open_settings(move || {
            let t0 = Instant::now();
            {
                let mut c = ctx.borrow_mut();
                c.shell.settings_open = true;
                focus_wijitify_if_no_modules(&mut c);
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_shell(&ui, &ctx.borrow());
                apply_settings(&ui, &ctx.borrow());
            }
            log_cb("open_settings", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_close_settings(move || {
            let t0 = Instant::now();
            ctx.borrow_mut().shell.settings_open = false;
            if let Some(ui) = ui_weak.upgrade() {
                apply_shell(&ui, &ctx.borrow());
            }
            log_cb("close_settings", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_settings_select_page(move |i| {
            let t0 = Instant::now();
            {
                let mut c = ctx.borrow_mut();
                c.shell.settings_page = if any_module_enabled(&c.config) {
                    i
                } else {
                    WIJITIFY_SETTINGS_PAGE
                };
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_settings(&ui, &ctx.borrow());
            }
            log_cb("settings_select_page", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_settings_set_theme(move |mode| {
            let t0 = Instant::now();
            {
                let mut c = ctx.borrow_mut();
                let theme = ThemeMode::from_i32(mode);
                c.config.theme = theme;
                c.config.palette = ThemePalette::for_mode(theme);
                c.config.accent = c.config.palette.accent.clone();
                c.theme_banner = format!(
                    "{} theme loaded — component colors reset to defaults.",
                    theme.label()
                );
                c.theme_banner_kind = 1;
                let cfg_ref = c.config.clone();
                if let Some(ui) = ui_weak.upgrade() {
                    apply_theme(&ui, &cfg_ref);
                }
                c.schedule_config_save();
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_settings(&ui, &ctx.borrow());
            }
            log_cb("settings_set_theme", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_settings_set_theme_color(move |i, hex| {
            let t0 = Instant::now();
            let mut c = ctx.borrow_mut();
            let labels = ThemePalette::labels();
            let index = i as usize;
            if let Some((key, label)) = labels.get(index) {
                let normalized = normalize_hex(&hex);
                c.config.palette.set(key, normalized.clone());
                if *key == "accent" {
                    c.config.accent = normalized.clone();
                }
                c.schedule_config_save();
                if let Some(ui) = ui_weak.upgrade() {
                    apply_theme(&ui, &c.config);
                    // Replacing the theme-colors model rebuilds every row and
                    // closes the picker on the click that chose the color.
                    if !patch_theme_color(&ui, index, key, label, &normalized) {
                        apply_settings(&ui, &c);
                    }
                }
            }
            log_cb("settings_set_theme_color", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_settings_reset_theme_colors(move || {
            let t0 = Instant::now();
            {
                let mut c = ctx.borrow_mut();
                let theme = c.config.theme;
                c.config.palette = ThemePalette::for_mode(theme);
                c.config.accent = c.config.palette.accent.clone();
                c.theme_banner = format!(
                    "{} defaults restored — component colors reset.",
                    theme.label()
                );
                c.theme_banner_kind = 1;
                let cfg_ref = c.config.clone();
                if let Some(ui) = ui_weak.upgrade() {
                    apply_theme(&ui, &cfg_ref);
                }
                c.schedule_config_save();
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_settings(&ui, &ctx.borrow());
            }
            log_cb("settings_reset_theme_colors", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_settings_toggle_module(move |i| {
            let t0 = Instant::now();
            {
                let mut c = ctx.borrow_mut();
                let metas = registry::all_metas();
                if let Some(m) = metas.get(i as usize) {
                    let en = !c.config.module_enabled(m.id);
                    c.config.set_module_enabled(m.id, en);
                    c.schedule_config_save();
                    if m.id == "zalender" {
                        c.shell.active_module = if en { "zalender".into() } else { String::new() };
                    }
                    c.sync_modules();
                    focus_wijitify_if_no_modules(&mut c);
                }
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_settings(&ui, &ctx.borrow());
                apply_shell(&ui, &ctx.borrow());
            }
            log_cb("settings_toggle_module", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_settings_toggle_feed_visible(move |i| {
            let t0 = Instant::now();
            {
                let mut c = ctx.borrow_mut();
                let mut z = c.config.zalender();
                if let Some(f) = z.feeds.get_mut(i as usize) {
                    f.visible = !f.visible;
                }
                c.config.set_zalender(z);
                c.schedule_config_save();
            }
            if let Some(ui) = ui_weak.upgrade() {
                let c = ctx.borrow();
                let z = c.config.zalender();
                if let Some(feed) = z.feeds.get(i as usize) {
                    if !patch_settings_feed(&ui, i as usize, feed) {
                        apply_settings(&ui, &c);
                    }
                }
                apply_zalender(&ui, &c);
            }
            log_cb("settings_toggle_feed_visible", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_settings_remove_feed(move |i| {
            let t0 = Instant::now();
            let mut refresh = false;
            {
                let mut c = ctx.borrow_mut();
                let mut z = c.config.zalender();
                match z.feeds.get(i as usize) {
                    Some(f) if f.local => {
                        c.settings_feed_banner = "Local calendar cannot be removed.".into();
                        c.settings_feed_banner_kind = 3;
                    }
                    Some(_) => {
                        z.feeds.remove(i as usize);
                        z.ensure_default_feed();
                        c.config.set_zalender(z);
                        c.schedule_config_save();
                        refresh = true;
                    }
                    None => {}
                }
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_settings(&ui, &ctx.borrow());
                apply_zalender(&ui, &ctx.borrow());
                if refresh {
                    maybe_start_refresh(&ui, &ctx);
                }
            }
            log_cb("settings_remove_feed", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_settings_add_feed(move || {
            let t0 = Instant::now();
            {
                let mut c = ctx.borrow_mut();
                if let Some(ui) = ui_weak.upgrade() {
                    c.settings_new_feed_name = ui.get_settings_new_feed_name().to_string();
                    c.settings_new_feed_url = ui.get_settings_new_feed_url().to_string();
                    c.settings_new_feed_color = ui.get_settings_new_feed_color().to_string();
                }
                let name = c.settings_new_feed_name.trim().to_string();
                let url = c.settings_new_feed_url.trim().to_string();
                if url.is_empty() {
                    c.settings_feed_banner = "Enter an ICS URL.".into();
                    c.settings_feed_banner_kind = 3;
                } else {
                    match zalender::normalize_ics_url(&url) {
                        Ok(url) => {
                            let mut z = c.config.zalender();
                            let id = z.alloc_feed_id();
                            let name = if name.is_empty() {
                                "Calendar".into()
                            } else {
                                name
                            };
                            z.feeds.push(FeedConfig {
                                id,
                                name,
                                url,
                                color: c.settings_new_feed_color.clone(),
                                visible: true,
                                writable: false,
                                local: false,
                            });
                            c.config.set_zalender(z);
                            c.settings_new_feed_name.clear();
                            c.settings_new_feed_url.clear();
                            c.settings_new_feed_color = "#3b82f6".into();
                            c.settings_feed_banner.clear();
                            c.settings_feed_banner_kind = 0;
                            c.schedule_config_save();
                        }
                        Err(_) => {
                            c.settings_feed_banner =
                                "URL must start with http:// or https://.".into();
                            c.settings_feed_banner_kind = 3;
                        }
                    }
                }
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_settings(&ui, &ctx.borrow());
                apply_zalender(&ui, &ctx.borrow());
                maybe_start_refresh(&ui, &ctx);
            }
            log_cb("settings_add_feed", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_settings_update_feed(move |i, name, url, color_hex| {
            let t0 = Instant::now();
            let mut refresh = false;
            {
                let mut c = ctx.borrow_mut();
                let index = i as usize;
                let mut z = c.config.zalender();
                if index < z.feeds.len() {
                    let mut feed = z.feeds[index].clone();
                    let mut banner = String::new();
                    let name = name.trim();
                    if feed.local {
                        if name.is_empty() {
                            banner = "Enter a name for your local calendar.".into();
                        } else {
                            feed.name = name.to_string();
                        }
                    } else {
                        feed.name = if name.is_empty() {
                            "Calendar".into()
                        } else {
                            name.to_string()
                        };
                        let url = url.trim();
                        if url.is_empty() {
                            banner = "Enter an ICS URL.".into();
                        } else {
                            match zalender::normalize_ics_url(url) {
                                Ok(normalized) => {
                                    if normalized != feed.url {
                                        feed.url = normalized;
                                        refresh = true;
                                    }
                                }
                                Err(_) => {
                                    banner = "URL must start with http:// or https://.".into();
                                }
                            }
                        }
                    }
                    match canonical_hex(&color_hex) {
                        Some(hex) => feed.color = hex,
                        None => {
                            if banner.is_empty() {
                                banner = "Color must be #RRGGBB.".into();
                            }
                        }
                    }
                    z.feeds[index] = feed.clone();
                    c.settings_feed_banner = banner;
                    c.settings_feed_banner_kind = if c.settings_feed_banner.is_empty() {
                        0
                    } else {
                        3
                    };
                    c.config.set_zalender(z);
                    c.schedule_config_save();
                    if let Some(ui) = ui_weak.upgrade() {
                        ui.set_settings_feed_banner(SharedString::from(
                            c.settings_feed_banner.as_str(),
                        ));
                        ui.set_settings_feed_banner_kind(c.settings_feed_banner_kind);
                        if !patch_settings_feed(&ui, index, &feed) {
                            apply_settings(&ui, &c);
                        }
                        apply_zalender(&ui, &c);
                    }
                }
            }
            if refresh {
                if let Some(ui) = ui_weak.upgrade() {
                    maybe_start_refresh(&ui, &ctx);
                }
            }
            log_cb("settings_update_feed", t0);
        });
    }
    bind_settings_preset(ui, ctx);
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_run_setup_again(move || {
            let t0 = Instant::now();
            {
                let mut c = ctx.borrow_mut();
                let metas = registry::all_metas();
                let ids: Vec<(&str, &str, &str)> = metas
                    .iter()
                    .map(|m| (m.id, m.name, m.description))
                    .collect();
                c.setup = SetupDraft::from_config(&c.config, &ids);
                c.shell.screen = Screen::Setup;
                c.shell.settings_open = false;
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_all(&ui, &ctx.borrow());
            }
            log_cb("run_setup_again", t0);
        });
    }
    {
        ui.on_quit_app(|| {
            let t0 = Instant::now();
            let _ = slint::quit_event_loop();
            log_cb("quit_app", t0);
        });
    }
}

fn bind_settings_preset(ui: &AppWindow, ctx: &std::rc::Rc<std::cell::RefCell<AppCtx>>) {
    fn with_selected_preset(c: &mut AppCtx, f: impl FnOnce(&mut ViewPreset)) -> bool {
        let idx = c.settings_preset_index;
        let mut z = c.config.zalender();
        if idx >= z.presets.len() {
            return false;
        }
        f(&mut z.presets[idx]);
        z.presets[idx].clamp_count();
        z.presets[idx].clamp_day_hours();
        z.presets[idx].clamp_show_days();
        z.presets[idx].clamp_month_weeks();
        z.presets[idx].clamp_title_lines();
        let id = z.presets[idx].id.clone();
        let unit = z.presets[idx].unit;
        let count = z.presets[idx].count;
        c.config.set_zalender(z);
        c.schedule_config_save();
        if c.active_preset_id == id {
            c.view_unit = unit;
            c.view_count = count.max(1);
        }
        true
    }

    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_settings_select_preset(move |i| {
            let t0 = Instant::now();
            {
                let mut c = ctx.borrow_mut();
                let z = c.config.zalender();
                if (i as usize) < z.presets.len() {
                    c.settings_preset_index = i as usize;
                    c.settings_preset_name = z.presets[i as usize].name.clone();
                }
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_settings(&ui, &ctx.borrow());
            }
            log_cb("settings_select_preset", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_settings_toggle_preset_visible(move |i| {
            let t0 = Instant::now();
            {
                let mut c = ctx.borrow_mut();
                let mut z = c.config.zalender();
                if let Some(p) = z.presets.get_mut(i as usize) {
                    p.visible = !p.visible;
                }
                c.config.set_zalender(z);
                c.schedule_config_save();
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_settings(&ui, &ctx.borrow());
                apply_zalender(&ui, &ctx.borrow());
            }
            log_cb("settings_toggle_preset_visible", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_settings_set_default_preset(move |index| {
            let t0 = Instant::now();
            {
                let mut c = ctx.borrow_mut();
                let mut z = c.config.zalender();
                if let Some(p) = z.presets.get(index as usize).cloned() {
                    z.hot_preset_id = p.id.clone();
                    c.settings_preset_index = index as usize;
                    // Default selection only affects the Default button / startup — do not
                    // switch the live dashboard view here.
                    c.config.set_zalender(z);
                    c.schedule_config_save();
                }
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_settings(&ui, &ctx.borrow());
            }
            log_cb("settings_set_default_preset", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_settings_add_preset(move || {
            let t0 = Instant::now();
            {
                let mut c = ctx.borrow_mut();
                let mut z = c.config.zalender();
                let id = z.alloc_preset_id();
                let mut preset = ViewPreset::builtin(ViewUnit::Month);
                preset.id = id;
                preset.name = "Custom".into();
                preset.builtin = false;
                preset.visible = true;
                z.presets.push(preset);
                c.settings_preset_index = z.presets.len().saturating_sub(1);
                c.settings_preset_name = "Custom".into();
                c.config.set_zalender(z);
                c.schedule_config_save();
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_settings(&ui, &ctx.borrow());
                apply_zalender(&ui, &ctx.borrow());
            }
            log_cb("settings_add_preset", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_settings_remove_preset(move |i| {
            let t0 = Instant::now();
            {
                let mut c = ctx.borrow_mut();
                let mut z = c.config.zalender();
                let idx = i as usize;
                if z.presets.get(idx).is_some_and(|p| !p.builtin) {
                    let removed_id = z.presets[idx].id.clone();
                    z.presets.remove(idx);
                    if z.hot_preset_id == removed_id {
                        z.hot_preset_id = "preset-month".into();
                    }
                    if c.active_preset_id == removed_id {
                        let hot = z.hot_preset();
                        c.active_preset_id = hot.id.clone();
                        c.view_unit = hot.unit;
                        c.view_count = hot.count.max(1);
                    }
                    z.ensure_presets();
                    c.settings_preset_index = c
                        .settings_preset_index
                        .min(z.presets.len().saturating_sub(1));
                    c.settings_preset_name = z
                        .presets
                        .get(c.settings_preset_index)
                        .map(|p| p.name.clone())
                        .unwrap_or_default();
                    c.config.set_zalender(z);
                    c.schedule_config_save();
                }
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_settings(&ui, &ctx.borrow());
                apply_zalender(&ui, &ctx.borrow());
            }
            log_cb("settings_remove_preset", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_settings_reset_preset(move |i| {
            let t0 = Instant::now();
            {
                let mut c = ctx.borrow_mut();
                let idx = i as usize;
                let mut z = c.config.zalender();
                if z.presets.get(idx).is_some_and(|p| p.builtin) {
                    let id = z.presets[idx].id.clone();
                    let visible = z.presets[idx].visible;
                    let unit = z.presets[idx].unit;
                    let mut fresh = ViewPreset::builtin(unit);
                    fresh.id = id.clone();
                    fresh.visible = visible;
                    let name = fresh.name.clone();
                    let count = fresh.count;
                    z.presets[idx] = fresh;
                    if c.settings_preset_index == idx {
                        c.settings_preset_name = name;
                    }
                    c.config.set_zalender(z);
                    c.schedule_config_save();
                    if c.active_preset_id == id {
                        c.view_unit = unit;
                        c.view_count = count.max(1);
                    }
                }
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_settings(&ui, &ctx.borrow());
                apply_zalender(&ui, &ctx.borrow());
            }
            log_cb("settings_reset_preset", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_settings_set_preset_name(move |name| {
            let t0 = Instant::now();
            {
                let mut c = ctx.borrow_mut();
                let name = name.trim().to_string();
                if !name.is_empty() {
                    c.settings_preset_name = name.clone();
                    let _ = with_selected_preset(&mut c, |p| {
                        if !p.builtin {
                            p.name = name;
                        }
                    });
                }
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_settings(&ui, &ctx.borrow());
                apply_zalender(&ui, &ctx.borrow());
            }
            log_cb("settings_set_preset_name", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_settings_set_preset_unit(move |v| {
            let t0 = Instant::now();
            {
                let mut c = ctx.borrow_mut();
                let _ = with_selected_preset(&mut c, |p| {
                    if !p.builtin {
                        p.unit = ViewUnit::from_i32(v);
                    }
                });
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_settings(&ui, &ctx.borrow());
                apply_zalender(&ui, &ctx.borrow());
            }
            log_cb("settings_set_preset_unit", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_settings_set_preset_count(move |v| {
            let t0 = Instant::now();
            {
                let mut c = ctx.borrow_mut();
                let _ = with_selected_preset(&mut c, |p| {
                    p.count = v.max(1) as u32;
                });
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_settings(&ui, &ctx.borrow());
                apply_zalender(&ui, &ctx.borrow());
            }
            log_cb("settings_set_preset_count", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_settings_set_preset_anchor(move |v| {
            let t0 = Instant::now();
            {
                let mut c = ctx.borrow_mut();
                let _ = with_selected_preset(&mut c, |p| {
                    p.anchor = AnchorMode::from_i32(v);
                });
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_settings(&ui, &ctx.borrow());
                apply_zalender(&ui, &ctx.borrow());
            }
            log_cb("settings_set_preset_anchor", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_settings_set_preset_day_start(move |v| {
            let t0 = Instant::now();
            {
                let mut c = ctx.borrow_mut();
                let _ = with_selected_preset(&mut c, |p| {
                    p.day_start_hour = (v as u32).min(23);
                    p.clamp_day_hours();
                });
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_settings(&ui, &ctx.borrow());
                apply_zalender(&ui, &ctx.borrow());
            }
            log_cb("settings_set_preset_day_start", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_settings_set_preset_day_end(move |v| {
            let t0 = Instant::now();
            {
                let mut c = ctx.borrow_mut();
                let _ = with_selected_preset(&mut c, |p| {
                    p.day_end_hour = (v as u32).min(23);
                    p.clamp_day_hours();
                });
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_settings(&ui, &ctx.borrow());
                apply_zalender(&ui, &ctx.borrow());
            }
            log_cb("settings_set_preset_day_end", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_settings_set_preset_day_interval(move |v| {
            let t0 = Instant::now();
            {
                let mut c = ctx.borrow_mut();
                let _ = with_selected_preset(&mut c, |p| {
                    p.day_interval_minutes = interval_minutes_from_index(v);
                    p.clamp_day_hours();
                });
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_settings(&ui, &ctx.borrow());
                apply_zalender(&ui, &ctx.borrow());
            }
            log_cb("settings_set_preset_day_interval", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_settings_set_preset_weekday(move |index, on| {
            let t0 = Instant::now();
            {
                let mut c = ctx.borrow_mut();
                let _ = with_selected_preset(&mut c, |p| {
                    let i = index as usize;
                    if i >= p.show_days.len() {
                        return;
                    }
                    let turning_off_last =
                        !on && p.show_days[i] && p.show_days.iter().filter(|d| **d).count() == 1;
                    if !turning_off_last {
                        p.show_days[i] = on;
                    }
                });
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_settings(&ui, &ctx.borrow());
                apply_zalender(&ui, &ctx.borrow());
            }
            log_cb("settings_set_preset_weekday", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_settings_set_preset_month_weeks(move |weeks| {
            let t0 = Instant::now();
            {
                let mut c = ctx.borrow_mut();
                let _ = with_selected_preset(&mut c, |p| {
                    p.month_weeks = weeks as u32;
                    p.clamp_month_weeks();
                });
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_settings(&ui, &ctx.borrow());
                apply_zalender(&ui, &ctx.borrow());
            }
            log_cb("settings_set_preset_month_weeks", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_settings_set_preset_month_from_week(move |on| {
            let t0 = Instant::now();
            {
                let mut c = ctx.borrow_mut();
                let _ = with_selected_preset(&mut c, |p| {
                    p.month_from_this_week = on;
                });
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_settings(&ui, &ctx.borrow());
                apply_zalender(&ui, &ctx.borrow());
            }
            log_cb("settings_set_preset_month_from_week", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_settings_set_preset_title_lines(move |lines| {
            let t0 = Instant::now();
            {
                let mut c = ctx.borrow_mut();
                let _ = with_selected_preset(&mut c, |p| {
                    p.title_lines = lines as u32;
                    p.clamp_title_lines();
                });
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_settings(&ui, &ctx.borrow());
                apply_zalender(&ui, &ctx.borrow());
            }
            log_cb("settings_set_preset_title_lines", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_settings_set_week_start(move |v| {
            let t0 = Instant::now();
            {
                let mut c = ctx.borrow_mut();
                let mut z = c.config.zalender();
                z.week_start = WeekStart::from_i32(v);
                c.config.set_zalender(z);
                c.schedule_config_save();
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_settings(&ui, &ctx.borrow());
                apply_zalender(&ui, &ctx.borrow());
            }
            log_cb("settings_set_week_start", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_settings_set_datetime_format(move |v| {
            let t0 = Instant::now();
            {
                let mut c = ctx.borrow_mut();
                c.config.datetime_format = DatetimeFormat::from_i32(v);
                c.schedule_config_save();
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_settings(&ui, &ctx.borrow());
                tick_clock_now(&ui, &ctx.borrow());
            }
            log_cb("settings_set_datetime_format", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_settings_set_font_family(move |v| {
            let t0 = Instant::now();
            {
                let mut c = ctx.borrow_mut();
                c.config.font_family = UiFontFamily::from_i32(v);
                c.schedule_config_save();
            }
            if let Some(ui) = ui_weak.upgrade() {
                let cfg = ctx.borrow().config.clone();
                apply_theme(&ui, &cfg);
                apply_settings(&ui, &ctx.borrow());
            }
            log_cb("settings_set_font_family", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_settings_set_font_scale(move |v| {
            let t0 = Instant::now();
            {
                let mut c = ctx.borrow_mut();
                c.config.font_scale = FontScale::from_i32(v);
                c.schedule_config_save();
            }
            if let Some(ui) = ui_weak.upgrade() {
                let cfg = ctx.borrow().config.clone();
                apply_theme(&ui, &cfg);
                apply_settings(&ui, &ctx.borrow());
            }
            log_cb("settings_set_font_scale", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_settings_set_event_font_scale(move |v| {
            let t0 = Instant::now();
            {
                let mut c = ctx.borrow_mut();
                c.config.event_font_scale = FontScale::from_i32(v);
                c.schedule_config_save();
            }
            if let Some(ui) = ui_weak.upgrade() {
                let cfg = ctx.borrow().config.clone();
                apply_theme(&ui, &cfg);
                apply_settings(&ui, &ctx.borrow());
            }
            log_cb("settings_set_event_font_scale", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_settings_set_modal_show_time(move |v| {
            patch_day_modal(&ui_weak, &ctx, |d| d.show_time = v);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_settings_set_modal_show_title(move |v| {
            patch_day_modal(&ui_weak, &ctx, |d| d.show_title = v);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_settings_set_modal_show_calendar(move |v| {
            patch_day_modal(&ui_weak, &ctx, |d| d.show_calendar = v);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_settings_set_modal_show_description(move |v| {
            patch_day_modal(&ui_weak, &ctx, |d| d.show_description = v);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_settings_set_dim_past(move |v| {
            patch_display(&ui_weak, &ctx, |d| d.dim_past = v);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_settings_set_shade_weekends(move |v| {
            patch_display(&ui_weak, &ctx, |d| d.shade_weekends = v);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_settings_set_new_field_type(move |v| {
            let mut c = ctx.borrow_mut();
            if let Some(ui) = ui_weak.upgrade() {
                c.new_field_name = ui.get_new_field_name().to_string();
            }
            c.new_field_type = CustomFieldType::from_i32(v);
            if c.new_field_type == CustomFieldType::Choice {
                ensure_two_choices(&mut c.new_field_options);
            }
            drop(c);
            if let Some(ui) = ui_weak.upgrade() {
                apply_settings(&ui, &ctx.borrow());
            }
        });
    }
    {
        let ctx = ctx.clone();
        ui.on_settings_set_new_field_option(move |i, text| {
            let mut c = ctx.borrow_mut();
            let i = i as usize;
            if i < c.new_field_options.len() {
                // Blank choices are allowed; keep the text as entered.
                c.new_field_options[i] = text.to_string();
            }
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_settings_add_field_option(move || {
            let mut c = ctx.borrow_mut();
            if let Some(ui) = ui_weak.upgrade() {
                c.new_field_name = ui.get_new_field_name().to_string();
            }
            c.new_field_options.push(String::new());
            drop(c);
            if let Some(ui) = ui_weak.upgrade() {
                apply_settings(&ui, &ctx.borrow());
            }
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_settings_remove_field_option(move |i| {
            let mut c = ctx.borrow_mut();
            if let Some(ui) = ui_weak.upgrade() {
                c.new_field_name = ui.get_new_field_name().to_string();
            }
            let i = i as usize;
            if c.new_field_options.len() > 2 && i < c.new_field_options.len() {
                c.new_field_options.remove(i);
            }
            drop(c);
            if let Some(ui) = ui_weak.upgrade() {
                apply_settings(&ui, &ctx.borrow());
            }
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_settings_add_custom_field(move || {
            let t0 = Instant::now();
            {
                let mut c = ctx.borrow_mut();
                if let Some(ui) = ui_weak.upgrade() {
                    c.new_field_name = ui.get_new_field_name().to_string();
                }
                let name = c.new_field_name.trim().to_string();
                let choice = c.new_field_type == CustomFieldType::Choice;
                if !name.is_empty() && (!choice || c.new_field_options.len() > 1) {
                    let options = if choice {
                        c.new_field_options.clone()
                    } else {
                        Vec::new()
                    };
                    let mut z = c.config.zalender();
                    let id = z.alloc_field_id();
                    z.custom_fields.push(CustomFieldDef {
                        id,
                        name,
                        field_type: c.new_field_type,
                        options,
                    });
                    c.config.set_zalender(z);
                    c.new_field_name.clear();
                    c.new_field_options.clear();
                    if choice {
                        ensure_two_choices(&mut c.new_field_options);
                    }
                    c.schedule_config_save();
                }
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_settings(&ui, &ctx.borrow());
            }
            log_cb("settings_add_custom_field", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_settings_remove_custom_field(move |i| {
            let t0 = Instant::now();
            {
                let mut c = ctx.borrow_mut();
                let mut z = c.config.zalender();
                if (i as usize) < z.custom_fields.len() {
                    z.custom_fields.remove(i as usize);
                }
                c.config.set_zalender(z);
                c.schedule_config_save();
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_settings(&ui, &ctx.borrow());
            }
            log_cb("settings_remove_custom_field", t0);
        });
    }
}

fn patch_day_modal(
    ui_weak: &Weak<AppWindow>,
    ctx: &std::rc::Rc<std::cell::RefCell<AppCtx>>,
    f: impl FnOnce(&mut crate::services::config::DayModalFields),
) {
    {
        let mut c = ctx.borrow_mut();
        let mut z = c.config.zalender();
        f(&mut z.day_modal);
        c.config.set_zalender(z);
        c.schedule_config_save();
    }
    if let Some(ui) = ui_weak.upgrade() {
        apply_settings(&ui, &ctx.borrow());
        apply_zalender(&ui, &ctx.borrow());
    }
}

fn shift_view_anchor(c: &mut AppCtx, dir: i32) {
    if c.view_unit == ViewUnit::Month {
        let z = c.config.zalender();
        if let Some(p) = z.presets.iter().find(|p| p.id == c.active_preset_id) {
            if p.month_from_this_week {
                let weeks = p.month_weeks.clamp(1, 6) as i32;
                c.anchor = zal_view::shift_anchor(c.anchor, ViewUnit::Week, dir * weeks);
                return;
            }
        }
    }
    let step = if c.view_unit == ViewUnit::Agenda {
        c.view_count.max(1) as i32
    } else {
        1
    };
    c.anchor = zal_view::shift_anchor(c.anchor, c.view_unit, dir * step);
}

fn patch_display(
    ui_weak: &Weak<AppWindow>,
    ctx: &std::rc::Rc<std::cell::RefCell<AppCtx>>,
    f: impl FnOnce(&mut DisplayOptions),
) {
    {
        let mut c = ctx.borrow_mut();
        let mut z = c.config.zalender();
        f(&mut z.display);
        c.config.set_zalender(z);
        c.schedule_config_save();
    }
    if let Some(ui) = ui_weak.upgrade() {
        apply_settings(&ui, &ctx.borrow());
        apply_zalender(&ui, &ctx.borrow());
    }
}

fn bind_zalender(ui: &AppWindow, ctx: &std::rc::Rc<std::cell::RefCell<AppCtx>>) {
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_zal_prev_period(move || {
            let t0 = Instant::now();
            {
                let mut c = ctx.borrow_mut();
                shift_view_anchor(&mut c, -1);
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_zalender(&ui, &ctx.borrow());
            }
            log_cb("zal_prev_period", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_zal_next_period(move || {
            let t0 = Instant::now();
            {
                let mut c = ctx.borrow_mut();
                shift_view_anchor(&mut c, 1);
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_zalender(&ui, &ctx.borrow());
            }
            log_cb("zal_next_period", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_zal_zoom_out(move || {
            let t0 = Instant::now();
            {
                let mut c = ctx.borrow_mut();
                c.view_unit = c.view_unit.zoom_out();
                c.view_count = 1;
                let z = c.config.zalender();
                if let Some(p) = z
                    .presets
                    .iter()
                    .find(|p| p.visible && p.unit == c.view_unit && p.count == 1)
                {
                    c.active_preset_id = p.id.clone();
                }
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_zalender(&ui, &ctx.borrow());
            }
            log_cb("zal_zoom_out", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_zal_go_today(move || {
            let t0 = Instant::now();
            {
                let mut c = ctx.borrow_mut();
                let today = Local::now().date_naive();
                c.selected_day = today;
                c.anchor = today;
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_zalender(&ui, &ctx.borrow());
            }
            log_cb("zal_go_today", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_zal_go_preset(move || {
            let t0 = Instant::now();
            {
                let mut c = ctx.borrow_mut();
                let mut preset = c.config.zalender().hot_preset();
                preset.clamp_count();
                c.active_preset_id = preset.id.clone();
                c.view_unit = preset.unit;
                c.view_count = preset.count.max(1);
                let today = Local::now().date_naive();
                c.selected_day = today;
                c.anchor = today;
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_zalender(&ui, &ctx.borrow());
            }
            log_cb("zal_go_preset", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_zal_select_preset(move |index| {
            let t0 = Instant::now();
            {
                let mut c = ctx.borrow_mut();
                let z = c.config.zalender();
                let visible = dashboard_presets(&z);
                if let Some(preset) = visible.get(index as usize) {
                    c.active_preset_id = preset.id.clone();
                    c.view_unit = preset.unit;
                    c.view_count = preset.count.max(1);
                }
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_zalender(&ui, &ctx.borrow());
            }
            log_cb("zal_select_preset", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_zal_select_date(move |key| {
            let t0 = Instant::now();
            if let Ok(d) = NaiveDate::parse_from_str(&key, "%Y-%m-%d") {
                let mut c = ctx.borrow_mut();
                c.selected_day = d;
                c.event_detail_open = false;
                match c.view_unit {
                    ViewUnit::Day | ViewUnit::Week => {
                        c.anchor = d;
                        let start = c.create_start_mins.max(0);
                        open_create_event_draft(&mut c, d, start, false);
                    }
                    ViewUnit::Month | ViewUnit::Year | ViewUnit::Agenda => {
                        // Keep the grid on the month already on screen. Neighboring-month
                        // days still open the modal; they must not retarget the view.
                        c.create_event_open = false;
                        c.day_modal_open = true;
                    }
                }
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_zalender(&ui, &ctx.borrow());
            }
            log_cb("zal_select_date", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_zal_open_create_event(move |key, start_mins, all_day| {
            let t0 = Instant::now();
            if let Ok(d) = NaiveDate::parse_from_str(&key, "%Y-%m-%d") {
                let mut c = ctx.borrow_mut();
                c.selected_day = d;
                c.anchor = d;
                c.event_detail_open = false;
                c.day_modal_open = false;
                open_create_event_draft(&mut c, d, start_mins.max(0), all_day);
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_zalender(&ui, &ctx.borrow());
            }
            log_cb("zal_open_create_event", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_zal_close_create_event(move || {
            let mut c = ctx.borrow_mut();
            c.create_event_open = false;
            c.create_repeat_open = false;
            if let Some(ui) = ui_weak.upgrade() {
                apply_zalender(&ui, &c);
            }
        });
    }
    {
        let ctx = ctx.clone();
        ui.on_zal_create_set_title(move |t| {
            ctx.borrow_mut().create_title = t.to_string();
        });
    }
    {
        let ctx = ctx.clone();
        ui.on_zal_create_set_location(move |t| {
            ctx.borrow_mut().create_location = t.to_string();
        });
    }
    {
        let ctx = ctx.clone();
        ui.on_zal_create_set_description(move |t| {
            ctx.borrow_mut().create_description = t.to_string();
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_zal_create_shift_date(move |delta| {
            {
                let mut c = ctx.borrow_mut();
                shift_create_date(&mut c, delta);
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_zalender(&ui, &ctx.borrow());
            }
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_zal_create_open_repeat(move || {
            {
                let mut c = ctx.borrow_mut();
                c.create_repeat = true;
                c.create_repeat_open = true;
                if !c.create_weekdays.iter().any(|on| *on) {
                    if let Ok(day) = NaiveDate::parse_from_str(&c.create_date_key, "%Y-%m-%d") {
                        c.create_weekdays[sunday_index(day)] = true;
                    }
                }
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_zalender(&ui, &ctx.borrow());
            }
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_zal_create_close_repeat(move || {
            ctx.borrow_mut().create_repeat_open = false;
            if let Some(ui) = ui_weak.upgrade() {
                apply_zalender(&ui, &ctx.borrow());
            }
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_zal_create_clear_repeat(move || {
            let mut c = ctx.borrow_mut();
            c.create_repeat = false;
            c.create_repeat_open = false;
            if let Some(ui) = ui_weak.upgrade() {
                apply_zalender(&ui, &c);
            }
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_zal_create_set_repeat_interval(move |n| {
            ctx.borrow_mut().create_repeat_interval = n.clamp(1, 99);
            if let Some(ui) = ui_weak.upgrade() {
                apply_zalender(&ui, &ctx.borrow());
            }
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_zal_create_set_repeat_unit(move |n| {
            let mut c = ctx.borrow_mut();
            c.create_repeat_unit = n.clamp(0, 4);
            if c.create_repeat_unit == 1 && !c.create_weekdays.iter().any(|on| *on) {
                if let Ok(day) = NaiveDate::parse_from_str(&c.create_date_key, "%Y-%m-%d") {
                    c.create_weekdays[sunday_index(day)] = true;
                }
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_zalender(&ui, &c);
            }
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_zal_create_set_repeat_end(move |n| {
            ctx.borrow_mut().create_repeat_end = n.clamp(0, 2);
            if let Some(ui) = ui_weak.upgrade() {
                apply_zalender(&ui, &ctx.borrow());
            }
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_zal_create_shift_until(move |delta| {
            {
                let mut c = ctx.borrow_mut();
                let base = NaiveDate::parse_from_str(&c.create_repeat_until, "%Y-%m-%d")
                    .or_else(|_| NaiveDate::parse_from_str(&c.create_date_key, "%Y-%m-%d"));
                if let Ok(day) = base {
                    let next = day + chrono::Duration::days(i64::from(delta));
                    let floor =
                        NaiveDate::parse_from_str(&c.create_date_key, "%Y-%m-%d").unwrap_or(next);
                    let next = if next < floor { floor } else { next };
                    c.create_repeat_until = next.format("%Y-%m-%d").to_string();
                }
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_zalender(&ui, &ctx.borrow());
            }
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_zal_create_set_repeat_count(move |n| {
            ctx.borrow_mut().create_repeat_count = n.clamp(1, 999);
            if let Some(ui) = ui_weak.upgrade() {
                apply_zalender(&ui, &ctx.borrow());
            }
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_zal_create_toggle_weekday(move |index| {
            let mut c = ctx.borrow_mut();
            let index = index.max(0) as usize;
            if index < 7 {
                let turning_off = c.create_weekdays[index];
                let selected = c.create_weekdays.iter().filter(|on| **on).count();
                if !(turning_off && selected <= 1) {
                    c.create_weekdays[index] = !turning_off;
                }
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_zalender(&ui, &c);
            }
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_zal_create_set_by_weekday(move |v| {
            ctx.borrow_mut().create_by_weekday = v;
            if let Some(ui) = ui_weak.upgrade() {
                apply_zalender(&ui, &ctx.borrow());
            }
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_zal_create_set_all_day(move |v| {
            ctx.borrow_mut().create_all_day = v;
            if let Some(ui) = ui_weak.upgrade() {
                apply_zalender(&ui, &ctx.borrow());
            }
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_zal_create_set_start_mins(move |m| {
            let mut c = ctx.borrow_mut();
            c.create_start_mins = m.clamp(0, 24 * 60 - 1);
            if c.create_end_mins <= c.create_start_mins {
                c.create_end_mins = (c.create_start_mins + 60).min(24 * 60);
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_zalender(&ui, &c);
            }
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_zal_create_set_end_mins(move |m| {
            let mut c = ctx.borrow_mut();
            c.create_end_mins = m.clamp(1, 24 * 60);
            if c.create_end_mins <= c.create_start_mins {
                c.create_start_mins = (c.create_end_mins - 60).max(0);
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_zalender(&ui, &c);
            }
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_zal_create_save(move || {
            let t0 = Instant::now();
            {
                let mut c = ctx.borrow_mut();
                if let Some(ui) = ui_weak.upgrade() {
                    c.create_title = ui.get_zal_create_title().to_string();
                    c.create_location = ui.get_zal_create_location().to_string();
                    c.create_description = ui.get_zal_create_description().to_string();
                }
                let title = c.create_title.trim().to_string();
                if title.is_empty() {
                    c.status = "Enter an event title.".into();
                    c.status_kind = 3;
                } else if let Ok(day) = NaiveDate::parse_from_str(&c.create_date_key, "%Y-%m-%d") {
                    let z = c.config.zalender();
                    let feed = z.default_feed();
                    let feed_id = feed.map(|f| f.id.as_str()).unwrap_or(LOCAL_FEED_ID);
                    let feed_name = feed
                        .map(|f| f.name.as_str())
                        .filter(|n| !n.is_empty())
                        .unwrap_or("Local");
                    let color = feed.map(|f| f.color.as_str()).unwrap_or("#22c55e");
                    let (start, end, all_day) = if c.create_all_day {
                        let start = day
                            .and_hms_opt(0, 0, 0)
                            .and_then(|t| Local.from_local_datetime(&t).single())
                            .map(|t| t.with_timezone(&Utc))
                            .unwrap_or_else(Utc::now);
                        let end = (day + chrono::Duration::days(1))
                            .and_hms_opt(0, 0, 0)
                            .and_then(|t| Local.from_local_datetime(&t).single())
                            .map(|t| t.with_timezone(&Utc))
                            .unwrap_or(start);
                        (start, end, true)
                    } else {
                        let start = local_mins_to_utc(day, c.create_start_mins as u32);
                        let end = local_mins_to_utc(day, c.create_end_mins as u32);
                        (start, end.max(start + chrono::Duration::minutes(15)), false)
                    };
                    let mut ev = local_events::new_local_event(
                        title, start, end, all_day, feed_id, feed_name, color,
                    );
                    ev.location = c.create_location.trim().to_string();
                    ev.description = c.create_description.trim().to_string();
                    if c.create_repeat {
                        let tz = recur::parse_tz(&z.timezone);
                        ev.rrule = Some(recur::rrule_from_draft(day, &repeat_draft(&c), tz));
                    }
                    c.local_events.add(ev);
                    if let Err(e) = c.local_events.save_if_dirty() {
                        c.status = format!("Could not save event: {e:#}");
                        c.status_kind = 3;
                    } else {
                        merge_local_into_snapshot(&mut c);
                        c.create_event_open = false;
                        c.create_repeat_open = false;
                        c.status = "Event created.".into();
                        c.status_kind = 1;
                    }
                }
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_zalender(&ui, &ctx.borrow());
            }
            log_cb("zal_create_save", t0);
        });
    }
    {
        let ctx = ctx.clone();
        ui.on_zal_name_local_set(move |t| {
            ctx.borrow_mut().name_local_draft = t.to_string();
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_zal_name_local_save(move || {
            let t0 = Instant::now();
            {
                let mut c = ctx.borrow_mut();
                if let Some(ui) = ui_weak.upgrade() {
                    c.name_local_draft = ui.get_zal_name_local_draft().to_string();
                }
                let name = c.name_local_draft.trim().to_string();
                if name.is_empty() {
                    c.status = "Enter a name for your local calendar.".into();
                    c.status_kind = 3;
                } else {
                    let mut z = c.config.zalender();
                    if let Some(f) = z.local_feed_mut() {
                        f.name = name;
                    }
                    c.config.set_zalender(z);
                    c.schedule_config_save();
                    c.name_local_open = false;
                    c.name_local_draft.clear();
                }
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_settings(&ui, &ctx.borrow());
                apply_zalender(&ui, &ctx.borrow());
            }
            log_cb("zal_name_local_save", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_zal_open_event_uid(move |date_key, uid| {
            let t0 = Instant::now();
            if let Ok(d) = NaiveDate::parse_from_str(&date_key, "%Y-%m-%d") {
                let mut c = ctx.borrow_mut();
                c.selected_day = d;
                c.detail_uid = Some(uid.to_string());
                c.detail_index = None;
                c.event_detail_open = true;
                c.create_event_open = false;
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_zalender(&ui, &ctx.borrow());
            }
            log_cb("zal_open_event_uid", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_zal_select_side_date(move |key| {
            let t0 = Instant::now();
            if let Ok(d) = NaiveDate::parse_from_str(&key, "%Y-%m-%d") {
                let mut c = ctx.borrow_mut();
                c.selected_day = d;
                c.anchor = d;
                c.view_unit = ViewUnit::Day;
                c.view_count = 1;
                c.day_modal_open = false;
                c.event_detail_open = false;
                let z = c.config.zalender();
                if let Some(preset) = dashboard_presets(&z)
                    .into_iter()
                    .find(|p| p.unit == ViewUnit::Day && p.count == 1)
                {
                    c.active_preset_id = preset.id;
                }
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_zalender(&ui, &ctx.borrow());
            }
            log_cb("zal_select_side_date", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_zal_refresh(move || {
            let t0 = Instant::now();
            if let Some(ui) = ui_weak.upgrade() {
                maybe_start_refresh(&ui, &ctx);
            }
            log_cb("zal_refresh", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_zal_toggle_feed(move |i| {
            let t0 = Instant::now();
            {
                let mut c = ctx.borrow_mut();
                let mut z = c.config.zalender();
                if let Some(f) = z.feeds.get_mut(i as usize) {
                    f.visible = !f.visible;
                }
                c.config.set_zalender(z);
                c.schedule_config_save();
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_zalender(&ui, &ctx.borrow());
            }
            log_cb("zal_toggle_feed", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_zal_set_default_feed(move |i| {
            let t0 = Instant::now();
            {
                let mut c = ctx.borrow_mut();
                let mut z = c.config.zalender();
                let pick = z
                    .feeds
                    .get(i as usize)
                    .filter(|f| f.writable)
                    .map(|f| f.id.clone());
                if let Some(id) = pick {
                    if z.default_feed_id != id {
                        z.default_feed_id = id;
                        c.config.set_zalender(z);
                        c.schedule_config_save();
                    }
                }
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_zalender(&ui, &ctx.borrow());
            }
            log_cb("zal_set_default_feed", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_zal_close_day_modal(move || {
            ctx.borrow_mut().day_modal_open = false;
            ctx.borrow_mut().event_detail_open = false;
            if let Some(ui) = ui_weak.upgrade() {
                apply_zalender(&ui, &ctx.borrow());
            }
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_zal_open_event(move |i| {
            {
                let mut c = ctx.borrow_mut();
                c.detail_index = Some(i as usize);
                c.detail_uid = None;
                c.event_detail_open = true;
                c.create_event_open = false;
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_zalender(&ui, &ctx.borrow());
            }
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_zal_close_event_detail(move || {
            ctx.borrow_mut().event_detail_open = false;
            if let Some(ui) = ui_weak.upgrade() {
                apply_zalender(&ui, &ctx.borrow());
            }
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_zal_open_customize(move || {
            {
                let mut c = ctx.borrow_mut();
                if let Some(ev) = current_detail_event(&c).cloned() {
                    let z = c.config.zalender();
                    let base = z
                        .feeds
                        .iter()
                        .find(|f| f.id == ev.feed_id)
                        .map(|f| f.color.as_str())
                        .unwrap_or(ev.color.as_str());
                    let resolved = c.overrides.resolve_over(&ev, base);
                    c.customize_color = resolved.color;
                    c.customize_icon = resolved.icon;
                    c.customize_fields = resolved.fields;
                    c.customize_occurrence = ev.recurrence_id.is_some();
                    c.customize_open = true;
                }
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_zalender(&ui, &ctx.borrow());
            }
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_zal_open_edit(move || {
            {
                let mut c = ctx.borrow_mut();
                let feed_id = current_detail_event(&c).map(|ev| ev.feed_id.clone());
                let writable = feed_id
                    .as_ref()
                    .map(|id| {
                        c.config
                            .zalender()
                            .feeds
                            .iter()
                            .find(|f| &f.id == id)
                            .map(|f| f.writable)
                            .unwrap_or(false)
                    })
                    .unwrap_or(false);
                if writable {
                    c.edit_placeholder_open = true;
                }
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_zalender(&ui, &ctx.borrow());
            }
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_zal_close_customize(move || {
            ctx.borrow_mut().customize_open = false;
            if let Some(ui) = ui_weak.upgrade() {
                apply_zalender(&ui, &ctx.borrow());
            }
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_zal_close_edit_placeholder(move || {
            ctx.borrow_mut().edit_placeholder_open = false;
            if let Some(ui) = ui_weak.upgrade() {
                apply_zalender(&ui, &ctx.borrow());
            }
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_zal_customize_set_color(move |c| {
            ctx.borrow_mut().customize_color = c.to_string();
            if let Some(ui) = ui_weak.upgrade() {
                apply_zalender(&ui, &ctx.borrow());
            }
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_zal_customize_set_icon(move |i| {
            ctx.borrow_mut().customize_icon = i.to_string();
            if let Some(ui) = ui_weak.upgrade() {
                apply_zalender(&ui, &ctx.borrow());
            }
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_zal_customize_set_occurrence(move |o| {
            ctx.borrow_mut().customize_occurrence = o;
            if let Some(ui) = ui_weak.upgrade() {
                apply_zalender(&ui, &ctx.borrow());
            }
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_zal_customize_set_field(move |id, val| {
            let id = id.to_string();
            // Rebuilding customize-fields replaces every TextInput, which drops
            // the cursor after a keystroke. Yes/no and choice still need a refresh
            // so the control shows the new value; text stays in the input itself.
            let refresh = {
                let mut c = ctx.borrow_mut();
                c.customize_fields.insert(id.clone(), val.to_string());
                c.config.zalender().custom_fields.iter().any(|f| {
                    f.id == id
                        && matches!(f.field_type, CustomFieldType::Bool | CustomFieldType::Choice)
                })
            };
            if refresh {
                if let Some(ui) = ui_weak.upgrade() {
                    apply_zalender(&ui, &ctx.borrow());
                }
            }
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_zal_customize_save(move || {
            {
                let mut c = ctx.borrow_mut();
                if let Some(ev) = current_detail_event(&c).cloned() {
                    let key = OverrideKey::from_event(&ev, c.customize_occurrence);
                    let value = EventOverride {
                        color: Some(c.customize_color.clone()).filter(|s| !s.is_empty()),
                        icon: Some(c.customize_icon.clone()).filter(|s| !s.is_empty()),
                        fields: c.customize_fields.clone(),
                    };
                    c.overrides.set(key, value);
                    c.customize_open = false;
                    c.schedule_overrides_save();
                }
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_zalender(&ui, &ctx.borrow());
            }
        });
    }
}

fn current_detail_event(c: &AppCtx) -> Option<&zal_model::CalEvent> {
    let snap = c.snapshot.as_ref()?;
    let visible = visible_feed_ids(&c.config.zalender());
    if let Some(uid) = c.detail_uid.as_ref() {
        return snap
            .events
            .iter()
            .find(|e| e.uid == *uid && visible.contains(&e.feed_id));
    }
    let idx = c.detail_index?;
    snap.events_on(c.selected_day)
        .into_iter()
        .filter(|e| visible.contains(&e.feed_id))
        .nth(idx)
}

fn visible_feed_ids(z: &ZalenderConfig) -> std::collections::HashSet<String> {
    z.feeds
        .iter()
        .filter(|f| f.visible)
        .map(|f| f.id.clone())
        .collect()
}

fn finish_setup_inner(c: &mut AppCtx) {
    let draft = std::mem::replace(&mut c.setup, SetupDraft::from_config(&c.config, &[]));
    draft.apply_to(&mut c.config);
    let _ = c.config.save(&c.paths);
    c.shell.screen = Screen::Home;
    c.shell.active_module = if c.config.module_enabled("zalender") {
        "zalender".into()
    } else {
        String::new()
    };
    focus_wijitify_if_no_modules(c);
    let metas = registry::all_metas();
    let ids: Vec<(&str, &str, &str)> = metas
        .iter()
        .map(|m| (m.id, m.name, m.description))
        .collect();
    c.setup = SetupDraft::from_config(&c.config, &ids);
    let mut z = c.config.zalender();
    z.ensure_presets();
    let preset = z.hot_preset();
    c.active_preset_id = preset.id.clone();
    c.view_unit = preset.unit;
    c.view_count = preset.count.max(1);
    c.sync_modules();
    let mut z = c.config.zalender();
    z.ensure_local_feed();
    z.ensure_default_feed();
    let needs_name = z
        .local_feed()
        .map(|f| f.name.trim().is_empty())
        .unwrap_or(true);
    c.config.set_zalender(z);
    if needs_name {
        c.name_local_open = true;
        c.name_local_draft.clear();
    }
}

fn maybe_start_refresh(ui: &AppWindow, ctx: &std::rc::Rc<std::cell::RefCell<AppCtx>>) {
    let (feeds, cache_dir, runtime, gen, windowed) = {
        let mut c = ctx.borrow_mut();
        if c.refreshing {
            return;
        }
        let z = c.config.zalender();
        if z.feeds.is_empty() {
            c.status = "No feeds configured".into();
            c.status_kind = 3;
            apply_zalender(ui, &c);
            return;
        }
        c.refresh_gen += 1;
        let gen = c.refresh_gen;
        c.refreshing = true;
        c.status = String::new();
        c.status_kind = 2;
        let cache_dir = match c.paths.module_cache("zalender") {
            Ok(d) => d,
            Err(e) => {
                c.refreshing = false;
                c.status = format!("Cache error: {e:#}");
                c.status_kind = 3;
                apply_zalender(ui, &c);
                return;
            }
        };
        let today = Local::now().date_naive();
        let window_start = today - chrono::Duration::days(90);
        let window_end = today + chrono::Duration::days(365);
        (
            z.feeds,
            cache_dir,
            c.runtime.clone(),
            gen,
            (window_start, window_end),
        )
    };
    apply_zalender(ui, &ctx.borrow());

    let ui_weak = ui.as_weak();
    let ctx = ctx.clone();
    let (window_start, window_end) = windowed;
    let task = runtime.handle.spawn(async move {
        zalender::refresh_feeds(&feeds, &cache_dir, window_start, window_end).await
    });
    let _ = slint::spawn_local(async move {
        let result = task.await.map_err(anyhow::Error::from).and_then(|r| r);
        let mut c = ctx.borrow_mut();
        if gen != c.refresh_gen {
            return;
        }
        c.refreshing = false;
        match result {
            Ok((snap, statuses)) => {
                let offline = statuses.iter().any(|s| s.from_cache && !s.ok);
                c.snapshot = Some(snap);
                merge_local_into_snapshot(&mut c);
                let n = c.snapshot.as_ref().map(|s| s.events.len()).unwrap_or(0);
                c.status = if offline {
                    format!(
                        "Offline — loaded {n} events · last updated {}",
                        c.snapshot
                            .as_ref()
                            .map(|s| s.generated_at.format("%Y-%m-%d %H:%M UTC").to_string())
                            .unwrap_or_default()
                    )
                } else {
                    format!(
                        "Loaded {n} events · updated {}",
                        c.snapshot
                            .as_ref()
                            .map(|s| s.generated_at.format("%Y-%m-%d %H:%M UTC").to_string())
                            .unwrap_or_default()
                    )
                };
                c.status_kind = if offline { 2 } else { 1 };
            }
            Err(e) => {
                c.status = format!("Refresh failed: {e:#}");
                c.status_kind = 3;
                merge_local_into_snapshot(&mut c);
            }
        }
        if let Some(ui) = ui_weak.upgrade() {
            apply_zalender(&ui, &c);
        }
    });
}

fn apply_all(ui: &AppWindow, c: &AppCtx) {
    apply_theme(ui, &c.config);
    apply_shell(ui, c);
    apply_setup(ui, c);
    apply_settings(ui, c);
    apply_zalender(ui, c);
}

fn apply_theme(ui: &AppWindow, cfg: &Config) {
    let theme = ui.global::<Theme>();
    let p = &cfg.palette;
    theme.set_is_dark(cfg.theme.is_dark());
    theme.set_bg(color::parse_hex(&p.bg));
    theme.set_surface(color::parse_hex(&p.surface));
    theme.set_surface_2(color::parse_hex(&p.surface_2));
    theme.set_surface_press(color::parse_hex(&p.surface_press));
    theme.set_text(color::parse_hex(&p.text));
    theme.set_muted(color::parse_hex(&p.muted));
    theme.set_accent(color::parse_hex(&p.accent));
    theme.set_accent_press(color::parse_hex(&p.accent_press));
    theme.set_danger(color::parse_hex(&p.danger));
    theme.set_danger_press(color::parse_hex(&p.danger_press));
    theme.set_ok(color::parse_hex(&p.ok));
    theme.set_text_on_accent(color::parse_hex(&p.text_on_accent));
    theme.set_font_family(SharedString::from(cfg.font_family.family_name()));
    theme.set_font_scale(cfg.font_scale.factor());
    theme.set_event_font_scale(cfg.event_font_scale.factor());
}

fn canonical_hex(raw: &str) -> Option<String> {
    let n = normalize_hex(raw);
    let body = n.strip_prefix('#')?;
    if body.len() == 6 && body.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(n)
    } else {
        None
    }
}

fn settings_feed_row(f: &FeedConfig) -> SettingsFeedRow {
    let name = if f.local && f.name.trim().is_empty() {
        "Local calendar"
    } else {
        f.name.as_str()
    };
    SettingsFeedRow {
        id: SharedString::from(f.id.as_str()),
        name: SharedString::from(name),
        url: SharedString::from(f.url.as_str()),
        color: color::parse_hex(&f.color),
        color_hex: SharedString::from(f.color.as_str()),
        visible: f.visible,
        writable: f.writable,
        local: f.local,
    }
}

fn open_create_event_draft(c: &mut AppCtx, day: NaiveDate, start_mins: i32, all_day: bool) {
    let start = start_mins.clamp(0, 24 * 60 - 15);
    c.create_date_key = day.format("%Y-%m-%d").to_string();
    c.create_start_mins = start;
    c.create_end_mins = (start + 60).min(24 * 60);
    c.create_all_day = all_day;
    c.create_title.clear();
    c.create_location.clear();
    c.create_description.clear();
    c.create_repeat = false;
    c.create_repeat_open = false;
    c.create_repeat_interval = 1;
    c.create_repeat_unit = 0;
    c.create_repeat_end = 0;
    c.create_repeat_until = c.create_date_key.clone();
    c.create_repeat_count = 10;
    c.create_weekdays = [false; 7];
    c.create_weekdays[sunday_index(day)] = true;
    c.create_by_weekday = false;
    c.create_event_open = true;
    c.day_modal_open = false;
}

fn sunday_index(day: NaiveDate) -> usize {
    day.weekday().num_days_from_sunday() as usize
}

fn shift_create_date(c: &mut AppCtx, delta: i32) {
    let Ok(day) = NaiveDate::parse_from_str(&c.create_date_key, "%Y-%m-%d") else {
        return;
    };
    let next = day + chrono::Duration::days(i64::from(delta));
    let old_i = sunday_index(day);
    let new_i = sunday_index(next);
    let selected = c.create_weekdays.iter().filter(|on| **on).count();
    if selected == 1 && c.create_weekdays[old_i] {
        c.create_weekdays = [false; 7];
        c.create_weekdays[new_i] = true;
    }
    c.create_date_key = next.format("%Y-%m-%d").to_string();
    if let Ok(until) = NaiveDate::parse_from_str(&c.create_repeat_until, "%Y-%m-%d") {
        if until < next {
            c.create_repeat_until = c.create_date_key.clone();
        }
    }
}

fn repeat_draft(c: &AppCtx) -> RepeatDraft {
    RepeatDraft {
        interval: c.create_repeat_interval.max(1) as u32,
        unit: c.create_repeat_unit.clamp(0, 4) as u8,
        end: c.create_repeat_end.clamp(0, 2) as u8,
        until: NaiveDate::parse_from_str(&c.create_repeat_until, "%Y-%m-%d").ok(),
        count: c.create_repeat_count.max(1) as u32,
        weekdays: c.create_weekdays,
        by_weekday: c.create_by_weekday,
    }
}

fn local_mins_to_utc(day: NaiveDate, mins: u32) -> chrono::DateTime<Utc> {
    if mins >= 24 * 60 {
        let next = day + chrono::Duration::days(1);
        return local_mins_to_utc(next, 0);
    }
    let h = mins / 60;
    let m = mins % 60;
    let naive = day
        .and_hms_opt(h, m, 0)
        .or_else(|| day.and_hms_opt(0, 0, 0))
        .unwrap_or(day.and_time(NaiveTime::MIN));
    Local
        .from_local_datetime(&naive)
        .single()
        .map(|t| t.with_timezone(&Utc))
        .unwrap_or_else(Utc::now)
}

fn merge_local_into_snapshot(c: &mut AppCtx) {
    let today = Local::now().date_naive();
    let window_start = today - chrono::Duration::days(90);
    let window_end = today + chrono::Duration::days(365);
    let from = window_start
        .and_hms_opt(0, 0, 0)
        .map(|n| Utc.from_utc_datetime(&n))
        .unwrap_or_else(Utc::now);
    let to = window_end
        .and_hms_opt(23, 59, 59)
        .map(|n| Utc.from_utc_datetime(&n))
        .unwrap_or_else(Utc::now);
    let tz = recur::parse_tz(&c.config.zalender().timezone);
    let stored: std::collections::HashSet<&str> = c
        .local_events
        .events()
        .iter()
        .map(|e| e.uid.as_str())
        .collect();
    let mut events: Vec<zal_model::CalEvent> = c
        .snapshot
        .as_ref()
        .map(|s| {
            s.events
                .iter()
                .filter(|e| e.feed_id != LOCAL_FEED_ID && !stored.contains(e.uid.as_str()))
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    events.extend(
        c.local_events
            .events()
            .iter()
            .flat_map(|ev| recur::expand_stored(ev, from, to, tz)),
    );
    c.snapshot = Some(CalendarSnapshot::from_events(
        events,
        window_start,
        window_end,
    ));
}

fn patch_theme_color(ui: &AppWindow, index: usize, id: &str, label: &str, hex: &str) -> bool {
    let model = ui.get_theme_colors();
    let Some(rows) = model.as_any().downcast_ref::<VecModel<ThemeColorRow>>() else {
        return false;
    };
    if index >= rows.row_count() {
        return false;
    }
    rows.set_row_data(
        index,
        ThemeColorRow {
            id: SharedString::from(id),
            label: SharedString::from(label),
            hex: SharedString::from(hex),
            swatch: color::parse_hex(hex),
            tip: SharedString::from(theme_color_tip(id)),
        },
    );
    true
}

fn patch_settings_feed(ui: &AppWindow, index: usize, feed: &FeedConfig) -> bool {
    let model = ui.get_settings_feeds();
    let Some(rows) = model.as_any().downcast_ref::<VecModel<SettingsFeedRow>>() else {
        return false;
    };
    if index >= rows.row_count() {
        return false;
    }
    rows.set_row_data(index, settings_feed_row(feed));
    true
}

fn event_style_color(c: &AppCtx, z: &ZalenderConfig, e: &zal_model::CalEvent) -> String {
    let base = z
        .feeds
        .iter()
        .find(|f| f.id == e.feed_id)
        .map(|f| f.color.as_str())
        .unwrap_or(e.color.as_str());
    c.overrides.resolve_over(e, base).color
}

fn normalize_hex(raw: &str) -> String {
    let s = raw.trim();
    let hex = s.strip_prefix('#').unwrap_or(s);
    if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        format!("#{}", hex.to_ascii_lowercase())
    } else if hex.len() == 3 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        let mut out = String::from("#");
        for c in hex.chars() {
            out.push(c);
            out.push(c);
        }
        out.make_ascii_lowercase();
        out
    } else {
        s.to_string()
    }
}

fn theme_color_tip(key: &str) -> &'static str {
    match key {
        "bg" => "Main window and active calendar view background.",
        "surface" => "Chrome panels: top bar, sidebar, cards, and settings surfaces.",
        "surface_2" => "Secondary fills: pills, chips, and inset controls.",
        "surface_press" => "Pressed / hover highlight on surface controls.",
        "text" => "Primary body and title text color.",
        "muted" => "Secondary captions, hints, and status italics.",
        "accent" => "Selected controls, today highlight, and primary buttons.",
        "accent_press" => "Pressed state for accent-colored controls.",
        "danger" => "Destructive actions such as Remove and Quit.",
        "danger_press" => "Pressed state for danger buttons.",
        "ok" => "Success banners and positive status accents.",
        "text_on_accent" => "Text and icons drawn on accent or filled buttons.",
        _ => "Theme color token.",
    }
}

fn sync_feed_fields_from_ui(ui: &AppWindow, setup: &mut SetupDraft) {
    setup.new_feed_name = ui.get_new_feed_name().to_string();
    setup.new_feed_url = ui.get_new_feed_url().to_string();
    setup.new_feed_color = ui.get_new_feed_color().to_string();
}

fn any_module_enabled(config: &Config) -> bool {
    registry::all_metas()
        .iter()
        .any(|m| config.module_enabled(m.id))
}

fn focus_wijitify_if_no_modules(c: &mut AppCtx) {
    if !any_module_enabled(&c.config) {
        c.shell.settings_page = WIJITIFY_SETTINGS_PAGE;
    }
}

fn apply_shell(ui: &AppWindow, c: &AppCtx) {
    ui.set_screen(c.shell.screen.as_i32());
    ui.set_active_module(SharedString::from(c.shell.active_module.as_str()));
    ui.set_settings_open(c.shell.settings_open);
    tick_clock_now(ui, c);
}

fn apply_setup(ui: &AppWindow, c: &AppCtx) {
    ui.set_setup_step(c.setup.step);
    let metas = registry::all_metas();
    let rows: Vec<SetupModuleRow> = metas
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let selected = c.setup.selected.get(i).map(|(_, on)| *on).unwrap_or(false);
            SetupModuleRow {
                id: SharedString::from(m.id),
                name: SharedString::from(m.name),
                description: SharedString::from(m.description),
                selected,
            }
        })
        .collect();
    ui.set_setup_modules(ModelRc::new(VecModel::from(rows)));
    ui.set_zalender_timezone(SharedString::from(c.setup.zalender.timezone.as_str()));
    let feeds: Vec<FeedRow> = c
        .setup
        .zalender
        .feeds
        .iter()
        .map(|f| FeedRow {
            name: SharedString::from(f.name.as_str()),
            url: SharedString::from(f.url.as_str()),
            color: color::parse_hex(&f.color),
        })
        .collect();
    ui.set_zalender_feeds(ModelRc::new(VecModel::from(feeds)));
    ui.set_new_feed_name(SharedString::from(c.setup.new_feed_name.as_str()));
    ui.set_new_feed_url(SharedString::from(c.setup.new_feed_url.as_str()));
    ui.set_new_feed_color(SharedString::from(c.setup.new_feed_color.as_str()));
    ui.set_show_zalender_setup(c.setup.zalender_selected());
    ui.set_setup_banner_message(SharedString::from(c.setup.banner_message.as_str()));
    ui.set_setup_banner_kind(c.setup.banner_kind);
}

fn apply_settings(ui: &AppWindow, c: &AppCtx) {
    let page_names = [
        "Calendars",
        "Global",
        "Theme",
        "Preset views",
        "Calendar display",
        "Custom fields",
        "Invites",
        "Help",
        "About",
        "Wijitify",
    ];
    ui.set_current_version(SharedString::from(env!("CARGO_PKG_VERSION")));
    let modules_enabled = any_module_enabled(&c.config);
    let page = if modules_enabled {
        c.shell.settings_page
    } else {
        WIJITIFY_SETTINGS_PAGE
    };
    ui.set_settings_page(page);
    ui.set_settings_page_names(ModelRc::new(VecModel::from(
        page_names
            .iter()
            .map(|s| SharedString::from(*s))
            .collect::<Vec<_>>(),
    )));
    let visible: Vec<i32> = if modules_enabled {
        (0..page_names.len() as i32).collect()
    } else {
        vec![WIJITIFY_SETTINGS_PAGE]
    };
    ui.set_settings_visible_pages(ModelRc::new(VecModel::from(visible)));

    let metas = registry::all_metas();
    let rows: Vec<ModuleToggleRow> = metas
        .iter()
        .map(|m| ModuleToggleRow {
            id: SharedString::from(m.id),
            name: SharedString::from(m.name),
            description: SharedString::from(m.description),
            enabled: c.config.module_enabled(m.id),
        })
        .collect();
    ui.set_settings_modules(ModelRc::new(VecModel::from(rows)));
    ui.set_theme_is_dark(c.config.theme.is_dark());
    ui.set_theme_banner(SharedString::from(c.theme_banner.as_str()));
    ui.set_theme_banner_kind(c.theme_banner_kind);

    let theme_colors: Vec<ThemeColorRow> = ThemePalette::labels()
        .iter()
        .map(|(id, label)| {
            let hex = c.config.palette.get(id).to_string();
            ThemeColorRow {
                id: SharedString::from(*id),
                label: SharedString::from(*label),
                hex: SharedString::from(hex.as_str()),
                swatch: color::parse_hex(&hex),
                tip: SharedString::from(theme_color_tip(id)),
            }
        })
        .collect();
    ui.set_theme_colors(ModelRc::new(VecModel::from(theme_colors)));

    let z = c.config.zalender();
    let feeds: Vec<SettingsFeedRow> = z.feeds.iter().map(settings_feed_row).collect();
    ui.set_settings_feeds(ModelRc::new(VecModel::from(feeds)));
    ui.set_settings_feed_banner(SharedString::from(c.settings_feed_banner.as_str()));
    ui.set_settings_feed_banner_kind(c.settings_feed_banner_kind);
    ui.set_settings_new_feed_name(SharedString::from(c.settings_new_feed_name.as_str()));
    ui.set_settings_new_feed_url(SharedString::from(c.settings_new_feed_url.as_str()));
    ui.set_settings_new_feed_color(SharedString::from(c.settings_new_feed_color.as_str()));

    let sel_idx = if c.settings_preset_index < z.presets.len() {
        c.settings_preset_index
    } else {
        0
    };
    let selected = z.presets.get(sel_idx);
    let preset_rows: Vec<SettingsPresetRow> = z
        .presets
        .iter()
        .enumerate()
        .map(|(i, p)| SettingsPresetRow {
            id: SharedString::from(p.id.as_str()),
            name: SharedString::from(p.name.as_str()),
            summary: SharedString::from(preset_row_summary(p)),
            visible: p.visible,
            builtin: p.builtin,
            selected: i == sel_idx,
            is_hot: p.id == z.hot_preset_id,
        })
        .collect();
    ui.set_settings_presets(ModelRc::new(VecModel::from(preset_rows)));
    ui.set_preset_selected(selected.is_some());
    ui.set_preset_is_builtin(selected.map(|p| p.builtin).unwrap_or(true));
    ui.set_preset_index(sel_idx as i32);
    let name = if c.settings_preset_name.is_empty() {
        selected.map(|p| p.name.as_str()).unwrap_or("")
    } else {
        c.settings_preset_name.as_str()
    };
    ui.set_preset_name(SharedString::from(name));
    ui.set_preset_unit(selected.map(|p| p.unit.as_i32()).unwrap_or(2));
    ui.set_preset_count(selected.map(|p| p.count as i32).unwrap_or(1));
    ui.set_preset_anchor(selected.map(|p| p.anchor.as_i32()).unwrap_or(0));
    ui.set_preset_day_start_hour(selected.map(|p| p.day_start_hour as i32).unwrap_or(6));
    ui.set_preset_day_end_hour(selected.map(|p| p.day_end_hour as i32).unwrap_or(21));
    ui.set_preset_day_interval(
        selected
            .map(|p| interval_index_from_minutes(p.day_interval_minutes))
            .unwrap_or(2),
    );
    let show_days = selected.map(|p| p.show_days).unwrap_or([true; 7]);
    ui.set_preset_weekdays(ModelRc::new(VecModel::from(weekday_checks(
        z.week_start,
        &show_days,
    ))));
    ui.set_preset_month_weeks(selected.map(|p| p.month_weeks as i32).unwrap_or(6));
    ui.set_preset_month_from_week(selected.map(|p| p.month_from_this_week).unwrap_or(false));
    ui.set_preset_title_lines(selected.map(|p| p.title_lines as i32).unwrap_or(1));
    ui.set_title_line_options(string_model(&[
        "1", "2", "3", "4", "5", "6", "7", "8", "9", "10",
    ]));
    ui.set_week_count_options(string_model(&["1", "2", "3", "4", "5", "6"]));
    ui.set_week_start(z.week_start.as_i32());
    ui.set_datetime_format(c.config.datetime_format.as_i32());
    ui.set_font_family(c.config.font_family.as_i32());
    ui.set_font_scale(c.config.font_scale.as_i32());
    ui.set_event_font_scale(c.config.event_font_scale.as_i32());
    ui.set_modal_show_time(z.day_modal.show_time);
    ui.set_modal_show_title(z.day_modal.show_title);
    ui.set_modal_show_calendar(z.day_modal.show_calendar);
    ui.set_modal_show_description(z.day_modal.show_description);
    ui.set_dim_past(z.display.dim_past);
    ui.set_shade_weekends(z.display.shade_weekends);

    ui.set_unit_options(string_model(&[
        ViewUnit::Day.label(),
        ViewUnit::Week.label(),
        ViewUnit::Month.label(),
        ViewUnit::Year.label(),
        ViewUnit::Agenda.label(),
    ]));
    let max = selected.map(|p| p.unit.max_count()).unwrap_or(6) as usize;
    let counts: Vec<SharedString> = (1..=max)
        .map(|n| SharedString::from(n.to_string()))
        .collect();
    ui.set_count_options(ModelRc::new(VecModel::from(counts)));
    ui.set_anchor_options(string_model(&[
        AnchorMode::CurrentFirst.label(),
        AnchorMode::Sunday.label(),
        AnchorMode::Monday.label(),
        AnchorMode::Tuesday.label(),
        AnchorMode::Wednesday.label(),
        AnchorMode::Thursday.label(),
        AnchorMode::Friday.label(),
        AnchorMode::Saturday.label(),
    ]));
    ui.set_hour_options(ModelRc::new(VecModel::from(hour_option_labels())));
    ui.set_interval_options(string_model(&[
        "15 minutes",
        "30 minutes",
        "1 hour",
        "2 hours",
    ]));
    ui.set_week_start_options(string_model(&[
        WeekStart::Sunday.label(),
        WeekStart::Monday.label(),
        WeekStart::Tuesday.label(),
        WeekStart::Wednesday.label(),
        WeekStart::Thursday.label(),
        WeekStart::Friday.label(),
        WeekStart::Saturday.label(),
    ]));
    ui.set_datetime_options(string_model(&[
        DatetimeFormat::DdMmYyAmPm.label(),
        DatetimeFormat::MmDdYyAmPm.label(),
        DatetimeFormat::YyyyMmDd24h.label(),
        DatetimeFormat::DdMmYyyy24h.label(),
    ]));
    let font_choices = UiFontFamily::all();
    ui.set_font_family_options(string_model(&font_choices.map(UiFontFamily::label)));
    ui.set_font_family_faces(string_model(&font_choices.map(UiFontFamily::family_name)));
    ui.set_font_scale_options(string_model(&[
        FontScale::Small.label(),
        FontScale::Medium.label(),
        FontScale::Large.label(),
        FontScale::XLarge.label(),
    ]));
    ui.set_field_type_options(string_model(&[
        CustomFieldType::Text.label(),
        CustomFieldType::LongText.label(),
        CustomFieldType::Bool.label(),
        CustomFieldType::Choice.label(),
    ]));
    ui.set_new_field_type(c.new_field_type.as_i32());
    ui.set_new_field_name(SharedString::from(c.new_field_name.as_str()));
    ui.set_new_field_options(ModelRc::new(VecModel::from(
        c.new_field_options
            .iter()
            .map(|s| SharedString::from(s.as_str()))
            .collect::<Vec<_>>(),
    )));

    let defs: Vec<SettingsFieldDefRow> = z
        .custom_fields
        .iter()
        .map(|f| SettingsFieldDefRow {
            id: SharedString::from(f.id.as_str()),
            name: SharedString::from(f.name.as_str()),
            type_label: SharedString::from(field_type_label(f)),
        })
        .collect();
    ui.set_custom_field_defs(ModelRc::new(VecModel::from(defs)));
    ui.set_help_text(SharedString::from(HELP_TEXT));

    if c.perf_enabled {
        ui.set_perf_overlay(SharedString::from("WIJITIFY_PERF=1"));
    } else {
        ui.set_perf_overlay(SharedString::from(""));
    }
}

fn ensure_two_choices(options: &mut Vec<String>) {
    while options.len() < 2 {
        options.push(String::new());
    }
}

fn field_type_label(f: &CustomFieldDef) -> String {
    if f.field_type != CustomFieldType::Choice {
        return f.field_type.label().to_string();
    }
    let list = f
        .options
        .iter()
        .map(|o| if o.is_empty() { "(blank)" } else { o.as_str() })
        .collect::<Vec<_>>()
        .join(" · ");
    format!("Choice: {list}")
}

fn string_model(items: &[&str]) -> ModelRc<SharedString> {
    ModelRc::new(VecModel::from(
        items
            .iter()
            .map(|s| SharedString::from(*s))
            .collect::<Vec<_>>(),
    ))
}

fn hour_option_labels() -> Vec<SharedString> {
    (0u32..24)
        .map(|h| {
            let (h12, suffix) = match h {
                0 => (12, "AM"),
                1..=11 => (h, "AM"),
                12 => (12, "PM"),
                _ => (h - 12, "PM"),
            };
            SharedString::from(format!("{h12}:00 {suffix}"))
        })
        .collect()
}

fn interval_minutes_from_index(i: i32) -> u32 {
    match i {
        0 => 15,
        1 => 30,
        3 => 120,
        _ => 60,
    }
}

fn interval_index_from_minutes(m: u32) -> i32 {
    match m {
        15 => 0,
        30 => 1,
        120 => 3,
        _ => 2,
    }
}

/// Presets shown in the dashboard view pill (visible ones, or all if none marked visible).
fn dashboard_presets(z: &ZalenderConfig) -> Vec<ViewPreset> {
    let visible: Vec<ViewPreset> = z.presets.iter().filter(|p| p.visible).cloned().collect();
    if visible.is_empty() {
        z.presets.clone()
    } else {
        visible
    }
}

fn apply_update(ui: &AppWindow, state: &UpdateState) {
    ui.set_current_version(SharedString::from(env!("CARGO_PKG_VERSION")));
    match state {
        UpdateState::Unsupported => {
            ui.set_update_status(status::UNSUPPORTED);
        }
        UpdateState::UpToDate { checked } => {
            ui.set_update_status(status::UP_TO_DATE);
            ui.set_update_checked(SharedString::from(checked.as_str()));
        }
        UpdateState::Available {
            version,
            notes,
            checked,
        } => {
            ui.set_update_status(status::AVAILABLE);
            ui.set_update_version(SharedString::from(version.as_str()));
            ui.set_update_notes(SharedString::from(notes.as_str()));
            ui.set_update_checked(SharedString::from(checked.as_str()));
        }
        UpdateState::Installing { percent } => {
            ui.set_update_status(status::INSTALLING);
            ui.set_update_percent(*percent as i32);
        }
        UpdateState::ReadyToRestart => {
            ui.set_update_status(status::READY_TO_RESTART);
        }
        UpdateState::Failed(message) => {
            ui.set_update_status(status::FAILED);
            ui.set_update_error(SharedString::from(message.as_str()));
        }
    }
}

fn send_update_cmd(ctx: &std::rc::Rc<std::cell::RefCell<AppCtx>>, cmd: UpdateCmd) {
    if let Some(tx) = &ctx.borrow().update_tx {
        let _ = tx.try_send(cmd);
    }
}

fn start_updater(ui: &AppWindow, ctx: &std::rc::Rc<std::cell::RefCell<AppCtx>>) {
    let (tx, rx) = tokio::sync::mpsc::channel(8);
    ctx.borrow_mut().update_tx = Some(tx);
    let runtime = ctx.borrow().runtime.clone();
    let ui_weak = ui.as_weak();
    let on_state: Arc<dyn Fn(UpdateState) + Send + Sync> = Arc::new(move |state| {
        let _ = ui_weak.upgrade_in_event_loop(move |ui| apply_update(&ui, &state));
    });
    update::start(runtime, rx, on_state);
    apply_update(ui, &UpdateState::Unsupported);
}

fn apply_zalender(ui: &AppWindow, c: &AppCtx) {
    let today = Local::now().date_naive();
    let z = c.config.zalender();
    let week_start = z.week_start;
    let visible = visible_feed_ids(&z);

    let title = zal_view::title_for_view(c.anchor, c.view_unit, c.view_count, week_start);
    ui.set_zal_title_period(SharedString::from(title));
    ui.set_zal_status(SharedString::from(c.status.as_str()));
    ui.set_zal_status_kind(c.status_kind);
    ui.set_zal_refreshing(c.refreshing);
    ui.set_zal_empty_feeds(z.feeds.is_empty());
    ui.set_zal_view_unit(c.view_unit.as_i32());
    ui.set_zal_view_count(if c.view_unit == ViewUnit::Month {
        1
    } else {
        c.view_count as i32
    });

    let dash = dashboard_presets(&z);
    let options: Vec<SharedString> = dash
        .iter()
        .map(|p| SharedString::from(p.display_label()))
        .collect();
    let selected = dash
        .iter()
        .position(|p| p.id == c.active_preset_id)
        .unwrap_or(0);
    let label = dash
        .get(selected)
        .map(|p| p.display_label())
        .unwrap_or_else(|| {
            if c.view_count > 1 {
                format!("{} {}", c.view_unit.label(), c.view_count)
            } else {
                c.view_unit.label().into()
            }
        });
    ui.set_zal_preset_options(ModelRc::new(VecModel::from(options)));
    ui.set_zal_preset_selected(selected as i32);
    ui.set_zal_preset_label(SharedString::from(label.as_str()));

    let hour_preset = z
        .presets
        .iter()
        .find(|p| p.id == c.active_preset_id)
        .cloned()
        .unwrap_or_else(|| {
            let mut p = ViewPreset::builtin(c.view_unit);
            p.count = c.view_count.max(1);
            p
        });
    let hour_slots: Vec<DayHourBucket> = hour_preset
        .hour_slots()
        .into_iter()
        .map(|(label, start, end)| DayHourBucket {
            label: SharedString::from(label),
            start_mins: start as i32,
            end_mins: end as i32,
        })
        .collect();
    let slot_grid = hour_slots
        .first()
        .map(|s| (s.end_mins - s.start_mins, s.start_mins))
        .unwrap_or((60, 0));
    ui.set_zal_hour_slots(ModelRc::new(VecModel::from(hour_slots)));

    let labels = zal_view::week_labels(week_start);
    ui.set_zal_week_labels(ModelRc::new(VecModel::from(
        labels
            .iter()
            .map(|s| SharedString::from(*s))
            .collect::<Vec<_>>(),
    )));
    let show_days = hour_preset.show_days;
    let month_rows = if c.view_unit == ViewUnit::Month {
        zal_view::month_week_span(
            c.anchor,
            hour_preset.month_weeks,
            hour_preset.month_from_this_week,
            week_start,
        )
        .1
    } else {
        6
    };
    ui.set_zal_month_rows(month_rows as i32);
    ui.set_zal_title_lines(hour_preset.title_lines.clamp(1, 10) as i32);
    let grid_cols = zal_view::shown_week_columns(week_start, &show_days);
    ui.set_zal_grid_columns(grid_cols.len() as i32);
    ui.set_zal_grid_labels(ModelRc::new(VecModel::from(
        grid_cols
            .iter()
            .map(|&col| SharedString::from(labels[col]))
            .collect::<Vec<_>>(),
    )));

    let side = c.anchor;
    ui.set_zal_side_title(SharedString::from(side.format("%B %Y").to_string()));
    let side_days = build_month_cells(
        side, today, week_start, c, &z, &visible, None, false, &[true; 7], 6,
    );
    ui.set_zal_side_days(ModelRc::new(VecModel::from(side_days)));

    let (main_days, year_titles) =
        build_main_days(c, &z, today, week_start, &visible, slot_grid, &hour_preset);
    let all_day_rows = if matches!(c.view_unit, ViewUnit::Day | ViewUnit::Week) {
        main_days.iter().map(|cell| cell.all_day_count).max().unwrap_or(0)
    } else {
        0
    };
    ui.set_zal_all_day_rows(all_day_rows);
    ui.set_zal_main_days(ModelRc::new(VecModel::from(main_days)));
    ui.set_zal_year_month_titles(ModelRc::new(VecModel::from(
        year_titles
            .into_iter()
            .map(SharedString::from)
            .collect::<Vec<_>>(),
    )));

    let feeds: Vec<FeedChip> = z
        .feeds
        .iter()
        .map(|f| {
            let name = if f.local && f.name.trim().is_empty() {
                "Local calendar"
            } else {
                f.name.as_str()
            };
            FeedChip {
                id: SharedString::from(f.id.as_str()),
                name: SharedString::from(name),
                color: color::parse_hex(&f.color),
                visible: f.visible,
                writable: f.writable,
                is_default: f.writable && f.id == z.default_feed_id,
            }
        })
        .collect();
    ui.set_zal_feeds(ModelRc::new(VecModel::from(feeds)));

    let now = Local::now();
    ui.set_zal_now_mins((now.hour() * 60 + now.minute()) as i32);
    let show_event_time = matches!(c.view_unit, ViewUnit::Day | ViewUnit::Agenda)
        || (c.view_unit == ViewUnit::Week && c.view_count <= 1);
    ui.set_zal_show_event_time(show_event_time);
    ui.set_zal_dim_past(z.display.dim_past);
    ui.set_zal_shade_weekends(z.display.shade_weekends);

    ui.set_zal_day_modal_open(c.day_modal_open);
    ui.set_zal_day_modal_title(SharedString::from(
        c.selected_day.format("%A, %B %e %Y").to_string(),
    ));
    ui.set_zal_selected_date_key(SharedString::from(
        c.selected_day.format("%Y-%m-%d").to_string(),
    ));
    let day_events = build_day_events(c, &visible, &z);
    ui.set_zal_day_events(ModelRc::new(VecModel::from(day_events)));

    ui.set_zal_name_local_open(c.name_local_open);
    ui.set_zal_name_local_draft(SharedString::from(c.name_local_draft.as_str()));
    ui.set_zal_create_event_open(c.create_event_open);
    ui.set_zal_create_title(SharedString::from(c.create_title.as_str()));
    ui.set_zal_create_location(SharedString::from(c.create_location.as_str()));
    ui.set_zal_create_description(SharedString::from(c.create_description.as_str()));
    ui.set_zal_create_date_key(SharedString::from(c.create_date_key.as_str()));
    let create_day =
        NaiveDate::parse_from_str(&c.create_date_key, "%Y-%m-%d").unwrap_or(c.selected_day);
    ui.set_zal_create_date_label(SharedString::from(
        create_day.format("%A, %B %e").to_string(),
    ));
    ui.set_zal_create_start_mins(c.create_start_mins);
    ui.set_zal_create_end_mins(c.create_end_mins);
    ui.set_zal_create_all_day(c.create_all_day);
    let create_name = z
        .default_feed()
        .map(|f| {
            if f.name.trim().is_empty() {
                if f.local {
                    "Local calendar"
                } else {
                    "Calendar"
                }
            } else {
                f.name.as_str()
            }
        })
        .unwrap_or("Local calendar");
    ui.set_zal_create_calendar_name(SharedString::from(create_name));
    ui.set_zal_create_start_label(SharedString::from(mins_label(c.create_start_mins)));
    ui.set_zal_create_end_label(SharedString::from(mins_label(c.create_end_mins)));
    let draft = repeat_draft(c);
    let (month_dom, month_nth) = recur::month_choice_labels(create_day);
    let (quarter_dom, quarter_nth) = recur::quarter_choice_labels(create_day);
    ui.set_zal_create_repeat(c.create_repeat);
    ui.set_zal_create_repeat_open(c.create_repeat_open);
    ui.set_zal_create_repeat_interval(c.create_repeat_interval);
    ui.set_zal_create_repeat_unit(c.create_repeat_unit);
    ui.set_zal_create_repeat_end(c.create_repeat_end);
    ui.set_zal_create_repeat_count(c.create_repeat_count);
    ui.set_zal_create_by_weekday(c.create_by_weekday);
    ui.set_zal_create_repeat_summary(SharedString::from(recur::repeat_summary(
        create_day, &draft,
    )));
    ui.set_zal_create_until_label(SharedString::from(
        NaiveDate::parse_from_str(&c.create_repeat_until, "%Y-%m-%d")
            .map(|d| d.format("%A, %B %e").to_string())
            .unwrap_or_else(|_| c.create_repeat_until.clone()),
    ));
    ui.set_zal_create_month_dom_label(SharedString::from(month_dom));
    ui.set_zal_create_month_nth_label(SharedString::from(month_nth));
    ui.set_zal_create_quarter_dom_label(SharedString::from(quarter_dom));
    ui.set_zal_create_quarter_nth_label(SharedString::from(quarter_nth));
    let unit_words: [&str; 5] = if c.create_repeat_interval == 1 {
        ["Day", "Week", "Month", "Quarterly", "Year"]
    } else {
        ["Days", "Weeks", "Months", "Quarters", "Years"]
    };
    ui.set_zal_create_repeat_units(ModelRc::new(VecModel::from(
        unit_words
            .into_iter()
            .map(SharedString::from)
            .collect::<Vec<_>>(),
    )));
    const WEEK_LETTERS: [&str; 7] = ["S", "M", "T", "W", "T", "F", "S"];
    let weekdays: Vec<RepeatWeekday> = WEEK_LETTERS
        .iter()
        .enumerate()
        .map(|(i, letter)| RepeatWeekday {
            letter: SharedString::from(*letter),
            index: i as i32,
            on: c.create_weekdays[i],
        })
        .collect();
    ui.set_zal_create_weekdays(ModelRc::new(VecModel::from(weekdays)));

    ui.set_zal_event_detail_open(c.event_detail_open);
    let detail = detail_event_row(c, &visible, &z);
    ui.set_zal_detail_event(detail.0);
    ui.set_zal_detail_custom_summary(SharedString::from(detail.1));

    ui.set_zal_customize_open(c.customize_open);
    ui.set_zal_customize_color(color::parse_hex(if c.customize_color.is_empty() {
        "#3b82f6"
    } else {
        c.customize_color.as_str()
    }));
    ui.set_zal_customize_icon(SharedString::from(c.customize_icon.as_str()));
    ui.set_zal_customize_occurrence(c.customize_occurrence);
    ui.set_zal_customize_has_recurrence(
        current_detail_event(c)
            .and_then(|e| e.recurrence_id.as_ref())
            .is_some(),
    );
    let custom_rows: Vec<CustomFieldRow> = z
        .custom_fields
        .iter()
        .map(|f| CustomFieldRow {
            id: SharedString::from(f.id.as_str()),
            name: SharedString::from(f.name.as_str()),
            field_type: f.field_type.as_i32(),
            value: SharedString::from(c.customize_fields.get(&f.id).cloned().unwrap_or_default()),
            option_count: f.options.len() as i32,
            has_blank: f.options.iter().any(|o| o.is_empty()),
        })
        .collect();
    ui.set_zal_customize_fields(ModelRc::new(VecModel::from(custom_rows)));
    let choice_rows: Vec<ChoiceOption> = z
        .custom_fields
        .iter()
        .filter(|f| f.field_type == CustomFieldType::Choice)
        .flat_map(|f| {
            f.options.iter().map(|label| ChoiceOption {
                field_id: SharedString::from(f.id.as_str()),
                label: SharedString::from(label.as_str()),
            })
        })
        .collect();
    ui.set_zal_customize_choices(ModelRc::new(VecModel::from(choice_rows)));
    ui.set_zal_edit_placeholder_open(c.edit_placeholder_open);
}

#[allow(clippy::too_many_arguments)]
fn build_month_cells(
    month: NaiveDate,
    today: NaiveDate,
    week_start: WeekStart,
    c: &AppCtx,
    z: &ZalenderConfig,
    visible: &std::collections::HashSet<String>,
    slots: Option<(i32, i32)>,
    // Prefix other-month day labels with a 3-letter month abbr (main month grid only).
    other_month_abbr: bool,
    show_days: &[bool; 7],
    rows: usize,
) -> Vec<DayCell> {
    let grid = zal_view::month_grid(month.year(), month.month(), today, week_start);
    let cols = zal_view::shown_week_columns(week_start, show_days);
    let rows = rows.clamp(1, 6);
    let mut cells = Vec::with_capacity(rows * cols.len());
    for row in 0..rows {
        for &col in &cols {
            let (d, in_month, is_today) = grid[row * 7 + col];
            cells.push(cell_for_day(
                d,
                in_month,
                is_today,
                c,
                z,
                visible,
                slots,
                other_month_abbr,
            ));
        }
    }
    cells
}

/// `slots` = (slot step, day start) in minutes; `Some` attaches that day's event rows.
#[allow(clippy::too_many_arguments)]
fn cell_for_day(
    d: NaiveDate,
    in_month: bool,
    is_today: bool,
    c: &AppCtx,
    z: &ZalenderConfig,
    visible: &std::collections::HashSet<String>,
    slots: Option<(i32, i32)>,
    other_month_abbr: bool,
) -> DayCell {
    let today = Local::now().date_naive();
    let day_events: Vec<_> = c
        .snapshot
        .as_ref()
        .map(|s| {
            s.events_on(d)
                .into_iter()
                .filter(|e| visible.contains(&e.feed_id))
                .collect()
        })
        .unwrap_or_default();
    let has = !day_events.is_empty();
    let event_count = day_events.len() as i32;
    let all_day_count = day_events.iter().filter(|e| e.all_day).count() as i32;
    let event_color = day_events
        .first()
        .map(|e| color::parse_hex(&event_style_color(c, z, e)))
        .unwrap_or_else(|| color::parse_hex("#3b82f6"));
    let wd = d.weekday().num_days_from_sunday();
    let is_selected = d == c.selected_day;
    let in_selected_week = matches!(c.view_unit, ViewUnit::Week) && {
        let ws = zal_view::week_start_num(z.week_start);
        let sel_week = zal_model::start_of_week(c.selected_day, ws);
        let day_week = zal_model::start_of_week(d, ws);
        sel_week == day_week
    };
    let events = slots
        .map(|s| day_rows(&day_events, d, c, z, s))
        .unwrap_or_default();
    DayCell {
        day: d.day() as i32,
        in_month,
        is_today,
        is_past: d < today,
        is_weekend: wd == 0 || wd == 6,
        has_events: has,
        event_count,
        all_day_count,
        is_selected,
        in_selected_week,
        // Main month view: other-month cells show "Oct 5"; elsewhere day number only.
        label: SharedString::from(if other_month_abbr && !in_month {
            format!("{} {}", d.format("%b"), d.day())
        } else {
            d.day().to_string()
        }),
        event_color,
        date_key: SharedString::from(d.format("%Y-%m-%d").to_string()),
        events: ModelRc::new(VecModel::from(events)),
    }
}

fn build_main_days(
    c: &AppCtx,
    z: &ZalenderConfig,
    today: NaiveDate,
    week_start: WeekStart,
    visible: &std::collections::HashSet<String>,
    slots: (i32, i32),
    preset: &ViewPreset,
) -> (Vec<DayCell>, Vec<String>) {
    let mut cells = Vec::new();
    let mut titles = Vec::new();
    let slots = Some(slots);
    let show_days = &preset.show_days;
    let shown = |d: NaiveDate| preset.shows_date(d);
    match c.view_unit {
        ViewUnit::Day => {
            for i in 0..c.view_count {
                let d = c.anchor + chrono::Duration::days(i64::from(i));
                if !shown(d) {
                    continue;
                }
                cells.push(cell_for_day(
                    d,
                    true,
                    d == today,
                    c,
                    z,
                    visible,
                    slots,
                    false,
                ));
                if let Some(last) = cells.last_mut() {
                    // Full weekday name; day number is shown separately in the header.
                    last.label = SharedString::from(d.format("%A").to_string());
                }
            }
        }
        ViewUnit::Week => {
            let start = zal_model::start_of_week(c.anchor, zal_view::week_start_num(week_start));
            for i in 0..(c.view_count * 7) {
                let d = start + chrono::Duration::days(i64::from(i));
                if !shown(d) {
                    continue;
                }
                cells.push(cell_for_day(
                    d,
                    true,
                    d == today,
                    c,
                    z,
                    visible,
                    slots,
                    false,
                ));
                if let Some(last) = cells.last_mut() {
                    // Weekday only — day number is shown in the header badge (keeps columns equal-width).
                    last.label = SharedString::from(d.format("%a").to_string().to_uppercase());
                }
            }
        }
        ViewUnit::Month => {
            let (start, weeks) = zal_view::month_week_span(
                c.anchor,
                preset.month_weeks,
                preset.month_from_this_week,
                week_start,
            );
            cells.extend(build_rolling_weeks(
                start, weeks, c.anchor, today, week_start, c, z, visible, slots, show_days,
            ));
        }
        ViewUnit::Year => {
            for y in 0..c.view_count {
                let year = c.anchor.year() + y as i32;
                for month in 1..=12 {
                    let first = NaiveDate::from_ymd_opt(year, month, 1).unwrap_or(today);
                    titles.push(first.format("%b %Y").to_string());
                    cells.extend(build_month_cells(
                        first, today, week_start, c, z, visible, None, false, show_days, 6,
                    ));
                }
            }
        }
        ViewUnit::Agenda => {
            let weeks = c.view_count.max(1);
            let days_n = weeks * 7;
            for i in 0..days_n {
                let d = c.anchor + chrono::Duration::days(i64::from(i));
                if !shown(d) {
                    continue;
                }
                let cell = cell_for_day(d, true, d == today, c, z, visible, slots, false);
                if cell.has_events {
                    cells.push(cell);
                    if let Some(last) = cells.last_mut() {
                        last.label = SharedString::from(d.format("%A, %b %e").to_string());
                    }
                }
            }
        }
    }
    (cells, titles)
}

#[allow(clippy::too_many_arguments)]
fn build_rolling_weeks(
    start: NaiveDate,
    weeks: u32,
    anchor: NaiveDate,
    today: NaiveDate,
    week_start: WeekStart,
    c: &AppCtx,
    z: &ZalenderConfig,
    visible: &std::collections::HashSet<String>,
    slots: Option<(i32, i32)>,
    show_days: &[bool; 7],
) -> Vec<DayCell> {
    let cols = zal_view::shown_week_columns(week_start, show_days);
    let mut cells = Vec::with_capacity(weeks as usize * cols.len());
    for w in 0..weeks {
        let week = start + chrono::Duration::days(7 * i64::from(w));
        for &col in &cols {
            let d = week + chrono::Duration::days(col as i64);
            let in_month = d.month() == anchor.month() && d.year() == anchor.year();
            cells.push(cell_for_day(
                d,
                in_month,
                d == today,
                c,
                z,
                visible,
                slots,
                true,
            ));
        }
    }
    cells
}

fn preset_row_summary(p: &ViewPreset) -> String {
    let days = shown_day_summary(&p.show_days);
    let lines = if p.title_lines > 1 {
        format!(" · {} lines", p.title_lines)
    } else {
        String::new()
    };
    let auto = if p.builtin { " · auto" } else { "" };
    if p.unit == ViewUnit::Month {
        format!(
            "{}{}{}{}{}",
            p.unit.label(),
            month_layout_summary(p),
            days,
            lines,
            auto
        )
    } else {
        format!(
            "{} · count {}{}{}{}",
            p.unit.label(),
            p.count,
            days,
            lines,
            auto
        )
    }
}

fn month_layout_summary(p: &ViewPreset) -> String {
    if p.unit != ViewUnit::Month {
        return String::new();
    }
    if p.month_from_this_week {
        format!(" · {}w from this week", p.month_weeks)
    } else {
        format!(" · {}w from month start", p.month_weeks)
    }
}

fn shown_day_summary(show_days: &[bool; 7]) -> String {
    let n = show_days.iter().filter(|on| **on).count();
    if n == 7 {
        String::new()
    } else {
        format!(" · {n} days")
    }
}

fn weekday_checks(week_start: WeekStart, show_days: &[bool; 7]) -> Vec<WeekdayCheck> {
    let labels = zal_view::week_labels(week_start);
    let start = week_start.as_chrono_num();
    (0..7)
        .map(|col| {
            let sun_index = ((start + col) % 7 + 1) % 7;
            WeekdayCheck {
                label: SharedString::from(labels[col as usize]),
                index: sun_index as i32,
                on: show_days[sun_index as usize],
            }
        })
        .collect()
}

fn build_day_events(
    c: &AppCtx,
    visible: &std::collections::HashSet<String>,
    z: &ZalenderConfig,
) -> Vec<EventRow> {
    let modal = &z.day_modal;
    let Some(snap) = c.snapshot.as_ref() else {
        return Vec::new();
    };
    let day_events: Vec<_> = snap
        .events_on(c.selected_day)
        .into_iter()
        .filter(|e| visible.contains(&e.feed_id))
        .collect();
    let all_day_total = day_events.iter().filter(|e| e.all_day).count() as i32;
    let mut all_day_ord = 0i32;
    let mut rows = Vec::with_capacity(day_events.len());
    for (day_ord, e) in day_events.iter().enumerate() {
        let mut row = event_to_row(e, c.selected_day, c, z, modal);
        row.day_ord = day_ord as i32;
        if e.all_day {
            row.slot_ord = all_day_ord;
            row.slot_total = all_day_total;
            all_day_ord += 1;
        }
        rows.push(row);
    }
    rows
}

/// One day's chip rows with all-day / per-hour-slot ordinals for overflow ("+N more").
fn day_rows(
    day_events: &[&zal_model::CalEvent],
    day: NaiveDate,
    c: &AppCtx,
    z: &ZalenderConfig,
    (slot_step, day_start_mins): (i32, i32),
) -> Vec<EventRow> {
    let all_day_total = day_events.iter().filter(|e| e.all_day).count() as i32;
    let mut slot_totals: std::collections::BTreeMap<i32, i32> = std::collections::BTreeMap::new();
    for e in day_events.iter().filter(|e| !e.all_day) {
        let local = e.start.with_timezone(&Local);
        let start_mins = (local.hour() * 60 + local.minute()) as i32;
        *slot_totals
            .entry(slot_bucket_start(start_mins, slot_step, day_start_mins))
            .or_default() += 1;
    }
    let mut all_day_ord = 0i32;
    let mut slot_ords: std::collections::BTreeMap<i32, i32> = std::collections::BTreeMap::new();
    let mut rows = Vec::with_capacity(day_events.len());
    for (day_ord, e) in day_events.iter().enumerate() {
        let mut row = event_to_row(e, day, c, z, &z.day_modal);
        row.day_ord = day_ord as i32;
        if e.all_day {
            row.slot_ord = all_day_ord;
            row.slot_total = all_day_total;
            all_day_ord += 1;
        } else {
            let key = slot_bucket_start(row.start_mins, slot_step, day_start_mins);
            let ord = slot_ords.entry(key).or_default();
            row.slot_ord = *ord;
            row.slot_total = *slot_totals.get(&key).unwrap_or(&0);
            *ord += 1;
        }
        rows.push(row);
    }
    rows
}

/// Align timed events to the same slot starts the day/week grid uses.
fn slot_bucket_start(start_mins: i32, step: i32, day_start_mins: i32) -> i32 {
    let step = step.max(1);
    if start_mins <= day_start_mins {
        return day_start_mins;
    }
    let rel = start_mins - day_start_mins;
    day_start_mins + (rel - rel % step)
}

fn event_to_row(
    e: &zal_model::CalEvent,
    day: NaiveDate,
    c: &AppCtx,
    z: &ZalenderConfig,
    modal: &crate::services::config::DayModalFields,
) -> EventRow {
    let feed = z.feeds.iter().find(|f| f.id == e.feed_id);
    let base = feed.map(|f| f.color.as_str()).unwrap_or(e.color.as_str());
    let resolved = c.overrides.resolve_over(e, base);
    let writable = feed.map(|f| f.writable).unwrap_or(false);
    let feed_name = feed
        .map(|f| f.name.as_str())
        .unwrap_or(e.feed_name.as_str());
    let time = if e.all_day {
        "All day".into()
    } else {
        let start = e.start.with_timezone(&Local);
        let end = e.end.with_timezone(&Local);
        format!("{} - {}", start.format("%H:%M"), end.format("%H:%M"))
    };
    let start_mins = if e.all_day {
        -1
    } else {
        let local = e.start.with_timezone(&Local);
        (local.hour() * 60 + local.minute()) as i32
    };
    let end_mins = if e.all_day {
        -1
    } else {
        let local = e.end.with_timezone(&Local);
        (local.hour() * 60 + local.minute()) as i32
    };
    let time_short = if e.all_day {
        "All day".into()
    } else {
        format!("{} – {}", mins_label(start_mins), mins_label(end_mins))
    };
    EventRow {
        uid: SharedString::from(e.uid.as_str()),
        recurrence_id: SharedString::from(e.recurrence_id.as_deref().unwrap_or("")),
        feed_id: SharedString::from(e.feed_id.as_str()),
        date_key: SharedString::from(day.format("%Y-%m-%d").to_string()),
        time: SharedString::from(if modal.show_time { time } else { String::new() }),
        time_short: SharedString::from(time_short),
        title: SharedString::from(if modal.show_title {
            e.title.as_str()
        } else {
            ""
        }),
        calendar: SharedString::from(if modal.show_calendar { feed_name } else { "" }),
        description: SharedString::from(if modal.show_description {
            e.description.as_str()
        } else {
            ""
        }),
        location: SharedString::from(e.location.as_str()),
        color: color::parse_hex(&resolved.color),
        icon: SharedString::from(resolved.icon.as_str()),
        writable,
        all_day: e.all_day,
        start_mins,
        day_ord: 0,
        slot_ord: 0,
        slot_total: 0,
    }
}

fn mins_label(mins: i32) -> String {
    let mins = mins.clamp(0, 24 * 60) as u32;
    if mins >= 24 * 60 {
        return "12 AM".into();
    }
    let h24 = mins / 60;
    let m = mins % 60;
    let (h12, suffix) = match h24 {
        0 => (12, "AM"),
        1..=11 => (h24, "AM"),
        12 => (12, "PM"),
        _ => (h24 - 12, "PM"),
    };
    if m == 0 {
        format!("{h12} {suffix}")
    } else {
        format!("{h12}:{m:02} {suffix}")
    }
}

fn detail_event_row(
    c: &AppCtx,
    visible: &std::collections::HashSet<String>,
    z: &ZalenderConfig,
) -> (EventRow, String) {
    let empty = EventRow {
        uid: SharedString::from(""),
        recurrence_id: SharedString::from(""),
        feed_id: SharedString::from(""),
        date_key: SharedString::from(""),
        time: SharedString::from(""),
        time_short: SharedString::from(""),
        title: SharedString::from(""),
        calendar: SharedString::from(""),
        description: SharedString::from(""),
        location: SharedString::from(""),
        color: color::parse_hex("#3b82f6"),
        icon: SharedString::from(""),
        writable: false,
        all_day: false,
        start_mins: -1,
        day_ord: 0,
        slot_ord: 0,
        slot_total: 0,
    };
    let Some(ev) = current_detail_event(c) else {
        return (empty, String::new());
    };
    let row = event_to_row(ev, c.selected_day, c, z, &z.day_modal);
    let resolved = c.overrides.resolve(ev);
    let summary = if resolved.fields.is_empty() {
        String::new()
    } else {
        z.custom_fields
            .iter()
            .filter_map(|f| {
                resolved
                    .fields
                    .get(&f.id)
                    .map(|v| format!("{}: {}", f.name, v))
            })
            .collect::<Vec<_>>()
            .join(" · ")
    };
    let _ = visible;
    (row, summary)
}

fn tick_clock_now(ui: &AppWindow, c: &AppCtx) {
    let now = Local::now();
    let pattern = c.config.datetime_format.chrono_pattern();
    ui.set_clock_text(SharedString::from(now.format(pattern).to_string()));
    ui.set_zal_now_mins((now.hour() * 60 + now.minute()) as i32);
}

fn start_clock(ui_weak: Weak<AppWindow>, ctx: std::rc::Rc<std::cell::RefCell<AppCtx>>) {
    let now = Local::now();
    let secs = 60 - now.timestamp() % 60;
    let delay = Duration::from_secs(secs as u64);
    slint::Timer::single_shot(delay, move || {
        tick_clock(ui_weak, ctx);
    });
}

fn tick_clock(ui_weak: Weak<AppWindow>, ctx: std::rc::Rc<std::cell::RefCell<AppCtx>>) {
    if let Some(ui) = ui_weak.upgrade() {
        tick_clock_now(&ui, &ctx.borrow());
    }
    let ui_weak2 = ui_weak.clone();
    let ctx2 = ctx.clone();
    slint::Timer::single_shot(Duration::from_secs(60), move || {
        tick_clock(ui_weak2, ctx2);
    });
}

fn install_perf(ui: &AppWindow) {
    let mut last = Instant::now();
    ui.window()
        .set_rendering_notifier(move |state, _| {
            if matches!(state, slint::RenderingState::AfterRendering) {
                let now = Instant::now();
                let dt = now.duration_since(last);
                last = now;
                if dt > Duration::from_micros(16_700) {
                    tracing::warn!("slow frame: {:.2} ms", dt.as_secs_f64() * 1000.0);
                }
            }
        })
        .ok();
}

fn log_cb(name: &str, t0: Instant) {
    let dt = t0.elapsed();
    if dt > Duration::from_millis(2) {
        tracing::warn!(
            "slow UI callback {name}: {:.2} ms",
            dt.as_secs_f64() * 1000.0
        );
    }
}
