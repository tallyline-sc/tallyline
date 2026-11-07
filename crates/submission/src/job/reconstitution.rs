// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Validated reconstruction of complete write-ahead and recovery history.

use super::storage::{StoredSubmissionAttempt, StoredSubmissionJob};
use super::submission_job::{AttemptOutcome, JobPhase, SubmissionAttempt, SubmissionJob};
use super::submission_result::SubmissionResult;
use crate::api::model::InvalidSubmission;
use crate::api::model::SubmissionPauseReason;
use crate::api::model::{SubmissionAccountCheck, SubmissionAccountEvidence};
use std::borrow::Cow;
use tallyline_shared_kernel::api::model::Timestamp;

impl SubmissionJob {
    /// Reconstitutes validated durable state for the persistence adapter.
    ///
    /// # Errors
    /// Refuses missing, inconsistent or unsupported attempt history rather than inventing state.
    pub fn reconstitute(stored: StoredSubmissionJob) -> Result<Self, InvalidSubmission> {
        let mut job = Self::queued(stored.id, stored.request)?;
        for (index, attempt) in stored.attempts.iter().enumerate() {
            if index > 0 {
                job = match (job.result(), attempt.client_update) {
                    (Some(SubmissionResult::VersionLockedOut(_)), Some(version)) => {
                        job.retry_after_update(version)
                    }
                    (Some(result), _) if result.daily_quota_reset().is_some() => {
                        job.resume_quota(result.daily_quota_reset().ok_or(InvalidSubmission)?)
                    }
                    _ => job.retry(),
                }
                .map_err(|_| InvalidSubmission)?;
            }
            if attempt.rows != job.request.rows().len() {
                return Err(InvalidSubmission);
            }
            job = job
                .begin_prepared(attempt.started_at, attempt.payload_hash, attempt.date_added)
                .map_err(|_| InvalidSubmission)?;
            job = match (attempt.ended_at, attempt.outcome) {
                (Some(ended_at), Some(outcome)) => {
                    let result = if index + 1 == stored.attempts.len() {
                        stored.result.clone()
                    } else {
                        None
                    }
                    .or(match outcome {
                        AttemptOutcome::NotProcessed(reason) => {
                            Some(SubmissionResult::NotProcessed(reason))
                        }
                        AttemptOutcome::Held(reason) => Some(SubmissionResult::Held(reason)),
                        AttemptOutcome::Paused(reason) => Some(SubmissionResult::Paused(reason)),
                        AttemptOutcome::VersionLockedOut(version) => {
                            Some(SubmissionResult::VersionLockedOut(version))
                        }
                        AttemptOutcome::OutcomeCheckRequired
                        | AttemptOutcome::ProductionSucceeded
                        | AttemptOutcome::Unknown
                        | AttemptOutcome::Rejected
                        | AttemptOutcome::DryRunSucceeded
                        | AttemptOutcome::Partial
                        | AttemptOutcome::Returned(_) => None,
                    })
                    .ok_or(InvalidSubmission)?;
                    let result = if result.account_evidence().is_none() {
                        match attempt.account_evidence.as_ref() {
                            Some(evidence) => evidence.wrap(result),
                            None => result,
                        }
                    } else {
                        result
                    };
                    job.rebuild_completed_attempt(
                        result,
                        attempt,
                        ended_at,
                        stored.service_pause_acknowledged && index + 1 == stored.attempts.len(),
                    )?
                }
                (None, None) if index + 1 == stored.attempts.len() && stored.result.is_none() => {
                    job
                }
                _ => return Err(InvalidSubmission),
            };
        }
        if stored.result.is_none() && !stored.attempts.is_empty() && job.phase != JobPhase::Sending
        {
            job = match (job.result(), stored.client_update) {
                (Some(SubmissionResult::VersionLockedOut(_)), Some(version)) => {
                    job.retry_after_update(version)
                }
                (Some(result), _) if result.daily_quota_reset().is_some() => {
                    job.resume_quota(result.daily_quota_reset().ok_or(InvalidSubmission)?)
                }
                _ => job.retry(),
            }
            .map_err(|_| InvalidSubmission)?;
        }
        if let Some(reason) = stored.review_reason {
            job = job
                .return_for_review(reason)
                .map_err(|_| InvalidSubmission)?;
        }
        if job.phase != stored.phase {
            job = match stored.phase {
                JobPhase::Cancelled => match stored.resolution_actions.as_slice() {
                    [crate::api::model::SubmissionResolutionAction::Split { report, at }] => {
                        job.cancel_for_split(*report, *at)
                    }
                    _ => job.cancel(),
                },
                JobPhase::Returned => job.recover(Timestamp::from_seconds(0)),
                JobPhase::Queued
                | JobPhase::Succeeded
                | JobPhase::PartiallyAccepted
                | JobPhase::Sending
                | JobPhase::OutcomeUnknown
                | JobPhase::Rejected
                | JobPhase::Held
                | JobPhase::Paused => return Err(InvalidSubmission),
            }
            .map_err(|_| InvalidSubmission)?;
        }
        if stored.service_pause_acknowledged && !job.service_pause_acknowledged() {
            job = match (job.result(), stored.client_update) {
                (Some(SubmissionResult::VersionLockedOut(_)), Some(version)) => {
                    job.retry_after_update(version)
                }
                _ if job.can_acknowledge_service_pause() => job.retry(),
                _ => return Err(InvalidSubmission),
            }
            .map_err(|_| InvalidSubmission)?;
        }
        for action in &stored.resolution_actions {
            if matches!(
                action,
                crate::api::model::SubmissionResolutionAction::Split { .. }
            ) {
                if job.resolution_actions != stored.resolution_actions {
                    return Err(InvalidSubmission);
                }
                continue;
            }
            let previous_revision = job.revision();
            job = job
                .resolve_delivery(action.clone())
                .map_err(|_| InvalidSubmission)?;
            if previous_revision == job.revision() {
                return Err(InvalidSubmission);
            }
        }
        let attempts: Vec<_> = stored
            .attempts
            .into_iter()
            .map(|attempt| SubmissionAttempt {
                started_at: attempt.started_at,
                rows: attempt.rows,
                payload_hash: attempt.payload_hash,
                date_added: attempt.date_added,
                ended_at: attempt.ended_at,
                outcome: attempt.outcome,
                receipt_checked: attempt.receipt_checked,
                account_checks: account_checks(&attempt).into_owned(),
                account_evidence: attempt.account_evidence,
                client_update: attempt.client_update,
            })
            .collect();
        if job.phase != stored.phase
            || job.revision != stored.revision
            || job.attempts != attempts
            || job.result != stored.result
            || job.review_reason != stored.review_reason
            || job.service_pause_acknowledged != stored.service_pause_acknowledged
            || job.resolution_actions != stored.resolution_actions
            || job.client_update != stored.client_update
        {
            return Err(InvalidSubmission);
        }
        Ok(job)
    }
    fn rebuild_completed_attempt(
        mut self,
        result: SubmissionResult,
        attempt: &StoredSubmissionAttempt,
        ended_at: Timestamp,
        acknowledged: bool,
    ) -> Result<Self, InvalidSubmission> {
        if let SubmissionResult::Rechecked { code, outcome } = result {
            if !attempt.receipt_checked {
                return Err(InvalidSubmission);
            }
            let initial = SubmissionResult::NeedsOutcomeCheck {
                code,
                http_status: attempt
                    .account_evidence
                    .as_ref()
                    .map_or(400, SubmissionAccountEvidence::http_status),
            };
            let initial = match attempt.account_evidence.as_ref() {
                Some(evidence) => evidence.pending().wrap(initial),
                None => initial,
            };
            self = self
                .complete(initial, ended_at)
                .map_err(|_| InvalidSubmission)?;
            self = self.replay_account_checks(account_checks(attempt).as_ref())?;
            if acknowledged && self.can_acknowledge_service_pause() {
                self = self.retry().map_err(|_| InvalidSubmission)?;
            }
            self.resolve_outcome_check(*outcome)
                .map_err(|_| InvalidSubmission)
        } else {
            self = self
                .complete(result.pending_account_check(), ended_at)
                .map_err(|_| InvalidSubmission)?;
            self = self.replay_account_checks(account_checks(attempt).as_ref())?;
            if attempt.receipt_checked {
                if !matches!(
                    self.result().map(SubmissionResult::final_result),
                    Some(
                        SubmissionResult::Rejected(_)
                            | SubmissionResult::PartialError { .. }
                            | SubmissionResult::Returned(_)
                            | SubmissionResult::Paused(_)
                            | SubmissionResult::PausedCode { .. }
                    )
                ) {
                    return Err(InvalidSubmission);
                }
                self = self.advance().map_err(|_| InvalidSubmission)?;
                self.attempts
                    .last_mut()
                    .ok_or(InvalidSubmission)?
                    .receipt_checked = true;
            }
            Ok(self)
        }
    }
    fn replay_account_checks(
        mut self,
        checks: &[SubmissionPauseReason],
    ) -> Result<Self, InvalidSubmission> {
        for &reason in checks {
            let previous_revision = self.revision();
            self = self
                .resolve_account_check(reason)
                .map_err(|_| InvalidSubmission)?;
            if self.revision() == previous_revision {
                return Err(InvalidSubmission);
            }
        }
        Ok(self)
    }
}
fn account_checks(attempt: &StoredSubmissionAttempt) -> Cow<'_, [SubmissionPauseReason]> {
    if let Some(checks) = &attempt.account_checks {
        Cow::Borrowed(checks)
    } else if let Some(SubmissionAccountCheck::Completed(reason)) = attempt
        .account_evidence
        .as_ref()
        .map(SubmissionAccountEvidence::check)
    {
        Cow::Owned(vec![reason])
    } else {
        Cow::Borrowed(&[])
    }
}
