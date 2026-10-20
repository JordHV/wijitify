use crate::services::config::{Config, FeedConfig, ZalenderConfig};

#[derive(Debug, Clone)]
pub struct SetupDraft {
    pub step: i32,
    pub selected: Vec<(String, bool)>,
    pub zalender: ZalenderConfig,
    pub new_feed_name: String,
    pub new_feed_url: String,
    pub new_feed_color: String,
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
        }
    }

    pub fn toggle(&mut self, index: usize) {
        if let Some((_, sel)) = self.selected.get_mut(index) {
            *sel = !*sel;
        }
    }

    pub fn add_feed(&mut self) {
        let name = self.new_feed_name.trim().to_string();
        let url = self.new_feed_url.trim().to_string();
        if name.is_empty() || url.is_empty() {
            return;
        }
        self.zalender.feeds.push(FeedConfig {
            name,
            url,
            color: self.new_feed_color.clone(),
        });
        self.new_feed_name.clear();
        self.new_feed_url.clear();
    }

    pub fn remove_feed(&mut self, index: usize) {
        if index < self.zalender.feeds.len() {
            self.zalender.feeds.remove(index);
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
