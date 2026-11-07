// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! App-owned bounded activation worker and startup path resolution.

use std::io;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{SyncSender, TrySendError},
};
use std::thread::JoinHandle;
use tallyline_adapter_files::api::StorageLayout;
use tallyline_adapter_platform::api::{ActivationListener, InstancePaths};
use tallyline_ui::api::model::WindowCommand;

const LISTENER_POLL: std::time::Duration = std::time::Duration::from_millis(10);

pub(crate) fn paths(layout: &StorageLayout) -> io::Result<InstancePaths> {
    #[cfg(windows)]
    return Ok(InstancePaths::windows(
        layout.data(),
        &crate::settings_wiring::profile_path("LOCALAPPDATA")?,
    ));
    #[cfg(unix)]
    {
        let home = crate::settings_wiring::profile_path("HOME")?;
        let state = std::env::var_os("XDG_STATE_HOME")
            .map(std::path::PathBuf::from)
            .filter(|path| path.is_absolute())
            .unwrap_or_else(|| home.join(".local/state"));
        let runtime = std::env::var_os("XDG_RUNTIME_DIR").map(std::path::PathBuf::from);
        Ok(InstancePaths::linux(
            layout.data(),
            &state,
            runtime.as_deref(),
        ))
    }
}

pub(crate) struct ListenerWorker {
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<io::Result<()>>>,
}

pub(crate) fn start(
    mut listener: ActivationListener,
    commands: SyncSender<WindowCommand>,
    context: egui::Context,
) -> io::Result<ListenerWorker> {
    let stop = Arc::new(AtomicBool::new(false));
    let stopping = Arc::clone(&stop);
    let worker = crate::executor::spawn("tallyline-activation", move || {
        while !stopping.load(Ordering::Acquire) {
            if listener.poll_activation()? {
                match commands.try_send(WindowCommand::Activate) {
                    Ok(()) | Err(TrySendError::Full(WindowCommand::Activate)) => {
                        context.request_repaint()
                    }
                    Err(TrySendError::Disconnected(WindowCommand::Activate)) => return Ok(()),
                }
            }
            std::thread::sleep(LISTENER_POLL);
        }
        Ok(())
    })?;
    Ok(ListenerWorker {
        stop,
        worker: Some(worker),
    })
}

impl ListenerWorker {
    pub(crate) fn finish(mut self) -> io::Result<()> {
        self.join()
    }

    fn join(&mut self) -> io::Result<()> {
        self.stop.store(true, Ordering::Release);
        match self.worker.take() {
            Some(worker) => worker
                .join()
                .map_err(|_| io::Error::other("activation worker terminated unexpectedly"))?,
            None => Ok(()),
        }
    }
}

impl Drop for ListenerWorker {
    fn drop(&mut self) {
        if let Err(error) = self.join() {
            tracing::error!(%error, "Activation worker shutdown failed");
        }
    }
}
