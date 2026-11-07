// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Validation and immutable completion of exactly one write-ahead attempt.

use super::submission_job::{AttemptOutcome, JobPhase, QueueRefusal, SubmissionJob};
use super::submission_result::{
    NotProcessed, SubmissionPauseReason, SubmissionResult, SubmissionReturnReason,
};
use crate::api::model::SubmissionAccountCheck;
use std::collections::BTreeSet;
use tallyline_shared_kernel::api::model::{SubmissionMode, Timestamp};

pub(super) struct CompletionDisposition {
    pub(super) phase: JobPhase,
    pub(super) outcome: AttemptOutcome,
}

impl SubmissionJob {
    /// Returns the completed result without inventing row identities.
    #[must_use]
    pub const fn result(&self) -> Option<&SubmissionResult> {
        self.result.as_ref()
    }
    /// Closes one sending attempt exactly once.
    ///
    /// # Errors
    /// Refuses a final job, invalid attempt state or dry-run success for production.
    pub fn complete(&self, result: SubmissionResult, now: Timestamp) -> Result<Self, QueueRefusal> {
        if self.phase != JobPhase::Sending {
            return Err(QueueRefusal::InvalidTransition);
        }
        if matches!(
            &result,
            SubmissionResult::AccountRefusal {
                check: SubmissionAccountCheck::Completed(_),
                ..
            }
        ) {
            return Err(QueueRefusal::InvalidTransition);
        }
        let disposition = self.completion_disposition(&result)?;
        let mut next = self.advance()?;
        let attempt = next
            .attempts
            .last_mut()
            .ok_or(QueueRefusal::InvalidTransition)?;
        next.phase = disposition.phase;
        attempt.outcome = Some(disposition.outcome);
        attempt.ended_at = Some(now);
        attempt.account_evidence = result
            .account_evidence()
            .map(|evidence| evidence.at_revision(next.revision));
        next.result = Some(result);
        Ok(next)
    }
    pub(super) fn completion_disposition(
        &self,
        result: &SubmissionResult,
    ) -> Result<CompletionDisposition, QueueRefusal> {
        if *result == SubmissionResult::DryRunSucceeded
            && self.request.header().mode != SubmissionMode::DryRun
        {
            return Err(QueueRefusal::InvalidTransition);
        }
        if self.request.header().mode == SubmissionMode::DryRun
            && matches!(result, SubmissionResult::Partial(ids) if !ids.is_empty())
        {
            return Err(QueueRefusal::InvalidTransition);
        }
        if let SubmissionResult::ProductionAccepted(ids) = result {
            let attempt = self
                .attempts
                .last()
                .ok_or(QueueRefusal::InvalidTransition)?;
            let mut seen = BTreeSet::new();
            if self.request.header().mode != SubmissionMode::Production
                || ids.len() != self.request.rows().len()
                || ids
                    .iter()
                    .any(|id| id.date_added() != attempt.date_added() || !seen.insert(id.id()))
            {
                return Err(QueueRefusal::InvalidTransition);
            }
        }
        if let SubmissionResult::ProductionSucceeded(receipts) = result {
            let sent_at = self
                .attempts
                .last()
                .ok_or(QueueRefusal::InvalidTransition)?
                .date_added();
            let mut rows = BTreeSet::new();
            let mut ids = BTreeSet::new();
            if self.request.header().mode != SubmissionMode::Production
                || receipts.len() != self.request.rows().len()
                || receipts.iter().any(|receipt| {
                    !receipt.matches_at(&self.request, sent_at)
                        || !rows.insert(receipt.row().0)
                        || !ids.insert(receipt.id())
                })
            {
                return Err(QueueRefusal::InvalidTransition);
            }
        }
        if let SubmissionResult::PartialError {
            row,
            source,
            receipts,
            ..
        } = result.final_result()
        {
            if row.is_none() != (*source == crate::api::model::RowFailureSource::Unmapped) {
                return Err(QueueRefusal::InvalidTransition);
            }
            if let Some(failed) = row
                && receipts.iter().any(|proof| {
                    self.request()
                        .rows()
                        .iter()
                        .position(|row| row.id() == proof.row())
                        .is_none_or(|index| index >= *failed)
                })
            {
                return Err(QueueRefusal::InvalidTransition);
            }
            if !receipts.is_empty() {
                self.validate_row_receipts(receipts)?;
            }
        }
        if matches!(result, SubmissionResult::PartialError { row: Some(row), .. } if *row >= self.request.rows().len())
        {
            return Err(QueueRefusal::InvalidTransition);
        }
        let (phase, outcome) = match result {
            SubmissionResult::AccountRefusal {
                http_status,
                reason,
                outcome,
                code,
                check,
            } => {
                let action = match check {
                    SubmissionAccountCheck::Completed(action) => action,
                    SubmissionAccountCheck::NotRequired | SubmissionAccountCheck::Pending => reason,
                };
                let consistent = match outcome.as_ref() {
                    SubmissionResult::NeedsOutcomeCheck {
                        code: receipt_code,
                        http_status: receipt_status,
                    } => code.as_ref() == Some(receipt_code) && http_status == receipt_status,
                    SubmissionResult::PartialError {
                        code: receipt_code, ..
                    } => code.as_ref() == Some(receipt_code),
                    SubmissionResult::Rejected(refusal) => code.as_ref() == Some(refusal.code()),
                    SubmissionResult::Paused(receipt_reason) => action == receipt_reason,
                    SubmissionResult::PausedCode {
                        code: receipt_code,
                        reason: receipt_reason,
                    } => code.as_ref() == Some(receipt_code) && action == receipt_reason,
                    SubmissionResult::Unknown => code.is_none(),
                    SubmissionResult::AccountRefusal { .. }
                    | SubmissionResult::Rechecked { .. }
                    | SubmissionResult::DuplicateEvidence(_)
                    | SubmissionResult::RateLimitedCode(_)
                    | SubmissionResult::ProductionSucceeded(_)
                    | SubmissionResult::ProductionAccepted(_)
                    | SubmissionResult::DryRunSucceeded
                    | SubmissionResult::Partial(_)
                    | SubmissionResult::NotProcessed(_)
                    | SubmissionResult::VersionLockedOut(_)
                    | SubmissionResult::Held(_)
                    | SubmissionResult::Returned(_) => false,
                };
                if !consistent
                    || !(100..=599).contains(http_status)
                    || *reason == SubmissionPauseReason::UpdateRequired
                    || !matches!(
                        outcome.as_ref(),
                        SubmissionResult::NeedsOutcomeCheck { .. }
                            | SubmissionResult::PartialError { .. }
                            | SubmissionResult::Unknown
                            | SubmissionResult::Rejected(_)
                            | SubmissionResult::Paused(_)
                            | SubmissionResult::PausedCode { .. }
                    )
                {
                    return Err(QueueRefusal::InvalidTransition);
                }
                return self.completion_disposition(outcome);
            }
            SubmissionResult::DryRunSucceeded => {
                (JobPhase::Succeeded, AttemptOutcome::DryRunSucceeded)
            }
            SubmissionResult::ProductionSucceeded(_) | SubmissionResult::ProductionAccepted(_) => {
                (JobPhase::Succeeded, AttemptOutcome::ProductionSucceeded)
            }
            SubmissionResult::Partial(_) | SubmissionResult::PartialError { .. } => {
                (JobPhase::PartiallyAccepted, AttemptOutcome::Partial)
            }
            SubmissionResult::NeedsOutcomeCheck { .. } => {
                (JobPhase::Held, AttemptOutcome::OutcomeCheckRequired)
            }
            SubmissionResult::Rechecked { .. } => return Err(QueueRefusal::InvalidTransition),
            SubmissionResult::Unknown => (JobPhase::OutcomeUnknown, AttemptOutcome::Unknown),
            SubmissionResult::Rejected(_) => (JobPhase::Rejected, AttemptOutcome::Rejected),
            SubmissionResult::PausedCode { reason, .. } => {
                (JobPhase::Paused, AttemptOutcome::Paused(*reason))
            }
            SubmissionResult::VersionLockedOut(version) => {
                (JobPhase::Paused, AttemptOutcome::VersionLockedOut(*version))
            }
            SubmissionResult::Paused(reason) => (JobPhase::Paused, AttemptOutcome::Paused(*reason)),
            SubmissionResult::Held(reason) => (JobPhase::Held, AttemptOutcome::Held(*reason)),
            SubmissionResult::DuplicateEvidence(previous)
                if previous.0 > 0 && *previous != self.id =>
            {
                (
                    JobPhase::Returned,
                    AttemptOutcome::Returned(SubmissionReturnReason::PossiblyAlreadyReceived),
                )
            }
            SubmissionResult::DuplicateEvidence(_) => return Err(QueueRefusal::InvalidTransition),
            SubmissionResult::RateLimitedCode(_) => (
                JobPhase::Held,
                AttemptOutcome::NotProcessed(NotProcessed::RateLimited),
            ),
            SubmissionResult::NotProcessed(reason) => {
                (JobPhase::Held, AttemptOutcome::NotProcessed(*reason))
            }
            SubmissionResult::Returned(reason) => {
                (JobPhase::Returned, AttemptOutcome::Returned(*reason))
            }
        };
        Ok(CompletionDisposition { phase, outcome })
    }
}
