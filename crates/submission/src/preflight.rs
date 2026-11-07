// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Dispatch-time account, acceptance, version and age checks.

use crate::api::model::{AccountState, SubmissionRequest};
use tallyline_shared_kernel::api::model::{
    AgeConfirmation, AgeValidity, ObservationAgeSettings, Timestamp, UexEnvironment,
};

const MAX_REFERENCE_AGE_MILLIS: u128 = 15 * 60 * 1000;

/// Verified acceptance of commodity submissions in the selected target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubmissionAcceptance {
    /// The datacentre, environment and commodity type currently accept reports.
    Open,
    /// At least one verified acceptance flag is closed.
    Closed,
}

/// Independently dated version and acceptance facts from the reference application.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SendReferenceState {
    /// The explicitly mapped environment of these facts.
    pub environment: UexEnvironment,
    /// The newest successfully observed version, absent when unknown.
    pub version: Option<String>,
    /// The successful fetch time of the version endpoint.
    pub versions_fetched_at: Timestamp,
    /// The combined datacentre, environment and commodity acceptance.
    pub acceptance: SubmissionAcceptance,
    /// The successful fetch time of the parameter endpoint.
    pub parameters_fetched_at: Timestamp,
}

/// A held request can remain durable but must not send yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SendHoldReason {
    /// No currently validated credential standing is available.
    AccountUnavailable,
    /// The accepted credentials belong to another account.
    AccountChanged,
    /// Current matching-environment version or parameter facts are unavailable.
    ReferenceUnavailable,
    /// A verified acceptance flag is closed.
    AcceptanceClosed,
}

/// A dispatch refusal that requires renewed Report release.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SendReturnReason {
    /// Fresh numerical, history, attachment or context decisions require renewed review.
    ReviewRequired,
    /// The selected observation version no longer matches the newest observed version.
    GameVersionChanged,
    /// The observation needs a fresh value-bound age decision.
    ObservationAgeUnconfirmed,
    /// The fixed observation-age hard limit has expired.
    ObservationTooOld,
    /// The observation lies after the current wall clock.
    ObservationInFuture,
}

/// Account and reference preconditions, independent of fresh numeric and attachment validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SendReadiness {
    /// These preconditions pass for this exact request snapshot.
    Ready,
    /// Dispatch must wait without changing account, mode or values.
    Held(SendHoldReason),
    /// The Report must return to Draft for renewed release.
    Returned(SendReturnReason),
}

/// Rechecks immutable release metadata immediately before reserving an attempt.
pub struct SubmissionPreflight;
impl SubmissionPreflight {
    /// Checks account binding, independent reference freshness, acceptance, version and age.
    #[must_use]
    pub fn evaluate(
        request: &SubmissionRequest,
        account: &AccountState,
        references: Option<&SendReferenceState>,
        confirmation: Option<AgeConfirmation>,
        age_settings: ObservationAgeSettings,
        now: Timestamp,
    ) -> SendReadiness {
        let username = match account {
            AccountState::Validated { username, .. } => username,
            AccountState::Unconfigured | AccountState::Rejected(_) => {
                return SendReadiness::Held(SendHoldReason::AccountUnavailable);
            }
        };
        if username != &request.header().account {
            return SendReadiness::Held(SendHoldReason::AccountChanged);
        }
        let Some(references) = references.filter(|facts| {
            facts.environment == request.header().environment
                && fresh(facts.versions_fetched_at, now)
                && fresh(facts.parameters_fetched_at, now)
        }) else {
            return SendReadiness::Held(SendHoldReason::ReferenceUnavailable);
        };
        if references.acceptance == SubmissionAcceptance::Closed {
            return SendReadiness::Held(SendHoldReason::AcceptanceClosed);
        }
        let Some(version) = references
            .version
            .as_deref()
            .filter(|version| !version.is_empty())
        else {
            return SendReadiness::Held(SendHoldReason::ReferenceUnavailable);
        };
        if version != request.header().game_version {
            return SendReadiness::Returned(SendReturnReason::GameVersionChanged);
        }
        match age_settings.assess(request.header().observed_at, confirmation, now) {
            AgeValidity::Fresh | AgeValidity::Confirmed => SendReadiness::Ready,
            AgeValidity::Unconfirmed => {
                SendReadiness::Returned(SendReturnReason::ObservationAgeUnconfirmed)
            }
            AgeValidity::TooOld => SendReadiness::Returned(SendReturnReason::ObservationTooOld),
            AgeValidity::Future => SendReadiness::Returned(SendReturnReason::ObservationInFuture),
        }
    }
}
fn fresh(at: Timestamp, now: Timestamp) -> bool {
    now.millis()
        .checked_sub(at.millis())
        .is_some_and(|age| age <= MAX_REFERENCE_AGE_MILLIS)
}
