// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Receipt enrichment bound to an already successful immutable submission.

use crate::api::model::{
    InvalidSubmission, JobId, JobPhase, SubmissionJob, SubmissionResult, VerifiedUexReport,
};

/// Complete verified identities for an exact committed successful job version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifiedReceiptMapping {
    accepted: SubmissionJob,
    receipts: Vec<VerifiedUexReport>,
}
impl VerifiedReceiptMapping {
    /// Binds lookup proofs to every actual accepted ID without changing successful attempt state.
    ///
    /// # Errors
    /// Refuses incomplete, duplicate or unrelated identities, stale values and non-successful jobs.
    pub fn new(
        job: &SubmissionJob,
        receipts: Vec<VerifiedUexReport>,
    ) -> Result<Self, InvalidSubmission> {
        let Some(SubmissionResult::ProductionAccepted(accepted)) = job.result() else {
            return Err(InvalidSubmission);
        };
        let date_added = job.attempts().last().ok_or(InvalidSubmission)?.date_added();
        let mut rows = std::collections::BTreeSet::new();
        let mut ids = std::collections::BTreeSet::new();
        if job.phase() != JobPhase::Succeeded
            || receipts.len() != accepted.len()
            || receipts.iter().any(|receipt| {
                !receipt.matches_at(job.request(), date_added)
                    || !accepted.iter().any(|id| {
                        id.id() == receipt.id() && id.date_added() == receipt.date_added()
                    })
                    || !rows.insert(receipt.row().0)
                    || !ids.insert(receipt.id())
            })
        {
            return Err(InvalidSubmission);
        }
        Ok(Self {
            accepted: job.clone(),
            receipts,
        })
    }
    /// Returns the successful local submission identity.
    #[must_use]
    pub const fn job(&self) -> JobId {
        self.accepted.id()
    }
    /// Returns the exact successful job version against which the lookup was verified.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.accepted.revision()
    }
    /// Borrows complete identity-bound mappings; response positions are never used.
    #[must_use]
    pub fn receipts(&self) -> &[VerifiedUexReport] {
        &self.receipts
    }
    /// Checks the complete accepted snapshot and attempt state before persistence or publication.
    #[must_use]
    pub fn matches_job(&self, job: &SubmissionJob) -> bool {
        self.accepted == *job
    }
}
