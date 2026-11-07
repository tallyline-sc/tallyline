// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact manual release checks over a borrowed view of the queue resources.

use super::resources;
use super::{SubmissionControlError, SubmissionQueuePreferences};
use crate::api::HttpClient;
use tallyline_adapter_files::api::ManualWorkingCopies;
use tallyline_adapter_storage::api::SqliteReferenceCache;
use tallyline_reporting::api::ReportRepository;
use tallyline_shared_kernel::api::Clock;
use tallyline_shared_kernel::api::model::{ReportId, SubmissionMode};
use tallyline_submission::api::CredentialApplication;
use tallyline_workflows::api::{
    ManualRelease, ManualReleaseError, ManualReleaseStore, SubmissionQueueStore,
};

#[derive(Clone, Copy)]
pub(super) enum ReleaseKind {
    Manual(SubmissionMode),
    OnboardingTest,
}

pub(super) struct ReleaseControl<'a> {
    store: &'a mut SqliteReferenceCache,
    client: &'a HttpClient,
    preferences: &'a mut (dyn FnMut() -> std::io::Result<SubmissionQueuePreferences> + Send),
    copies: &'a mut Option<ManualWorkingCopies>,
    clock: &'a dyn Clock,
}
impl<'a> ReleaseControl<'a> {
    pub(super) fn new(
        store: &'a mut SqliteReferenceCache,
        client: &'a HttpClient,
        preferences: &'a mut (dyn FnMut() -> std::io::Result<SubmissionQueuePreferences> + Send),
        copies: &'a mut Option<ManualWorkingCopies>,
        clock: &'a dyn Clock,
    ) -> Self {
        Self {
            store,
            client,
            preferences,
            copies,
            clock,
        }
    }
    pub(super) fn release(
        &mut self,
        id: ReportId,
        expected: u64,
        kind: ReleaseKind,
        credentials: &CredentialApplication,
    ) -> Result<ManualRelease, SubmissionControlError> {
        let preferences = (self.preferences)().map_err(|_| SubmissionControlError::Unavailable)?;
        let mode = match kind {
            ReleaseKind::Manual(mode) if mode == preferences.mode => mode,
            ReleaseKind::Manual(_) => return Err(SubmissionControlError::Conflict),
            ReleaseKind::OnboardingTest => SubmissionMode::DryRun,
        };
        let report = self
            .store
            .load_draft(id)
            .map_err(|_| SubmissionControlError::Unavailable)?
            .ok_or(SubmissionControlError::Invalid)?;
        if report.version() != expected {
            return Err(SubmissionControlError::Conflict);
        }
        let Some(username) = credentials.release_account() else {
            return Err(SubmissionControlError::Release(
                ManualReleaseError::Blocked(vec![
                    tallyline_reporting::api::model::ReleaseBlockReason::AccountUnavailable,
                ]),
            ));
        };
        let (gate, encoded) = resources::gate(
            self.store,
            self.copies,
            report,
            username,
            mode,
            &preferences,
            self.clock.now(),
        )?;
        let blocks = gate.blocks(gate.report());
        if !blocks.is_empty() {
            return Err(SubmissionControlError::Release(
                ManualReleaseError::Blocked(blocks),
            ));
        }
        let request = tallyline_workflows::api::manual_submission_request(
            gate.report(),
            gate.snapshot().environment(),
            username,
            mode,
        )
        .map_err(|_| SubmissionControlError::Invalid)?;
        let image = encoded
            .as_ref()
            .map(|encoded| encoded.base64_for(gate.report()))
            .transpose()
            .map_err(|_| SubmissionControlError::AttachmentUnavailable)?;
        drop(
            self.client
                .prepare_with_screenshot(&request, image)
                .map_err(|_| SubmissionControlError::Invalid)?,
        );
        let current = (self.preferences)().map_err(|_| SubmissionControlError::Unavailable)?;
        if current.reviewer != preferences.reviewer
            || current.age != preferences.age
            || matches!(kind, ReleaseKind::Manual(_)) && current.mode != preferences.mode
        {
            return Err(SubmissionControlError::Conflict);
        }
        let refreshed = gate.reassess_at(self.clock.now());
        let history = self
            .store
            .queue_jobs()
            .map_err(SubmissionControlError::Repository)?;
        let refreshed = tallyline_workflows::api::assess_release_observation_history(
            &refreshed,
            Some(&history),
        );
        let blocks = refreshed.blocks(refreshed.report());
        if !blocks.is_empty() {
            return Err(SubmissionControlError::Release(
                ManualReleaseError::Blocked(blocks),
            ));
        }
        if refreshed.report() != gate.report() {
            return Err(SubmissionControlError::Conflict);
        }
        let gate = refreshed;
        let release = self
            .store
            .release_manual(&gate)
            .map_err(SubmissionControlError::Release)?;
        Ok(release)
    }
}
