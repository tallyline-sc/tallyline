// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Bounded app-owned manual Report persistence worker.

use std::io;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError},
};
use std::thread::JoinHandle;
use std::time::Duration;
use tallyline_game::api::model::EnvironmentMapping;
use tallyline_reporting::api::{ReportRepository, ReportingApplication};
use tallyline_shared_kernel::api::Clock;
use tallyline_ui::api::model::{BackgroundEvent, ManualCommand};

const CONTROL_POLL: Duration = Duration::from_millis(100);
const DELIVERY_POLL: Duration = Duration::from_millis(10);

/// Owns a bounded Draft worker until shutdown joins its database connection.
pub struct ManualWorker {
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<io::Result<()>>>,
}
impl ManualWorker {
    /// Starts local Draft persistence after coordinated startup migration.
    ///
    /// The app holds its writer lease until this worker is joined.
    ///
    /// # Errors
    /// Returns a worker-thread creation failure.
    pub fn start<R, C>(
        store: R,
        clock: C,
        incoming: Receiver<ManualCommand>,
        outgoing: SyncSender<BackgroundEvent>,
        mapping: impl FnMut() -> io::Result<EnvironmentMapping> + Send + 'static,
        wake: impl Fn() + Send + 'static,
    ) -> io::Result<Self>
    where
        R: ReportRepository + Send + 'static,
        C: Clock + Send + 'static,
    {
        Self::start_configured(store, clock, incoming, outgoing, None, mapping, wake)
    }
    /// Starts local Draft and private attachment work on the bounded below-normal worker.
    ///
    /// # Errors
    /// Returns thread creation failure; unavailable storage is delivered as passive state.
    pub fn start_with_attachments<R, C>(
        store: R,
        clock: C,
        incoming: Receiver<ManualCommand>,
        outgoing: SyncSender<BackgroundEvent>,
        storage: crate::api::ManualAttachmentStorage,
        mapping: impl FnMut() -> io::Result<EnvironmentMapping> + Send + 'static,
        wake: impl Fn() + Send + 'static,
    ) -> io::Result<Self>
    where
        R: ReportRepository + Send + 'static,
        C: Clock + Send + 'static,
    {
        Self::start_configured(
            store,
            clock,
            incoming,
            outgoing,
            Some(storage),
            mapping,
            wake,
        )
    }
    fn start_configured<R, C>(
        store: R,
        clock: C,
        incoming: Receiver<ManualCommand>,
        outgoing: SyncSender<BackgroundEvent>,
        storage: Option<crate::api::ManualAttachmentStorage>,
        mapping: impl FnMut() -> io::Result<EnvironmentMapping> + Send + 'static,
        wake: impl Fn() + Send + 'static,
    ) -> io::Result<Self>
    where
        R: ReportRepository + Send + 'static,
        C: Clock + Send + 'static,
    {
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = Arc::clone(&stop);
        let worker = crate::executor::spawn("tallyline-manual", move || {
            if let Err(error) = tallyline_adapter_platform::api::lower_worker_priority() {
                deliver(
                    &outgoing,
                    BackgroundEvent::ReportFailed(
                        tallyline_reporting::api::RepositoryError::Unavailable,
                    ),
                    &wake,
                    &stopping,
                )?;
                return Err(error);
            }
            run(
                ReportingApplication::new(store, clock),
                incoming,
                outgoing,
                mapping,
                wake,
                &stopping,
                storage.as_ref(),
            )
        })?;
        Ok(Self {
            stop,
            worker: Some(worker),
        })
    }
    /// Stops accepting commands and joins the worker before the writer lease ends.
    ///
    /// # Errors
    /// Returns a worker failure or panic.
    pub fn finish(mut self) -> io::Result<()> {
        self.join()
    }
    fn join(&mut self) -> io::Result<()> {
        self.stop.store(true, Ordering::Release);
        match self.worker.take() {
            Some(worker) => worker
                .join()
                .map_err(|_| io::Error::other("manual Report worker terminated unexpectedly"))?,
            None => Ok(()),
        }
    }
}
impl Drop for ManualWorker {
    fn drop(&mut self) {
        if let Err(error) = self.join() {
            tracing::error!(%error,"Manual Report worker shutdown failed");
        }
    }
}
fn run<R: ReportRepository, C: Clock>(
    mut application: ReportingApplication<R, C>,
    incoming: Receiver<ManualCommand>,
    outgoing: SyncSender<BackgroundEvent>,
    mut mapping: impl FnMut() -> io::Result<EnvironmentMapping>,
    wake: impl Fn(),
    stop: &AtomicBool,
    storage: Option<&crate::api::ManualAttachmentStorage>,
) -> io::Result<()> {
    let reports = match application.drafts() {
        Ok(reports) => reports,
        Err(error) => {
            deliver(&outgoing, BackgroundEvent::ReportFailed(error), &wake, stop)?;
            return Ok(());
        }
    };
    deliver(
        &outgoing,
        BackgroundEvent::ReportsLoaded {
            reports,
            selected: None,
        },
        &wake,
        stop,
    )?;
    while !stop.load(Ordering::Acquire) {
        let command = match incoming.recv_timeout(CONTROL_POLL) {
            Ok(command) => command,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        };
        let command = match command {
            ManualCommand::Attachment(command) => {
                let id = command.id;
                let expected = command.expected;
                let result = crate::manual_attachments::execute(&mut application, command, storage);
                match result {
                    Ok(events) => {
                        for event in events {
                            deliver(&outgoing, event, &wake, stop)?;
                        }
                    }
                    Err(failure) => deliver(
                        &outgoing,
                        BackgroundEvent::AttachmentFailed {
                            id,
                            expected,
                            failure,
                        },
                        &wake,
                        stop,
                    )?,
                }
                continue;
            }
            command @ (ManualCommand::Create { .. }
            | ManualCommand::Edit { .. }
            | ManualCommand::Review(_)) => command,
        };
        match mapping() {
            Ok(mapping) => application.set_mapping(mapping),
            Err(_) => {
                deliver(
                    &outgoing,
                    match &command {
                        ManualCommand::Review(request) => BackgroundEvent::ReviewFailed {
                            id: request.id,
                            expected: request.expected,
                            error: tallyline_reporting::api::RepositoryError::Unavailable,
                        },
                        ManualCommand::Edit {
                            request,
                            id,
                            expected,
                            ..
                        } => BackgroundEvent::ReportEditFinished {
                            request: *request,
                            id: *id,
                            expected: *expected,
                            result: Err(tallyline_reporting::api::RepositoryError::Unavailable),
                        },
                        ManualCommand::Create { .. } | ManualCommand::Attachment(_) => {
                            BackgroundEvent::ReportFailed(
                                tallyline_reporting::api::RepositoryError::Unavailable,
                            )
                        }
                    },
                    &wake,
                    stop,
                )?;
                continue;
            }
        }
        if let ManualCommand::Review(request) = command {
            let id = request.id;
            let expected = request.expected;
            let event = match crate::manual_reviews::execute(&mut application, request) {
                Ok(event) => event,
                Err(error) => BackgroundEvent::ReviewFailed {
                    id,
                    expected,
                    error,
                },
            };
            deliver(&outgoing, event, &wake, stop)?;
            continue;
        }
        if let ManualCommand::Edit {
            request,
            id,
            expected,
            edits,
            snapshot,
        } = command
        {
            let result = application
                .edit_batch(id, expected, &edits, &snapshot)
                .map(Box::new);
            deliver(
                &outgoing,
                BackgroundEvent::ReportEditFinished {
                    request,
                    id,
                    expected,
                    result,
                },
                &wake,
                stop,
            )?;
            continue;
        }
        let result = match command {
            ManualCommand::Attachment(_)
            | ManualCommand::Review(_)
            | ManualCommand::Edit { .. } => {
                return Err(io::Error::other(
                    "specialised manual command was not dispatched",
                ));
            }
            ManualCommand::Create { key, snapshot } => {
                application.create_from_reference(key, &snapshot)
            }
        };
        let event = match result {
            Ok(report) => match application.drafts() {
                Ok(reports) => BackgroundEvent::ReportsLoaded {
                    reports,
                    selected: Some(report.id()),
                },
                Err(error) => BackgroundEvent::ReportFailed(error),
            },
            Err(error) => BackgroundEvent::ReportFailed(error),
        };
        deliver(&outgoing, event, &wake, stop)?;
    }
    Ok(())
}
fn deliver(
    sender: &SyncSender<BackgroundEvent>,
    mut event: BackgroundEvent,
    wake: &impl Fn(),
    stop: &AtomicBool,
) -> io::Result<()> {
    loop {
        if stop.load(Ordering::Acquire) {
            return Ok(());
        }
        match sender.try_send(event) {
            Ok(()) => {
                wake();
                return Ok(());
            }
            Err(TrySendError::Full(unsent)) => {
                event = unsent;
                std::thread::sleep(DELIVERY_POLL);
            }
            Err(TrySendError::Disconnected(_)) => return Ok(()),
        }
    }
}
