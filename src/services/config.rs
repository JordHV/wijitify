use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::storage::{self, Paths};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub schema_version: u32,
    pub setup_complete: bool,
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
    #[serde(default)]
    pub feeds: Vec<FeedConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedConfig {
    pub name: String,
    pub url: String,
    #[serde(default = "default_color")]
    pub color: String,
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

impl Default for Config {
    fn default() -> Self {
        let mut modules = BTreeMap::new();
        modules.insert(
            "zalender".into(),
            ModuleConfig::Zalender(ZalenderConfig {
                enabled: true,
                timezone: default_tz(),
                refresh_minutes: default_refresh(),
                feeds: Vec::new(),
            }),
        );
        Self {
            schema_version: SCHEMA_VERSION,
            setup_complete: false,
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
            feeds: Vec::new(),
        }
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
        cfg.schema_version += 1;
    }
    if !cfg.modules.contains_key("zalender") {
        cfg.set_zalender(ZalenderConfig::default());
    }
    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_default() {
        let cfg = Config::default();
        let text = toml::to_string_pretty(&cfg).unwrap();
        let back: Config = toml::from_str(&text).unwrap();
        assert!(!back.setup_complete);
        assert!(back.module_enabled("zalender"));
    }

    #[test]
    fn migrate_missing_zalender() {
        let mut cfg = Config {
            schema_version: 1,
            setup_complete: true,
            modules: BTreeMap::new(),
        };
        cfg = migrate(cfg).unwrap();
        assert!(cfg.modules.contains_key("zalender"));
    }
}
