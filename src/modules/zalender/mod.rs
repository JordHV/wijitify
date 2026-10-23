pub mod local_events;
pub mod model;
pub mod overrides;
mod recur;
mod source;
pub mod view;

pub use model::CalendarSnapshot;
pub use source::{load_from_cache, normalize_ics_url, refresh_feeds};

use anyhow::Result;

use crate::modules::{Module, ModuleContext, ModuleMeta};

pub struct ZalenderModule;

impl ZalenderModule {
    pub fn meta_static() -> &'static ModuleMeta {
        static META: ModuleMeta = ModuleMeta {
            id: "zalender",
            name: "Zalender",
            description: "Family calendar from ICS/iCal feeds, offline-friendly.",
        };
        &META
    }
}

impl Module for ZalenderModule {
    fn meta(&self) -> &ModuleMeta {
        Self::meta_static()
    }

    fn start(&mut self, ctx: ModuleContext) -> Result<()> {
        tracing::info!(cache = %ctx.cache_dir.display(), "zalender started");
        Ok(())
    }

    fn stop(&mut self) {
        tracing::info!("zalender stopped");
    }
}
