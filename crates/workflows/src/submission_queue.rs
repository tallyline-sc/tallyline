// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Durable cross-context queue inspection and startup recovery port.

use crate::api::ManualReleaseStore;
use tallyline_reporting::api::model::Report;
use tallyline_shared_kernel::api::model::Timestamp;
use tallyline_submission::api::SubmissionJobRepository;
use tallyline_submission::api::model::{JobId, SubmissionJob, SubmissionRepositoryError};

/// Owns same-database queue reads and atomic startup recovery before dispatch starts.
pub trait SubmissionQueueStore: ManualReleaseStore + SubmissionJobRepository {
    /// Loads retained jobs without silently omitting unknown outcomes or attempt history.
    ///
    /// # Errors
    /// Returns unavailable or invalid history instead of authorising from an empty list.
    fn queue_jobs(&mut self) -> Result<Vec<SubmissionJob>, SubmissionRepositoryError>;
    /// Loads the immutable Report snapshot originally committed with this job.
    ///
    /// # Errors
    /// Refuses a corrupt or mismatched release binding, version or value snapshot.
    fn released_report(&mut self, id: JobId) -> Result<Option<Report>, SubmissionRepositoryError>;
    /// Atomically closes interrupted work before any dispatch can start.
    ///
    /// Sending becomes unknown; other unfinished work returns to Draft for renewed
    /// release. Reports, jobs and outcome events share the same transaction.
    ///
    /// # Errors
    /// Returns a failure without publishing speculative recovery state.
    fn recover_unfinished(
        &mut self,
        now: Timestamp,
    ) -> Result<Vec<SubmissionJob>, SubmissionRepositoryError>;
}
