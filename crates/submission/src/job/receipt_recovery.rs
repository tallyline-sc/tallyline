// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Resolution of a held refusal by a later read-only receipt check.

use super::submission_job::{JobPhase, QueueRefusal, SubmissionJob};
use super::submission_result::SubmissionResult;

impl SubmissionJob {
    /// Resolves a held receipt check without creating or replaying an attempt.
    ///
    /// # Errors
    /// Refuses another phase, a pending check, nested outcomes or fabricated acceptance.
    pub fn resolve_outcome_check(&self, result: SubmissionResult) -> Result<Self, QueueRefusal> {
        let Some(SubmissionResult::NeedsOutcomeCheck { code, .. }) =
            self.result.as_ref().map(SubmissionResult::final_result)
        else {
            return Err(QueueRefusal::InvalidTransition);
        };
        if self.phase != JobPhase::Held
            || !matches!(
                result.final_result(),
                SubmissionResult::Rejected(_)
                    | SubmissionResult::PartialError { .. }
                    | SubmissionResult::Returned(_)
                    | SubmissionResult::Paused(_)
                    | SubmissionResult::PausedCode { .. }
            )
        {
            return Err(QueueRefusal::InvalidTransition);
        }
        let result = if let Some(SubmissionResult::AccountRefusal {
            code: original_code,
            http_status: original_status,
            reason: original_reason,
            check: original_check,
            ..
        }) = self.result.as_ref()
        {
            if let SubmissionResult::AccountRefusal {
                code, http_status, ..
            } = &result
            {
                if code != original_code || http_status != original_status {
                    return Err(QueueRefusal::InvalidTransition);
                }
                result
            } else {
                SubmissionResult::AccountRefusal {
                    code: original_code.clone(),
                    http_status: *original_status,
                    reason: *original_reason,
                    check: *original_check,
                    outcome: Box::new(result),
                }
            }
        } else {
            result
        };
        let code = code.clone();
        let disposition = self.completion_disposition(&result)?;
        let mut next = self.advance()?;
        next.phase = disposition.phase;
        let attempt = next
            .attempts
            .last_mut()
            .ok_or(QueueRefusal::InvalidTransition)?;
        attempt.outcome = Some(disposition.outcome);
        attempt.receipt_checked = true;
        let origin = attempt
            .account_evidence
            .as_ref()
            .and_then(|evidence| evidence.origin_revision());
        next.result = Some(SubmissionResult::Rechecked {
            code,
            outcome: Box::new(result),
        });
        if let Some(origin) = origin {
            next.attempts
                .last_mut()
                .ok_or(QueueRefusal::InvalidTransition)?
                .account_evidence = next
                .result
                .as_ref()
                .and_then(SubmissionResult::account_evidence)
                .map(|evidence| evidence.at_revision(origin));
        }
        Ok(next)
    }
}
