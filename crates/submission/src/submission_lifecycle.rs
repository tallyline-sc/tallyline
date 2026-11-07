// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Independently versioned lifecycle evidence preserves the immutable delivery aggregate.

use crate::api::model::{
    InvalidSubmission, JobId, LifecycleRefresh, SubmissionJob, SubmissionResult,
    UexLifecycleObservation, VerifiedReceiptMapping, VerifiedUexReport,
};
use std::collections::{BTreeMap, BTreeSet};
use tallyline_shared_kernel::api::model::{RowId, Timestamp};

/// Latest verified lifecycle values and durable read admission for one accepted submission.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubmissionLifecycle {
    source: SubmissionJob,
    receipts: Vec<VerifiedUexReport>,
    revision: u64,
    admitted_at: Option<Timestamp>,
    manual_admitted_at: Option<Timestamp>,
    observations: Vec<UexLifecycleObservation>,
    row_admissions: BTreeMap<u64, Timestamp>,
}

/// An exact durable read admission whose revision rejects superseded replies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LifecycleReadClaim {
    lifecycle: SubmissionLifecycle,
    targets: Vec<RowId>,
}

/// Persistence-only lifecycle values validated against the current accepted source and proofs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredSubmissionLifecycle {
    /// Independent optimistic-concurrency version.
    pub revision: u64,
    /// Latest committed read admission, including interrupted reads.
    pub admitted_at: Option<Timestamp>,
    /// Latest explicit manual admission, retained through automatic reads.
    pub manual_admitted_at: Option<Timestamp>,
    /// Latest positive evidence per verified row.
    pub observations: Vec<UexLifecycleObservation>,
    /// Latest durable admission for each previously admitted row.
    pub row_admissions: Vec<StoredLifecycleRowAdmission>,
}

/// Persistence-only admission of one verified local row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StoredLifecycleRowAdmission {
    /// Exact verified local row identity.
    pub row: RowId,
    /// Latest durable admission for this row.
    pub admitted_at: Timestamp,
}

impl SubmissionLifecycle {
    /// Seeds lifecycle targets from exact verified full production acceptance.
    ///
    /// # Errors
    /// Refuses proof metadata belonging to another successful job version.
    pub fn try_new(
        job: &SubmissionJob,
        mapping: &VerifiedReceiptMapping,
    ) -> Result<Self, InvalidSubmission> {
        if !mapping.matches_job(job) {
            return Err(InvalidSubmission);
        }
        Ok(Self::with_receipts(job, mapping.receipts().to_vec()))
    }
    /// Seeds targets from positive receipts already retained by the original job.
    ///
    /// # Errors
    /// Refuses test submissions, unverified identities and user statements without remote proof.
    pub fn try_from_job(job: &SubmissionJob) -> Result<Self, InvalidSubmission> {
        if job.request().header().mode
            != tallyline_shared_kernel::api::model::SubmissionMode::Production
        {
            return Err(InvalidSubmission);
        }
        let receipts = if let Some(SubmissionResult::ProductionSucceeded(receipts)) =
            job.result().map(SubmissionResult::final_result)
        {
            receipts.clone()
        } else {
            job.verified_row_receipts().cloned().collect::<Vec<_>>()
        };
        if receipts.is_empty() {
            return Err(InvalidSubmission);
        }
        Ok(Self::with_receipts(job, receipts))
    }
    fn with_receipts(job: &SubmissionJob, receipts: Vec<VerifiedUexReport>) -> Self {
        Self {
            source: job.clone(),
            receipts,
            revision: 1,
            admitted_at: None,
            manual_admitted_at: None,
            observations: Vec::new(),
            row_admissions: BTreeMap::new(),
        }
    }
    /// Restores lifecycle values only against the exact accepted source and verified row proofs.
    ///
    /// # Errors
    /// Refuses missing versions, duplicate rows or evidence outside the verified source.
    pub fn reconstitute(
        source: &Self,
        stored: StoredSubmissionLifecycle,
    ) -> Result<Self, InvalidSubmission> {
        let mut lifecycle = source.clone();
        let sent_at = lifecycle
            .source
            .attempts()
            .last()
            .ok_or(InvalidSubmission)?
            .started_at();
        for admission in &stored.row_admissions {
            if !lifecycle
                .receipts
                .iter()
                .any(|receipt| receipt.row() == admission.row)
                || admission.admitted_at < sent_at
                || lifecycle
                    .row_admissions
                    .insert(admission.row.0, admission.admitted_at)
                    .is_some()
            {
                return Err(InvalidSubmission);
            }
        }
        let mut rows = BTreeSet::new();
        if stored.admitted_at != lifecycle.row_admissions.values().copied().max()
            || stored
                .manual_admitted_at
                .is_some_and(|manual| manual < sent_at)
            || stored.revision == 0
            || stored
                .manual_admitted_at
                .is_some_and(|manual| stored.admitted_at.is_none_or(|at| manual > at))
            || stored.observations.iter().any(|observation| {
                !lifecycle.receipts.contains(observation.receipt())
                    || observation.checked_at() < sent_at
                    || lifecycle
                        .row_admitted_at(observation.receipt().row())
                        .is_none_or(|at| {
                            observation.status().is_final() && observation.checked_at() < at
                        })
                    || !rows.insert(observation.receipt().row().0)
            })
        {
            return Err(InvalidSubmission);
        }
        lifecycle.revision = stored.revision;
        lifecycle.admitted_at = stored.admitted_at;
        lifecycle.manual_admitted_at = stored.manual_admitted_at;
        lifecycle.observations = stored.observations;
        Ok(lifecycle)
    }
    /// Borrows the exact immutable accepted job to which this ledger belongs.
    #[must_use]
    pub const fn source(&self) -> &SubmissionJob {
        &self.source
    }
    /// Returns the original accepted submission identity.
    #[must_use]
    pub const fn job(&self) -> JobId {
        self.source.id()
    }
    /// Returns the lifecycle ledger's independent concurrency version.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }
    /// Borrows exact verified row identities admitted for lifecycle reads.
    #[must_use]
    pub fn receipts(&self) -> &[VerifiedUexReport] {
        &self.receipts
    }
    /// Borrows the latest positive evidence for the selected local row.
    #[must_use]
    pub fn observation(&self, row: RowId) -> Option<&UexLifecycleObservation> {
        self.observations
            .iter()
            .find(|observation| observation.receipt().row() == row)
    }
    /// Borrows the latest positive evidence without inventing values for missing rows.
    #[must_use]
    pub fn observations(&self) -> &[UexLifecycleObservation] {
        &self.observations
    }
    /// Returns the latest durable admission, including interrupted reads.
    #[must_use]
    pub const fn admitted_at(&self) -> Option<Timestamp> {
        self.admitted_at
    }
    /// Returns the latest explicit manual admission independently of automatic reads.
    #[must_use]
    pub const fn manual_admitted_at(&self) -> Option<Timestamp> {
        self.manual_admitted_at
    }
    /// Returns the latest durable admission for one verified row.
    #[must_use]
    pub fn row_admitted_at(&self, row: RowId) -> Option<Timestamp> {
        self.row_admissions.get(&row.0).copied()
    }
    /// Iterates durable per-row admission values for persistence.
    pub fn row_admissions(&self) -> impl Iterator<Item = StoredLifecycleRowAdmission> + '_ {
        self.row_admissions
            .iter()
            .map(|(&row, &admitted_at)| StoredLifecycleRowAdmission {
                row: RowId(row),
                admitted_at,
            })
    }
    /// Prepares an independently versioned read admission without changing delivery state.
    ///
    /// # Errors
    /// Refuses an exhausted ledger version or time preceding the original attempt.
    pub fn claim(
        &self,
        refresh: LifecycleRefresh,
        at: Timestamp,
    ) -> Result<Option<LifecycleReadClaim>, InvalidSubmission> {
        let sent_at = self
            .source
            .attempts()
            .last()
            .ok_or(InvalidSubmission)?
            .started_at();
        if at < sent_at || self.admitted_at.is_some_and(|previous| at < previous) {
            return Err(InvalidSubmission);
        }
        let settings = crate::api::model::SubmissionLifecycleSettings::default();
        if !settings.within_age(sent_at, at) {
            return Ok(None);
        }
        if refresh == LifecycleRefresh::Manual
            && settings
                .manual_due_at(sent_at, self.manual_admitted_at)
                .is_none_or(|due| at < due)
        {
            return Ok(None);
        }
        let targets: Vec<_> = self
            .receipts
            .iter()
            .filter(|receipt| {
                !self
                    .observation(receipt.row())
                    .is_some_and(|observation| observation.status().is_final())
            })
            .filter(|receipt| {
                refresh == LifecycleRefresh::Manual
                    || settings
                        .automatic_due_at(sent_at, self.row_admitted_at(receipt.row()))
                        .is_some_and(|due| at >= due)
            })
            .map(VerifiedUexReport::row)
            .collect();
        if targets.is_empty() {
            return Ok(None);
        }
        let mut next = self.clone();
        next.revision = next.revision.checked_add(1).ok_or(InvalidSubmission)?;
        next.admitted_at = Some(at);
        for row in &targets {
            next.row_admissions.insert(row.0, at);
        }
        next.manual_admitted_at = match refresh {
            LifecycleRefresh::Automatic => self.manual_admitted_at,
            LifecycleRefresh::Manual => Some(at),
        };
        Ok(Some(LifecycleReadClaim {
            lifecycle: next,
            targets,
        }))
    }
}
impl LifecycleReadClaim {
    /// Borrows the exact independently versioned state that must commit before HTTP starts.
    #[must_use]
    pub const fn lifecycle(&self) -> &SubmissionLifecycle {
        &self.lifecycle
    }
    /// Borrows only the exact non-final rows admitted for this read.
    pub fn targets(&self) -> impl Iterator<Item = &VerifiedUexReport> {
        self.lifecycle
            .receipts
            .iter()
            .filter(|receipt| self.targets.contains(&receipt.row()))
    }
    /// Applies only positive responses matching this exact admitted target set.
    ///
    /// # Errors
    /// Refuses unrelated or duplicate rows, pre-admission timestamps and version exhaustion.
    pub fn observed(
        &self,
        observations: &[UexLifecycleObservation],
    ) -> Result<SubmissionLifecycle, InvalidSubmission> {
        let admitted_at = self.lifecycle.admitted_at.ok_or(InvalidSubmission)?;
        let mut rows = BTreeSet::new();
        if observations.iter().any(|observation| {
            !self.targets.contains(&observation.receipt().row())
                || !self.lifecycle.receipts.contains(observation.receipt())
                || observation.checked_at() < admitted_at
                || !rows.insert(observation.receipt().row().0)
        }) {
            return Err(InvalidSubmission);
        }
        let mut next = self.lifecycle.clone();
        for observation in observations {
            next.observations
                .retain(|previous| previous.receipt().row() != observation.receipt().row());
            next.observations.push(observation.clone());
        }
        next.observations
            .sort_by_key(|observation| observation.receipt().row().0);
        next.revision = next.revision.checked_add(1).ok_or(InvalidSubmission)?;
        Ok(next)
    }
}
