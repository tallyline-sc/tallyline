// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Owned exact read admissions with fixed account, time and pending-work bounds.

use crate::api::model::{InvalidSubmission, LifecycleReadClaim, SubmissionLifecycleSettings};
use std::collections::BTreeSet;
use tallyline_shared_kernel::api::model::Timestamp;

/// One owned bounded request for exact lifecycle read admissions.
#[derive(Debug, PartialEq, Eq)]
pub struct LifecycleReadBatch {
    account: String,
    admitted_at: Timestamp,
    claims: Vec<LifecycleReadClaim>,
}
impl LifecycleReadBatch {
    /// Binds a nonempty bounded set of unique jobs to one account and admission time.
    ///
    /// # Errors
    /// Refuses mixed accounts or times, repeated local or remote identities and excessive job or target counts.
    pub fn try_new(claims: Vec<LifecycleReadClaim>) -> Result<Self, InvalidSubmission> {
        let settings = SubmissionLifecycleSettings::default();
        let first = claims.first().ok_or(InvalidSubmission)?;
        if claims.len() > settings.max_batch_jobs() {
            return Err(InvalidSubmission);
        }
        let account = first
            .lifecycle()
            .source()
            .request()
            .header()
            .account
            .clone();
        let admitted_at = first.lifecycle().admitted_at().ok_or(InvalidSubmission)?;
        let mut identities = BTreeSet::new();
        let mut targets = 0usize;
        let mut remote_identities = BTreeSet::new();
        for claim in &claims {
            if claim.lifecycle().source().request().header().account != account
                || claim.lifecycle().admitted_at() != Some(admitted_at)
                || !identities.insert(claim.lifecycle().job())
            {
                return Err(InvalidSubmission);
            }
            for target in claim.targets() {
                if !remote_identities.insert((target.id(), target.date_added())) {
                    return Err(InvalidSubmission);
                }
                targets += 1;
                if targets > settings.max_batch_targets() {
                    return Err(InvalidSubmission);
                }
            }
        }
        Ok(Self {
            account,
            admitted_at,
            claims,
        })
    }
    /// Borrows the exact username shared by every admitted target.
    #[must_use]
    pub fn account(&self) -> &str {
        &self.account
    }
    /// Returns the common durable admission time.
    #[must_use]
    pub const fn admitted_at(&self) -> Timestamp {
        self.admitted_at
    }
    /// Borrows exact independently versioned claims without copying their released snapshots.
    #[must_use]
    pub fn claims(&self) -> &[LifecycleReadClaim] {
        &self.claims
    }
    /// Returns the owned claims without duplicating their immutable source state.
    #[must_use]
    pub fn into_claims(self) -> Vec<LifecycleReadClaim> {
        self.claims
    }
}
