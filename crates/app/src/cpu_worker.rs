// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Owned CPU worker start boundary, separate from bounded OCR scheduling.

use std::io;
use std::thread::JoinHandle;

/// Joins one CPU task that runs only after verified below-normal thread priority.
/// This primitive does not provide a pool, admission, cancellation or OCR scheduling.
pub struct CpuWorker<T> {
    worker: Option<JoinHandle<io::Result<T>>>,
}

impl<T: Send + 'static> CpuWorker<T> {
    /// Starts a named CPU task in the submitting tracing span.
    /// The task is never invoked if setting or verifying OS priority fails.
    ///
    /// # Errors
    /// Returns thread creation failure; priority or task-panic failures reach `finish`.
    pub fn start(name: &str, task: impl FnOnce() -> T + Send + 'static) -> io::Result<Self> {
        let worker = crate::executor::spawn(name, move || {
            tallyline_adapter_platform::api::lower_worker_priority()?;
            Ok(task())
        })?;
        Ok(Self {
            worker: Some(worker),
        })
    }

    /// Joins the owned task and returns its result after the task has stopped.
    ///
    /// # Errors
    /// Returns priority setup failure or a worker panic.
    pub fn finish(mut self) -> io::Result<T> {
        self.join()
    }
}

impl<T> CpuWorker<T> {
    fn join(&mut self) -> io::Result<T> {
        self.worker
            .take()
            .ok_or_else(|| io::Error::other("CPU worker has already been joined"))?
            .join()
            .map_err(|_| io::Error::other("CPU worker terminated unexpectedly"))?
    }
}

impl<T> Drop for CpuWorker<T> {
    fn drop(&mut self) {
        if self.worker.is_some()
            && let Err(error) = self.join()
        {
            tracing::error!(%error, "CPU worker shutdown failed");
        }
    }
}
