// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Explicit age and older-row decisions against current policy and committed history.

use super::{SubmissionControlError, SubmissionQueuePreferences};
use tallyline_adapter_storage::api::SqliteReferenceCache;
use tallyline_reporting::api::ReportRepository;
use tallyline_reporting::api::model::{NewerObservationEvidence, Report};
use tallyline_shared_kernel::api::Clock;
use tallyline_shared_kernel::api::model::{ReportId, RowId};
use tallyline_submission::api::CredentialApplication;
use tallyline_submission::api::model::AccountState;
use tallyline_workflows::api::{ManualReleaseError, SubmissionQueueStore};

pub(super) struct DecisionControl<'a> {
    store: &'a mut SqliteReferenceCache,
    preferences: &'a mut (dyn FnMut() -> std::io::Result<SubmissionQueuePreferences> + Send),
    clock: &'a dyn Clock,
}
impl<'a> DecisionControl<'a> {
    pub(super) fn new(
        store: &'a mut SqliteReferenceCache,
        preferences: &'a mut (dyn FnMut() -> std::io::Result<SubmissionQueuePreferences> + Send),
        clock: &'a dyn Clock,
    ) -> Self {
        Self {
            store,
            preferences,
            clock,
        }
    }

    /// Records an explicit exact-row approval after verifying the newest committed history.
    ///
    /// # Errors
    /// Refuses stale Drafts, changed accounts/mappings, fabricated evidence and newer uncertainty.
    pub(super) fn confirm_newer_observation(
        &mut self,
        id: ReportId,
        expected: u64,
        row: RowId,
        evidence: NewerObservationEvidence,
        credentials: &CredentialApplication,
    ) -> Result<Report, SubmissionControlError> {
        if !matches!(credentials.state(),AccountState::Validated {username,..} if username == evidence.account())
        {
            return Err(SubmissionControlError::Conflict);
        }
        let report = self
            .store
            .load_draft(id)
            .map_err(|_| SubmissionControlError::Unavailable)?
            .ok_or(SubmissionControlError::Invalid)?;
        if report.version() != expected {
            return Err(SubmissionControlError::Conflict);
        }
        let policy = (self.preferences)().map_err(|_| SubmissionControlError::Unavailable)?;
        if policy.reviewer.environment_for(report.key().environment()) != evidence.environment() {
            return Err(SubmissionControlError::Conflict);
        }
        let history = self
            .store
            .queue_jobs()
            .map_err(SubmissionControlError::Repository)?;
        let committed = tallyline_workflows::api::confirm_older_observation(
            &report,
            row,
            evidence,
            &history,
            self.clock.now(),
        )
        .map_err(|error| match error {
            ManualReleaseError::Conflict => SubmissionControlError::Conflict,
            error @ (ManualReleaseError::Blocked(_)
            | ManualReleaseError::Invalid
            | ManualReleaseError::Unavailable) => SubmissionControlError::Release(error),
        })?;
        if committed.version() != expected {
            self.store
                .save_draft(&committed, expected)
                .map_err(|error| match error {
                    tallyline_reporting::api::RepositoryError::Conflict => {
                        SubmissionControlError::Conflict
                    }
                    tallyline_reporting::api::RepositoryError::Invalid => {
                        SubmissionControlError::Invalid
                    }
                    tallyline_reporting::api::RepositoryError::Unavailable
                    | tallyline_reporting::api::RepositoryError::AssessmentUnavailable => {
                        SubmissionControlError::Unavailable
                    }
                })?;
        }
        Ok(committed)
    }

    /// Records an explicit age confirmation against the exact displayed Draft and real clock.
    ///
    /// # Errors
    /// Refuses stale versions, released work, future or hard-expired observations and failed saves.
    pub(super) fn confirm_observation_age(
        &mut self,
        id: ReportId,
        expected: u64,
    ) -> Result<tallyline_reporting::api::model::Report, SubmissionControlError> {
        let report = self
            .store
            .load_draft(id)
            .map_err(|_| SubmissionControlError::Unavailable)?
            .ok_or(SubmissionControlError::Invalid)?;
        if report.version() != expected {
            return Err(SubmissionControlError::Conflict);
        }
        let change = report
            .confirm_observation_age(self.clock.now())
            .map_err(|_| SubmissionControlError::Invalid)?;
        let committed = change.into_state();
        if committed.version() != expected {
            self.store
                .save_draft(&committed, expected)
                .map_err(|_| SubmissionControlError::Unavailable)?;
        }
        Ok(committed)
    }
}
