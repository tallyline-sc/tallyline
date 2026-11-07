// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Read-only account-check completion without receipt or replay authority.

use super::submission_job::{AttemptOutcome, JobPhase, QueueRefusal, SubmissionJob};
use crate::api::model::{SubmissionAccountCheck, SubmissionPauseReason, SubmissionResult};

fn checked(
    result: &SubmissionResult,
    reason: SubmissionPauseReason,
) -> Result<SubmissionResult, QueueRefusal> {
    match result {
        SubmissionResult::AccountRefusal {
            code,
            http_status,
            reason: original,
            check,
            outcome,
        } => {
            if !matches!(
                check,
                SubmissionAccountCheck::Pending
                    | SubmissionAccountCheck::Completed(SubmissionPauseReason::KeyCheckUnavailable)
            ) || reason == SubmissionPauseReason::UpdateRequired
                || reason == SubmissionPauseReason::AppToken
            {
                return Err(QueueRefusal::InvalidTransition);
            }
            let outcome = if matches!(outcome.as_ref(), SubmissionResult::Paused(_)) {
                SubmissionResult::Paused(reason)
            } else {
                outcome.as_ref().clone()
            };
            Ok(SubmissionResult::AccountRefusal {
                code: code.clone(),
                http_status: *http_status,
                reason: *original,
                check: SubmissionAccountCheck::Completed(reason),
                outcome: Box::new(outcome),
            })
        }
        SubmissionResult::Rechecked { code, outcome } => Ok(SubmissionResult::Rechecked {
            code: code.clone(),
            outcome: Box::new(checked(outcome, reason)?),
        }),
        SubmissionResult::PausedCode { .. }
        | SubmissionResult::DuplicateEvidence(_)
        | SubmissionResult::RateLimitedCode(_)
        | SubmissionResult::PartialError { .. }
        | SubmissionResult::NeedsOutcomeCheck { .. }
        | SubmissionResult::Rejected(_)
        | SubmissionResult::ProductionSucceeded(_)
        | SubmissionResult::ProductionAccepted(_)
        | SubmissionResult::DryRunSucceeded
        | SubmissionResult::Partial(_)
        | SubmissionResult::NotProcessed(_)
        | SubmissionResult::Paused(_)
        | SubmissionResult::VersionLockedOut(_)
        | SubmissionResult::Held(_)
        | SubmissionResult::Returned(_)
        | SubmissionResult::Unknown => Err(QueueRefusal::InvalidTransition),
    }
}
impl SubmissionJob {
    /// Commits a bound account GET result without changing receipt certainty or replay eligibility.
    ///
    /// # Errors
    /// Refuses an unrelated phase, absent account evidence or an inapplicable action.
    pub fn resolve_account_check(
        &self,
        reason: SubmissionPauseReason,
    ) -> Result<Self, QueueRefusal> {
        if matches!(
            self.phase,
            JobPhase::Queued | JobPhase::Sending | JobPhase::Succeeded
        ) {
            return Err(QueueRefusal::InvalidTransition);
        }
        let result = checked(
            self.result
                .as_ref()
                .ok_or(QueueRefusal::InvalidTransition)?,
            reason,
        )?;
        if self.result.as_ref() == Some(&result) {
            return Ok(self.clone());
        }
        let origin = self
            .attempts
            .last()
            .and_then(|attempt| attempt.account_evidence.as_ref())
            .and_then(|evidence| evidence.origin_revision())
            .ok_or(QueueRefusal::InvalidTransition)?;
        let mut next = self.advance()?;
        let attempt = next
            .attempts
            .last_mut()
            .ok_or(QueueRefusal::InvalidTransition)?;
        if matches!(result.final_result(), SubmissionResult::Paused(_)) {
            attempt.outcome = Some(AttemptOutcome::Paused(reason));
        }
        attempt.account_checks.push(reason);
        attempt.account_evidence = result
            .account_evidence()
            .map(|evidence| evidence.at_revision(origin));
        next.result = Some(result);
        Ok(next)
    }
}
