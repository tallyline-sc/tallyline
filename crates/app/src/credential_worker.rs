// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Bounded serial credential work outside the UI thread.

use std::io;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError},
};
use std::thread::JoinHandle;
use std::time::Duration;
use tallyline_shared_kernel::api::Clock;
use tallyline_submission::api::model::{
    CredentialFailure, Persistence, ReleaseToken, SavedCredentials, StorageFailure,
};
use tallyline_submission::api::{CredentialApplication, CredentialValidator, SecretStore};
use tallyline_ui::api::model::{BackgroundEvent, CredentialCommand};

const CONTROL_POLL: Duration = Duration::from_millis(100);

/// Store instances created on the credential worker without reading the fallback record.
pub struct CredentialStores<S> {
    primary: Result<S, StorageFailure>,
    fallback: Option<S>,
    token: Option<ReleaseToken>,
}
impl<S> CredentialStores<S> {
    /// Supplies packaged token data prepared on this same worker.
    #[must_use]
    pub fn with_release_token(mut self, token: Option<ReleaseToken>) -> Self {
        self.token = token;
        self
    }
    /// Supplies an OS store and an optional independently checked Linux fallback.
    #[must_use]
    pub const fn new(primary: Result<S, StorageFailure>, fallback: Option<S>) -> Self {
        Self {
            primary,
            fallback,
            token: None,
        }
    }
}
impl<S> From<Result<S, StorageFailure>> for CredentialStores<S> {
    fn from(primary: Result<S, StorageFailure>) -> Self {
        Self::new(primary, None)
    }
}

/// Owns one serial credential runtime and joins before the writer lease ends.
pub struct CredentialWorker {
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<io::Result<()>>>,
}
impl CredentialWorker {
    /// Starts isolated credential work without inheriting dependency logging subscribers.
    ///
    /// # Errors
    /// Returns a thread creation failure.
    /// Starts credential-only work through the existing bounded owner.
    ///
    /// # Errors
    /// Returns a thread creation failure.
    pub fn start<S, V, C, F, Fut, I>(
        initialise: F,
        validator: V,
        clock: C,
        token: Option<ReleaseToken>,
        incoming: Receiver<CredentialCommand>,
        outgoing: SyncSender<BackgroundEvent>,
        wake: impl Fn() + Send + 'static,
    ) -> io::Result<Self>
    where
        S: SecretStore + 'static,
        V: CredentialValidator + Send + 'static,
        C: Clock + Send + 'static,
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = I>,
        I: Into<CredentialStores<S>>,
    {
        Self::start_configured(
            initialise,
            validator,
            clock,
            token,
            (incoming, outgoing),
            wake,
            None,
        )
    }
    /// Composes one activated submission queue after coordinated storage bootstrap.
    /// Queue CPU work runs on this dedicated owner only after verified below-normal priority.
    ///
    /// # Errors
    /// Returns thread creation failure; priority or queue failures surface when the owner joins.
    pub fn start_with_queue<S, V, C, F, Fut, I>(
        initialise: F,
        validator: V,
        clock: C,
        token: Option<ReleaseToken>,
        channels: (Receiver<CredentialCommand>, SyncSender<BackgroundEvent>),
        queues: Receiver<Box<dyn crate::api::CredentialQueue>>,
        wake: impl Fn() + Send + 'static,
    ) -> io::Result<Self>
    where
        S: SecretStore + 'static,
        V: CredentialValidator + Send + 'static,
        C: Clock + Send + 'static,
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = I>,
        I: Into<CredentialStores<S>>,
    {
        Self::start_configured(
            initialise,
            validator,
            clock,
            token,
            channels,
            wake,
            Some(queues),
        )
    }
    fn start_configured<S, V, C, F, Fut, I>(
        initialise: F,
        validator: V,
        clock: C,
        token: Option<ReleaseToken>,
        channels: (Receiver<CredentialCommand>, SyncSender<BackgroundEvent>),
        wake: impl Fn() + Send + 'static,
        queues: Option<Receiver<Box<dyn crate::api::CredentialQueue>>>,
    ) -> io::Result<Self>
    where
        S: SecretStore + 'static,
        V: CredentialValidator + Send + 'static,
        C: Clock + Send + 'static,
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = I>,
        I: Into<CredentialStores<S>>,
    {
        let (incoming, outgoing) = channels;
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = Arc::clone(&stop);
        let worker = crate::executor::spawn("tallyline-credentials", move || {
            if queues.is_some() {
                tallyline_adapter_platform::api::lower_worker_priority()?;
            }
            let mut queue = queues.map(crate::credential_queue::QueueComposition::new);
            tracing::subscriber::with_default(tracing::subscriber::NoSubscriber::default(), || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()?;
                let Some(stores) = runtime.block_on(interruptible(initialise(), &stopping)) else {
                    return Ok(());
                };
                let stores = stores.into();
                let mut session = Session {
                    application: CredentialApplication::default(),
                    store: stores.primary,
                    fallback: stores.fallback,
                    validator,
                    clock,
                    token: stores.token.or(token),
                    reconcile: false,
                    fallback_available: false,
                    startup_pending: false,
                };
                let Some(failure) = runtime.block_on(interruptible(session.restore(), &stopping))
                else {
                    return Ok(());
                };
                session.fallback_available = failure
                    == Some(CredentialFailure::Storage(StorageFailure::Unavailable))
                    && session.fallback.is_some();
                deliver(&outgoing, session.event(failure), &wake, &stopping)?;
                let result = (|| {
                    while !stopping.load(Ordering::Acquire) {
                        if session.startup_pending && session.validator.is_available() {
                            let Some(failure) =
                                runtime.block_on(interruptible(session.restore(), &stopping))
                            else {
                                break;
                            };
                            deliver(&outgoing, session.event(failure), &wake, &stopping)?;
                        }
                        match incoming.try_recv() {
                            Ok(command) => {
                                let Some(failure) = runtime
                                    .block_on(interruptible(session.process(command), &stopping))
                                else {
                                    break;
                                };
                                session.reconcile |= failure
                                    == Some(CredentialFailure::Storage(
                                        StorageFailure::OutcomeUnknown,
                                    ));
                                deliver(&outgoing, session.event(failure), &wake, &stopping)?;
                                continue;
                            }
                            Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
                            Err(std::sync::mpsc::TryRecvError::Empty) => {}
                        }
                        if let Some(queue) = &mut queue {
                            queue.poll(
                                &session.application,
                                session.token.as_ref(),
                                session.clock.now(),
                            )?;
                        }
                        let command = match incoming.recv_timeout(CONTROL_POLL) {
                            Ok(command) => command,
                            Err(RecvTimeoutError::Timeout) => continue,
                            Err(RecvTimeoutError::Disconnected) => break,
                        };
                        let Some(failure) =
                            runtime.block_on(interruptible(session.process(command), &stopping))
                        else {
                            break;
                        };
                        session.reconcile |= failure
                            == Some(CredentialFailure::Storage(StorageFailure::OutcomeUnknown));
                        deliver(&outgoing, session.event(failure), &wake, &stopping)?;
                    }
                    Ok(())
                })();
                let shutdown = match &mut queue {
                    Some(queue) => queue.shutdown(session.clock.now()),
                    None => Ok(()),
                };
                result.and(shutdown)
            })
        })?;
        Ok(Self {
            stop,
            worker: Some(worker),
        })
    }
    /// Stops accepting commands and joins the bounded worker.
    ///
    /// # Errors
    /// Returns a runtime failure or worker panic.
    pub fn finish(mut self) -> io::Result<()> {
        self.join()
    }
    fn join(&mut self) -> io::Result<()> {
        self.stop.store(true, Ordering::Release);
        match self.worker.take() {
            Some(worker) => worker
                .join()
                .map_err(|_| io::Error::other("credential worker terminated unexpectedly"))?,
            None => Ok(()),
        }
    }
}
impl Drop for CredentialWorker {
    fn drop(&mut self) {
        if self.join().is_err() {
            tracing::error!("Credential worker shutdown failed");
        }
    }
}
struct Session<S, V, C> {
    application: CredentialApplication,
    store: Result<S, StorageFailure>,
    fallback: Option<S>,
    validator: V,
    clock: C,
    token: Option<ReleaseToken>,
    reconcile: bool,
    fallback_available: bool,
    startup_pending: bool,
}
impl<S: SecretStore, V: CredentialValidator, C: Clock> Session<S, V, C> {
    async fn restore(&mut self) -> Option<CredentialFailure> {
        self.startup_pending = false;
        let store = match &mut self.store {
            Ok(store) => store,
            Err(failure) => return Some(CredentialFailure::Storage(*failure)),
        };
        let restored = match CredentialApplication::load(store).await {
            Ok(application) => application,
            Err(failure) => return Some(CredentialFailure::Storage(failure)),
        };
        if matches!(
            restored.state(),
            tallyline_submission::api::model::AccountState::Unconfigured
        ) {
            return None;
        }
        self.application = restored;
        if !self.validator.is_available() {
            self.startup_pending = true;
            return None;
        }
        let mut request = self.application.begin_revalidation()?;
        if let Err(failure) = request
            .lookup_token_admission(store, self.token.as_ref().map(ReleaseToken::for_validation))
            .await
        {
            return Some(CredentialFailure::Storage(failure));
        }
        let result = request
            .validate_with_release_token(
                &self.validator,
                self.token.as_ref().map(ReleaseToken::for_validation),
            )
            .await;
        let failure = self
            .application
            .accept(request, result, self.clock.now(), store)
            .await
            .err();
        self.reconcile |=
            failure == Some(CredentialFailure::Storage(StorageFailure::OutcomeUnknown));
        if failure.is_none() {
            self.application
                .establish_token_scope(
                    store,
                    self.token.as_ref().map(ReleaseToken::for_validation),
                    Persistence::Session,
                )
                .await
                .err()
                .map(CredentialFailure::Storage)
        } else {
            failure
        }
    }
    async fn process(&mut self, command: CredentialCommand) -> Option<CredentialFailure> {
        let candidate = match command {
            CredentialCommand::ConfirmFallback => {
                if !self.fallback_available || self.reconcile {
                    return Some(CredentialFailure::Storage(StorageFailure::Unavailable));
                }
                let fallback = self.fallback.take()?;
                self.store = Ok(fallback);
                self.fallback_available = false;
                return self.restore().await;
            }
            CredentialCommand::Validate(candidate) => candidate,
        };
        if self.reconcile && candidate.persistence() == Persistence::Durable {
            let loaded = match &mut self.store {
                Ok(store) => store.load().await.map(|_| ()),
                Err(failure) => Err(*failure),
            };
            if let Err(failure) = loaded {
                return Some(CredentialFailure::Storage(failure));
            }
            self.reconcile = false;
        }
        let persistence = candidate.persistence();
        self.startup_pending = false;
        let mut request = self.application.begin(candidate);
        let scope_failure = match &mut self.store {
            Ok(store) => request
                .lookup_token_admission(
                    store,
                    self.token.as_ref().map(ReleaseToken::for_validation),
                )
                .await
                .err(),
            Err(failure) => Some(*failure),
        };
        if persistence == Persistence::Durable
            && let Some(failure) = scope_failure
        {
            return Some(CredentialFailure::Storage(failure));
        }
        let result = request
            .validate_with_release_token(
                &self.validator,
                self.token.as_ref().map(ReleaseToken::for_validation),
            )
            .await;
        match &mut self.store {
            Ok(store) => {
                match self
                    .application
                    .accept(request, result, self.clock.now(), store)
                    .await
                {
                    Ok(()) if scope_failure.is_some() => {
                        scope_failure.map(CredentialFailure::Storage)
                    }
                    Ok(()) => self
                        .application
                        .establish_token_scope(
                            store,
                            self.token.as_ref().map(ReleaseToken::for_validation),
                            persistence,
                        )
                        .await
                        .err()
                        .map(CredentialFailure::Storage),
                    Err(failure) => Some(failure),
                }
            }
            Err(failure) => self
                .application
                .accept(
                    request,
                    result,
                    self.clock.now(),
                    &mut UnavailableStore(*failure),
                )
                .await
                .err(),
        }
    }
    fn event(&self, failure: Option<CredentialFailure>) -> BackgroundEvent {
        BackgroundEvent::CredentialsUpdated {
            account: self.application.state().clone(),
            failure,
            available_token: self.token.is_some(),
            fallback_available: self.fallback_available,
        }
    }
}
struct UnavailableStore(StorageFailure);
impl SecretStore for UnavailableStore {
    async fn load(&mut self) -> Result<Option<SavedCredentials>, StorageFailure> {
        Err(self.0)
    }
    async fn save(&mut self, _: &SavedCredentials) -> Result<(), StorageFailure> {
        Err(self.0)
    }
    async fn save_passive(&mut self, _: &SavedCredentials) -> Result<(), StorageFailure> {
        Err(self.0)
    }
    async fn remove(&mut self) -> Result<(), StorageFailure> {
        Err(self.0)
    }
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
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(TrySendError::Disconnected(_)) => return Ok(()),
        }
    }
}

async fn interruptible<T>(future: impl Future<Output = T>, stop: &AtomicBool) -> Option<T> {
    let mut future = std::pin::pin!(future);
    while !stop.load(Ordering::Acquire) {
        match tokio::time::timeout(CONTROL_POLL, future.as_mut()).await {
            Ok(result) => return Some(result),
            Err(_) => continue,
        }
    }
    None
}
