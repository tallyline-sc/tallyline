// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Closed transport results and durable recovery reasons.

use super::submission_job::JobId;
use crate::api::model::{SubmissionAccountCheck, SubmissionAccountEvidence, SubmissionRejection};
use crate::api::model::{UexErrorCode, VerifiedUexReport};
use tallyline_shared_kernel::api::model::UexReportId;

/// A closed transport result without speculative UEX row identities.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubmissionResult {
    /// An account refusal remains independent of receipt certainty.
    AccountRefusal {
        /// The original safe technical code; unsafe identifiers are absent.
        code: Option<UexErrorCode>,
        /// The original HTTP status without remote message text.
        http_status: u16,
        /// The explicit account recovery action.
        reason: SubmissionPauseReason,
        /// Progress of the separate account read.
        check: SubmissionAccountCheck,
        /// The independently established receipt disposition and safe technical code.
        outcome: Box<SubmissionResult>,
    },
    /// An unlisted authentication code retains its confirmed recovery action.
    PausedCode {
        /// The original bounded technical identifier.
        code: UexErrorCode,
        /// The read-only account check result.
        reason: SubmissionPauseReason,
    },
    /// A duplicate is evidence that this earlier uncertain attempt may have arrived.
    DuplicateEvidence(JobId),
    /// An unlisted HTTP-429 status remains visible while processing was excluded.
    RateLimitedCode(UexErrorCode),
    /// A held refusal was resolved by a later read-only receipt check.
    Rechecked {
        /// The original bounded technical status identifier.
        code: UexErrorCode,
        /// The confirmed refusal or partial-acceptance class.
        outcome: Box<SubmissionResult>,
    },
    /// A rejected production request may have stored earlier rows.
    PartialError {
        /// The exact bounded technical identifier.
        code: UexErrorCode,
        /// Zero-based failed row when UEX identified it.
        row: Option<usize>,
        /// The exact identification provenance, absent only when no row was identified.
        source: crate::api::model::RowFailureSource,
        /// Positive identities returned by the original refusal safety read.
        receipts: Vec<VerifiedUexReport>,
    },
    /// The read-only safety lookup must succeed before a refusal takes effect.
    NeedsOutcomeCheck {
        /// The bounded original UEX status identifier.
        code: UexErrorCode,
        /// The original status needed to classify an unlisted authentication refusal.
        http_status: u16,
    },
    /// A permanent refusal retains its bounded technical code for an actionable message.
    Rejected(SubmissionRejection),
    /// Every positive remote identity was verified against its released row.
    ProductionSucceeded(Vec<VerifiedUexReport>),
    /// UEX accepted every row; positive returned identities remain unmapped until verified.
    ProductionAccepted(Vec<UexReportId>),
    /// Test validation succeeded and created no UEX reports.
    DryRunSucceeded,
    /// Rows may have been stored and need individual resolution.
    Partial(Vec<UexReportId>),
    /// The adapter established that no row could have been processed.
    NotProcessed(NotProcessed),
    /// An account or client-token refusal requires an explicit user action.
    Paused(SubmissionPauseReason),
    /// UEX locked out this compiled stable release; absence is unverified provenance.
    VersionLockedOut(Option<tallyline_shared_kernel::api::model::ReleaseVersion>),
    /// A documented service gate holds dispatch without returning the Report.
    Held(SubmissionHoldReason),
    /// A closed report-fixable refusal requires renewed release.
    Returned(SubmissionReturnReason),
    /// Processing cannot be excluded.
    Unknown,
}
impl SubmissionResult {
    /// Returns the supplied token quota reset without granting retry authority.
    #[must_use]
    pub fn daily_quota_reset(&self) -> Option<tallyline_shared_kernel::api::model::Timestamp> {
        if let Self::NotProcessed(NotProcessed::DailyQuotaUntil(evidence)) = self.final_result() {
            Some(evidence.reset())
        } else {
            None
        }
    }
    /// Returns the exact response's server time without substituting local persistence time.
    #[must_use]
    pub fn daily_quota_observed_at(
        &self,
    ) -> Option<tallyline_shared_kernel::api::model::Timestamp> {
        if let Self::NotProcessed(NotProcessed::DailyQuotaUntil(evidence)) = self.final_result() {
            evidence.observed_at()
        } else {
            None
        }
    }
    /// Reports a proven unprocessed app-token quota refusal.
    #[must_use]
    pub fn is_daily_quota(&self) -> bool {
        matches!(
            self.final_result(),
            Self::NotProcessed(NotProcessed::DailyQuota | NotProcessed::DailyQuotaUntil(_))
        )
    }

    /// Retains original account provenance independently of the receipt class.
    #[must_use]
    pub fn account_evidence(&self) -> Option<SubmissionAccountEvidence> {
        match self {
            Self::AccountRefusal {
                code,
                http_status,
                reason,
                check,
                ..
            } => Some(SubmissionAccountEvidence::new(
                code.clone(),
                *http_status,
                *reason,
                *check,
                None,
            )),
            Self::Rechecked { outcome, .. } => outcome.account_evidence(),
            Self::PausedCode { .. }
            | Self::DuplicateEvidence(_)
            | Self::RateLimitedCode(_)
            | Self::PartialError { .. }
            | Self::NeedsOutcomeCheck { .. }
            | Self::Rejected(_)
            | Self::ProductionSucceeded(_)
            | Self::ProductionAccepted(_)
            | Self::DryRunSucceeded
            | Self::Partial(_)
            | Self::NotProcessed(_)
            | Self::Paused(_)
            | Self::VersionLockedOut(_)
            | Self::Held(_)
            | Self::Returned(_)
            | Self::Unknown => None,
        }
    }
    pub(crate) fn pending_account_check(&self) -> Self {
        if let Self::AccountRefusal {
            code,
            http_status,
            reason,
            check,
            outcome,
        } = self
        {
            let evidence =
                SubmissionAccountEvidence::new(code.clone(), *http_status, *reason, *check, None);
            let outcome = if matches!(outcome.as_ref(), Self::Paused(_)) {
                Self::Paused(evidence.original_action())
            } else {
                outcome.as_ref().clone()
            };
            evidence.pending().wrap(outcome)
        } else {
            self.clone()
        }
    }
    /// Returns a global account action independently of the receipt disposition.
    #[must_use]
    pub fn account_pause(&self) -> Option<SubmissionPauseReason> {
        match self {
            Self::AccountRefusal { reason, check, .. } => Some(match check {
                SubmissionAccountCheck::Completed(action) => *action,
                SubmissionAccountCheck::NotRequired | SubmissionAccountCheck::Pending => *reason,
            }),
            Self::Paused(reason) | Self::PausedCode { reason, .. } => Some(*reason),
            Self::VersionLockedOut(_) => Some(SubmissionPauseReason::UpdateRequired),
            Self::Rechecked { outcome, .. } => outcome.account_pause(),
            Self::DuplicateEvidence(_)
            | Self::RateLimitedCode(_)
            | Self::PartialError { .. }
            | Self::NeedsOutcomeCheck { .. }
            | Self::Rejected(_)
            | Self::ProductionSucceeded(_)
            | Self::ProductionAccepted(_)
            | Self::DryRunSucceeded
            | Self::Partial(_)
            | Self::NotProcessed(_)
            | Self::Held(_)
            | Self::Returned(_)
            | Self::Unknown => None,
        }
    }
    /// Borrows the final class after a retained read-only check, if one completed.
    #[must_use]
    pub fn final_result(&self) -> &Self {
        match self {
            Self::Rechecked { outcome, .. } | Self::AccountRefusal { outcome, .. } => {
                outcome.final_result()
            }
            Self::VersionLockedOut(_)
            | Self::PausedCode { .. }
            | Self::DuplicateEvidence(_)
            | Self::RateLimitedCode(_)
            | Self::PartialError { .. }
            | Self::NeedsOutcomeCheck { .. }
            | Self::Rejected(_)
            | Self::ProductionSucceeded(_)
            | Self::ProductionAccepted(_)
            | Self::DryRunSucceeded
            | Self::Partial(_)
            | Self::NotProcessed(_)
            | Self::Paused(_)
            | Self::Held(_)
            | Self::Returned(_)
            | Self::Unknown => self,
        }
    }
}
/// Verified failures that precede storage of any Report row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotProcessed {
    /// Connection refusal or a connect-phase timeout before HTTP request writing.
    ConnectFailure,
    /// UEX refused the request before any row was processed.
    ServiceUnavailable,
    /// HTTP rate limiting refused the request before processing.
    RateLimited,
    /// The account row window refused the complete request.
    RowBudget,
    /// The screenshot upload failed before processing.
    ImageUpload,
    /// The screenshot could not be stored before processing.
    ImageStorage,
    /// The shared token quota is exhausted; reset scheduling remains closed.
    DailyQuota,
    /// The token quota is exhausted until the supplied Unix instant.
    DailyQuotaUntil(crate::api::model::TokenQuotaReset),
}

/// A report-fixable outcome that requires a new release decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubmissionReturnReason {
    /// A duplicate followed an uncertain attempt of a shared row key.
    PossiblyAlreadyReceived,
    /// The account must attach confirmed private screenshot evidence.
    ScreenshotRequired,
    /// The application proved that final checks failed before any HTTP request started.
    PreflightChanged,
    /// UEX refused the encoded observation time; review the time and system clock.
    InvalidObservationTime,
    /// The Report exceeds the accepted row limit.
    MaxRowsExceeded,
    /// UEX no longer recognises the selected terminal.
    TerminalNotFound,
    /// The screenshot encoding was refused.
    InvalidScreenshot,
    /// The screenshot exceeds the accepted encoded size.
    ScreenshotTooLarge,
}

/// A closed account recovery action without remote message text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubmissionPauseReason {
    /// The app token is missing or invalid.
    AppToken,
    /// UEX refuses this application version until it is updated.
    UpdateRequired,
    /// A read-only account check confirmed the current account.
    KeyChecked,
    /// A read-only account check refused the current account.
    Reauthenticate,
    /// The read-only account check could not establish the current account.
    KeyCheckUnavailable,
}

/// A durable gate that cannot authorise an automatic POST.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubmissionHoldReason {
    /// UEX does not currently accept this report target.
    AcceptanceClosed,
    /// The first row met an existing same-account commodity cooldown.
    Cooldown,
    /// The write endpoint returned a non-JSON 404.
    ApiChanged,
}
