// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Bounded nonsecret quota evidence from authenticated read responses.

use crate::api::model::{TokenQuotaReset, TokenScope};
const MAX_OBSERVATIONS: usize = 64;

/// A quota answer from an authenticated read, without a fabricated submission attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TokenQuotaObservation {
    scope: Option<TokenScope>,
    reset: Option<TokenQuotaReset>,
}
impl TokenQuotaObservation {
    /// Retains exact token association and optional response-bound reset evidence.
    #[must_use]
    pub const fn new(scope: Option<TokenScope>, reset: Option<TokenQuotaReset>) -> Self {
        Self { scope, reset }
    }
    /// Returns the proven token association, absent when it is unavailable.
    #[must_use]
    pub const fn scope(self) -> Option<TokenScope> {
        self.scope
    }
    /// Returns supplied reset evidence without inventing a deadline.
    #[must_use]
    pub const fn reset(self) -> Option<TokenQuotaReset> {
        self.reset
    }
    /// Blocks unproven identity while accepting exact protected inequality evidence.
    #[must_use]
    pub fn blocks_admission(self, proof: &crate::api::model::TokenAdmission) -> bool {
        self.scope.is_none_or(|scope| !proof.proves_distinct(scope))
    }
}
/// A bounded handoff that conservatively collapses overflow into an undated hold.
#[derive(Default, Debug)]
pub struct TokenQuotaObservations {
    observations: Vec<TokenQuotaObservation>,
    overflowed: bool,
}
impl TokenQuotaObservations {
    /// Retains distinct response evidence without unbounded allocation.
    pub fn record(&mut self, observation: TokenQuotaObservation) {
        if self.overflowed || self.observations.contains(&observation) {
            return;
        }
        if self.observations.len() == MAX_OBSERVATIONS {
            self.observations.clear();
            self.observations
                .push(TokenQuotaObservation::new(None, None));
            self.overflowed = true;
        } else {
            self.observations.push(observation);
        }
    }
    /// Transfers retained evidence once to its persistence owner.
    pub fn take(&mut self) -> Vec<TokenQuotaObservation> {
        self.overflowed = false;
        std::mem::take(&mut self.observations)
    }
}

impl AsRef<[TokenQuotaObservation]> for TokenQuotaObservations {
    fn as_ref(&self) -> &[TokenQuotaObservation] {
        &self.observations
    }
}
