// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! One-shot optional enrichment of already committed production acceptance.

use tallyline_submission::api::model::{
    JobId, SubmissionJob, SubmissionRepositoryError, VerifiedReceiptMapping,
};

/// Stores optional exact ID mappings independently of the successful attempt and Report state.
pub trait ReceiptEnrichmentStore {
    /// Claims one bounded lookup only after full production acceptance committed.
    /// A retained claim prevents replay after failure, shutdown or restart.
    ///
    /// # Errors
    /// Refuses non-accepted, stale or unavailable source jobs without starting a lookup.
    fn claim_receipt_enrichment(
        &mut self,
        accepted: &SubmissionJob,
    ) -> Result<bool, SubmissionRepositoryError>;
    /// Commits an exact accepted-ID mapping without changing success, attempts or priors.
    ///
    /// # Errors
    /// Refuses missing claims, stale proofs, conflicting mappings and failed transactions.
    fn save_receipt_mapping(
        &mut self,
        mapping: &VerifiedReceiptMapping,
    ) -> Result<(), SubmissionRepositoryError>;
    /// Loads a validated proof or absence independently of the unchanged successful job.
    ///
    /// # Errors
    /// Refuses corrupt, unrelated or unavailable persisted proof metadata.
    fn receipt_mapping(
        &mut self,
        job: JobId,
    ) -> Result<Option<VerifiedReceiptMapping>, SubmissionRepositoryError>;
}
