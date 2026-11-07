// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Immutable submission phases and write-ahead attempts.

use super::submission_result::{
    NotProcessed, SubmissionHoldReason, SubmissionPauseReason, SubmissionResult,
    SubmissionReturnReason,
};
use crate::api::model::SubmissionResolutionAction;
use crate::api::model::{InvalidSubmission, SendReturnReason, SubmissionRequest};
use crate::api::model::{SubmissionAccountCheck, SubmissionAccountEvidence};
use tallyline_shared_kernel::api::model::Timestamp;

pub use tallyline_shared_kernel::api::model::SubmissionJobId as JobId;

/// SHA-256 of the exact prepared request bytes, without those bytes in diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PayloadHash([u8; 32]);
impl PayloadHash {
    /// Wraps an adapter-computed digest.
    #[must_use]
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    /// Returns the digest for durable storage.
    #[must_use]
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// The durable phase of a single submission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobPhase {
    /// Released and awaiting dispatch.
    Queued,
    /// Every request row was accepted; a test request creates no UEX rows.
    Succeeded,
    /// Some rows may be stored and require individual resolution.
    PartiallyAccepted,
    /// An attempt was committed before network dispatch.
    Sending,
    /// Sending may have stored rows and must never be replayed automatically.
    OutcomeUnknown,
    /// A permanent refusal ended this immutable release.
    Rejected,
    /// The Report requires renewed release.
    Returned,
    /// The user cancelled before sending began.
    Cancelled,
    /// An account or client-token refusal requires an explicit user action.
    Paused,
    /// A known unprocessed attempt awaits an explicit scheduling decision.
    Held,
}

/// A command refused without changing a submission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueueRefusal {
    /// User cancellation is forbidden after dispatch begins.
    AlreadySending,
    /// This command cannot apply to the current phase.
    InvalidTransition,
    /// The prepared time is later than the observation or is not a whole second.
    InvalidPreparedPayload,
}

/// The durable result of one started attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttemptOutcome {
    /// A production refusal awaits a read-only safety check.
    OutcomeCheckRequired,
    /// Every production row was accepted, with any unverified identities kept unmapped.
    ProductionSucceeded,
    /// Processing cannot be excluded.
    Unknown,
    /// UEX permanently refused the request.
    Rejected,
    /// UEX validated a test request without storing any row.
    DryRunSucceeded,
    /// Some rows may have been stored without a complete mapping.
    Partial,
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
}

/// A write-ahead record retained independently of restart recovery.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubmissionAttempt {
    pub(super) started_at: Timestamp,
    pub(super) rows: usize,
    pub(super) payload_hash: PayloadHash,
    pub(super) date_added: Timestamp,
    pub(super) ended_at: Option<Timestamp>,
    pub(super) outcome: Option<AttemptOutcome>,
    pub(super) receipt_checked: bool,
    pub(super) account_checks: Vec<SubmissionPauseReason>,
    pub(super) account_evidence: Option<SubmissionAccountEvidence>,
    pub(super) client_update: Option<tallyline_shared_kernel::api::model::ReleaseVersion>,
}
impl SubmissionAttempt {
    /// Borrows original account refusal evidence retained for this exact attempt.
    #[must_use]
    pub const fn account_evidence(&self) -> Option<&SubmissionAccountEvidence> {
        self.account_evidence.as_ref()
    }
    /// Borrows changed account-check actions in their validated transition order.
    #[must_use]
    pub fn account_checks(&self) -> &[SubmissionPauseReason] {
        &self.account_checks
    }
    /// Returns the newer compiled release that authorised a preceding version-lockout retry.
    #[must_use]
    pub const fn client_update(
        &self,
    ) -> Option<tallyline_shared_kernel::api::model::ReleaseVersion> {
        self.client_update
    }
    /// Returns the committed attempt start.
    #[must_use]
    pub const fn started_at(&self) -> Timestamp {
        self.started_at
    }
    /// Returns the complete request row count.
    #[must_use]
    pub const fn rows(&self) -> usize {
        self.rows
    }
    /// Returns the digest of the exact request bytes.
    #[must_use]
    pub const fn payload_hash(&self) -> PayloadHash {
        self.payload_hash
    }
    /// Returns the exact whole-second timestamp encoded in the attempted body.
    #[must_use]
    pub const fn date_added(&self) -> Timestamp {
        self.date_added
    }
    /// Reports whether a held refusal was resolved by a later read-only receipt check.
    #[must_use]
    pub const fn receipt_checked(&self) -> bool {
        self.receipt_checked
    }
    /// Returns the recorded attempt completion time.
    #[must_use]
    pub const fn ended_at(&self) -> Option<Timestamp> {
        self.ended_at
    }
    /// Returns the completion outcome, absent while sending.
    #[must_use]
    pub const fn outcome(&self) -> Option<AttemptOutcome> {
        self.outcome
    }
}

/// A credential-free immutable submission and its durable attempts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubmissionJob {
    pub(super) id: JobId,
    pub(super) revision: u64,
    pub(super) request: SubmissionRequest,
    pub(super) phase: JobPhase,
    pub(super) attempts: Vec<SubmissionAttempt>,
    pub(super) result: Option<SubmissionResult>,
    pub(super) review_reason: Option<SendReturnReason>,
    pub(super) service_pause_acknowledged: bool,
    pub(super) client_update: Option<tallyline_shared_kernel::api::model::ReleaseVersion>,
    pub(crate) resolution_actions: Vec<SubmissionResolutionAction>,
}
impl SubmissionJob {
    /// Creates a queued job for a valid released snapshot.
    ///
    /// # Errors
    /// Refuses a missing job identity.
    pub fn queued(id: JobId, request: SubmissionRequest) -> Result<Self, InvalidSubmission> {
        if id.0 == 0 {
            return Err(InvalidSubmission);
        }
        Ok(Self {
            id,
            revision: 1,
            request,
            phase: JobPhase::Queued,
            attempts: Vec::new(),
            result: None,
            review_reason: None,
            service_pause_acknowledged: false,
            client_update: None,
            resolution_actions: Vec::new(),
        })
    }
    /// Returns the stable local identity.
    #[must_use]
    pub const fn id(&self) -> JobId {
        self.id
    }
    /// Returns the optimistic persistence version of this immutable job.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }
    pub(crate) fn advance(&self) -> Result<Self, QueueRefusal> {
        let mut next = self.clone();
        next.revision = self
            .revision
            .checked_add(1)
            .ok_or(QueueRefusal::InvalidTransition)?;
        Ok(next)
    }
    /// Borrows the fixed release snapshot.
    #[must_use]
    pub const fn request(&self) -> &SubmissionRequest {
        &self.request
    }
    /// Returns the current durable phase.
    #[must_use]
    pub const fn phase(&self) -> JobPhase {
        self.phase
    }
    pub(crate) fn observation_order_phase(&self) -> JobPhase {
        if matches!(
            self.result.as_ref().map(SubmissionResult::final_result),
            Some(SubmissionResult::NeedsOutcomeCheck { .. })
        ) {
            JobPhase::OutcomeUnknown
        } else {
            self.phase
        }
    }
    /// Borrows every write-ahead attempt.
    #[must_use]
    pub fn attempts(&self) -> &[SubmissionAttempt] {
        &self.attempts
    }
    /// Starts one attempt; the repository must commit it before network dispatch.
    ///
    /// # Errors
    /// Refuses every phase except queued.
    pub fn begin(&self, now: Timestamp, payload_hash: PayloadHash) -> Result<Self, QueueRefusal> {
        self.begin_prepared(
            now,
            payload_hash,
            Timestamp::from_seconds(self.request.header().observed_at.seconds()),
        )
    }
    /// Starts an attempt whose recorded timestamp exactly matches the prepared body.
    ///
    /// # Errors
    /// Refuses non-queued jobs, fractional seconds and a later time than the released observation.
    pub fn begin_prepared(
        &self,
        now: Timestamp,
        payload_hash: PayloadHash,
        date_added: Timestamp,
    ) -> Result<Self, QueueRefusal> {
        if self.phase != JobPhase::Queued {
            return Err(QueueRefusal::InvalidTransition);
        }
        if date_added > self.request.header().observed_at
            || !date_added.millis().is_multiple_of(1000)
        {
            return Err(QueueRefusal::InvalidPreparedPayload);
        }
        let mut next = self.advance()?;
        next.phase = JobPhase::Sending;
        next.attempts.push(SubmissionAttempt {
            started_at: now,
            rows: self.request.rows().len(),
            payload_hash,
            date_added,
            ended_at: None,
            outcome: None,
            receipt_checked: false,
            account_checks: Vec::new(),
            account_evidence: None,
            client_update: self.client_update,
        });
        Ok(next)
    }
    /// Reports whether the user acknowledged this retained recoverable service cause.
    #[must_use]
    pub const fn service_pause_acknowledged(&self) -> bool {
        self.service_pause_acknowledged
    }
    /// Reports whether explicit queue retry can acknowledge a final recoverable service cause.
    #[must_use]
    pub fn can_acknowledge_service_pause(&self) -> bool {
        if self.account_check_pending() {
            return false;
        }
        if self.service_pause_acknowledged {
            return false;
        }
        self.result
            .as_ref()
            .and_then(SubmissionResult::account_pause)
            .is_some()
            && matches!(
                self.phase,
                JobPhase::Held
                    | JobPhase::PartiallyAccepted
                    | JobPhase::OutcomeUnknown
                    | JobPhase::Rejected
            )
            || matches!(self.phase, JobPhase::Cancelled | JobPhase::Returned)
                && matches!(
                    self.result.as_ref().map(SubmissionResult::final_result),
                    Some(
                        SubmissionResult::Held(SubmissionHoldReason::ApiChanged)
                            | SubmissionResult::Paused(
                                SubmissionPauseReason::AppToken
                                    | SubmissionPauseReason::KeyChecked
                                    | SubmissionPauseReason::Reauthenticate
                                    | SubmissionPauseReason::KeyCheckUnavailable
                            )
                            | SubmissionResult::PausedCode {
                                reason: SubmissionPauseReason::AppToken
                                    | SubmissionPauseReason::KeyChecked
                                    | SubmissionPauseReason::Reauthenticate
                                    | SubmissionPauseReason::KeyCheckUnavailable,
                                ..
                            }
                    )
                )
    }
    fn account_check_pending(&self) -> bool {
        matches!(
            self.attempts
                .last()
                .and_then(|attempt| attempt.account_evidence.as_ref())
                .map(|evidence| evidence.check()),
            Some(SubmissionAccountCheck::Pending)
        )
    }
    /// Reports whether explicit retry or queue acknowledgement is available.
    #[must_use]
    pub fn can_retry(&self) -> bool {
        if self.service_pause_acknowledged {
            return false;
        }
        if self.account_check_pending() {
            return false;
        }
        self.can_acknowledge_service_pause()
            || matches!(self.phase, JobPhase::Held | JobPhase::Paused)
                && matches!(
                    self.result.as_ref().map(SubmissionResult::final_result),
                    Some(
                        SubmissionResult::NotProcessed(_)
                            | SubmissionResult::Held(_)
                            | SubmissionResult::Paused(_)
                            | SubmissionResult::RateLimitedCode(_)
                            | SubmissionResult::PausedCode { .. }
                    )
                )
                && !matches!(
                    self.result.as_ref().map(SubmissionResult::final_result),
                    Some(
                        SubmissionResult::NotProcessed(
                            NotProcessed::DailyQuota | NotProcessed::DailyQuotaUntil(_)
                        ) | SubmissionResult::Paused(SubmissionPauseReason::UpdateRequired)
                            | SubmissionResult::PausedCode {
                                reason: SubmissionPauseReason::UpdateRequired,
                                ..
                            }
                    )
                )
    }
    /// Retries an eligible request or acknowledges only its global service cause.
    ///
    /// # Errors
    /// Refuses origin replay after queue-only acknowledgement and ineligible outcomes.
    pub fn retry(&self) -> Result<Self, QueueRefusal> {
        if self.can_acknowledge_service_pause() {
            let mut next = self.advance()?;
            next.service_pause_acknowledged = true;
            return Ok(next);
        }
        if !self.can_retry() {
            return Err(QueueRefusal::InvalidTransition);
        }
        let mut next = self.advance()?;
        next.phase = JobPhase::Queued;
        next.result = None;
        Ok(next)
    }
    /// Returns a quota-held request to the complete gate only after a valid due reset.
    ///
    /// # Errors
    /// Refuses missing, stale or premature deadlines and exhausted automatic attempts.
    pub fn resume_quota(&self, now: Timestamp) -> Result<Self, QueueRefusal> {
        let reset = crate::api::model::SubmissionRetrySettings::default()
            .quota_retry_at(self)
            .ok_or(QueueRefusal::InvalidTransition)?;
        if now < reset {
            return Err(QueueRefusal::InvalidTransition);
        }
        let mut next = self.advance()?;
        next.phase = JobPhase::Queued;
        next.result = None;
        Ok(next)
    }
    /// Cancels only a queued request.
    ///
    /// # Errors
    /// Refuses sending and final phases.
    pub fn cancel(&self) -> Result<Self, QueueRefusal> {
        if matches!(
            self.result.as_ref().map(SubmissionResult::final_result),
            Some(SubmissionResult::NeedsOutcomeCheck { .. })
        ) {
            return Err(QueueRefusal::InvalidTransition);
        }
        match self.phase {
            JobPhase::Sending => Err(QueueRefusal::AlreadySending),
            JobPhase::Queued | JobPhase::Held | JobPhase::Paused => {
                let mut next = self.advance()?;
                next.phase = JobPhase::Cancelled;
                Ok(next)
            }
            JobPhase::OutcomeUnknown
            | JobPhase::Rejected
            | JobPhase::Returned
            | JobPhase::Cancelled
            | JobPhase::Succeeded
            | JobPhase::PartiallyAccepted => Err(QueueRefusal::InvalidTransition),
        }
    }
    /// Returns the durable preflight reason requiring renewed manual release.
    #[must_use]
    pub const fn review_reason(&self) -> Option<SendReturnReason> {
        self.review_reason
    }
    /// Closes an unsent job after a report-fixable preflight refusal.
    ///
    /// # Errors
    /// Refuses sending and final phases without changing attempts or the released snapshot.
    pub fn return_for_review(&self, reason: SendReturnReason) -> Result<Self, QueueRefusal> {
        match self.phase {
            JobPhase::Queued | JobPhase::Held | JobPhase::Paused => {
                let mut next = self.advance()?;
                next.phase = JobPhase::Returned;
                next.review_reason = Some(reason);
                Ok(next)
            }
            JobPhase::Sending => Err(QueueRefusal::AlreadySending),
            JobPhase::OutcomeUnknown
            | JobPhase::Rejected
            | JobPhase::Returned
            | JobPhase::Cancelled
            | JobPhase::Succeeded
            | JobPhase::PartiallyAccepted => Err(QueueRefusal::InvalidTransition),
        }
    }
    /// Closes interrupted work without ever making it dispatchable again.
    ///
    /// # Errors
    /// Refuses corrupt sending state without a write-ahead attempt.
    pub fn recover(&self, now: Timestamp) -> Result<Self, QueueRefusal> {
        let mut next = self.clone();
        if matches!(
            self.result.as_ref().map(SubmissionResult::final_result),
            Some(SubmissionResult::NeedsOutcomeCheck { .. })
        ) {
            return Ok(next);
        }
        match self.phase {
            JobPhase::Queued | JobPhase::Held | JobPhase::Paused => {
                next = self.advance()?;
                next.phase = JobPhase::Returned;
            }
            JobPhase::Sending => {
                next = self.advance()?;
                next.phase = JobPhase::OutcomeUnknown;
                let attempt = next
                    .attempts
                    .last_mut()
                    .ok_or(QueueRefusal::InvalidTransition)?;
                attempt.outcome = Some(AttemptOutcome::Unknown);
                next.result = Some(SubmissionResult::Unknown);
                attempt.ended_at = Some(now);
            }
            JobPhase::OutcomeUnknown
            | JobPhase::Rejected
            | JobPhase::Returned
            | JobPhase::Cancelled
            | JobPhase::Succeeded
            | JobPhase::PartiallyAccepted => return Ok(next),
        }
        Ok(next)
    }
}
