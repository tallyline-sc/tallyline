// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Atomic lifecycle evidence and accepted-prior projection coordination.

use tallyline_reference_data::api::model::{
    AcceptedPriorHistory, InvalidPrior, LocalPriorRevocation,
};
use tallyline_shared_kernel::api::model::Timestamp;
use tallyline_submission::api::model::{
    JobId, LifecycleReadBatch, LifecycleReadClaim, LifecycleRefresh, SubmissionJob,
    SubmissionLifecycle, SubmissionRepositoryError, UexLifecycleObservation, UexReportStatus,
};

/// Coordinates durable lifecycle read admission and exact same-database prior reversal.
pub trait SubmissionLifecycleStore {
    /// Loads the committed lifecycle independently of the original delivery state.
    ///
    /// # Errors
    /// Refuses corrupt proofs and unavailable storage.
    fn load_lifecycle(
        &mut self,
        job: JobId,
    ) -> Result<Option<SubmissionLifecycle>, SubmissionRepositoryError>;
    /// Commits one exact read admission before any lifecycle HTTP request starts.
    ///
    /// # Errors
    /// Refuses changed or unverified sources, invalid admission and unavailable storage.
    fn claim_lifecycle_read(
        &mut self,
        job: &SubmissionJob,
        refresh: LifecycleRefresh,
        at: Timestamp,
    ) -> Result<Option<LifecycleReadClaim>, SubmissionRepositoryError>;
    /// Claims due exact jobs for one username-filtered read with shared manual admission.
    ///
    /// # Errors
    /// Refuses mixed accounts, duplicate jobs, stale sources and failed atomic admission.
    fn claim_lifecycle_batch(
        &mut self,
        jobs: &[&SubmissionJob],
        refresh: LifecycleRefresh,
        at: Timestamp,
    ) -> Result<Option<LifecycleReadBatch>, SubmissionRepositoryError>;
    /// Commits positive lifecycle evidence and matching local-prior reversal atomically.
    ///
    /// # Errors
    /// Refuses stale claims, unrelated evidence and failed transactions without publishing state.
    fn commit_lifecycle_read(
        &mut self,
        claim: &LifecycleReadClaim,
        observations: &[UexLifecycleObservation],
    ) -> Result<SubmissionLifecycle, SubmissionRepositoryError>;
}

/// Applies verified decline and expiry through the existing accepted-history policy.
///
/// # Errors
/// Refuses missing or inconsistent accepted evidence and conflicting final reversal reasons.
pub fn lifecycle_prior_history(
    lifecycle: &SubmissionLifecycle,
    history: &AcceptedPriorHistory,
) -> Result<AcceptedPriorHistory, InvalidPrior> {
    let expected = crate::submission_priors::accepted_submission_priors(lifecycle.source())?;
    let mut next = history.clone();
    for receipt in lifecycle.receipts() {
        let row = lifecycle
            .source()
            .request()
            .rows()
            .iter()
            .find(|row| row.id() == receipt.row())
            .ok_or(InvalidPrior)?;
        let prior = expected
            .iter()
            .find(|prior| prior.commodity() == row.commodity())
            .ok_or(InvalidPrior)?;
        if !history
            .records()
            .iter()
            .any(|record| record.prior() == prior)
        {
            return Err(InvalidPrior);
        }
        let Some(observation) = lifecycle.observation(receipt.row()) else {
            continue;
        };
        let reason = match observation.status() {
            UexReportStatus::Declined => LocalPriorRevocation::Declined,
            UexReportStatus::Expired => LocalPriorRevocation::Expired,
            UexReportStatus::Pending
            | UexReportStatus::UnderReview
            | UexReportStatus::Queued
            | UexReportStatus::Approved
            | UexReportStatus::Consolidated => continue,
        };
        next = next.revoked(lifecycle.job(), row.commodity(), reason)?;
    }
    Ok(next)
}
