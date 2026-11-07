// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Owned response-date observations with an injected monotonic clock.

use std::sync::{Arc, Mutex};
use tallyline_shared_kernel::api::model::Timestamp;
use tallyline_submission::api::SubmissionClock;
use tallyline_submission::api::model::ServerTimeEstimate;

/// An owned shared server-date cache without persisted or diagnostic response data.
#[derive(Clone)]
pub struct ServerTimeTracker {
    clock: Arc<dyn SubmissionClock>,
    observation: Arc<Mutex<Option<ServerTimeEstimate>>>,
}
impl ServerTimeTracker {
    /// Starts without a server estimate and retains one injected monotonic origin.
    #[must_use]
    pub fn new(clock: Arc<dyn SubmissionClock>) -> Self {
        Self {
            clock,
            observation: Arc::new(Mutex::new(None)),
        }
    }
    /// Starts from a verified server observation using the supplied clock's monotonic origin.
    #[must_use]
    pub fn from_estimate(clock: Arc<dyn SubmissionClock>, estimate: ServerTimeEstimate) -> Self {
        Self {
            clock,
            observation: Arc::new(Mutex::new(Some(estimate))),
        }
    }
    pub(crate) fn observe(
        &self,
        date: Option<&str>,
    ) -> Result<Option<Timestamp>, ServerTimeUnavailable> {
        let server = date.and_then(|value| crate::http_date::parse(value, self.clock.now()));
        let observation =
            server.map(|server| ServerTimeEstimate::new(server, self.clock.monotonic()));
        let mut current = self.observation.lock().map_err(|_| ServerTimeUnavailable)?;
        *current = observation;
        Ok(server)
    }
    /// Returns the fresh server estimate advanced solely by this tracker's monotonic clock.
    #[must_use]
    pub fn current(&self) -> Option<Timestamp> {
        self.observation
            .lock()
            .ok()?
            .as_ref()?
            .at(self.clock.monotonic())
    }
    /// Returns only an earlier fresh estimate, falling back to the original observation.
    #[must_use]
    pub fn clamp(&self, observed: Timestamp) -> Timestamp {
        match self.observation.lock() {
            Ok(current) => current.map_or(observed, |estimate| {
                estimate.clamp(observed, self.clock.monotonic())
            }),
            Err(_) => observed,
        }
    }
}
impl std::fmt::Debug for ServerTimeTracker {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ServerTimeTracker")
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ServerTimeUnavailable;
