use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::services::storage::{self, write_atomic};

use super::model::CalEvent;

pub const SCHEMA: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OverrideKey {
    pub feed_id: String,
    pub uid: String,
    pub recurrence_id: Option<String>,
}

impl OverrideKey {
    pub fn series(feed_id: impl Into<String>, uid: impl Into<String>) -> Self {
        Self {
            feed_id: feed_id.into(),
            uid: uid.into(),
            recurrence_id: None,
        }
    }

    pub fn instance(
        feed_id: impl Into<String>,
        uid: impl Into<String>,
        recurrence_id: impl Into<String>,
    ) -> Self {
        Self {
            feed_id: feed_id.into(),
            uid: uid.into(),
            recurrence_id: Some(recurrence_id.into()),
        }
    }

    pub fn from_event(ev: &CalEvent, occurrence: bool) -> Self {
        if occurrence {
            if let Some(rid) = &ev.recurrence_id {
                return Self::instance(&ev.feed_id, &ev.uid, rid);
            }
        }
        Self::series(&ev.feed_id, &ev.uid)
    }

    fn to_stored(&self) -> StoredKey {
        StoredKey {
            feed_id: self.feed_id.clone(),
            uid: self.uid.clone(),
            recurrence_id: self.recurrence_id.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct StoredKey {
    feed_id: String,
    uid: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    recurrence_id: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EventOverride {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub fields: HashMap<String, String>,
}

impl EventOverride {
    pub fn is_empty(&self) -> bool {
        self.color.is_none() && self.icon.is_none() && self.fields.is_empty()
    }
}

#[derive(Debug, Clone, Default)]
pub struct ResolvedStyle {
    pub color: String,
    pub icon: String,
    pub fields: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OverridesFile {
    schema_version: u32,
    entries: Vec<OverridesEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OverridesEntry {
    key: StoredKey,
    #[serde(flatten)]
    value: EventOverride,
}

#[derive(Debug, Clone, Default)]
pub struct OverrideStore {
    map: HashMap<OverrideKey, EventOverride>,
}

impl OverrideStore {
    pub fn load(path: &Path) -> Result<Self> {
        let Some(bytes) = storage::read_optional(path)? else {
            return Ok(Self::default());
        };
        match parse_file(&bytes) {
            Ok(store) => Ok(store),
            Err(e) => {
                tracing::warn!("corrupt overrides.json: {e:#}");
                quarantine(path);
                Ok(Self::default())
            }
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let file = OverridesFile {
            schema_version: SCHEMA,
            entries: self
                .map
                .iter()
                .filter(|(_, v)| !v.is_empty())
                .map(|(k, v)| OverridesEntry {
                    key: k.to_stored(),
                    value: v.clone(),
                })
                .collect(),
        };
        let bytes = serde_json::to_vec_pretty(&file).context("serialize overrides")?;
        write_atomic(path, &bytes)
    }

    #[allow(dead_code)]
    pub fn get(&self, key: &OverrideKey) -> Option<&EventOverride> {
        self.map.get(key)
    }

    pub fn set(&mut self, key: OverrideKey, value: EventOverride) {
        if value.is_empty() {
            self.map.remove(&key);
        } else {
            self.map.insert(key, value);
        }
    }

    pub fn resolve(&self, ev: &CalEvent) -> ResolvedStyle {
        self.resolve_over(ev, &ev.color)
    }

    /// Like `resolve`, but start from the calendar's current color instead of the color stored on the event.
    pub fn resolve_over(&self, ev: &CalEvent, base_color: &str) -> ResolvedStyle {
        let mut out = ResolvedStyle {
            color: base_color.to_string(),
            icon: String::new(),
            fields: HashMap::new(),
        };
        let series = OverrideKey::series(&ev.feed_id, &ev.uid);
        if let Some(o) = self.map.get(&series) {
            apply_overlay(&mut out, o);
        }
        if let Some(rid) = &ev.recurrence_id {
            let inst = OverrideKey::instance(&ev.feed_id, &ev.uid, rid);
            if let Some(o) = self.map.get(&inst) {
                apply_overlay(&mut out, o);
            }
        }
        out
    }
}

fn apply_overlay(out: &mut ResolvedStyle, o: &EventOverride) {
    if let Some(c) = &o.color {
        out.color = c.clone();
    }
    if let Some(i) = &o.icon {
        out.icon = i.clone();
    }
    for (k, v) in &o.fields {
        out.fields.insert(k.clone(), v.clone());
    }
}

fn parse_file(bytes: &[u8]) -> Result<OverrideStore> {
    let file: OverridesFile = serde_json::from_slice(bytes).context("parse overrides.json")?;
    if file.schema_version != SCHEMA {
        anyhow::bail!("unsupported overrides schema {}", file.schema_version);
    }
    let mut map = HashMap::new();
    for e in file.entries {
        let key = OverrideKey {
            feed_id: e.key.feed_id,
            uid: e.key.uid,
            recurrence_id: e.key.recurrence_id,
        };
        if !e.value.is_empty() {
            map.insert(key, e.value);
        }
    }
    Ok(OverrideStore { map })
}

fn quarantine(path: &Path) {
    let bad = PathBuf::from(format!("{}.bad", path.display()));
    if let Err(e) = std::fs::rename(path, &bad) {
        tracing::warn!("could not quarantine overrides: {e}");
    }
}

pub fn overrides_path(config_dir: &Path) -> PathBuf {
    config_dir.join("overrides.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn sample_event() -> CalEvent {
        CalEvent {
            uid: "uid-1".into(),
            recurrence_id: Some("20261015T090000Z".into()),
            title: "Meet".into(),
            start: Utc::now(),
            end: Utc::now(),
            all_day: false,
            feed_id: "feed-1".into(),
            feed_name: "Work".into(),
            color: "#3b82f6".into(),
            description: String::new(),
        }
    }

    #[test]
    fn resolve_series_then_instance() {
        let mut store = OverrideStore::default();
        let ev = sample_event();
        store.set(
            OverrideKey::series("feed-1", "uid-1"),
            EventOverride {
                color: Some("#111111".into()),
                icon: Some("star".into()),
                fields: HashMap::from([("field-1".into(), "a".into())]),
            },
        );
        store.set(
            OverrideKey::instance("feed-1", "uid-1", "20261015T090000Z"),
            EventOverride {
                color: None,
                icon: Some("flag".into()),
                fields: HashMap::new(),
            },
        );
        let r = store.resolve(&ev);
        assert_eq!(r.color, "#111111");
        assert_eq!(r.icon, "flag");
        assert_eq!(r.fields.get("field-1").map(String::as_str), Some("a"));
    }

    #[test]
    fn roundtrip_json() {
        let dir = tempfile_dir();
        let path = overrides_path(&dir);
        let mut store = OverrideStore::default();
        store.set(
            OverrideKey::series("feed-1", "uid-1"),
            EventOverride {
                color: Some("#ff0000".into()),
                icon: None,
                fields: HashMap::new(),
            },
        );
        store.save(&path).unwrap();
        let loaded = OverrideStore::load(&path).unwrap();
        assert_eq!(
            loaded.get(&OverrideKey::series("feed-1", "uid-1")),
            Some(&EventOverride {
                color: Some("#ff0000".into()),
                icon: None,
                fields: HashMap::new(),
            })
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn corrupt_quarantined() {
        let dir = tempfile_dir();
        let path = overrides_path(&dir);
        std::fs::write(&path, b"not json").unwrap();
        let store = OverrideStore::load(&path).unwrap();
        assert!(store.map.is_empty());
        assert!(dir.join("overrides.json.bad").is_file() || !path.exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    fn tempfile_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "wijitify-ovr-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}
