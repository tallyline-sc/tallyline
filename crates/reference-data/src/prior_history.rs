// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Idempotent accepted-row history and refresh-bounded active projection.

use crate::api::model::{AcceptedLocalPrior, InvalidPrior};
use tallyline_shared_kernel::api::model::{CommodityId, SubmissionJobId, UexEnvironment};

/// Whether a retained accepted row can still provide a local reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalPriorProjection {
    /// Accepted since the latest successful reference refresh.
    Active,
    /// A successful reference refresh discarded this local projection.
    Refreshed,
    /// A verified row outcome removed this local update.
    Revoked(LocalPriorRevocation),
}

/// A verified reason that an accepted local value must no longer act as a prior.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalPriorRevocation {
    /// A successful user-requested withdrawal removed the UEX row.
    Withdrawn,
    /// UEX subsequently declined the accepted row.
    Declined,
    /// UEX subsequently expired the accepted row.
    Expired,
}

/// Durable accepted evidence independent of its current reference eligibility.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcceptedPriorRecord {
    prior: AcceptedLocalPrior,
    projection: LocalPriorProjection,
}
impl AcceptedPriorRecord {
    /// Borrows the original accepted values and submission provenance.
    #[must_use]
    pub const fn prior(&self) -> &AcceptedLocalPrior {
        &self.prior
    }
    /// Returns the retained reference eligibility, without deleting history.
    #[must_use]
    pub const fn projection(&self) -> LocalPriorProjection {
        self.projection
    }
}

/// Immutable local acceptance history whose duplicate events never reactivate old values.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AcceptedPriorHistory {
    records: Vec<AcceptedPriorRecord>,
}
impl AcceptedPriorHistory {
    /// Borrows acceptance history in committed submission order.
    #[must_use]
    pub fn records(&self) -> &[AcceptedPriorRecord] {
        &self.records
    }
    /// Applies accepted production rows idempotently without changing earlier evidence.
    ///
    /// # Errors
    /// Refuses conflicting values or provenance for an already recorded submission row.
    pub fn accepted(&self, rows: &[AcceptedLocalPrior]) -> Result<Self, InvalidPrior> {
        let mut next = self.clone();
        for prior in rows {
            if next.records.iter().any(|record| {
                record.prior.context().submission == prior.context().submission
                    && record.prior.context() != prior.context()
            }) {
                return Err(InvalidPrior);
            }
            let existing = next.records.iter().find(|record| {
                record.prior.context().submission == prior.context().submission
                    && record.prior.commodity() == prior.commodity()
            });
            let Some(record) = existing else {
                next.records.push(AcceptedPriorRecord {
                    prior: prior.clone(),
                    projection: LocalPriorProjection::Active,
                });
                continue;
            };
            if record.prior != *prior {
                return Err(InvalidPrior);
            }
        }
        Ok(next)
    }
    /// Discards this environment's projection on every successful reference refresh.
    #[must_use]
    pub fn refreshed(&self, environment: UexEnvironment) -> Self {
        let mut next = self.clone();
        for record in &mut next.records {
            if record.prior.context().environment == environment
                && record.projection == LocalPriorProjection::Active
            {
                record.projection = LocalPriorProjection::Refreshed;
            }
        }
        next
    }
    /// Reverts one verified local update while retaining its accepted evidence.
    ///
    /// # Errors
    /// Refuses an unknown row or a conflicting final revocation reason.
    pub fn revoked(
        &self,
        submission: SubmissionJobId,
        commodity: CommodityId,
        reason: LocalPriorRevocation,
    ) -> Result<Self, InvalidPrior> {
        let mut next = self.clone();
        let record = next
            .records
            .iter_mut()
            .find(|record| {
                record.prior.context().submission == submission
                    && record.prior.commodity() == commodity
            })
            .ok_or(InvalidPrior)?;
        if matches!(record.projection, LocalPriorProjection::Revoked(previous) if previous != reason)
        {
            return Err(InvalidPrior);
        }
        record.projection = LocalPriorProjection::Revoked(reason);
        Ok(next)
    }
    /// Returns the last active accepted LIVE value per terminal, commodity and side.
    /// PTU records remain history and never become usable PTU price references.
    #[must_use]
    pub fn active(&self, environment: UexEnvironment) -> Vec<AcceptedLocalPrior> {
        if environment == UexEnvironment::Ptu {
            return Vec::new();
        }
        let mut active: Vec<AcceptedLocalPrior> = Vec::new();
        for record in &self.records {
            if record.projection != LocalPriorProjection::Active
                || record.prior.context().environment != environment
            {
                continue;
            }
            let prior = &record.prior;
            active.retain(|previous| {
                previous.context().terminal != prior.context().terminal
                    || previous.context().side != prior.context().side
                    || previous.commodity() != prior.commodity()
            });
            active.push(prior.clone());
        }
        active
    }
}

/// Persistence-only accepted evidence whose source and projection remain independent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredAcceptedPriorRecord {
    /// Exact already-validated accepted values and submission provenance.
    pub prior: AcceptedLocalPrior,
    /// Whether the retained local record may still supply a LIVE reference.
    pub projection: LocalPriorProjection,
}
impl AcceptedPriorHistory {
    /// Reconstitutes ordered durable evidence without reactivating replaced local values.
    ///
    /// # Errors
    /// Refuses duplicate submission rows and conflicting context for one submission.
    pub fn reconstitute(stored: Vec<StoredAcceptedPriorRecord>) -> Result<Self, InvalidPrior> {
        let mut identities = std::collections::BTreeSet::new();
        let mut contexts = std::collections::BTreeMap::<SubmissionJobId, AcceptedLocalPrior>::new();
        let mut records = Vec::with_capacity(stored.len());
        for record in stored {
            let prior = record.prior;
            let submission = prior.context().submission;
            if !identities.insert((submission, prior.commodity())) {
                return Err(InvalidPrior);
            }
            if let Some(previous) = contexts.get(&submission) {
                if previous.context() != prior.context() {
                    return Err(InvalidPrior);
                }
            } else {
                contexts.insert(submission, prior.clone());
            }
            records.push(AcceptedPriorRecord {
                prior,
                projection: record.projection,
            });
        }
        Ok(Self { records })
    }
}
