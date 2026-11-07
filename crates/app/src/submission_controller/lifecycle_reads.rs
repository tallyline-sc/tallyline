// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! One account lifecycle read retains sealed evidence until its exact save succeeds.

use super::SubmissionControlError;
use crate::api::HttpClient;
use tallyline_adapter_storage::api::SqliteReferenceCache;
use tallyline_shared_kernel::api::Clock;
use tallyline_submission::api::CredentialApplication;
use tallyline_submission::api::model::{
    CompletedLifecycleRead, LifecycleReadFailure, LifecycleReadResponse, LifecycleRefresh,
    ReleaseToken, SubmissionJob, SubmissionLifecycle, SubmissionRepositoryError, SubmissionResult,
    VerifiedReceiptMapping,
};
use tallyline_workflows::api::SubmissionLifecycleStore;
use tokio::sync::oneshot::{Receiver, error::TryRecvError};

pub(super) struct LifecycleReadContext<'context> {
    pub(super) store: &'context mut SqliteReferenceCache,
    pub(super) client: &'context HttpClient,
    pub(super) clock: &'context dyn Clock,
    pub(super) credentials: &'context CredentialApplication,
    pub(super) token: Option<&'context ReleaseToken>,
}
enum LifecycleWork {
    Reading(Receiver<Result<LifecycleReadResponse, LifecycleReadFailure>>),
    Saving(CompletedLifecycleRead),
}

pub(super) struct LifecycleReads {
    work: Option<LifecycleWork>,
    ledgers: Vec<SubmissionLifecycle>,
    failure: Option<SubmissionControlError>,
    manual_deadlines:
        std::collections::BTreeMap<String, tallyline_shared_kernel::api::model::Timestamp>,
}
impl LifecycleReads {
    pub(super) fn new(
        store: &mut SqliteReferenceCache,
        jobs: &[SubmissionJob],
    ) -> Result<Self, SubmissionControlError> {
        let mut ledgers = Vec::new();
        for job in jobs {
            if let Some(ledger) = store
                .load_lifecycle(job.id())
                .map_err(SubmissionControlError::Repository)?
            {
                ledgers.push(ledger);
            }
        }
        let mut reads = Self {
            work: None,
            ledgers: Vec::new(),
            failure: None,
            manual_deadlines: std::collections::BTreeMap::new(),
        };
        for ledger in ledgers {
            reads.update(ledger);
        }
        Ok(reads)
    }
    pub(super) const fn is_busy(&self) -> bool {
        self.work.is_some()
    }
    pub(super) fn ledgers(&self) -> &[SubmissionLifecycle] {
        &self.ledgers
    }
    pub(super) const fn failure(&self) -> Option<&SubmissionControlError> {
        self.failure.as_ref()
    }
    fn update(&mut self, ledger: SubmissionLifecycle) {
        let account = &ledger.source().request().header().account;
        let previous_manual = self
            .ledgers
            .iter()
            .filter(|current| &current.source().request().header().account == account)
            .filter_map(SubmissionLifecycle::manual_admitted_at)
            .max();
        if let Some(deadline) = ledger
            .manual_admitted_at()
            .filter(|at| previous_manual.is_none_or(|previous| *at > previous))
            .and_then(|at| {
                tallyline_submission::api::model::SubmissionLifecycleSettings::default()
                    .manual_due_at(at, Some(at))
            })
        {
            self.manual_deadlines
                .entry(account.clone())
                .and_modify(|current| *current = (*current).max(deadline))
                .or_insert(deadline);
        }
        if let Some(current) = self
            .ledgers
            .iter_mut()
            .find(|current| current.job() == ledger.job())
        {
            *current = ledger;
        } else {
            self.ledgers.push(ledger);
        }
    }
    pub(super) fn has_targets(job: &SubmissionJob, mappings: &[VerifiedReceiptMapping]) -> bool {
        job.request().header().mode
            == tallyline_shared_kernel::api::model::SubmissionMode::Production
            && (job.verified_row_receipts().next().is_some()
                || matches!(job.result().map(SubmissionResult::final_result), Some(SubmissionResult::ProductionSucceeded(receipts)) if !receipts.is_empty())
                || mappings.iter().any(|mapping| mapping.matches_job(job)))
    }
    pub(super) fn begin(
        &mut self,
        context: &mut LifecycleReadContext<'_>,
        jobs: &[&SubmissionJob],
        refresh: LifecycleRefresh,
    ) -> Result<bool, SubmissionControlError> {
        let store = &mut *context.store;
        let client = context.client;
        let clock = context.clock;
        let credentials = context.credentials;
        let token = context.token;
        if self.work.is_some() {
            return Err(SubmissionControlError::Unavailable);
        }
        let Some(first) = jobs.first() else {
            return Ok(false);
        };
        let Some(values) = credentials.credentials_for(&first.request().header().account) else {
            return Ok(false);
        };
        if client.quota_blocks(values.token_admission()) {
            return Ok(false);
        }
        let Some(batch) = store
            .claim_lifecycle_batch(jobs, refresh, clock.now())
            .map_err(SubmissionControlError::Repository)?
        else {
            return Ok(false);
        };
        for claim in batch.claims() {
            self.update(claim.lifecycle().clone());
        }
        let reply = client
            .begin_lifecycle_read(batch, values, token.map(ReleaseToken::for_validation))
            .map_err(|_| SubmissionControlError::Unavailable)?;
        self.work = Some(LifecycleWork::Reading(reply));
        self.failure = None;
        Ok(true)
    }
    fn advance(
        &mut self,
        store: &mut SqliteReferenceCache,
        clock: &dyn Clock,
    ) -> Result<bool, SubmissionControlError> {
        let Some(work) = self.work.take() else {
            return Ok(false);
        };
        let completed = match work {
            LifecycleWork::Reading(mut reply) => match reply.try_recv() {
                Ok(Ok(response)) => response
                    .complete(clock)
                    .map_err(|_| SubmissionControlError::Invalid)?,
                Ok(Err(_)) | Err(TryRecvError::Closed) => {
                    return Err(SubmissionControlError::Unavailable);
                }
                Err(TryRecvError::Empty) => {
                    self.work = Some(LifecycleWork::Reading(reply));
                    return Ok(false);
                }
            },
            LifecycleWork::Saving(completed) => completed,
        };
        for result in completed.claims() {
            match store.commit_lifecycle_read(result.claim(), result.observations()) {
                Ok(ledger) => self.update(ledger),
                Err(error) => {
                    if error == SubmissionRepositoryError::Unavailable {
                        self.work = Some(LifecycleWork::Saving(completed));
                    }
                    return Err(SubmissionControlError::Repository(error));
                }
            }
        }
        self.failure = None;
        Ok(true)
    }
    pub(super) fn poll(&mut self, store: &mut SqliteReferenceCache, clock: &dyn Clock) -> bool {
        match self.advance(store, clock) {
            Ok(changed) => changed,
            Err(error) => {
                self.failure = Some(error);
                false
            }
        }
    }
    fn refresh_sources(
        &mut self,
        store: &mut SqliteReferenceCache,
        jobs: &[SubmissionJob],
    ) -> Result<bool, SubmissionControlError> {
        let mut changed = false;
        for job in jobs {
            if self
                .ledgers
                .iter()
                .any(|ledger| ledger.job() == job.id() && ledger.source() != job)
            {
                let ledger = store
                    .load_lifecycle(job.id())
                    .map_err(SubmissionControlError::Repository)?
                    .ok_or(SubmissionControlError::Invalid)?;
                self.update(ledger);
                changed = true;
            }
        }
        Ok(changed)
    }
    pub(super) fn automatic(
        &mut self,
        context: &mut LifecycleReadContext<'_>,
        jobs: &[SubmissionJob],
        mappings: &[VerifiedReceiptMapping],
    ) -> bool {
        let had_work = self.work.is_some();
        let previous_failure = self.failure.clone();
        let source_changed = match self.refresh_sources(context.store, jobs) {
            Ok(changed) => changed,
            Err(error) => {
                let changed = self.failure.as_ref() != Some(&error);
                self.failure = Some(error);
                return changed;
            }
        };
        let mut changed = self.poll(context.store, context.clock) || source_changed;
        let previous_deadlines = self.manual_deadlines.len();
        let now = context.clock.now();
        self.manual_deadlines.retain(|_, deadline| now < *deadline);
        changed |= self.manual_deadlines.len() != previous_deadlines;
        if had_work || self.work.is_some() {
            return changed || had_work != self.work.is_some() || previous_failure != self.failure;
        }
        let accounts = jobs
            .iter()
            .filter(|job| Self::has_targets(job, mappings))
            .map(|job| job.request().header().account.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        for account in accounts {
            let targets = jobs
                .iter()
                .filter(|job| {
                    job.request().header().account == account && Self::has_targets(job, mappings)
                })
                .collect::<Vec<_>>();
            match self.begin(context, &targets, LifecycleRefresh::Automatic) {
                Ok(true) => return true,
                Ok(false) => {}
                Err(error) => {
                    self.failure = Some(error);
                    return changed;
                }
            }
        }
        changed
    }
}
