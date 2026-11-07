// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Owned positive read results sealed once for save-only persistence retries.

use crate::api::model::{
    InvalidSubmission, LifecycleReadBatch, LifecycleReadClaim, UexLifecycleObservation,
    UexLifecycleRow,
};
use std::collections::{BTreeMap, BTreeSet};
use tallyline_shared_kernel::api::Clock;

/// A closed read failure that supplies no negative lifecycle evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifecycleReadFailure {
    /// Authentication, quota, HTTP or processing prevented a usable answer.
    Unavailable,
    /// The bounded answer was malformed, ambiguous or undocumented.
    InvalidResponse,
}

/// A bounded positive result retaining ownership of the exact durable admissions.
#[derive(Debug, PartialEq, Eq)]
pub struct LifecycleReadResponse {
    batch: LifecycleReadBatch,
    rows: Vec<UexLifecycleRow>,
}
impl LifecycleReadResponse {
    /// Binds unique positive rows to exact admitted proofs without positional inference.
    ///
    /// # Errors
    /// Refuses unrelated or repeated remote identities.
    pub fn try_new(
        batch: LifecycleReadBatch,
        rows: Vec<UexLifecycleRow>,
    ) -> Result<Self, InvalidSubmission> {
        let mut seen = BTreeSet::new();
        if rows.iter().any(|row| {
            !batch
                .claims()
                .iter()
                .any(|claim| claim.targets().any(|target| target == row.receipt()))
                || !seen.insert((row.receipt().id(), row.receipt().date_added()))
        }) {
            return Err(InvalidSubmission);
        }
        Ok(Self { batch, rows })
    }
    /// Seals the controller-observed check time once after transport and verification.
    ///
    /// # Errors
    /// Refuses a clock preceding the durable read admission.
    pub fn complete(self, clock: &dyn Clock) -> Result<CompletedLifecycleRead, InvalidSubmission> {
        let checked_at = clock.now();
        if checked_at < self.batch.admitted_at() {
            return Err(InvalidSubmission);
        }
        let mut rows = self
            .rows
            .into_iter()
            .map(|row| ((row.receipt().id(), row.receipt().date_added()), row))
            .collect::<BTreeMap<_, _>>();
        let claims = self
            .batch
            .into_claims()
            .into_iter()
            .map(|claim| {
                let observations = claim
                    .targets()
                    .filter_map(|target| rows.remove(&(target.id(), target.date_added())))
                    .map(|row| row.checked(checked_at))
                    .collect();
                CompletedLifecycleClaim {
                    claim,
                    observations,
                }
            })
            .collect();
        Ok(CompletedLifecycleRead { claims })
    }
}

/// A completed read whose exact check time survives persistence retries.
#[derive(Debug, PartialEq, Eq)]
pub struct CompletedLifecycleRead {
    claims: Vec<CompletedLifecycleClaim>,
}
impl CompletedLifecycleRead {
    /// Borrows owned claims and their positive observations for save-only retries.
    #[must_use]
    pub fn claims(&self) -> &[CompletedLifecycleClaim] {
        &self.claims
    }
}

/// One exact durable admission and its completed positive observations.
#[derive(Debug, PartialEq, Eq)]
pub struct CompletedLifecycleClaim {
    claim: LifecycleReadClaim,
    observations: Vec<UexLifecycleObservation>,
}
impl CompletedLifecycleClaim {
    /// Borrows the exact source and generation admitted before HTTP.
    #[must_use]
    pub const fn claim(&self) -> &LifecycleReadClaim {
        &self.claim
    }
    /// Borrows positive observations without filling gaps in the returned window.
    #[must_use]
    pub fn observations(&self) -> &[UexLifecycleObservation] {
        &self.observations
    }
}
