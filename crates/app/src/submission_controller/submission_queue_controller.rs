// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Sole queue owner coordinating private release, dispatch, projection and receipt components.

use super::decision_control::DecisionControl;
use super::dispatch_preparation::DispatchPreparation;
use super::dispatch_state::{DispatchProgress, DispatchState};
use super::queue_state::QueueState;
use super::receipt_enrichment::ReceiptEnrichment;
use super::receipt_resolution::ReceiptResolution;
use super::release_control::{ReleaseControl, ReleaseKind};
use super::{SubmissionControlError, SubmissionQueuePreferences};
use crate::api::HttpClient;
use tallyline_adapter_files::api::ManualWorkingCopies;
use tallyline_adapter_storage::api::SqliteReferenceCache;
use tallyline_reporting::api::model::{NewerObservationEvidence, Report};
use tallyline_reporting::api::{ReportRepository, RepositoryError};
use tallyline_shared_kernel::api::Clock;
use tallyline_shared_kernel::api::model::{ReportId, RowId, SubmissionMode};
use tallyline_submission::api::model::{
    JobId, ReleaseToken, SubmissionCommand, SubmissionJob, SubmissionResult, SubmissionWait,
};
use tallyline_submission::api::{CredentialApplication, SubmissionJobRepository};
use tallyline_workflows::api::{
    EventOutbox, ManualRelease, OutboxError, OutboxEvent, SubmissionQueueStore,
};

/// One concrete queue controller; call its synchronous operations only on the dedicated CPU owner.
pub struct SubmissionQueueController {
    store: SqliteReferenceCache,
    client: HttpClient,
    preferences: Box<dyn FnMut() -> std::io::Result<SubmissionQueuePreferences> + Send>,
    copies: Option<ManualWorkingCopies>,
    clock: Box<dyn Clock + Send>,
    queue: QueueState,
    dispatch: DispatchState,
    client_version: Option<tallyline_shared_kernel::api::model::ReleaseVersion>,
    receipt: ReceiptEnrichment,
    lifecycle: super::lifecycle_reads::LifecycleReads,
    recovery: super::refusal_recovery::RefusalRecovery,
    row_receipts: ReceiptResolution,
    pending_quotas: Vec<tallyline_submission::api::model::TokenQuotaObservation>,
}
impl SubmissionQueueController {
    /// Loads authenticated-read quota holds without fabricating submission history.
    ///
    /// # Errors
    /// Refuses corrupt or unavailable quota evidence.
    pub fn authenticated_quotas(
        &mut self,
    ) -> Result<Vec<tallyline_submission::api::model::TokenQuotaObservation>, SubmissionControlError>
    {
        tallyline_submission::api::SubmissionQuotaRepository::authenticated_quotas(&mut self.store)
            .map_err(SubmissionControlError::Repository)
    }
    /// Recovers interrupted work before accepting any new release or dispatch.
    ///
    /// # Errors
    /// Refuses unavailable or inconsistent durable history without assuming an empty queue.
    pub fn new(
        store: SqliteReferenceCache,
        client: HttpClient,
        preferences: impl FnMut() -> std::io::Result<SubmissionQueuePreferences> + Send + 'static,
        copies: Option<ManualWorkingCopies>,
        clock: impl Clock + Send + 'static,
    ) -> Result<Self, SubmissionControlError> {
        let information = crate::api::compiled_build_information()
            .map_err(|_| SubmissionControlError::Invalid)?;
        Self::new_with_build_information(store, client, preferences, copies, clock, information)
    }
    /// Recovers interrupted work with one immutable validated compiled identity.
    ///
    /// # Errors
    /// Refuses unavailable or inconsistent durable history.
    pub fn new_with_build_information(
        mut store: SqliteReferenceCache,
        client: HttpClient,
        preferences: impl FnMut() -> std::io::Result<SubmissionQueuePreferences> + Send + 'static,
        copies: Option<ManualWorkingCopies>,
        clock: impl Clock + Send + 'static,
        information: crate::api::model::BuildInformation,
    ) -> Result<Self, SubmissionControlError> {
        let client_version = (information.kind() == crate::api::model::BuildKind::Release)
            .then_some(information.update_baseline());
        store
            .recover_unfinished(clock.now())
            .map_err(SubmissionControlError::Repository)?;
        quota_control::publish(&mut store, &client)?;
        let jobs = store
            .queue_jobs()
            .map_err(SubmissionControlError::Repository)?;
        let mut mappings = Vec::new();
        for job in &jobs {
            if let Some(mapping) =
                tallyline_workflows::api::ReceiptEnrichmentStore::receipt_mapping(
                    &mut store,
                    job.id(),
                )
                .map_err(SubmissionControlError::Repository)?
            {
                mappings.push(mapping);
            }
        }
        let lifecycle = super::lifecycle_reads::LifecycleReads::new(&mut store, &jobs)?;
        Ok(Self {
            store,
            client,
            preferences: Box::new(preferences),
            copies,
            clock: Box::new(clock),
            queue: QueueState::new(jobs),
            dispatch: DispatchState::new(client_version),
            client_version,
            receipt: ReceiptEnrichment::new(mappings),
            lifecycle,
            recovery: super::refusal_recovery::RefusalRecovery::default(),
            row_receipts: ReceiptResolution::default(),
            pending_quotas: Vec::new(),
        })
    }

    /// Returns the compiled stable release identity; development supplies no recovery evidence.
    #[must_use]
    pub const fn client_version(
        &self,
    ) -> Option<tallyline_shared_kernel::api::model::ReleaseVersion> {
        self.client_version
    }
    /// Loads durable global service blocks independently of Report phases.
    ///
    /// # Errors
    /// Refuses unavailable or corrupt service state without opening dispatch.
    pub fn service_fences(
        &mut self,
    ) -> Result<Vec<tallyline_submission::api::model::SubmissionServiceFence>, SubmissionControlError>
    {
        tallyline_submission::api::SubmissionServiceFenceRepository::service_fences(&mut self.store)
            .map_err(SubmissionControlError::Repository)
    }
    /// Borrows only committed queue state; failed outcome saves remain Sending here.
    #[must_use]
    pub fn jobs(&self) -> &[SubmissionJob] {
        self.queue.jobs()
    }

    /// Borrows only persisted identity-bound mappings for already successful jobs.
    #[must_use]
    pub fn mappings(&self) -> &[tallyline_submission::api::model::VerifiedReceiptMapping] {
        self.receipt.mappings()
    }

    /// Borrows optional enrichment feedback without downgrading known submission success.
    #[must_use]
    pub const fn enrichment_failure(&self) -> Option<&SubmissionControlError> {
        self.receipt.failure()
    }

    /// Releases the exact displayed Draft after full review and bounded payload preparation.
    ///
    /// # Errors
    /// Refuses stale input, changed global mode, missing identity or any current release block.
    pub fn release_manual(
        &mut self,
        id: ReportId,
        expected: u64,
        mode: SubmissionMode,
        credentials: &CredentialApplication,
    ) -> Result<ManualRelease, SubmissionControlError> {
        self.release_selected(id, expected, ReleaseKind::Manual(mode), credentials)
    }

    /// Releases an onboarding test through the full manual gate with fixed dry-run mode.
    ///
    /// # Errors
    /// Refuses every ordinary release block; test mode grants no validation exemption.
    pub fn release_test(
        &mut self,
        id: ReportId,
        expected: u64,
        credentials: &CredentialApplication,
    ) -> Result<ManualRelease, SubmissionControlError> {
        self.release_selected(id, expected, ReleaseKind::OnboardingTest, credentials)
    }

    fn release_selected(
        &mut self,
        id: ReportId,
        expected: u64,
        kind: ReleaseKind,
        credentials: &CredentialApplication,
    ) -> Result<ManualRelease, SubmissionControlError> {
        let release = ReleaseControl::new(
            &mut self.store,
            &self.client,
            &mut *self.preferences,
            &mut self.copies,
            &*self.clock,
        )
        .release(id, expected, kind, credentials)?;
        self.queue.update(release.job().clone());
        Ok(release)
    }

    /// Cancels only an unsent job after its atomic Report and outbox save succeeds.
    ///
    /// # Errors
    /// Refuses stale revisions, Sending work and failed persistence without aborting a request.
    pub fn cancel(
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
        let change = job
            .apply(SubmissionCommand::Cancel)
            .map_err(SubmissionControlError::Refused)?;
        SubmissionJobRepository::save(&mut self.store, &change)
            .map_err(SubmissionControlError::Repository)?;
        self.dispatch.cancel_reservation(id);
        let committed = change.into_state();
        self.queue.update(committed.clone());
        Ok(committed)
    }

    /// Commits an explicit exact-version retry through the complete send-time gate.
    ///
    /// # Errors
    /// Refuses changed versions, uncertain or final outcomes, and failed persistence.
    pub fn retry_now(
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
        let command = if job.can_retry_after_update(self.client_version) {
            SubmissionCommand::RetryAfterUpdate {
                version: self.client_version.ok_or(SubmissionControlError::Invalid)?,
            }
        } else {
            SubmissionCommand::Retry
        };
        let change = job
            .apply(command)
            .map_err(SubmissionControlError::Refused)?;
        SubmissionJobRepository::save(&mut self.store, &change)
            .map_err(SubmissionControlError::Repository)?;
        let committed = change.into_state();
        self.queue.update(committed.clone());
        Ok(committed)
    }
    /// Records an explicit age confirmation against the exact displayed Draft and real clock.
    ///
    /// # Errors
    /// Refuses stale versions, released work, future or hard-expired observations and failed saves.
    pub fn confirm_observation_age(
        &mut self,
        id: ReportId,
        expected: u64,
    ) -> Result<tallyline_reporting::api::model::Report, SubmissionControlError> {
        DecisionControl::new(&mut self.store, &mut *self.preferences, &*self.clock)
            .confirm_observation_age(id, expected)
    }

    /// Records an explicit exact-row approval after verifying the newest committed history.
    ///
    /// # Errors
    /// Refuses stale Drafts, changed accounts/mappings, fabricated evidence and newer uncertainty.
    pub fn confirm_newer_observation(
        &mut self,
        id: ReportId,
        expected: u64,
        row: RowId,
        evidence: NewerObservationEvidence,
        credentials: &CredentialApplication,
    ) -> Result<Report, SubmissionControlError> {
        DecisionControl::new(&mut self.store, &mut *self.preferences, &*self.clock)
            .confirm_newer_observation(id, expected, row, evidence, credentials)
    }

    pub(crate) fn schedule_time(&self) -> tallyline_shared_kernel::api::model::Timestamp {
        self.clock.now()
    }

    /// Borrows passive waits for exact committed job revisions without granting send authority.
    #[must_use]
    pub fn waiting(&self) -> &[SubmissionWait] {
        self.queue.waiting()
    }

    /// Advances one bounded control step and republishes only changed committed state or waits.
    ///
    /// # Errors
    /// Retains known HTTP results after failed persistence; later steps retry their save only.
    pub fn tick(
        &mut self,
        credentials: &CredentialApplication,
        token: Option<&ReleaseToken>,
    ) -> Result<bool, SubmissionControlError> {
        let quota_progress = self.advance_quotas()?;
        let resolved = self.row_receipts.advance(
            &mut self.store,
            &mut self.queue,
            &*self.clock,
            credentials,
        )?;
        let recovered = self.recovery.advance(
            &mut self.store,
            &self.client,
            &mut self.queue,
            &*self.clock,
            credentials,
            token,
        )?;
        let previous = self.queue.waiting().to_vec();
        let result = self.advance(credentials, token);
        let refreshed = self.queue.refresh_waiting(credentials, &*self.clock);
        let progress = result?;
        quota_control::publish(&mut self.store, &self.client)?;
        refreshed?;
        let lifecycle_progress = self.lifecycle.automatic(
            &mut super::lifecycle_reads::LifecycleReadContext {
                store: &mut self.store,
                client: &self.client,
                clock: &*self.clock,
                credentials,
                token,
            },
            self.queue.jobs(),
            self.receipt.mappings(),
        );
        Ok(quota_progress
            || resolved
            || recovered
            || progress
            || lifecycle_progress
            || previous != self.queue.waiting())
    }
    fn advance(
        &mut self,
        credentials: &CredentialApplication,
        token: Option<&ReleaseToken>,
    ) -> Result<bool, SubmissionControlError> {
        if self.dispatch.is_active() {
            let committed =
                self.dispatch
                    .commit_completion(&mut self.store, &*self.clock, &mut self.queue)?;
            if let Some(job) = &committed
                && job.request().header().mode == SubmissionMode::Production
                && matches!(
                    job.result().map(SubmissionResult::final_result),
                    Some(SubmissionResult::Partial(_))
                )
            {
                self.row_receipts
                    .begin(&self.client, job.clone(), credentials, token)?;
            }
            if let Some(job) = &committed
                && let Err(error) = self.receipt.begin_enrichment(
                    &mut self.store,
                    &self.client,
                    job,
                    credentials,
                    token,
                )
            {
                self.receipt.failed(error);
            }
            let enriched = self.receipt.poll_enrichment(&mut self.store);
            return Ok(committed.is_some() || enriched);
        }
        let enriched = self.receipt.poll_enrichment(&mut self.store);
        let settings = tallyline_submission::api::model::SubmissionRetrySettings::default();
        let due = self
            .queue
            .jobs()
            .iter()
            .filter(|job| {
                settings
                    .automatic_retry_at(job)
                    .is_some_and(|at| at <= self.clock.now())
            })
            .cloned()
            .collect::<Vec<_>>();
        for job in due {
            let change = job
                .apply(SubmissionCommand::Retry)
                .map_err(SubmissionControlError::Refused)?;
            SubmissionJobRepository::save(&mut self.store, &change)
                .map_err(SubmissionControlError::Repository)?;
            self.queue.update(change.into_state());
        }
        let context = DispatchPreparation::new(
            &mut self.store,
            &self.client,
            &mut *self.preferences,
            &mut self.copies,
            &*self.clock,
            &mut self.queue,
        );
        match self.dispatch.advance(context, credentials, token)? {
            DispatchProgress::Idle => Ok(false),
            DispatchProgress::Started => Ok(true),
            DispatchProgress::Scheduled => Ok(enriched),
        }
    }

    /// Closes an interrupted request as unknown, or saves its already known result, without replay.
    ///
    /// # Errors
    /// Leaves durable write-ahead state intact when outcome persistence remains unavailable.
    pub fn shutdown(&mut self) -> Result<(), SubmissionControlError> {
        self.dispatch
            .shutdown(&mut self.store, &*self.clock, &mut self.queue)?;
        self.receipt.poll_enrichment(&mut self.store);
        Ok(())
    }
    pub(crate) fn draft(&mut self, id: ReportId) -> Result<Option<Report>, RepositoryError> {
        self.store.load_draft(id)
    }
    pub(crate) fn drafts(&mut self) -> Result<Vec<Report>, RepositoryError> {
        self.store.drafts()
    }
    pub(crate) fn pending_events(&mut self) -> Result<Vec<OutboxEvent>, OutboxError> {
        self.store.pending_events()
    }
    pub(crate) fn mark_delivered(&mut self, event: OutboxEvent) -> Result<(), OutboxError> {
        self.store.mark_delivered(event)
    }
}

mod resolution_control;

mod quota_control;

mod cooldown_split;

impl SubmissionQueueController {
    /// Borrows only durable lifecycle admissions and committed positive observations.
    #[must_use]
    pub fn lifecycles(&self) -> &[tallyline_submission::api::model::SubmissionLifecycle] {
        self.lifecycle.ledgers()
    }
    /// Reports whether one lifecycle read or a completed save-only retry is retained.
    #[must_use]
    pub const fn lifecycle_busy(&self) -> bool {
        self.lifecycle.is_busy()
    }
    /// Borrows optional read feedback without changing known delivery success.
    #[must_use]
    pub const fn lifecycle_failure(&self) -> Option<&SubmissionControlError> {
        self.lifecycle.failure()
    }
    /// Requests one account batch from an exact displayed source and lifecycle version.
    ///
    /// # Errors
    /// Refuses stale display state, active lifecycle work or unavailable admission.
    pub fn refresh_lifecycle(
        &mut self,
        id: JobId,
        expected: u64,
        expected_lifecycle: Option<u64>,
        credentials: &CredentialApplication,
        token: Option<&ReleaseToken>,
    ) -> Result<bool, SubmissionControlError> {
        let job = self
            .queue
            .jobs()
            .iter()
            .find(|job| job.id() == id && job.revision() == expected)
            .ok_or(SubmissionControlError::Conflict)?;
        if self
            .lifecycles()
            .iter()
            .find(|ledger| ledger.job() == id)
            .map(tallyline_submission::api::model::SubmissionLifecycle::revision)
            != expected_lifecycle
            || !super::lifecycle_reads::LifecycleReads::has_targets(job, self.receipt.mappings())
        {
            return Err(SubmissionControlError::Conflict);
        }
        let jobs = self
            .queue
            .jobs()
            .iter()
            .filter(|candidate| {
                candidate.request().header().account == job.request().header().account
                    && super::lifecycle_reads::LifecycleReads::has_targets(
                        candidate,
                        self.receipt.mappings(),
                    )
            })
            .collect::<Vec<_>>();
        self.lifecycle.begin(
            &mut super::lifecycle_reads::LifecycleReadContext {
                store: &mut self.store,
                client: &self.client,
                clock: &*self.clock,
                credentials,
                token,
            },
            &jobs,
            tallyline_submission::api::model::LifecycleRefresh::Manual,
        )
    }
}
