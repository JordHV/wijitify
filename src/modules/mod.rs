pub mod registry;
pub mod zalender;

use std::path::PathBuf;

use anyhow::Result;

#[derive(Debug, Clone)]
pub struct ModuleMeta {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
}

/// Narrow services exposed to a module. No access to other modules' internals.
pub struct ModuleContext {
    pub cache_dir: PathBuf,
}

pub trait Module: Send {
    fn meta(&self) -> &ModuleMeta;
    fn start(&mut self, ctx: ModuleContext) -> Result<()>;
    fn stop(&mut self);
}
