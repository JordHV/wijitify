use super::zalender::ZalenderModule;
use super::{Module, ModuleMeta};

pub fn all_metas() -> Vec<ModuleMeta> {
    vec![ZalenderModule::meta_static().clone()]
}

pub fn create_enabled(enabled_ids: &[String]) -> Vec<Box<dyn Module>> {
    let mut out: Vec<Box<dyn Module>> = Vec::new();
    if enabled_ids.iter().any(|id| id == "zalender") {
        out.push(Box::new(ZalenderModule));
    }
    out
}
