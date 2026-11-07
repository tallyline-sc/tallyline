// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Fresh server-time observations advanced only by the injected monotonic clock.

use std::time::Duration;
use tallyline_shared_kernel::api::model::Timestamp;

const MAX_ESTIMATE_AGE: Duration = Duration::from_secs(15 * 60);

/// A verified response date bound to its monotonic reception time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ServerTimeEstimate {
    server: Timestamp,
    received_at: Duration,
}
impl ServerTimeEstimate {
    /// Records a server date and a reading from the submission clock's fixed monotonic origin.
    #[must_use]
    pub const fn new(server: Timestamp, received_at: Duration) -> Self {
        Self {
            server,
            received_at,
        }
    }
    /// Returns the advanced estimate only within fifteen minutes of monotonic reception.
    #[must_use]
    pub fn at(self, now: Duration) -> Option<Timestamp> {
        let elapsed = now.checked_sub(self.received_at)?;
        if elapsed > MAX_ESTIMATE_AGE {
            return None;
        }
        self.server.checked_add(elapsed)
    }
    /// Selects the earlier valid estimate while preserving the original observation otherwise.
    #[must_use]
    pub fn clamp(self, observed: Timestamp, now: Duration) -> Timestamp {
        self.at(now)
            .map_or(observed, |estimate| estimate.min(observed))
    }
}
