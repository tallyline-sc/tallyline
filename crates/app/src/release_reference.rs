// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! App-owned acknowledgement of bounded public reference attempts before release.

use std::io;
use tallyline_shared_kernel::api::model::TerminalId;

/// Acknowledges a fresh reference attempt independently of cached-reference publication.
pub trait ReleaseReferenceRefresh: Send {
    /// Starts one bounded refresh cycle for the selected terminal.
    ///
    /// # Errors
    /// Returns an unavailable or occupied owner without granting release permission.
    fn begin(&mut self, terminal: TerminalId) -> io::Result<()>;
    /// Reports whether the requested cycle attempted every required endpoint.
    /// Offline failures permit cached reassessment; unavailable owners refuse release.
    ///
    /// # Errors
    /// Returns a stopped owner or an inconsistent request acknowledgement.
    fn completed(&mut self) -> io::Result<bool>;
}

use std::collections::VecDeque;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc::{Receiver, SyncSender},
};
use tallyline_reference_data::api::model::Endpoint;

struct Completion {
    stopped: AtomicBool,
    acknowledged: AtomicU64,
}
struct Request {
    identity: u64,
    terminal: TerminalId,
}
/// A single bounded release-refresh requester with no runtime, thread or credential ownership.
pub struct ReleaseReferenceClient {
    requests: SyncSender<Request>,
    completion: Arc<Completion>,
    next: u64,
    pending: Option<u64>,
}
/// Completion capability owned exclusively by the joined reference worker.
pub struct ReleaseReferenceOwner {
    requests: Receiver<Request>,
    completion: Arc<Completion>,
    active: Option<(u64, Vec<Endpoint>)>,
}
impl ReleaseReferenceClient {
    /// Creates a bounded requester and its unique worker completion capability.
    #[must_use]
    pub fn channel() -> (Self, ReleaseReferenceOwner) {
        let (requests, incoming) = std::sync::mpsc::sync_channel(1);
        let completion = Arc::new(Completion {
            stopped: AtomicBool::new(false),
            acknowledged: AtomicU64::new(0),
        });
        (
            Self {
                requests,
                completion: Arc::clone(&completion),
                next: 1,
                pending: None,
            },
            ReleaseReferenceOwner {
                requests: incoming,
                completion,
                active: None,
            },
        )
    }
}
impl ReleaseReferenceRefresh for ReleaseReferenceClient {
    fn begin(&mut self, terminal: TerminalId) -> io::Result<()> {
        if self.completion.stopped.load(Ordering::Acquire) || self.next == u64::MAX {
            return Err(io::Error::other("reference owner unavailable"));
        }
        if self.pending.is_some_and(|identity| {
            self.completion.acknowledged.load(Ordering::Acquire) != identity
        }) {
            return Err(io::Error::other("reference attempt remains pending"));
        }
        self.requests
            .try_send(Request {
                identity: self.next,
                terminal,
            })
            .map_err(|_| io::Error::other("reference request unavailable"))?;
        self.pending = Some(self.next);
        self.next += 1;
        Ok(())
    }
    fn completed(&mut self) -> io::Result<bool> {
        if self.completion.stopped.load(Ordering::Acquire) {
            return Err(io::Error::other("reference owner stopped"));
        }
        let identity = self
            .pending
            .ok_or_else(|| io::Error::other("reference request unavailable"))?;
        let completed = self.completion.acknowledged.load(Ordering::Acquire) == identity;
        if completed {
            self.pending = None;
        }
        Ok(completed)
    }
}
impl ReleaseReferenceOwner {
    pub(crate) fn schedule(&mut self, work: &mut VecDeque<Endpoint>) {
        if self.active.is_none()
            && let Ok(request) = self.requests.try_recv()
        {
            let endpoints = vec![
                Endpoint::Commodities,
                Endpoint::Terminals,
                Endpoint::GameVersions,
                Endpoint::StatusLevels,
                Endpoint::Parameters,
                Endpoint::TerminalPriors(request.terminal),
            ];
            *work = VecDeque::from(endpoints.clone());
            self.active = Some((request.identity, endpoints));
        }
        if let Some((_, endpoints)) = &self.active {
            for endpoint in endpoints {
                if !work.contains(endpoint) {
                    work.push_back(*endpoint);
                }
            }
        }
    }
    pub(crate) fn attempted(&mut self, endpoint: Endpoint) {
        if let Some((_, endpoints)) = &mut self.active {
            endpoints.retain(|pending| *pending != endpoint);
        }
    }
    pub(crate) fn acknowledge(&mut self) {
        if self
            .active
            .as_ref()
            .is_some_and(|(_, endpoints)| endpoints.is_empty())
            && let Some((identity, _)) = self.active.take()
        {
            self.completion
                .acknowledged
                .store(identity, Ordering::Release);
        }
    }
}
impl Drop for ReleaseReferenceOwner {
    fn drop(&mut self) {
        self.completion.stopped.store(true, Ordering::Release);
    }
}
