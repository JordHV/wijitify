use std::sync::Arc;
use std::thread;

use tokio::runtime::Runtime;
use tokio::sync::mpsc;

/// Background Tokio runtime living on its own OS thread.
pub struct TaskRuntime {
    pub handle: tokio::runtime::Handle,
    _shutdown: mpsc::Sender<()>,
}

impl TaskRuntime {
    pub fn start() -> Arc<Self> {
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);

        thread::Builder::new()
            .name("wijitify-tokio".into())
            .spawn(move || {
                let rt = Runtime::new().expect("tokio runtime");
                let handle = rt.handle().clone();
                ready_tx.send(handle).expect("send handle");
                rt.block_on(async {
                    let _ = shutdown_rx.recv().await;
                });
            })
            .expect("spawn tokio thread");

        let handle = ready_rx.recv().expect("recv handle");
        Arc::new(Self {
            handle,
            _shutdown: shutdown_tx,
        })
    }
}
