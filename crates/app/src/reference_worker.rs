// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! App-owned reference runtime, cache owner and cancellable public GET scheduling.

use std::collections::VecDeque;
use std::io;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{Receiver, RecvTimeoutError, SyncSender, TryRecvError, TrySendError},
};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tallyline_adapter_uex::api::ReadError;
use tallyline_reference_data::api::model::{Endpoint, EndpointData, UexEnvironment};
use tallyline_reference_data::api::{
    ReferenceApplication, ReferenceDataCache, ReferenceDataFetcher,
};
use tallyline_shared_kernel::api::Clock;
use tallyline_shared_kernel::api::model::{TerminalId, Timestamp};
use tallyline_ui::api::model::{BackgroundEvent, ReferenceCommand, ReferenceDiagnostic};

type HistoryReader = Box<
    dyn FnMut() -> io::Result<tallyline_reference_data::api::model::AcceptedPriorHistory> + Send,
>;

const CONTROL_POLL: Duration = Duration::from_millis(100);
const RETRY_CHECK: Duration = Duration::from_secs(60);

/// Owns the reference runtime and cache until explicit shutdown or drop has joined it.
pub struct ReferenceWorker {
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<io::Result<()>>>,
}
impl ReferenceWorker {
    /// Starts a bounded reference worker that publishes cache data before requesting HTTP data.
    ///
    /// The caller holds the writer lease through this handle's joined lifetime.
    ///
    /// # Errors
    /// Returns a thread creation failure.
    pub fn start<S, C, F, E>(
        store: S,
        clock: C,
        incoming: Receiver<ReferenceCommand>,
        outgoing: SyncSender<BackgroundEvent>,
        make_fetcher: impl FnMut() -> Result<F, E> + Send + 'static,
        wake: impl Fn() + Send + 'static,
    ) -> io::Result<Self>
    where
        S: ReferenceDataCache + Send + 'static,
        S::Error: std::fmt::Display,
        C: Clock + Send + 'static,
        F: ReferenceDataFetcher<Error = ReadError>,
    {
        Self::start_configured(
            store,
            clock,
            (incoming, outgoing),
            make_fetcher,
            wake,
            (None, None),
        )
    }
    /// Publishes actual durable local acceptance history on the verified below-normal owner.
    /// An unavailable history refuses publication instead of substituting empty reference data.
    ///
    /// # Errors
    /// Returns thread creation failure; cache, priority and history failures surface on join.
    pub fn start_with_history<S, C, F, E>(
        store: S,
        clock: C,
        channels: (Receiver<ReferenceCommand>, SyncSender<BackgroundEvent>),
        make_fetcher: impl FnMut() -> Result<F, E> + Send + 'static,
        history: impl FnMut() -> io::Result<tallyline_reference_data::api::model::AcceptedPriorHistory>
        + Send
        + 'static,
        wake: impl Fn() + Send + 'static,
    ) -> io::Result<Self>
    where
        S: ReferenceDataCache + Send + 'static,
        S::Error: std::fmt::Display,
        C: Clock + Send + 'static,
        F: ReferenceDataFetcher<Error = ReadError>,
    {
        Self::start_configured(
            store,
            clock,
            channels,
            make_fetcher,
            wake,
            (Some(Box::new(history)), None),
        )
    }
    /// Acknowledges release refresh attempts after each required response or offline failure.
    ///
    /// # Errors
    /// Returns thread creation failure; worker failures close the completion capability.
    pub fn start_with_history_and_refresh<S, C, F, E>(
        store: S,
        clock: C,
        channels: (Receiver<ReferenceCommand>, SyncSender<BackgroundEvent>),
        make_fetcher: impl FnMut() -> Result<F, E> + Send + 'static,
        history: impl FnMut() -> io::Result<tallyline_reference_data::api::model::AcceptedPriorHistory>
        + Send
        + 'static,
        refresh: crate::api::ReleaseReferenceOwner,
        wake: impl Fn() + Send + 'static,
    ) -> io::Result<Self>
    where
        S: ReferenceDataCache + Send + 'static,
        S::Error: std::fmt::Display,
        C: Clock + Send + 'static,
        F: ReferenceDataFetcher<Error = ReadError>,
    {
        Self::start_configured(
            store,
            clock,
            channels,
            make_fetcher,
            wake,
            (Some(Box::new(history)), Some(refresh)),
        )
    }
    fn start_configured<S, C, F, E>(
        store: S,
        clock: C,
        channels: (Receiver<ReferenceCommand>, SyncSender<BackgroundEvent>),
        make_fetcher: impl FnMut() -> Result<F, E> + Send + 'static,
        wake: impl Fn() + Send + 'static,
        mut release: (
            Option<HistoryReader>,
            Option<crate::api::ReleaseReferenceOwner>,
        ),
    ) -> io::Result<Self>
    where
        S: ReferenceDataCache + Send + 'static,
        S::Error: std::fmt::Display,
        C: Clock + Send + 'static,
        F: ReferenceDataFetcher<Error = ReadError>,
    {
        let (incoming, outgoing) = channels;
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = Arc::clone(&stop);
        let worker = crate::executor::spawn("tallyline-reference", move || {
            let result = tallyline_adapter_platform::api::lower_worker_priority().and_then(|_| {
                run(
                    store,
                    clock,
                    (&incoming, &outgoing),
                    make_fetcher,
                    &wake,
                    &stopping,
                    &mut release,
                )
            });
            if let Err(error) = &result {
                if outgoing
                    .try_send(BackgroundEvent::Failure(error.to_string()))
                    .is_ok()
                {
                    wake();
                } else {
                    tracing::warn!("Reference worker failure could not be displayed");
                }
            }
            result
        })?;
        Ok(Self {
            stop,
            worker: Some(worker),
        })
    }
    /// Cancels pending read work and waits until the cache connection is closed.
    ///
    /// # Errors
    /// Returns a worker panic, cache failure or runtime initialisation failure.
    pub fn finish(mut self) -> io::Result<()> {
        self.join()
    }
    fn join(&mut self) -> io::Result<()> {
        self.stop.store(true, Ordering::Release);
        match self.worker.take() {
            Some(worker) => worker
                .join()
                .map_err(|_| io::Error::other("reference worker terminated unexpectedly"))?,
            None => Ok(()),
        }
    }
}
impl Drop for ReferenceWorker {
    fn drop(&mut self) {
        if let Err(error) = self.join() {
            tracing::error!(%error,"Reference worker shutdown failed");
        }
    }
}
struct BorrowedClock<'a, C>(&'a C);
impl<C: Clock> Clock for BorrowedClock<'_, C> {
    fn now(&self) -> Timestamp {
        self.0.now()
    }
}
#[derive(Default)]
struct Pending {
    publish: bool,
    refresh: bool,
    terminal: Option<TerminalId>,
}
impl Pending {
    fn receive(&mut self, command: ReferenceCommand) {
        match command {
            ReferenceCommand::PublishCached => self.publish = true,
            ReferenceCommand::Refresh => self.refresh = true,
            ReferenceCommand::OpenTerminal(id) => self.terminal = Some(id),
        }
    }
}
fn run<S, C, F, E>(
    store: S,
    clock: C,
    channels: (&Receiver<ReferenceCommand>, &SyncSender<BackgroundEvent>),
    mut make_fetcher: impl FnMut() -> Result<F, E>,
    wake: &impl Fn(),
    stop: &AtomicBool,
    release: &mut (
        Option<HistoryReader>,
        Option<crate::api::ReleaseReferenceOwner>,
    ),
) -> io::Result<()>
where
    S: ReferenceDataCache,
    S::Error: std::fmt::Display,
    C: Clock,
    F: ReferenceDataFetcher<Error = ReadError>,
{
    let (incoming, outgoing) = channels;
    let (history, refresh) = release;
    let mut application = ReferenceApplication::load(store, BorrowedClock(&clock))
        .map_err(|error| io::Error::other(error.to_string()))?;
    if !snapshot(&application, &clock, outgoing, wake, stop, history)? {
        return Ok(());
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let mut fetcher = None;
    let mut work = VecDeque::from(application.due());
    if !work.contains(&Endpoint::GameVersions) {
        work.push_back(Endpoint::GameVersions);
    }
    let mut active = None;
    let mut pending = Pending::default();
    let mut next_check = Instant::now() + RETRY_CHECK;
    while !stop.load(Ordering::Acquire) {
        match incoming.try_recv() {
            Ok(command) => pending.receive(command),
            Err(TryRecvError::Empty) => (),
            Err(TryRecvError::Disconnected) => return Ok(()),
        }
        if pending.publish {
            pending.publish = false;
            if !snapshot(&application, &clock, outgoing, wake, stop, history)? {
                return Ok(());
            }
        }
        if pending.refresh {
            work = VecDeque::from([
                Endpoint::Commodities,
                Endpoint::Terminals,
                Endpoint::GameVersions,
                Endpoint::StatusLevels,
                Endpoint::Parameters,
            ]);
            if let Some(id) = active {
                work.push_front(Endpoint::TerminalPriors(id));
            }
            pending.refresh = false;
        }
        if let Some(id) = pending.terminal.take() {
            active = Some(id);
            if application.terminal_priors_due(id) {
                let endpoint = Endpoint::TerminalPriors(id);
                if !work.contains(&endpoint) {
                    work.push_front(endpoint);
                }
            } else if !snapshot(&application, &clock, outgoing, wake, stop, history)? {
                return Ok(());
            }
        }
        if let Some(refresh) = refresh {
            refresh.schedule(&mut work);
        }
        if let Some(endpoint) = work.pop_front() {
            if fetcher.is_none() {
                fetcher = make_fetcher().ok();
            }
            if let Some(fetcher) = &fetcher {
                if !deliver(
                    BackgroundEvent::ReferenceRefreshStarted,
                    outgoing,
                    wake,
                    stop,
                ) {
                    return Ok(());
                }
                let response =
                    runtime.block_on(fetch(fetcher.fetch(endpoint), incoming, &mut pending, stop));
                let Some(response) = response else {
                    return Ok(());
                };
                match response {
                    Ok(data) => {
                        if application.refreshed(data).is_err() {
                            application.refresh_failed(endpoint);
                        }
                    }
                    Err(error) => {
                        application.refresh_failed(endpoint);
                        if let ReadError::NotJson {
                            http_status,
                            cf_ray,
                        } = error
                            && !deliver(
                                BackgroundEvent::ReferenceDiagnostic(ReferenceDiagnostic {
                                    http_status,
                                    cf_ray,
                                    observed_at: clock.now(),
                                }),
                                outgoing,
                                wake,
                                stop,
                            )
                        {
                            return Ok(());
                        }
                    }
                }
            } else {
                application.refresh_failed(endpoint);
                for endpoint in work.drain(..) {
                    application.refresh_failed(endpoint);
                    if let Some(refresh) = refresh {
                        refresh.attempted(endpoint);
                    }
                }
            }
            if !snapshot(&application, &clock, outgoing, wake, stop, history)? {
                return Ok(());
            }
            if let Some(refresh) = refresh {
                refresh.attempted(endpoint);
                refresh.acknowledge();
            }
            next_check = Instant::now() + RETRY_CHECK;
            continue;
        }
        match incoming.recv_timeout(CONTROL_POLL) {
            Ok(command) => pending.receive(command),
            Err(RecvTimeoutError::Disconnected) => return Ok(()),
            Err(RecvTimeoutError::Timeout) => {
                if Instant::now() >= next_check {
                    work = VecDeque::from(application.due());
                    if let Some(id) = active
                        && application.terminal_priors_due(id)
                        && !work.contains(&Endpoint::TerminalPriors(id))
                    {
                        work.push_front(Endpoint::TerminalPriors(id));
                    }
                    next_check = Instant::now() + RETRY_CHECK;
                }
            }
        }
    }
    Ok(())
}
async fn fetch(
    future: impl std::future::Future<Output = Result<EndpointData, ReadError>>,
    incoming: &Receiver<ReferenceCommand>,
    pending: &mut Pending,
    stop: &AtomicBool,
) -> Option<Result<EndpointData, ReadError>> {
    let mut future = std::pin::pin!(future);
    while !stop.load(Ordering::Acquire) {
        match tokio::time::timeout(CONTROL_POLL, future.as_mut()).await {
            Ok(response) => return Some(response),
            Err(_) => match incoming.try_recv() {
                Ok(command) => pending.receive(command),
                Err(TryRecvError::Empty) => (),
                Err(TryRecvError::Disconnected) => return None,
            },
        }
    }
    None
}
fn snapshot<S: ReferenceDataCache, C: Clock>(
    application: &ReferenceApplication<S, BorrowedClock<'_, C>>,
    clock: &C,
    outgoing: &SyncSender<BackgroundEvent>,
    wake: &impl Fn(),
    stop: &AtomicBool,
    history: &mut Option<HistoryReader>,
) -> io::Result<bool> {
    let mut live = application.snapshot(UexEnvironment::Live);
    let mut ptu = application.snapshot(UexEnvironment::Ptu);
    if let Some(read) = history {
        let history = read()?;
        live = live
            .with_local_priors(&history.active(UexEnvironment::Live))
            .map_err(|_| io::Error::other("local reference projection is invalid"))?;
        ptu = ptu
            .with_local_priors(&history.active(UexEnvironment::Ptu))
            .map_err(|_| io::Error::other("local reference projection is invalid"))?;
    }
    Ok(deliver(
        BackgroundEvent::ReferenceUpdated {
            live,
            ptu,
            observed_at: clock.now(),
        },
        outgoing,
        wake,
        stop,
    ))
}
fn deliver(
    mut event: BackgroundEvent,
    outgoing: &SyncSender<BackgroundEvent>,
    wake: &impl Fn(),
    stop: &AtomicBool,
) -> bool {
    while !stop.load(Ordering::Acquire) {
        match outgoing.try_send(event) {
            Ok(()) => {
                wake();
                return true;
            }
            Err(TrySendError::Disconnected(_)) => return false,
            Err(TrySendError::Full(returned)) => {
                event = returned;
                std::thread::sleep(CONTROL_POLL);
            }
        }
    }
    false
}
