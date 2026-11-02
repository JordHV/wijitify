//! Local (non-ICS) events for the built-in writable calendar.

use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::services::config::LOCAL_FEED_ID;
use crate::services::storage::{self, write_atomic};

use super::model::CalEvent;

pub const SCHEMA: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LocalEventsFile {
    schema_version: u32,
    events: Vec<CalEvent>,
}

#[derive(Debug, Clone, Default)]
pub struct LocalEventStore {
    events: Vec<CalEvent>,
    path: PathBuf,
    dirty: bool,
}

impl LocalEventStore {
    pub fn empty(config_dir: &Path) -> Self {
        Self {
            events: Vec::new(),
            path: config_dir.join("local_events.json"),
            dirty: false,
        }
    }

    pub fn load(config_dir: &Path) -> Result<Self> {
        let path = config_dir.join("local_events.json");
        let events = match storage::read_optional(&path)? {
            None => Vec::new(),
            Some(bytes) => match serde_json::from_slice::<LocalEventsFile>(&bytes) {
                Ok(f) if f.schema_version == SCHEMA => f.events,
                Ok(_) => {
                    tracing::warn!("discarding old local_events schema");
                    Vec::new()
                }
                Err(e) => {
                    tracing::warn!("corrupt local_events discarded: {e:#}");
                    Vec::new()
                }
            },
        };
        Ok(Self {
            events,
            path,
            dirty: false,
        })
    }

    pub fn events(&self) -> &[CalEvent] {
        &self.events
    }

    pub fn add(&mut self, ev: CalEvent) {
        self.events.push(ev);
        self.events.sort_by_key(|e| e.start);
        self.dirty = true;
    }

    /// Clear dirty and return a snapshot for a background write. Call from the UI
    /// thread; write with [`write_events`] on Tokio.
    pub fn take_save_snapshot(&mut self) -> Option<(PathBuf, Vec<CalEvent>)> {
        if !self.dirty {
            return None;
        }
        self.dirty = false;
        Some((self.path.clone(), self.events.clone()))
    }

    pub fn write_events(path: &Path, events: &[CalEvent]) -> Result<()> {
        let file = LocalEventsFile {
            schema_version: SCHEMA,
            events: events.to_vec(),
        };
        let bytes = serde_json::to_vec_pretty(&file)?;
        write_atomic(path, &bytes)
    }
}

pub fn new_local_event(
    title: String,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    all_day: bool,
    feed_id: &str,
    feed_name: &str,
    color: &str,
) -> CalEvent {
    let uid = format!("local-{}", local_uid());
    let feed_id = if feed_id.is_empty() {
        LOCAL_FEED_ID
    } else {
        feed_id
    };
    CalEvent {
        uid,
        recurrence_id: None,
        title,
        start,
        end,
        all_day,
        feed_id: feed_id.into(),
        feed_name: feed_name.into(),
        color: color.into(),
        description: String::new(),
        location: String::new(),
        organizer: String::new(),
        attendees: Vec::new(),
        rrule: None,
    }
}

/// Tiny unique-ish id without pulling in the uuid crate.
fn local_uid() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{t:x}")
}
