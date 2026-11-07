// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Injected bounded response processing without adapter-owned threads or executors.

use std::sync::Arc;

/// A closed, value-free processing-admission or worker-lifetime failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcessingUnavailable;

/// Opaque work whose captured response and credential values cannot be formatted.
pub struct ResponseTask(Box<dyn FnOnce() + Send>);
impl ResponseTask {
    /// Executes the work on the application's verified dedicated CPU worker.
    pub fn execute(self) {
        (self.0)();
    }
}

/// Application-owned bounded admission; implementations must never run work on the caller.
pub trait ResponseExecutor: Send + Sync {
    /// Enqueues one opaque task without blocking or growing an unbounded queue.
    ///
    /// # Errors
    /// Refuses admission when the worker is unavailable or its fixed queue is full.
    fn enqueue(&self, task: ResponseTask) -> Result<(), ProcessingUnavailable>;
}

/// A shareable injected CPU-processing boundary with asynchronous completion.
#[derive(Clone)]
pub struct ResponseProcessor(Arc<dyn ResponseExecutor>);
impl ResponseProcessor {
    /// Captures an executor owned and joined by the application.
    #[must_use]
    pub fn new(executor: Arc<dyn ResponseExecutor>) -> Self {
        Self(executor)
    }
    /// Processes captured owned values without parsing them during an asynchronous poll.
    ///
    /// # Errors
    /// Refuses bounded admission or a worker that stopped before returning its result.
    pub async fn process<T: Send + 'static>(
        &self,
        work: impl FnOnce() -> T + Send + 'static,
    ) -> Result<T, ProcessingUnavailable> {
        let (send, receive) = tokio::sync::oneshot::channel();
        self.0.enqueue(ResponseTask(Box::new(move || {
            drop(send.send(work()));
        })))?;
        receive.await.map_err(|_| ProcessingUnavailable)
    }
}
impl std::fmt::Debug for ResponseProcessor {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ResponseProcessor(<opaque>)")
    }
}
