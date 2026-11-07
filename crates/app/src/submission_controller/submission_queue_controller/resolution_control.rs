// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact-version explicit resolution commands without submission authority.

use super::SubmissionQueueController;
use crate::api::SubmissionControlError;
use tallyline_shared_kernel::api::model::RowId;
use tallyline_submission::api::model::{
    JobId, JobPhase, ReceiptDecision, ReleaseToken, SubmissionCommand, SubmissionJob,
    SubmissionResolutionAction,
};
use tallyline_submission::api::{CredentialApplication, SubmissionJobRepository};
use tallyline_workflows::api::ResolutionDraft;

impl SubmissionQueueController {
    /// Records an explicit received or not-received statement for the displayed unknown origin.
    ///
    /// # Errors
    /// Refuses stale revisions, ineligible phases and failed local persistence.
    pub fn decide_unknown(
        &mut self,
        id: JobId,
        expected: u64,
        decision: ReceiptDecision,
    ) -> Result<SubmissionJob, SubmissionControlError> {
        self.apply_resolution(
            id,
            expected,
            SubmissionResolutionAction::Decision {
                decision,
                at: self.clock.now(),
            },
        )
    }

    /// Creates a separate unreviewed Draft from an eligible immutable origin.
    ///
    /// # Errors
    /// Refuses stale or ineligible origins and failed atomic persistence.
    pub fn duplicate_resolved(
        &mut self,
        id: JobId,
        expected: u64,
    ) -> Result<ResolutionDraft, SubmissionControlError> {
        use tallyline_workflows::api::ResolutionDraftStore as _;
        let duplicated = self
            .store
            .duplicate_resolved(id, expected, self.clock.now())
            .map_err(SubmissionControlError::Repository)?;
        self.queue.update(duplicated.origin.clone());
        Ok(duplicated)
    }

    fn resolution_job(
        &mut self,
        id: JobId,
        expected: u64,
    ) -> Result<SubmissionJob, SubmissionControlError> {
        let job = SubmissionJobRepository::load(&mut self.store, id)
            .map_err(SubmissionControlError::Repository)?
            .ok_or(SubmissionControlError::Invalid)?;
        if job.revision() != expected {
            return Err(SubmissionControlError::Conflict);
        }
        if !matches!(
            job.phase(),
            JobPhase::OutcomeUnknown | JobPhase::PartiallyAccepted
        ) {
            return Err(SubmissionControlError::Invalid);
        }
        Ok(job)
    }
}

impl SubmissionQueueController {
    /// Starts a bounded read-only check for the exact displayed partial or unknown origin.
    ///
    /// # Errors
    /// Refuses stale origins, missing current account validation and occupied read work.
    pub fn check_receipts(
        &mut self,
        id: JobId,
        expected: u64,
        credentials: &CredentialApplication,
        token: Option<&ReleaseToken>,
    ) -> Result<(), SubmissionControlError> {
        let job = self.resolution_job(id, expected)?;
        if !job.can_check_receipts() {
            return Err(SubmissionControlError::Invalid);
        }
        self.row_receipts
            .begin(&self.client, job, credentials, token)
    }
}

impl SubmissionQueueController {
    /// Records a version-bound statement for one unresolved partial row.
    ///
    /// # Errors
    /// Refuses stale, resolved or foreign rows and failed atomic persistence.
    pub fn decide_row(
        &mut self,
        id: JobId,
        expected: u64,
        row: RowId,
        decision: ReceiptDecision,
    ) -> Result<SubmissionJob, SubmissionControlError> {
        self.apply_resolution(
            id,
            expected,
            SubmissionResolutionAction::RowDecision {
                row,
                decision,
                at: self.clock.now(),
            },
        )
    }
    fn apply_resolution(
        &mut self,
        id: JobId,
        expected: u64,
        action: SubmissionResolutionAction,
    ) -> Result<SubmissionJob, SubmissionControlError> {
        let job = self.resolution_job(id, expected)?;
        let change = job
            .apply(SubmissionCommand::ResolveDelivery(action))
            .map_err(SubmissionControlError::Refused)?;
        SubmissionJobRepository::save(&mut self.store, &change)
            .map_err(SubmissionControlError::Repository)?;
        let committed = change.into_state();
        self.queue.update(committed.clone());
        Ok(committed)
    }
}
