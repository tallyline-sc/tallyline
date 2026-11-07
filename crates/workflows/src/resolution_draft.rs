// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Explicit copying of eligible immutable origins into separate unreviewed Drafts.

use tallyline_reporting::api::model::Report;
use tallyline_shared_kernel::api::model::Timestamp;
use tallyline_submission::api::model::{JobId, SubmissionJob, SubmissionRepositoryError};

/// The atomically committed origin annotation and separate unreviewed Draft.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolutionDraft {
    /// The retained immutable origin with its explicit copying link.
    pub origin: SubmissionJob,
    /// The separate new Draft requiring complete review and release.
    pub draft: Report,
}

/// Atomically creates a separate Draft from eligible immutable submission history.
pub trait ResolutionDraftStore {
    /// Copies the exact displayed origin and retains both histories atomically.
    ///
    /// # Errors
    /// Refuses stale, unresolved or already copied origins and empty eligible subsets.
    fn duplicate_resolved(
        &mut self,
        id: JobId,
        expected_revision: u64,
        at: Timestamp,
    ) -> Result<ResolutionDraft, SubmissionRepositoryError>;
}
