// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! One application-owned response CPU worker with fixed non-blocking admission.

use crate::cpu_worker::CpuWorker;
use std::io;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use tallyline_adapter_uex::api::{
    ProcessingUnavailable, ResponseExecutor, ResponseProcessor, ResponseTask,
};

const PENDING_RESPONSES: usize = 2;
const CONTROL_POLL: std::time::Duration = std::time::Duration::from_millis(100);

struct Admission {
    sender: mpsc::SyncSender<ResponseTask>,
    stop: Arc<AtomicBool>,
}
impl ResponseExecutor for Admission {
    fn enqueue(&self, task: ResponseTask) -> Result<(), ProcessingUnavailable> {
        if self.stop.load(Ordering::Acquire) {
            return Err(ProcessingUnavailable);
        }
        self.sender
            .try_send(task)
            .map_err(|_| ProcessingUnavailable)
    }
}
/// Joins the application's sole bounded response parser, started at verified below-normal priority.
pub struct ResponseWorker {
    worker: Option<CpuWorker<()>>,
    stop: Arc<AtomicBool>,
}
impl ResponseWorker {
    /// Starts a dedicated parser; its work runs only after priority verification succeeds.
    ///
    /// # Errors
    /// Returns thread creation failure; priority failures close all processing requests.
    pub fn start() -> io::Result<(Self, ResponseProcessor)> {
        let (sender, receiver) = mpsc::sync_channel::<ResponseTask>(PENDING_RESPONSES);
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = Arc::clone(&stop);
        let worker = CpuWorker::start("tallyline-http-cpu", move || {
            while !stopping.load(Ordering::Acquire) {
                match receiver.recv_timeout(CONTROL_POLL) {
                    Ok(task) => tracing::subscriber::with_default(
                        tracing::subscriber::NoSubscriber::default(),
                        || task.execute(),
                    ),
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        })?;
        let processor = ResponseProcessor::new(Arc::new(Admission {
            sender,
            stop: Arc::clone(&stop),
        }));
        Ok((
            Self {
                worker: Some(worker),
                stop,
            },
            processor,
        ))
    }
    /// Stops admission, abandons queued work and joins any currently running bounded task.
    ///
    /// # Errors
    /// Returns verified-priority failure or an unexpected worker termination.
    pub fn finish(mut self) -> io::Result<()> {
        self.join()
    }
    fn join(&mut self) -> io::Result<()> {
        self.stop.store(true, Ordering::Release);
        match self.worker.take() {
            Some(worker) => worker.finish(),
            None => Ok(()),
        }
    }
}
impl Drop for ResponseWorker {
    fn drop(&mut self) {
        if self.join().is_err() {
            tracing::error!("Response CPU worker shutdown failed");
        }
    }
}
