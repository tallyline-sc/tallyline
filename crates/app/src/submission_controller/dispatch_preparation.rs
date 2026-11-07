// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Fresh reserved-request checks and write-ahead preparation over borrowed queue resources.

use super::queue_state::QueueState;
use super::resources;
use super::{SubmissionControlError, SubmissionQueuePreferences};
use crate::api::{HttpClient, PendingSubmission, PendingSubmissionReservation, SubmissionSlot};
use tallyline_adapter_files::api::ManualWorkingCopies;
use tallyline_adapter_files::api::model::EncodedAttachment;
use tallyline_adapter_storage::api::SqliteReferenceCache;
use tallyline_adapter_uex::api::model::PreparedUexSubmission;
use tallyline_reporting::api::model::{Report, SubmissionGate};
use tallyline_shared_kernel::api::Clock;
use tallyline_shared_kernel::api::model::Timestamp;
use tallyline_submission::api::model::{
    CredentialValues, JobId, JobPhase, ReleaseToken, ScheduleDecision, SubmissionJob,
    TokenAdmission,
};
use tallyline_submission::api::{
    CredentialApplication, SubmissionJobRepository, SubmissionQuotaRepository, SubmissionSchedule,
};
use tallyline_workflows::api::{
    ManualDispatch, ManualDispatchError, ManualDispatchStore, SubmissionQueueStore,
};

pub(super) struct DispatchPreparation<'a> {
    store: &'a mut SqliteReferenceCache,
    client: &'a HttpClient,
    preferences: &'a mut (dyn FnMut() -> std::io::Result<SubmissionQueuePreferences> + Send),
    copies: &'a mut Option<ManualWorkingCopies>,
    clock: &'a dyn Clock,
    queue: &'a mut QueueState,
}
impl<'a> DispatchPreparation<'a> {
    pub(super) fn new(
        store: &'a mut SqliteReferenceCache,
        client: &'a HttpClient,
        preferences: &'a mut (dyn FnMut() -> std::io::Result<SubmissionQueuePreferences> + Send),
        copies: &'a mut Option<ManualWorkingCopies>,
        clock: &'a dyn Clock,
        queue: &'a mut QueueState,
    ) -> Self {
        Self {
            store,
            client,
            preferences,
            copies,
            clock,
            queue,
        }
    }
    pub(super) fn prepare_reserved<'credentials>(
        &mut self,
        id: JobId,
        credentials: &'credentials CredentialApplication,
        token: Option<&ReleaseToken>,
    ) -> Result<Option<PreparedDispatch<'credentials>>, SubmissionControlError> {
        if !self.quota_allows(credentials, token)? {
            return Ok(None);
        }
        let job = SubmissionJobRepository::load(self.store, id)
            .map_err(SubmissionControlError::Repository)?
            .ok_or(SubmissionControlError::Invalid)?;
        let Some(values) = credentials.credentials_for(&job.request().header().account) else {
            return Ok(None);
        };
        let preferences = (self.preferences)().map_err(|_| SubmissionControlError::Unavailable)?;
        let report = self
            .store
            .released_report(id)
            .map_err(SubmissionControlError::Repository)?
            .ok_or(SubmissionControlError::Invalid)?;
        let (gate, encoded) = self.review_reserved(&job, report, &preferences, credentials)?;
        let image = encoded
            .as_ref()
            .map(|encoded| encoded.base64_for(gate.report()))
            .transpose()
            .map_err(|_| SubmissionControlError::AttachmentUnavailable)?;
        let (prepared, hash) = self
            .client
            .prepare_with_screenshot(job.request(), image)
            .map_err(|_| SubmissionControlError::Invalid)?;
        let current = (self.preferences)().map_err(|_| SubmissionControlError::Unavailable)?;
        if current.reviewer != preferences.reviewer || current.age != preferences.age {
            return Err(SubmissionControlError::Conflict);
        }
        let plan = ManualDispatch::prepare(&job, &gate, credentials, hash, prepared.date_added())
            .map_err(SubmissionControlError::Dispatch)?
            .with_selected_token(credentials, token.map(ReleaseToken::for_validation));
        Ok(Some(PreparedDispatch {
            plan,
            prepared,
            preferences,
            values,
        }))
    }
    fn review_reserved(
        &mut self,
        job: &SubmissionJob,
        report: Report,
        preferences: &SubmissionQueuePreferences,
        credentials: &CredentialApplication,
    ) -> Result<(SubmissionGate, Option<EncodedAttachment>), SubmissionControlError> {
        let context_snapshot = resources::reference_snapshot(
            self.store,
            job.request().header().environment,
            self.clock.now(),
        )?;
        if let tallyline_submission::api::model::SendReadiness::Held(reason) =
            tallyline_workflows::api::assess_manual_send_context(
                job.request(),
                &report,
                &context_snapshot,
                credentials.state(),
                preferences.age,
                self.clock.now(),
            )
        {
            self.queue.defer_context(job, reason, self.clock.now());
            return Err(SubmissionControlError::Dispatch(ManualDispatchError::Held(
                reason,
            )));
        }
        let (gate, encoded) = resources::gate(
            self.store,
            self.copies,
            report,
            &job.request().header().account,
            job.request().header().mode,
            preferences,
            self.clock.now(),
        )?;
        let blocks = gate.blocks(gate.report());
        if !blocks.is_empty() {
            let returned = self
                .store
                .return_manual_review(job, &gate, self.clock.now())
                .map_err(SubmissionControlError::Dispatch)?;
            self.queue.update(returned);
            return Err(SubmissionControlError::Dispatch(
                ManualDispatchError::Blocked(blocks),
            ));
        }
        Ok((gate, encoded))
    }
    pub(super) fn reserve_candidate(
        &mut self,
        credentials: &CredentialApplication,
        token: Option<&ReleaseToken>,
    ) -> Result<Option<(JobId, PendingSubmissionReservation)>, SubmissionControlError> {
        if !self.quota_allows(credentials, token)? {
            return Ok(None);
        }
        let attempts = self.queue.account_attempts()?;
        let mut candidates = self
            .queue
            .jobs()
            .iter()
            .filter(|job| job.phase() == JobPhase::Queued)
            .map(|job| (job.request().header().observed_at, job.id()))
            .collect::<Vec<_>>();
        candidates.sort_unstable();
        for (_, id) in candidates {
            let Some(job) = self.queue.jobs().iter().find(|job| job.id() == id) else {
                continue;
            };
            if credentials
                .credentials_for(&job.request().header().account)
                .is_none()
                || self.queue.context_deferred(id)
            {
                continue;
            }
            let schedule = SubmissionSchedule::assess(job, self.queue.jobs(), self.clock.now());
            if !matches!(
                schedule,
                ScheduleDecision::Ready
                    | ScheduleDecision::NewerObservationSent
                    | ScheduleDecision::UnresolvedNewerObservation
            ) {
                continue;
            }
            if schedule == ScheduleDecision::Ready
                && tallyline_submission::api::SubmissionBudget::assess(
                    job.request(),
                    &attempts,
                    self.clock.now(),
                ) != tallyline_submission::api::model::RowBudgetDecision::Ready
            {
                continue;
            }
            let reservation = self
                .client
                .begin_submission_reservation()
                .map_err(|_| SubmissionControlError::Unavailable)?;
            return Ok(Some((id, reservation)));
        }
        Ok(None)
    }
    fn quota_allows(
        &mut self,
        credentials: &CredentialApplication,
        token: Option<&ReleaseToken>,
    ) -> Result<bool, SubmissionControlError> {
        let unproven = TokenAdmission::default();
        let proof = credentials
            .token_admission(token.map(ReleaseToken::for_validation))
            .unwrap_or(&unproven);
        let durable = SubmissionQuotaRepository::quota_allows_admission(self.store, proof)
            .map_err(SubmissionControlError::Repository)?;
        Ok(durable && !self.client.quota_blocks(proof))
    }
    pub(super) fn begin_attempt(
        &mut self,
        prepared: &PreparedDispatch<'_>,
    ) -> Result<SubmissionJob, SubmissionControlError> {
        let sending = self
            .store
            .begin_manual_dispatch(&prepared.plan, self.clock.now())
            .map_err(SubmissionControlError::Dispatch)?;
        self.queue.update(sending.clone());
        Ok(sending)
    }
    pub(super) fn now(&self) -> Timestamp {
        self.clock.now()
    }
    pub(super) fn publish_started(&mut self, sending: SubmissionJob) {
        self.queue.update(sending);
    }
    pub(super) fn dispatch_if_current(
        &mut self,
        work: PreparedDispatch<'_>,
        slot: SubmissionSlot,
        token: Option<&ReleaseToken>,
    ) -> Result<Option<PendingSubmission>, SubmissionControlError> {
        let PreparedDispatch {
            plan,
            prepared,
            preferences,
            values,
        } = work;
        if tallyline_submission::api::SubmissionServiceFenceRepository::service_fences(self.store)
            .map_err(SubmissionControlError::Repository)?
            .iter()
            .any(|fence| fence.blocks_admission(plan.token_admission()))
        {
            return Ok(None);
        }
        let history = self
            .store
            .queue_jobs()
            .map_err(SubmissionControlError::Repository)?;
        let current = (self.preferences)().map_err(|_| SubmissionControlError::Unavailable)?;
        let checked = if current.reviewer == preferences.reviewer && current.age == preferences.age
        {
            plan.recheck(self.clock.now(), &history)
        } else {
            Err(ManualDispatchError::Conflict)
        };
        Ok(match checked {
            Ok(_) => Some(slot.dispatch(prepared, values, token.map(ReleaseToken::for_validation))),
            Err(_) => None,
        })
    }
}
pub(super) struct PreparedDispatch<'a> {
    plan: ManualDispatch,
    prepared: PreparedUexSubmission,
    preferences: SubmissionQueuePreferences,
    values: &'a CredentialValues,
}

impl PreparedDispatch<'_> {
    pub(super) fn plan(&self) -> &ManualDispatch {
        &self.plan
    }
}
