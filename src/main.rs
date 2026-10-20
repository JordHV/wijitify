mod app;
mod modules;
mod services;

use std::cell::RefCell;
use std::rc::Rc;

use anyhow::Context;
use slint::ComponentHandle;

use app::bridge::{self, AppWindow};
use services::config::Config;
use services::storage::Paths;
use services::tasks::TaskRuntime;

fn main() -> anyhow::Result<()> {
    services::logging::init();

    let windowed = std::env::args().any(|a| a == "--windowed");

    let paths = Paths::discover().context("resolve paths")?;
    let config = Config::load(&paths).context("load config")?;
    let runtime = TaskRuntime::start();

    let ui = AppWindow::new().context("create window")?;
    if !windowed {
        ui.window().set_fullscreen(true);
    }

    let ctx = Rc::new(RefCell::new(bridge::AppCtx::new(paths, config, runtime)));
    bridge::bind(&ui, ctx.clone());

    let ran = ui.run().context("run event loop");
    ctx.borrow_mut().stop_modules();
    ran
}
