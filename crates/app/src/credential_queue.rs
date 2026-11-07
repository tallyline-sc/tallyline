// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Submission composition inside the existing session credential owner.

use std::io;
use std::sync::mpsc::{Receiver, TryRecvError};
use tallyline_shared_kernel::api::model::Timestamp;
use tallyline_submission::api::CredentialApplication;
use tallyline_submission::api::model::ReleaseToken;

/// App-owned queue work called synchronously outside every asynchronous runtime poll.
/// Credentials remain borrowed from their session owner and are never persisted by this port.
pub trait CredentialQueue: Send {
    /// Performs one bounded control step using the current session's actual send authority.
    ///
    /// # Errors
    /// Returns a closed queue failure; the owner stops dispatch and runs shutdown.
    fn poll(
        &mut self,
        application: &CredentialApplication,
        token: Option<&ReleaseToken>,
        now: Timestamp,
    ) -> io::Result<()>;
    /// Retains known results and closes interrupted attempts without replay before the owner joins.
    ///
    /// # Errors
    /// Returns a persistence failure while durable write-ahead state remains unambiguous.
    fn shutdown(&mut self, now: Timestamp) -> io::Result<()>;
}
pub(crate) struct QueueComposition {
    incoming: Receiver<Box<dyn CredentialQueue>>,
    active: Option<Box<dyn CredentialQueue>>,
    last_now: Timestamp,
}
impl QueueComposition {
    pub(crate) fn new(incoming: Receiver<Box<dyn CredentialQueue>>) -> Self {
        Self {
            incoming,
            active: None,
            last_now: Timestamp::from_seconds(0),
        }
    }
    pub(crate) fn poll(
        &mut self,
        application: &CredentialApplication,
        token: Option<&ReleaseToken>,
        now: Timestamp,
    ) -> io::Result<()> {
        self.last_now = now;
        if self.active.is_none() {
            match self.incoming.try_recv() {
                Ok(queue) => self.active = Some(queue),
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => {}
            }
        }
        match &mut self.active {
            Some(queue) => queue.poll(application, token, now),
            None => Ok(()),
        }
    }
}
impl QueueComposition {
    pub(crate) fn shutdown(&mut self, now: Timestamp) -> io::Result<()> {
        self.last_now = now;
        match self.active.take() {
            Some(mut queue) => queue.shutdown(now),
            None => Ok(()),
        }
    }
}
impl Drop for QueueComposition {
    fn drop(&mut self) {
        if let Some(mut queue) = self.active.take()
            && queue.shutdown(self.last_now).is_err()
        {
            tracing::error!("Submission queue shutdown could not finish persistence");
        }
    }
}

type QueueInitialiser = Box<dyn FnOnce() -> io::Result<Box<dyn CredentialQueue>> + Send>;

/// Defers storage, decoding and recovery until activation on the verified CPU/session owner.
pub struct DeferredCredentialQueue {
    initialise: Option<QueueInitialiser>,
    active: Option<Box<dyn CredentialQueue>>,
}
impl DeferredCredentialQueue {
    /// Retains one bounded initialiser without invoking it on the composition or UI thread.
    #[must_use]
    pub fn new(
        initialise: impl FnOnce() -> io::Result<Box<dyn CredentialQueue>> + Send + 'static,
    ) -> Self {
        Self {
            initialise: Some(Box::new(initialise)),
            active: None,
        }
    }
}
impl CredentialQueue for DeferredCredentialQueue {
    fn poll(
        &mut self,
        application: &CredentialApplication,
        token: Option<&ReleaseToken>,
        now: Timestamp,
    ) -> io::Result<()> {
        if let Some(initialise) = self.initialise.take() {
            self.active = Some(initialise()?);
        }
        match &mut self.active {
            Some(queue) => queue.poll(application, token, now),
            None => Err(io::Error::other(
                "submission initialisation did not complete",
            )),
        }
    }
    fn shutdown(&mut self, now: Timestamp) -> io::Result<()> {
        self.initialise = None;
        match self.active.take() {
            Some(mut queue) => queue.shutdown(now),
            None => Ok(()),
        }
    }
}
