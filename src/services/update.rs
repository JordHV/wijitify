//! In-app update feed + Flatpak portal install/restart.
//!
//! Production: check `latest.json` at startup, then every 24h.
//! Dev: `WIJITIFY_UPDATE_INTERVAL_SECS` and `WIJITIFY_UPDATE_FEED` override.

// Portal install/restart is Linux-only; other targets never construct those states.
#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

use std::sync::Arc;

use serde::Deserialize;
use tokio::sync::mpsc;

use super::tasks::TaskRuntime;

pub const DEFAULT_FEED_URL: &str = "https://jordhv.github.io/wijitify/latest.json";
const DEFAULT_INTERVAL_SECS: u64 = 24 * 60 * 60;

/// UI-facing update status codes (match Slint `update-status` ints).
pub mod status {
    pub const UNSUPPORTED: i32 = 0;
    pub const UP_TO_DATE: i32 = 1;
    pub const AVAILABLE: i32 = 2;
    pub const INSTALLING: i32 = 3;
    pub const READY_TO_RESTART: i32 = 4;
    pub const FAILED: i32 = 5;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateState {
    Unsupported,
    UpToDate {
        checked: String,
    },
    Available {
        version: String,
        notes: String,
        checked: String,
    },
    Installing {
        percent: u32,
    },
    ReadyToRestart,
    Failed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateCmd {
    CheckNow,
    Install,
    Restart,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LatestFeed {
    pub version: String,
    #[serde(default)]
    pub notes: String,
}

/// Parse `x.y.z` (optional leading `v`) into comparable tuple parts.
pub fn parse_version(s: &str) -> Option<(u64, u64, u64)> {
    let s = s.trim().trim_start_matches('v');
    let mut parts = s.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

pub fn is_newer(remote: &str, current: &str) -> bool {
    match (parse_version(remote), parse_version(current)) {
        (Some(r), Some(c)) => r > c,
        _ => false,
    }
}

pub fn feed_url() -> String {
    std::env::var("WIJITIFY_UPDATE_FEED").unwrap_or_else(|_| DEFAULT_FEED_URL.to_string())
}

pub fn check_interval_secs() -> u64 {
    std::env::var("WIJITIFY_UPDATE_INTERVAL_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|&n| n > 0)
        .unwrap_or(DEFAULT_INTERVAL_SECS)
}

pub fn running_as_flatpak() -> bool {
    std::env::var_os("FLATPAK_ID").is_some()
}

fn checked_now() -> String {
    chrono::Local::now().format("%H:%M").to_string()
}

/// Map feed + current version into a UI state (no Flatpak required).
pub fn state_from_feed(feed: &LatestFeed, current: &str) -> UpdateState {
    let checked = checked_now();
    if is_newer(&feed.version, current) {
        UpdateState::Available {
            version: feed.version.clone(),
            notes: feed.notes.clone(),
            checked,
        }
    } else {
        UpdateState::UpToDate { checked }
    }
}

pub fn parse_feed_json(body: &str) -> Result<LatestFeed, String> {
    serde_json::from_str(body).map_err(|e| format!("bad latest.json: {e}"))
}

/// Progress event mapping for unit tests (no D-Bus).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressKind {
    Running,
    Empty,
    Done,
    Failed,
}

pub fn state_from_progress(
    kind: ProgressKind,
    percent: Option<u32>,
    error_message: Option<&str>,
) -> UpdateState {
    match kind {
        ProgressKind::Running => UpdateState::Installing {
            percent: percent.unwrap_or(0),
        },
        ProgressKind::Done => UpdateState::ReadyToRestart,
        ProgressKind::Empty => UpdateState::UpToDate {
            checked: "Not published yet, try again shortly".into(),
        },
        ProgressKind::Failed => {
            UpdateState::Failed(error_message.unwrap_or("update failed").to_string())
        }
    }
}

pub fn start(
    runtime: Arc<TaskRuntime>,
    mut cmd_rx: mpsc::Receiver<UpdateCmd>,
    on_state: Arc<dyn Fn(UpdateState) + Send + Sync>,
) {
    let feed_override = std::env::var_os("WIJITIFY_UPDATE_FEED").is_some();
    let as_flatpak = running_as_flatpak();

    if !as_flatpak && !feed_override {
        on_state(UpdateState::Unsupported);
        return;
    }

    runtime.handle.spawn(async move {
        let current = env!("CARGO_PKG_VERSION").to_string();
        let client = match reqwest::Client::builder()
            .user_agent(concat!("wijitify/", env!("CARGO_PKG_VERSION")))
            .timeout(std::time::Duration::from_secs(20))
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                on_state(UpdateState::Failed(format!("http client: {e}")));
                return;
            }
        };

        // Initial check immediately.
        check_feed(&client, &current, &on_state).await;

        #[cfg(target_os = "linux")]
        let monitor = if as_flatpak {
            match create_monitor().await {
                Ok(m) => Some(m),
                Err(e) => {
                    tracing::warn!("flatpak update monitor: {e:#}");
                    None
                }
            }
        } else {
            None
        };

        #[cfg(not(target_os = "linux"))]
        let monitor: Option<()> = None;

        let mut interval =
            tokio::time::interval(std::time::Duration::from_secs(check_interval_secs()));
        // First tick already covered by the immediate check.
        interval.tick().await;

        #[cfg(target_os = "linux")]
        {
            use futures_util::StreamExt;
            let mut available_stream = if let Some(ref m) = monitor {
                match m.receive_update_available().await {
                    Ok(s) => Some(Box::pin(s)),
                    Err(e) => {
                        tracing::warn!("update available stream: {e:#}");
                        None
                    }
                }
            } else {
                None
            };

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        check_feed(&client, &current, &on_state).await;
                    }
                    cmd = cmd_rx.recv() => {
                        match cmd {
                            None => break,
                            Some(UpdateCmd::CheckNow) => {
                                check_feed(&client, &current, &on_state).await;
                            }
                            Some(UpdateCmd::Install) => {
                                if let Some(ref m) = monitor {
                                    run_install(m, &on_state).await;
                                } else {
                                    on_state(UpdateState::Failed(
                                        "not running as a Flatpak".into(),
                                    ));
                                }
                            }
                            Some(UpdateCmd::Restart) => {
                                if as_flatpak {
                                    run_restart(&on_state).await;
                                } else {
                                    on_state(UpdateState::Failed(
                                        "not running as a Flatpak".into(),
                                    ));
                                }
                            }
                        }
                    }
                    maybe_info = async {
                        match available_stream.as_mut() {
                            Some(s) => s.next().await,
                            None => std::future::pending().await,
                        }
                    } => {
                        if maybe_info.is_some() {
                            check_feed(&client, &current, &on_state).await;
                        }
                    }
                }
            }
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = &monitor;
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        check_feed(&client, &current, &on_state).await;
                    }
                    cmd = cmd_rx.recv() => {
                        match cmd {
                            None => break,
                            Some(UpdateCmd::CheckNow) => {
                                check_feed(&client, &current, &on_state).await;
                            }
                            Some(UpdateCmd::Install) | Some(UpdateCmd::Restart) => {
                                on_state(UpdateState::Failed(
                                    "not running as a Flatpak".into(),
                                ));
                            }
                        }
                    }
                }
            }
        }
    });
}

async fn check_feed(
    client: &reqwest::Client,
    current: &str,
    on_state: &Arc<dyn Fn(UpdateState) + Send + Sync>,
) {
    let url = feed_url();
    match client.get(&url).send().await {
        Ok(resp) => match resp.error_for_status() {
            Ok(ok) => match ok.text().await {
                Ok(body) => match parse_feed_json(&body) {
                    Ok(feed) => on_state(state_from_feed(&feed, current)),
                    Err(e) => tracing::warn!("update feed: {e}"),
                },
                Err(e) => tracing::warn!("update feed body: {e}"),
            },
            Err(e) => tracing::warn!("update feed status: {e}"),
        },
        Err(e) => tracing::warn!("update feed request: {e}"),
    }
}

#[cfg(target_os = "linux")]
async fn create_monitor() -> anyhow::Result<ashpd::flatpak::UpdateMonitor<'static>> {
    let proxy = ashpd::flatpak::Flatpak::new().await?;
    Ok(proxy.create_update_monitor().await?)
}

#[cfg(target_os = "linux")]
async fn run_install(
    monitor: &ashpd::flatpak::UpdateMonitor<'_>,
    on_state: &Arc<dyn Fn(UpdateState) + Send + Sync>,
) {
    use ashpd::flatpak::UpdateStatus;
    use futures_util::StreamExt;

    // Subscribe before starting so early progress signals are not missed.
    let progress = match monitor.receive_progress().await {
        Ok(s) => s,
        Err(e) => {
            on_state(UpdateState::Failed(format!("progress stream: {e:#}")));
            return;
        }
    };
    let mut progress = Box::pin(progress);

    on_state(UpdateState::Installing { percent: 0 });
    if let Err(e) = monitor.update(None).await {
        on_state(UpdateState::Failed(format!("start update: {e:#}")));
        return;
    }

    while let Some(p) = progress.next().await {
        let kind = match p.status {
            Some(UpdateStatus::Running) | None => ProgressKind::Running,
            Some(UpdateStatus::Empty) => ProgressKind::Empty,
            Some(UpdateStatus::Done) => ProgressKind::Done,
            Some(UpdateStatus::Failed) => ProgressKind::Failed,
        };
        let state = state_from_progress(kind, p.progress, p.error_message.as_deref());
        on_state(state);
        if matches!(
            kind,
            ProgressKind::Done | ProgressKind::Empty | ProgressKind::Failed
        ) {
            break;
        }
    }
}

#[cfg(target_os = "linux")]
async fn run_restart(on_state: &Arc<dyn Fn(UpdateState) + Send + Sync>) {
    use ashpd::flatpak::{Flatpak, SpawnFlags, SpawnOptions};
    use std::collections::HashMap;

    let proxy = match Flatpak::new().await {
        Ok(p) => p,
        Err(e) => {
            on_state(UpdateState::Failed(format!("flatpak portal: {e:#}")));
            return;
        }
    };

    let argv: Vec<std::ffi::OsString> = std::iter::once("wijitify".into())
        .chain(std::env::args_os().skip(1))
        .collect();

    let fds: HashMap<u32, std::os::fd::OwnedFd> = HashMap::new();
    match proxy
        .spawn(
            "/",
            argv.as_slice(),
            fds,
            HashMap::new(),
            SpawnFlags::LatestVersion.into(),
            SpawnOptions::default(),
        )
        .await
    {
        Ok(_pid) => {
            let _ = slint::invoke_from_event_loop(|| {
                let _ = slint::quit_event_loop();
            });
        }
        Err(e) => {
            on_state(UpdateState::Failed(format!("restart: {e:#}")));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_versions() {
        assert_eq!(parse_version("0.2.0"), Some((0, 2, 0)));
        assert_eq!(parse_version("v1.0.10"), Some((1, 0, 10)));
        assert!(parse_version("1.0").is_none());
        assert!(parse_version("nope").is_none());
    }

    #[test]
    fn newer_compare() {
        assert!(is_newer("0.2.0", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.2.0"));
        assert!(!is_newer("0.1.0", "0.1.0"));
    }

    #[test]
    fn feed_parse_ok() {
        let f = parse_feed_json(r#"{"version":"0.2.1","notes":"fix","published":"2026-10-20"}"#)
            .unwrap();
        assert_eq!(f.version, "0.2.1");
        assert_eq!(f.notes, "fix");
    }

    #[test]
    fn feed_parse_bad() {
        assert!(parse_feed_json("{").is_err());
        assert!(parse_feed_json(r#"{"notes":"x"}"#).is_err());
    }

    #[test]
    fn state_from_feed_available() {
        let feed = LatestFeed {
            version: "9.9.9".into(),
            notes: "n".into(),
        };
        match state_from_feed(&feed, "0.1.0") {
            UpdateState::Available { version, notes, .. } => {
                assert_eq!(version, "9.9.9");
                assert_eq!(notes, "n");
            }
            other => panic!("expected Available, got {other:?}"),
        }
    }

    #[test]
    fn progress_mapping() {
        assert!(matches!(
            state_from_progress(ProgressKind::Running, Some(40), None),
            UpdateState::Installing { percent: 40 }
        ));
        assert!(matches!(
            state_from_progress(ProgressKind::Done, None, None),
            UpdateState::ReadyToRestart
        ));
        assert!(matches!(
            state_from_progress(ProgressKind::Empty, None, None),
            UpdateState::UpToDate { .. }
        ));
        assert!(matches!(
            state_from_progress(ProgressKind::Failed, None, Some("boom")),
            UpdateState::Failed(s) if s == "boom"
        ));
    }
}
