// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Same-database full pre-send reassessment and write-ahead persistence.

use crate::api::SqliteReferenceCache;
use tallyline_shared_kernel::api::model::Timestamp;
use tallyline_submission::api::model::{SubmissionJob, SubmissionRepositoryError};
use tallyline_workflows::api::{ManualDispatch, ManualDispatchError, ManualDispatchStore};

fn error(value: SubmissionRepositoryError) -> ManualDispatchError {
    match value {
        SubmissionRepositoryError::Invalid => ManualDispatchError::Invalid,
        SubmissionRepositoryError::Conflict => ManualDispatchError::Conflict,
        SubmissionRepositoryError::Unavailable => ManualDispatchError::Unavailable,
    }
}
impl ManualDispatchStore for SqliteReferenceCache {
    fn return_manual_review(
        &mut self,
        job: &SubmissionJob,
        gate: &tallyline_reporting::api::model::SubmissionGate,
        now: Timestamp,
    ) -> Result<SubmissionJob, ManualDispatchError> {
        self.return_review_transaction(job, gate, now)
    }
    fn begin_manual_dispatch(
        &mut self,
        plan: &ManualDispatch,
        now: Timestamp,
    ) -> Result<SubmissionJob, ManualDispatchError> {
        let transaction = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|_| ManualDispatchError::Unavailable)?;
        let current = crate::queue::load_job(&transaction, plan.source().id())
            .map_err(error)?
            .ok_or(ManualDispatchError::Invalid)?;
        if &current != plan.source() {
            return Err(ManualDispatchError::Conflict);
        }
        let source = crate::queue::load_report(&transaction, current.request().header().report)
            .map_err(error)?;
        if &source != plan.gate().source() {
            return Err(ManualDispatchError::Conflict);
        }
        crate::reference_consistency::check(&transaction, plan.gate().snapshot()).map_err(error)?;
        let history = crate::queue::history(&transaction).map_err(error)?;
        let gate = plan.recheck(now, &history)?;
        if crate::screenshot_policy::required_since(
            &transaction,
            &current.request().header().account,
        )
        .map_err(|_| ManualDispatchError::Unavailable)?
        .is_some()
            && gate.report().attachment().is_none()
        {
            return Err(ManualDispatchError::Blocked(vec![
                tallyline_reporting::api::model::ReleaseBlockReason::ImageRequired,
            ]));
        }
        crate::token_quota::bind(&transaction, &current, plan.token_scope()).map_err(error)?;
        let change = plan.change(now)?;
        crate::queue::save_change_with_admission(
            &transaction,
            &change,
            Some(plan.token_admission()),
        )
        .map_err(error)?;
        if gate.report() != gate.source() {
            crate::queue::save_report(&transaction, gate.report(), source.version())
                .map_err(error)?;
        }
        transaction
            .commit()
            .map_err(|_| ManualDispatchError::Unavailable)?;
        Ok(change.into_state())
    }
}

impl SqliteReferenceCache {
    fn return_review_transaction(
        &mut self,
        job: &SubmissionJob,
        gate: &tallyline_reporting::api::model::SubmissionGate,
        now: Timestamp,
    ) -> Result<SubmissionJob, ManualDispatchError> {
        let transaction = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|_| ManualDispatchError::Unavailable)?;
        let current = crate::queue::load_job(&transaction, job.id())
            .map_err(error)?
            .ok_or(ManualDispatchError::Invalid)?;
        if &current != job {
            return Err(ManualDispatchError::Conflict);
        }
        let source = crate::queue::load_report(&transaction, job.request().header().report)
            .map_err(error)?;
        if &source != gate.source() {
            return Err(ManualDispatchError::Conflict);
        }
        crate::reference_consistency::check(&transaction, gate.snapshot()).map_err(error)?;
        let history = crate::queue::history(&transaction).map_err(error)?;
        let reviewed = gate.reassess_at(now);
        let reviewed =
            tallyline_workflows::api::assess_release_observation_history(&reviewed, Some(&history));
        let reason = ManualDispatch::review_return_reason(&reviewed)?;
        let change = current
            .apply(tallyline_submission::api::model::SubmissionCommand::Return(
                reason,
            ))
            .map_err(|_| ManualDispatchError::Invalid)?;
        let returned =
            tallyline_workflows::api::apply_submission_delivery(reviewed.report(), change.state())
                .map_err(|_| ManualDispatchError::Invalid)?
                .ok_or(ManualDispatchError::Invalid)?;
        crate::queue::save_change(&transaction, &change).map_err(error)?;
        let intermediate = crate::queue::load_report(&transaction, source.id()).map_err(error)?;
        if &intermediate != returned.state() {
            crate::queue::save_report(&transaction, returned.state(), intermediate.version())
                .map_err(error)?;
        }
        transaction
            .commit()
            .map_err(|_| ManualDispatchError::Unavailable)?;
        Ok(change.into_state())
    }
}
