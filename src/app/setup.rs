use crate::modules::zalender::normalize_ics_url;
use crate::services::config::{Config, FeedConfig, ZalenderConfig};

#[derive(Debug, Clone)]
pub struct SetupDraft {
    pub step: i32,
    pub selected: Vec<(String, bool)>,
    pub zalender: ZalenderConfig,
    pub new_feed_name: String,
    pub new_feed_url: String,
    pub new_feed_color: String,
    pub banner_message: String,
    /// 0 muted, 1 ok, 2 accent, 3 danger
    pub banner_kind: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddFeedOutcome {
    Added,
    Err,
}

impl SetupDraft {
    pub fn from_config(cfg: &Config, module_ids: &[(&str, &str, &str)]) -> Self {
        let selected = module_ids
            .iter()
            .map(|(id, _, _)| {
                (
                    (*id).to_string(),
                    cfg.module_enabled(id) || !cfg.setup_complete,
                )
            })
            .collect();
        Self {
            step: 0,
            selected,
            zalender: cfg.zalender(),
            new_feed_name: String::new(),
            new_feed_url: String::new(),
            new_feed_color: "#3b82f6".into(),
            banner_message: String::new(),
            banner_kind: 0,
        }
    }

    pub fn toggle(&mut self, index: usize) {
        if let Some((_, sel)) = self.selected.get_mut(index) {
            *sel = !*sel;
        }
    }

    pub fn add_feed(&mut self) -> AddFeedOutcome {
        let mut name = self.new_feed_name.trim().to_string();
        let url = self.new_feed_url.trim().to_string();
        if url.is_empty() {
            self.banner_message = "Enter an ICS URL.".into();
            self.banner_kind = 3;
            return AddFeedOutcome::Err;
        }
        let lower = url.to_ascii_lowercase();
        if !(lower.starts_with("http://") || lower.starts_with("https://")) {
            self.banner_message = "URL must start with http:// or https://.".into();
            self.banner_kind = 3;
            return AddFeedOutcome::Err;
        }
        let url = match normalize_ics_url(&url) {
            Ok(u) => u,
            Err(_) => {
                self.banner_message = "URL must start with http:// or https://.".into();
                self.banner_kind = 3;
                return AddFeedOutcome::Err;
            }
        };
        if name.is_empty() {
            name = host_from_url(&url).unwrap_or_else(|| "Calendar".into());
        }
        let id = self.zalender.alloc_feed_id();
        self.zalender.feeds.push(FeedConfig {
            id,
            name: name.clone(),
            url,
            color: self.new_feed_color.clone(),
            visible: true,
            writable: false,
            local: false,
        });
        self.new_feed_name.clear();
        self.new_feed_url.clear();
        self.banner_message = format!("Added “{name}”.");
        self.banner_kind = 1;
        AddFeedOutcome::Added
    }

    pub fn remove_feed(&mut self, index: usize) {
        if let Some(f) = self.zalender.feeds.get(index) {
            if f.local {
                self.banner_message = "Local calendar cannot be removed.".into();
                self.banner_kind = 3;
                return;
            }
            let name = f.name.clone();
            self.zalender.feeds.remove(index);
            self.banner_message = format!("Removed “{name}”.");
            self.banner_kind = 1;
        }
    }

    pub fn zalender_selected(&self) -> bool {
        self.selected.iter().any(|(id, on)| id == "zalender" && *on)
    }

    pub fn apply_to(self, cfg: &mut Config) {
        for (id, on) in &self.selected {
            cfg.set_module_enabled(id, *on);
        }
        let mut z = self.zalender;
        z.enabled = self
            .selected
            .iter()
            .find(|(id, _)| id == "zalender")
            .map(|(_, on)| *on)
            .unwrap_or(true);
        cfg.set_zalender(z);
        cfg.setup_complete = true;
    }
}

fn host_from_url(url: &str) -> Option<String> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let host = rest.split('/').next()?.split('@').next_back()?;
    if host.is_empty() {
        None
    } else {
        Some(host.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_feed_defaults_name_from_host() {
        let mut d = SetupDraft {
            step: 1,
            selected: vec![("zalender".into(), true)],
            zalender: ZalenderConfig::default(),
            new_feed_name: String::new(),
            new_feed_url: "https://calendar.google.com/cal.ics".into(),
            new_feed_color: "#3b82f6".into(),
            banner_message: String::new(),
            banner_kind: 0,
        };
        assert_eq!(d.add_feed(), AddFeedOutcome::Added);
        let added = d
            .zalender
            .feeds
            .iter()
            .find(|f| !f.local)
            .expect("ics feed");
        assert_eq!(added.name, "calendar.google.com");
        assert_eq!(d.banner_kind, 1);
    }

    #[test]
    fn add_feed_rejects_empty_url() {
        let mut d = SetupDraft::from_config(&Config::default(), &[("zalender", "Z", "d")]);
        d.new_feed_name = "x".into();
        assert_eq!(d.add_feed(), AddFeedOutcome::Err);
        assert_eq!(d.banner_kind, 3);
    }
}
