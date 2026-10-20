use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{Datelike, Local, Months, NaiveDate};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel, Weak};

use crate::app::setup::SetupDraft;
use crate::app::shell::{Screen, ShellState};
use crate::modules::zalender::{self, model as zal_model, CalendarSnapshot};
use crate::modules::{registry, Module, ModuleContext};
use crate::services::config::Config;
use crate::services::storage::Paths;
use crate::services::tasks::TaskRuntime;
use crate::services::update::{self, UpdateCmd, UpdateState};

// Generated code is not ours to fix; keep the `unwrap_used` deny for hand-written code.
#[allow(clippy::unwrap_used)]
mod generated {
    slint::include_modules!();
}
pub use generated::*;

pub struct AppCtx {
    pub paths: Paths,
    pub config: Config,
    pub runtime: Arc<TaskRuntime>,
    pub shell: ShellState,
    pub setup: SetupDraft,
    pub snapshot: Option<CalendarSnapshot>,
    pub view_year: i32,
    pub view_month: u32,
    pub selected_day: NaiveDate,
    pub refreshing: bool,
    pub status: String,
    pub perf_enabled: bool,
    pub refresh_gen: u64,
    pub update_tx: Option<tokio::sync::mpsc::Sender<UpdateCmd>>,
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
        };
        Self {
            paths,
            config,
            runtime,
            shell,
            setup,
            snapshot: None,
            view_year: today.year(),
            view_month: today.month(),
            selected_day: today,
            refreshing: false,
            status: String::new(),
            perf_enabled: std::env::var_os("WIJITIFY_PERF").is_some(),
            refresh_gen: 0,
            update_tx: None,
            modules: Vec::new(),
        }
    }

    /// Restart modules so the running set matches the enabled config.
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
}

pub fn bind(ui: &AppWindow, ctx: std::rc::Rc<std::cell::RefCell<AppCtx>>) {
    start_updater(ui, &ctx);
    apply_all(ui, &ctx.borrow());

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
            ctx.borrow_mut().setup.add_feed();
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
            {
                let mut c = ctx.borrow_mut();
                if let Some(ui) = ui_weak.upgrade() {
                    c.setup.zalender.timezone = ui.get_zalender_timezone().to_string();
                }
                finish_setup_inner(&mut c);
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_all(&ui, &ctx.borrow());
                maybe_start_refresh(&ui, &ctx);
            }
            log_cb("setup_finish", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_open_settings(move || {
            let t0 = Instant::now();
            ctx.borrow_mut().shell.screen = Screen::Settings;
            if let Some(ui) = ui_weak.upgrade() {
                apply_shell(&ui, &ctx.borrow());
            }
            log_cb("open_settings", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_close_settings(move || {
            let t0 = Instant::now();
            ctx.borrow_mut().shell.screen = Screen::Home;
            if let Some(ui) = ui_weak.upgrade() {
                apply_shell(&ui, &ctx.borrow());
            }
            log_cb("close_settings", t0);
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
                    let _ = c.config.save(&c.paths);
                    if m.id == "zalender" {
                        c.shell.active_module = if en { "zalender".into() } else { String::new() };
                    }
                    c.sync_modules();
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
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_zal_prev_month(move || {
            let t0 = Instant::now();
            {
                let mut c = ctx.borrow_mut();
                let d = NaiveDate::from_ymd_opt(c.view_year, c.view_month, 1)
                    .unwrap_or(c.selected_day)
                    .checked_sub_months(Months::new(1))
                    .unwrap_or(c.selected_day);
                c.view_year = d.year();
                c.view_month = d.month();
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_zalender(&ui, &ctx.borrow());
            }
            log_cb("zal_prev_month", t0);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let ctx = ctx.clone();
        ui.on_zal_next_month(move || {
            let t0 = Instant::now();
            {
                let mut c = ctx.borrow_mut();
                let d = NaiveDate::from_ymd_opt(c.view_year, c.view_month, 1)
                    .unwrap_or(c.selected_day)
                    .checked_add_months(Months::new(1))
                    .unwrap_or(c.selected_day);
                c.view_year = d.year();
                c.view_month = d.month();
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_zalender(&ui, &ctx.borrow());
            }
            log_cb("zal_next_month", t0);
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
                c.view_year = today.year();
                c.view_month = today.month();
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
        ui.on_zal_select_day(move |i| {
            let t0 = Instant::now();
            {
                let mut c = ctx.borrow_mut();
                let today = Local::now().date_naive();
                let cells = zal_model::month_grid(c.view_year, c.view_month, today);
                if let Some((d, in_month, _)) = cells.get(i as usize) {
                    if *in_month {
                        c.selected_day = *d;
                    }
                }
            }
            if let Some(ui) = ui_weak.upgrade() {
                apply_zalender(&ui, &ctx.borrow());
            }
            log_cb("zal_select_day", t0);
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

    // Load cache immediately (sync small read is OK at startup before event loop stress).
    {
        let mut c = ctx.borrow_mut();
        if let Ok(dir) = c.paths.module_cache("zalender") {
            if let Ok(Some(snap)) = zalender::load_from_cache(&dir) {
                c.status = format!(
                    "Last updated {}",
                    snap.generated_at.format("%Y-%m-%d %H:%M UTC")
                );
                c.snapshot = Some(snap);
            }
        }
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
    let metas = registry::all_metas();
    let ids: Vec<(&str, &str, &str)> = metas
        .iter()
        .map(|m| (m.id, m.name, m.description))
        .collect();
    c.setup = SetupDraft::from_config(&c.config, &ids);
    c.sync_modules();
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
            apply_zalender(ui, &c);
            return;
        }
        c.refresh_gen += 1;
        let gen = c.refresh_gen;
        c.refreshing = true;
        c.status = "Refreshing…".into();
        let cache_dir = match c.paths.module_cache("zalender") {
            Ok(d) => d,
            Err(e) => {
                c.refreshing = false;
                c.status = format!("cache error: {e:#}");
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
    // Network work runs on Tokio; the result is awaited on the UI thread so `ctx` (!Send) stays there.
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
                c.status = if offline {
                    format!(
                        "Offline — last updated {}",
                        snap.generated_at.format("%Y-%m-%d %H:%M UTC")
                    )
                } else {
                    format!("Updated {}", snap.generated_at.format("%Y-%m-%d %H:%M UTC"))
                };
                c.snapshot = Some(snap);
            }
            Err(e) => {
                c.status = format!("Refresh failed: {e:#}");
            }
        }
        if let Some(ui) = ui_weak.upgrade() {
            apply_zalender(&ui, &c);
        }
    });
}

fn apply_all(ui: &AppWindow, c: &AppCtx) {
    apply_shell(ui, c);
    apply_setup(ui, c);
    apply_settings(ui, c);
    apply_zalender(ui, c);
}

fn apply_shell(ui: &AppWindow, c: &AppCtx) {
    ui.set_screen(c.shell.screen.as_i32());
    ui.set_active_module(SharedString::from(c.shell.active_module.as_str()));
    ui.set_clock_text(SharedString::from(
        Local::now().format("%a %b %e  %H:%M").to_string(),
    ));
    if c.perf_enabled {
        ui.set_perf_overlay(SharedString::from("WIJITIFY_PERF=1"));
    }
}

fn apply_setup(ui: &AppWindow, c: &AppCtx) {
    ui.set_setup_step(c.setup.step);
    ui.set_zalender_timezone(SharedString::from(c.setup.zalender.timezone.as_str()));
    ui.set_new_feed_name(SharedString::from(c.setup.new_feed_name.as_str()));
    ui.set_new_feed_url(SharedString::from(c.setup.new_feed_url.as_str()));
    ui.set_new_feed_color(SharedString::from(c.setup.new_feed_color.as_str()));
    ui.set_show_zalender_setup(c.setup.zalender_selected());

    let metas = registry::all_metas();
    let rows: Vec<SetupModuleRow> = metas
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let selected = c.setup.selected.get(i).map(|(_, s)| *s).unwrap_or(false);
            SetupModuleRow {
                id: SharedString::from(m.id),
                name: SharedString::from(m.name),
                description: SharedString::from(m.description),
                selected,
            }
        })
        .collect();
    ui.set_setup_modules(ModelRc::new(VecModel::from(rows)));

    let feeds: Vec<FeedRow> = c
        .setup
        .zalender
        .feeds
        .iter()
        .map(|f| FeedRow {
            name: SharedString::from(f.name.as_str()),
            url: SharedString::from(f.url.as_str()),
            color: SharedString::from(f.color.as_str()),
        })
        .collect();
    ui.set_zalender_feeds(ModelRc::new(VecModel::from(feeds)));
}

fn apply_settings(ui: &AppWindow, c: &AppCtx) {
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
}

fn apply_update(ui: &AppWindow, state: &UpdateState) {
    let label = if matches!(state, UpdateState::Available { .. }) {
        "Settings •"
    } else {
        "Settings"
    };
    ui.set_settings_label(SharedString::from(label));
    match state {
        UpdateState::Unsupported => {
            ui.set_update_status(update::status::UNSUPPORTED);
            ui.set_update_version(SharedString::new());
            ui.set_update_notes(SharedString::new());
            ui.set_update_percent(0);
            ui.set_update_error(SharedString::new());
            ui.set_update_checked(SharedString::new());
        }
        UpdateState::UpToDate { checked } => {
            ui.set_update_status(update::status::UP_TO_DATE);
            ui.set_update_version(SharedString::new());
            ui.set_update_notes(SharedString::new());
            ui.set_update_percent(0);
            ui.set_update_error(SharedString::new());
            ui.set_update_checked(SharedString::from(checked.as_str()));
        }
        UpdateState::Available {
            version,
            notes,
            checked,
        } => {
            ui.set_update_status(update::status::AVAILABLE);
            ui.set_update_version(SharedString::from(version.as_str()));
            ui.set_update_notes(SharedString::from(notes.as_str()));
            ui.set_update_percent(0);
            ui.set_update_error(SharedString::new());
            ui.set_update_checked(SharedString::from(checked.as_str()));
        }
        UpdateState::Installing { percent } => {
            ui.set_update_status(update::status::INSTALLING);
            ui.set_update_percent(*percent as i32);
            ui.set_update_error(SharedString::new());
        }
        UpdateState::ReadyToRestart => {
            ui.set_update_status(update::status::READY_TO_RESTART);
            ui.set_update_error(SharedString::new());
        }
        UpdateState::Failed(msg) => {
            ui.set_update_status(update::status::FAILED);
            ui.set_update_error(SharedString::from(msg.as_str()));
        }
    }
}

fn send_update_cmd(ctx: &std::rc::Rc<std::cell::RefCell<AppCtx>>, cmd: UpdateCmd) {
    if let Some(tx) = ctx.borrow().update_tx.as_ref() {
        let _ = tx.try_send(cmd);
    }
}

fn start_updater(ui: &AppWindow, ctx: &std::rc::Rc<std::cell::RefCell<AppCtx>>) {
    let (tx, rx) = tokio::sync::mpsc::channel::<UpdateCmd>(8);
    let runtime = ctx.borrow().runtime.clone();
    ctx.borrow_mut().update_tx = Some(tx);

    ui.set_current_version(SharedString::from(env!("CARGO_PKG_VERSION")));
    let ui_weak = ui.as_weak();
    let on_state: std::sync::Arc<dyn Fn(UpdateState) + Send + Sync> =
        std::sync::Arc::new(move |state| {
            let _ = ui_weak.upgrade_in_event_loop(move |ui| apply_update(&ui, &state));
        });

    update::start(runtime, rx, on_state);
}

fn apply_zalender(ui: &AppWindow, c: &AppCtx) {
    let today = Local::now().date_naive();
    let title = NaiveDate::from_ymd_opt(c.view_year, c.view_month, 1)
        .unwrap_or(today)
        .format("%B %Y")
        .to_string();
    ui.set_zal_title_month(SharedString::from(title));
    ui.set_zal_status(SharedString::from(c.status.as_str()));
    ui.set_zal_refreshing(c.refreshing);
    let feeds_empty = c.config.zalender().feeds.is_empty();
    ui.set_zal_empty_feeds(feeds_empty);

    let cells = zal_model::month_grid(c.view_year, c.view_month, today);
    let days: Vec<DayCell> = cells
        .iter()
        .map(|(d, in_month, is_today)| {
            let has = c
                .snapshot
                .as_ref()
                .map(|s| !s.events_on(*d).is_empty())
                .unwrap_or(false);
            DayCell {
                day: d.day() as i32,
                in_month: *in_month,
                is_today: *is_today,
                has_events: has,
                label: SharedString::from(d.day().to_string()),
            }
        })
        .collect();
    ui.set_zal_days(ModelRc::new(VecModel::from(days)));

    ui.set_zal_agenda_date(SharedString::from(
        c.selected_day.format("%A, %B %e").to_string(),
    ));
    let agenda: Vec<AgendaItem> = c
        .snapshot
        .as_ref()
        .map(|s| {
            s.events_on(c.selected_day)
                .into_iter()
                .map(|e| {
                    let time = if e.all_day {
                        "All day".into()
                    } else {
                        e.start.with_timezone(&Local).format("%H:%M").to_string()
                    };
                    AgendaItem {
                        time: SharedString::from(time),
                        title: SharedString::from(e.title.as_str()),
                        color: SharedString::from(e.color.as_str()),
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    ui.set_zal_agenda(ModelRc::new(VecModel::from(agenda)));
}

fn start_clock(ui_weak: Weak<AppWindow>, ctx: std::rc::Rc<std::cell::RefCell<AppCtx>>) {
    // Align to next minute boundary — no per-frame timer.
    let now = Local::now();
    let secs = 60 - now.timestamp() % 60;
    let delay = Duration::from_secs(secs as u64);
    slint::Timer::single_shot(delay, move || {
        tick_clock(ui_weak, ctx);
    });
}

fn tick_clock(ui_weak: Weak<AppWindow>, ctx: std::rc::Rc<std::cell::RefCell<AppCtx>>) {
    if let Some(ui) = ui_weak.upgrade() {
        ui.set_clock_text(SharedString::from(
            Local::now().format("%a %b %e  %H:%M").to_string(),
        ));
        let _ = &ctx;
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
