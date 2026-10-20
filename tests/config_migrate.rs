use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Config {
    schema_version: u32,
    setup_complete: bool,
    #[serde(default)]
    modules: BTreeMap<String, toml::Value>,
}

#[test]
fn default_toml_roundtrip() {
    let text = r#"
schema_version = 1
setup_complete = false

[modules.zalender]
enabled = true
timezone = "America/New_York"
refresh_minutes = 15
feeds = []
"#;
    let cfg: Config = toml::from_str(text).unwrap();
    assert_eq!(cfg.schema_version, 1);
    assert!(!cfg.setup_complete);
    assert!(cfg.modules.contains_key("zalender"));
}
