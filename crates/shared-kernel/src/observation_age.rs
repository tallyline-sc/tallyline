// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Value-bound observation-age limits shared by release and dispatch.

use crate::api::model::Timestamp;
use std::time::Duration;

const DEFAULT_MAXIMUM: Duration = Duration::from_secs(60 * 60);
const HARD_LIMIT: Duration = Duration::from_secs(24 * 60 * 60);

/// An observation-age setting or confirmation outside its valid range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidObservationAge;

/// A confirmation bound to one observation time and its own expiry origin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AgeConfirmation {
    observed_at: Timestamp,
    confirmed_at: Timestamp,
}
impl AgeConfirmation {
    /// Confirms an observation at or before the confirmation time.
    ///
    /// # Errors
    /// Refuses a confirmation earlier than its observation.
    pub fn new(
        observed_at: Timestamp,
        confirmed_at: Timestamp,
    ) -> Result<Self, InvalidObservationAge> {
        if confirmed_at < observed_at {
            return Err(InvalidObservationAge);
        }
        Ok(Self {
            observed_at,
            confirmed_at,
        })
    }
    /// Returns the observation to which the decision is bound.
    #[must_use]
    pub const fn observed_at(self) -> Timestamp {
        self.observed_at
    }
    /// Returns the start of the confirmation's validity period.
    #[must_use]
    pub const fn confirmed_at(self) -> Timestamp {
        self.confirmed_at
    }
}

/// The age decision used independently of numeric and screenshot readiness.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgeValidity {
    /// The observation is within the configured age limit.
    Fresh,
    /// A current value-bound age confirmation covers the observation.
    Confirmed,
    /// The observation requires a new explicit age decision.
    Unconfirmed,
    /// The fixed twenty-four-hour hard limit has expired.
    TooOld,
    /// The observation lies after the injected current time.
    Future,
}

/// Tighten-only observation-age limits, with a fixed twenty-four-hour hard limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObservationAgeSettings {
    maximum: Duration,
}
impl Default for ObservationAgeSettings {
    fn default() -> Self {
        Self::baseline()
    }
}
impl ObservationAgeSettings {
    /// Returns the fixed sixty-minute baseline without loosening the hard limit.
    #[must_use]
    pub const fn baseline() -> Self {
        Self {
            maximum: DEFAULT_MAXIMUM,
        }
    }
    /// Accepts a whole-millisecond limit from one millisecond through sixty minutes.
    ///
    /// # Errors
    /// Refuses zero, sub-millisecond precision and relaxed limits.
    pub fn new(maximum: Duration) -> Result<Self, InvalidObservationAge> {
        if maximum.is_zero()
            || maximum > DEFAULT_MAXIMUM
            || !maximum.subsec_nanos().is_multiple_of(1_000_000)
        {
            return Err(InvalidObservationAge);
        }
        Ok(Self { maximum })
    }
    /// Returns the validated maximum age and confirmation lifetime.
    #[must_use]
    pub const fn maximum(self) -> Duration {
        self.maximum
    }
    /// Evaluates exact millisecond age without extending or rebinding a confirmation.
    #[must_use]
    pub fn assess(
        self,
        observed: Timestamp,
        confirmation: Option<AgeConfirmation>,
        now: Timestamp,
    ) -> AgeValidity {
        let Some(age) = now.millis().checked_sub(observed.millis()) else {
            return AgeValidity::Future;
        };
        if age > HARD_LIMIT.as_millis() {
            return AgeValidity::TooOld;
        }
        if age <= self.maximum.as_millis() {
            return AgeValidity::Fresh;
        }
        if confirmation.is_some_and(|decision| {
            decision.observed_at == observed
                && now
                    .millis()
                    .checked_sub(decision.confirmed_at.millis())
                    .is_some_and(|elapsed| elapsed <= self.maximum.as_millis())
        }) {
            AgeValidity::Confirmed
        } else {
            AgeValidity::Unconfirmed
        }
    }
}
