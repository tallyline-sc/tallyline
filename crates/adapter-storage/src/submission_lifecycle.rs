// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Same-database lifecycle admission and exact accepted-prior reversal.

use crate::api::{SqliteReferenceCache, StorageError};
use rusqlite::TransactionBehavior;
use std::collections::{BTreeMap, BTreeSet};
use tallyline_shared_kernel::api::model::{EventId, EventSource, Timestamp};
use tallyline_submission::api::model::{
    JobId, LifecycleReadBatch, LifecycleReadClaim, LifecycleRefresh, SubmissionJob,
    SubmissionLifecycle, SubmissionLifecycleSettings, SubmissionRepositoryError,
    UexLifecycleObservation,
};
use tallyline_workflows::api::SubmissionLifecycleStore;

fn invalid<T>(_: T) -> SubmissionRepositoryError {
    SubmissionRepositoryError::Invalid
}
fn unavailable(_: rusqlite::Error) -> SubmissionRepositoryError {
    SubmissionRepositoryError::Unavailable
}
fn storage_error(error: StorageError) -> SubmissionRepositoryError {
    match error {
        StorageError::InvalidCache
        | StorageError::InvalidBackup
        | StorageError::MigrationBackupRequired
        | StorageError::UnsupportedSchema { .. } => SubmissionRepositoryError::Invalid,
        StorageError::Database(_) | StorageError::Filesystem(_) => {
            SubmissionRepositoryError::Unavailable
        }
    }
}
impl SubmissionLifecycleStore for SqliteReferenceCache {
    fn load_lifecycle(
        &mut self,
        id: JobId,
    ) -> Result<Option<SubmissionLifecycle>, SubmissionRepositoryError> {
        let Some(record) = crate::lifecycle_record::record(&self.connection, id)? else {
            return Ok(None);
        };
        let source = crate::lifecycle_source::load(&self.connection, id)?;
        record.decode(&source).map(Some)
    }
    fn claim_lifecycle_read(
        &mut self,
        expected: &SubmissionJob,
        refresh: LifecycleRefresh,
        at: Timestamp,
    ) -> Result<Option<LifecycleReadClaim>, SubmissionRepositoryError> {
        self.claim_lifecycle_batch(&[expected], refresh, at)
            .map(|batch| batch.and_then(|batch| batch.into_claims().into_iter().next()))
    }
    fn claim_lifecycle_batch(
        &mut self,
        jobs: &[&SubmissionJob],
        refresh: LifecycleRefresh,
        at: Timestamp,
    ) -> Result<Option<LifecycleReadBatch>, SubmissionRepositoryError> {
        let Some(first) = jobs.first() else {
            return Ok(None);
        };
        let account = &first.request().header().account;
        let mut identities = BTreeSet::new();
        for expected in jobs {
            if &expected.request().header().account != account || !identities.insert(expected.id())
            {
                return Err(SubmissionRepositoryError::Invalid);
            }
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(unavailable)?;
        if refresh == LifecycleRefresh::Manual {
            let latest = crate::lifecycle_record::latest_manual(&transaction, account)?;
            if SubmissionLifecycleSettings::default()
                .manual_due_at(at, latest)
                .is_none_or(|due| at < due)
            {
                return Ok(None);
            }
        }
        let settings = SubmissionLifecycleSettings::default();
        let mut claims = Vec::new();
        let mut expected_revisions = BTreeMap::new();
        let mut targets = 0usize;
        for expected in jobs {
            if claims.len() == settings.max_batch_jobs() || targets == settings.max_batch_targets()
            {
                break;
            }
            let source = crate::lifecycle_source::load(&transaction, expected.id())?;
            if source.source() != *expected {
                return Err(SubmissionRepositoryError::Conflict);
            }
            let stored = crate::lifecycle_record::load(&transaction, &source)?;
            let expected_revision = stored.as_ref().map(SubmissionLifecycle::revision);
            let lifecycle = stored.unwrap_or(source);
            if let Some(claim) = lifecycle.claim(refresh, at).map_err(invalid)? {
                let next_targets = targets + claim.targets().count();
                if !settings.admits_batch(claims.len() + 1, next_targets) {
                    continue;
                }
                expected_revisions.insert(claim.lifecycle().job(), expected_revision);
                targets = next_targets;
                claims.push(claim);
            }
        }
        if claims.is_empty() {
            return Ok(None);
        }
        let batch = LifecycleReadBatch::try_new(claims).map_err(invalid)?;
        for claim in batch.claims() {
            let expected = expected_revisions
                .get(&claim.lifecycle().job())
                .copied()
                .ok_or(SubmissionRepositoryError::Invalid)?;
            crate::lifecycle_record::save(&transaction, claim.lifecycle(), expected)?;
        }
        transaction.commit().map_err(unavailable)?;
        Ok(Some(batch))
    }
    fn commit_lifecycle_read(
        &mut self,
        claim: &LifecycleReadClaim,
        observations: &[UexLifecycleObservation],
    ) -> Result<SubmissionLifecycle, SubmissionRepositoryError> {
        let source = crate::lifecycle_source::load(&self.connection, claim.lifecycle().job())?;
        if source.source() != claim.lifecycle().source() {
            return Err(SubmissionRepositoryError::Conflict);
        }
        let next = claim.observed(observations).map_err(invalid)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(unavailable)?;
        crate::lifecycle_source::verify(&transaction, &source)?;
        let current = crate::lifecycle_record::load(&transaction, &source)?
            .ok_or(SubmissionRepositoryError::Invalid)?;
        if current == next {
            return Ok(current);
        }
        if &current != claim.lifecycle() {
            return Err(SubmissionRepositoryError::Conflict);
        }
        let history = crate::accepted_priors::load(&transaction).map_err(storage_error)?;
        let projected =
            tallyline_workflows::api::lifecycle_prior_history(&next, &history).map_err(invalid)?;
        crate::accepted_priors::save_projection(&transaction, &history, &projected)
            .map_err(storage_error)?;
        crate::lifecycle_record::save(&transaction, &next, Some(current.revision()))?;
        if current.observations() != next.observations() {
            let id = EventId::new(
                EventSource::SubmissionLifecycle,
                next.job().0,
                next.revision(),
            )
            .ok_or(SubmissionRepositoryError::Invalid)?;
            crate::queue::outbox(&transaction, "submission_lifecycle", id, "status_changed")?;
        }
        transaction.commit().map_err(unavailable)?;
        Ok(next)
    }
}
