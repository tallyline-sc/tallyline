// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Bounded shared HTTP runtime whose lifetime covers every pooled connection.

use crate::http_client::HttpClient;
use crate::http_protocol::{Command, HttpBackend};
use crate::http_quota::HttpQuota;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;
use tallyline_submission::api::model::TokenAdmission;
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tracing::instrument::WithSubscriber;

const IO_THREADS: usize = 2;
const MAX_ACTIVE: usize = 2;
const MAX_BLOCKING_THREADS: usize = 2;
const MAX_SUBMISSIONS: usize = 1;
const REQUEST_CAPACITY: usize = 2;
const CONTROL_POLL: Duration = Duration::from_millis(100);

/// Owns the sole HTTP backend and joins its bounded shared runtime after client workers stop.
pub struct HttpDriver {
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<io::Result<()>>>,
    processor: Option<crate::response_worker::ResponseWorker>,
}
impl HttpDriver {
    /// Starts one backend factory on a private thread and exactly two I/O runtime threads.
    ///
    /// # Errors
    /// Returns thread creation failure; asynchronous startup failures surface through the clients and finish.
    pub fn start<B, E>(
        factory: impl FnOnce() -> Result<B, E> + Send + 'static,
    ) -> io::Result<(Self, HttpClient)>
    where
        B: HttpBackend + 'static,
    {
        Self::start_configured(factory, None)
    }
    /// Shares one server-time observation between response handling and body preparation.
    ///
    /// # Errors
    /// Returns thread creation failure; backend startup failure closes its clients.
    pub fn start_with_server_time<B, E>(
        factory: impl FnOnce() -> Result<B, E> + Send + 'static,
        server_time: tallyline_adapter_uex::api::model::ServerTimeTracker,
    ) -> io::Result<(Self, HttpClient)>
    where
        B: HttpBackend + 'static,
    {
        Self::start_configured(factory, Some(server_time))
    }
    /// Owns and joins the bounded below-normal response CPU worker alongside the shared I/O runtime.
    ///
    /// # Errors
    /// Returns CPU or I/O thread creation failure; backend failures close their clients.
    pub fn start_with_processing<B, E>(
        factory: impl FnOnce(tallyline_adapter_uex::api::ResponseProcessor) -> Result<B, E>
        + Send
        + 'static,
        server_time: Option<tallyline_adapter_uex::api::model::ServerTimeTracker>,
    ) -> io::Result<(Self, HttpClient)>
    where
        B: HttpBackend + 'static,
    {
        let (worker, processor) = crate::response_worker::ResponseWorker::start()?;
        let (mut driver, client) = Self::start_configured(move || factory(processor), server_time)?;
        driver.processor = Some(worker);
        Ok((driver, client))
    }
    fn start_configured<B, E>(
        factory: impl FnOnce() -> Result<B, E> + Send + 'static,
        server_time: Option<tallyline_adapter_uex::api::model::ServerTimeTracker>,
    ) -> io::Result<(Self, HttpClient)>
    where
        B: HttpBackend + 'static,
    {
        let (requests, incoming) = mpsc::channel(REQUEST_CAPACITY);
        let mut initial_quota = crate::http_quota::HttpQuota::default();
        if server_time.is_none() {
            initial_quota.replace_holds(Vec::new(), &[]);
        }
        let quota = Arc::new(std::sync::Mutex::new(initial_quota));
        let driver_quota = Arc::clone(&quota);
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = Arc::clone(&stop);
        let worker = crate::executor::spawn("tallyline-http", move || {
            tracing::subscriber::with_default(tracing::subscriber::NoSubscriber::default(), || {
                run(factory, incoming, &stopping, driver_quota)
            })
        })?;
        Ok((
            Self {
                stop,
                worker: Some(worker),
                processor: None,
            },
            HttpClient {
                requests,
                server_time,
                quota,
            },
        ))
    }
    /// Stops the driver after all application clients have finished and joins every I/O thread.
    ///
    /// # Errors
    /// Returns a closed backend startup or worker failure.
    pub fn finish(mut self) -> io::Result<()> {
        self.join()
    }
    fn join(&mut self) -> io::Result<()> {
        self.stop.store(true, Ordering::Release);
        let io = match self.worker.take() {
            Some(worker) => worker
                .join()
                .map_err(|_| io::Error::other("shared HTTP driver terminated unexpectedly"))
                .and_then(std::convert::identity),
            None => Ok(()),
        };
        let processor = match self.processor.take() {
            Some(worker) => worker.finish(),
            None => Ok(()),
        };
        io.and(processor)
    }
}
impl Drop for HttpDriver {
    fn drop(&mut self) {
        if self.join().is_err() {
            tracing::error!("Shared HTTP driver shutdown failed");
        }
    }
}
fn run<B: HttpBackend + 'static, E>(
    factory: impl FnOnce() -> Result<B, E>,
    mut incoming: mpsc::Receiver<Command>,
    stop: &AtomicBool,
    quota: Arc<std::sync::Mutex<crate::http_quota::HttpQuota>>,
) -> io::Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(IO_THREADS)
        .max_blocking_threads(MAX_BLOCKING_THREADS)
        .thread_name("tallyline-http-io")
        .enable_all()
        .build()?;
    let backend = {
        let _entered = runtime.enter();
        let admission = Arc::clone(&quota);
        let gate = tallyline_adapter_uex::api::AuthenticatedReadGate::new(move |proof| {
            admission.lock().is_ok_and(|quota| !quota.blocks(proof))
        });
        Arc::new(
            factory()
                .map_err(|_| io::Error::other("shared HTTP backend is unavailable"))?
                .with_authenticated_read_gate(gate),
        )
    };
    let mut tasks = JoinSet::new();
    let submissions = Arc::new(tokio::sync::Semaphore::new(MAX_SUBMISSIONS));
    let result = drive(
        &runtime,
        &backend,
        &mut tasks,
        &mut incoming,
        stop,
        &submissions,
        &quota,
    );
    runtime.block_on(tasks.shutdown());
    drop(backend);
    drop(runtime);
    result
}
fn drive<B: HttpBackend + 'static>(
    runtime: &tokio::runtime::Runtime,
    backend: &Arc<B>,
    tasks: &mut JoinSet<()>,
    incoming: &mut mpsc::Receiver<Command>,
    stop: &AtomicBool,
    submissions: &Arc<tokio::sync::Semaphore>,
    quota: &Arc<std::sync::Mutex<crate::http_quota::HttpQuota>>,
) -> io::Result<()> {
    while !stop.load(Ordering::Acquire) {
        while let Some(result) = tasks.try_join_next() {
            result.map_err(|_| {
                io::Error::other("shared HTTP request task terminated unexpectedly")
            })?;
        }
        if tasks.len() >= MAX_ACTIVE {
            if let Ok(Some(result)) = runtime
                .block_on(async { tokio::time::timeout(CONTROL_POLL, tasks.join_next()).await })
            {
                result.map_err(|_| {
                    io::Error::other("shared HTTP request task terminated unexpectedly")
                })?;
            }
            continue;
        }
        match runtime.block_on(async { tokio::time::timeout(CONTROL_POLL, incoming.recv()).await })
        {
            Ok(Some(command)) => {
                let backend = Arc::clone(backend);
                tasks.spawn_on(
                    route(backend, command, Arc::clone(submissions), Arc::clone(quota))
                        .with_subscriber(tracing::subscriber::NoSubscriber::default()),
                    runtime.handle(),
                );
            }
            Ok(None) => break,
            Err(_) => continue,
        }
    }
    incoming.close();
    Ok(())
}
async fn route<B: HttpBackend + 'static>(
    backend: Arc<B>,
    command: Command,
    submissions: Arc<tokio::sync::Semaphore>,
    quota: Arc<std::sync::Mutex<crate::http_quota::HttpQuota>>,
) {
    match command {
        Command::ReadLifecycle(batch, values, token, reply) => {
            let blocked = quota
                .lock()
                .map_or(true, |quota| quota.blocks(values.token_admission()));
            let result = if blocked {
                Err(tallyline_submission::api::model::LifecycleReadFailure::Unavailable)
            } else {
                backend.read_lifecycle(batch, values, token).await
            };
            record_quota(&*backend, &quota);
            if reply.send(result).is_err() {
                tracing::debug!("Lifecycle read reply receiver closed");
            }
        }
        Command::VerifyRowReceipts(job, values, token, reply) => {
            let result = backend.verify_row_receipts(job, values, token).await;
            record_quota(&*backend, &quota);
            if reply.send(result).is_err() {
                tracing::debug!("Row receipt verification reply receiver closed");
            }
        }
        Command::ReserveSubmission(reply) => {
            let Ok(_permit) = submissions.acquire_owned().await else {
                return;
            };
            let (slot, request, result) = crate::api::SubmissionSlot::channel();
            if reply.send(slot).is_err() {
                return;
            }
            let Ok(request) = request.await else {
                return;
            };
            let blocked = post_blocked(&*backend, &quota, request.values.token_admission());
            let scope = request.values.token_scope();
            let answer = if blocked {
                tallyline_submission::api::model::SubmissionResult::Returned(
                    tallyline_submission::api::model::SubmissionReturnReason::PreflightChanged,
                )
            } else {
                backend
                    .submit(request.prepared, request.values, request.token)
                    .await
            };
            hold_post_quota(&quota, &answer, scope, request.quota_source);
            record_quota(&*backend, &quota);
            if result.send(answer).is_err() {
                tracing::debug!("Reserved submission HTTP reply receiver closed");
            }
        }
        Command::RecheckRefusal(job, values, token, reply) => {
            let result = backend.recheck_refusal(job, values, token).await;
            record_quota(&*backend, &quota);
            if reply.send(result).is_err() {
                tracing::debug!("Refusal recheck reply receiver closed");
            }
        }
        Command::VerifyReceipts(job, values, token, reply) => {
            let result = backend.verify_receipts(job, values, token).await;
            record_quota(&*backend, &quota);
            if reply.send(result).is_err() {
                tracing::debug!("Receipt verification reply receiver closed");
            }
        }
        Command::Fetch(endpoint, reply) => {
            let result = backend.fetch(endpoint).await;
            record_quota(&*backend, &quota);
            if reply.send(result).is_err() {
                tracing::debug!("Public HTTP reply receiver closed");
            }
        }
        Command::Priors(terminals, reply) => {
            let result = backend.fetch_terminal_priors(terminals).await;
            record_quota(&*backend, &quota);
            if reply.send(result).is_err() {
                tracing::debug!("Prior HTTP reply receiver closed");
            }
        }
        Command::Validate(values, reply) => {
            let blocked = quota
                .lock()
                .map_or(true, |quota| quota.blocks(values.token_admission()));
            let result = if blocked {
                Err(backend.check_account_while_token_held(values).await)
            } else {
                backend.validate(values).await
            };
            record_quota(&*backend, &quota);
            if reply.send(result).is_err() {
                tracing::debug!("Credential HTTP reply receiver closed");
            }
        }
        Command::Submit(prepared, values, token, reply) => {
            let Ok(_permit) = submissions.acquire_owned().await else {
                if reply
                    .send(tallyline_submission::api::model::SubmissionResult::Unknown)
                    .is_err()
                {
                    tracing::debug!("Submission HTTP reply receiver closed");
                }
                return;
            };
            let scope = values.token_scope();
            let blocked = post_blocked(&*backend, &quota, values.token_admission());
            let result = if blocked {
                tallyline_submission::api::model::SubmissionResult::Returned(
                    tallyline_submission::api::model::SubmissionReturnReason::PreflightChanged,
                )
            } else {
                backend.submit(prepared, values, token).await
            };
            hold_post_quota(&quota, &result, scope, None);
            record_quota(&*backend, &quota);
            if reply.send(result).is_err() {
                tracing::debug!("Submission HTTP reply receiver closed");
            }
        }
    }
}

fn record_quota(
    backend: &impl HttpBackend,
    quota: &std::sync::Mutex<crate::http_quota::HttpQuota>,
) {
    match quota.lock() {
        Ok(mut quota) => {
            record_pending_quota(backend, &mut quota);
        }
        Err(_) => tracing::error!("Shared HTTP quota state is unavailable"),
    }
}

fn post_blocked(
    backend: &impl HttpBackend,
    quota: &Mutex<HttpQuota>,
    proof: &TokenAdmission,
) -> bool {
    quota.lock().map_or(true, |mut quota| {
        record_pending_quota(backend, &mut quota);
        quota.blocks(proof)
    })
}

fn record_pending_quota(backend: &impl HttpBackend, quota: &mut HttpQuota) {
    for observation in backend.take_quota_observations() {
        quota.record(observation);
    }
}

fn hold_post_quota(
    quota: &std::sync::Mutex<crate::http_quota::HttpQuota>,
    result: &tallyline_submission::api::model::SubmissionResult,
    scope: Option<tallyline_submission::api::model::TokenScope>,
    source: Option<crate::http_quota::PostQuotaSource>,
) {
    if !result.is_daily_quota() {
        return;
    }
    let reset = result.daily_quota_reset().map(|reset| {
        tallyline_submission::api::model::TokenQuotaReset::new(
            reset,
            result.daily_quota_observed_at(),
        )
    });
    match quota.lock() {
        Ok(mut quota) => quota.hold_post(
            tallyline_submission::api::model::TokenQuotaObservation::new(scope, reset),
            source,
        ),
        Err(_) => tracing::error!("Shared HTTP quota state is unavailable"),
    }
}
