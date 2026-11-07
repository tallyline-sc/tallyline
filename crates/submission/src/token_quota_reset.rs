// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact token reset evidence bound to the original response server time.

use tallyline_shared_kernel::api::model::Timestamp;

/// A supplied Unix reset and the independently parsed time of that same response.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TokenQuotaReset {
    reset: Timestamp,
    observed_at: Option<Timestamp>,
}
impl TokenQuotaReset {
    /// Retains supplied metadata without inventing a missing server observation.
    #[must_use]
    pub const fn new(reset: Timestamp, observed_at: Option<Timestamp>) -> Self {
        Self { reset, observed_at }
    }
    /// Returns the supplied absolute reset.
    #[must_use]
    pub const fn reset(self) -> Timestamp {
        self.reset
    }
    /// Returns the server time of the exact originating response, when verified.
    #[must_use]
    pub const fn observed_at(self) -> Option<Timestamp> {
        self.observed_at
    }
    /// Returns a usable next reset only when it was future at the original response.
    #[must_use]
    pub fn usable_reset(self) -> Option<Timestamp> {
        self.observed_at
            .filter(|at| self.reset > *at)
            .map(|_| self.reset)
    }
}
