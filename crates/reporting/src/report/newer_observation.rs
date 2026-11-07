// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact row and release-scope decisions against a newer accepted observation.

use super::{InvalidReport, Report, ReportChange, ReportKey};
use tallyline_shared_kernel::api::model::{
    CommodityId, PricePerScu, RowId, ScuQuantity, SubmissionMode, Timestamp, UexEnvironment,
};

/// The account, mode and newest accepted observation shown for an explicit decision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewerObservationEvidence {
    account: String,
    mode: SubmissionMode,
    environment: UexEnvironment,
    observed_at: Timestamp,
}
impl NewerObservationEvidence {
    /// Captures the exact history scope; the workflow must verify it against committed jobs.
    ///
    /// # Errors
    /// Refuses an absent accepted account identity.
    pub fn new(
        account: &str,
        mode: SubmissionMode,
        environment: UexEnvironment,
        observed_at: Timestamp,
    ) -> Result<Self, InvalidReport> {
        if account.trim().is_empty() {
            return Err(InvalidReport);
        }
        Ok(Self {
            account: account.to_owned(),
            mode,
            environment,
            observed_at,
        })
    }
    /// Borrows the accepted account to which this decision applies.
    #[must_use]
    pub fn account(&self) -> &str {
        &self.account
    }
    /// Returns the exact release mode to which this decision applies.
    #[must_use]
    pub const fn mode(&self) -> SubmissionMode {
        self.mode
    }
    /// Returns the mapped target for which the decision was explicitly made.
    #[must_use]
    pub const fn environment(&self) -> UexEnvironment {
        self.environment
    }
    /// Returns the newest accepted observation covered by the decision.
    #[must_use]
    pub const fn observed_at(&self) -> Timestamp {
        self.observed_at
    }
}
/// Persistence-only exact values and context of a newer-observation decision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredNewerObservationApproval {
    /// Stable local row identity.
    pub row: RowId,
    /// Exact terminal, side and launcher environment.
    pub key: ReportKey,
    /// This Report's original observation time.
    pub observed_at: Timestamp,
    /// Exact commodity whose ordering was approved.
    pub commodity: CommodityId,
    /// Exact entered price approved by the user.
    pub price: PricePerScu,
    /// Exact entered stock approved by the user, retaining a known zero.
    pub stock: ScuQuantity,
    /// Exact entered inventory status approved by the user.
    pub status: std::num::NonZeroU8,
    /// Account, mode and newer accepted observation covered by the decision.
    pub evidence: NewerObservationEvidence,
    /// Time of the explicit user decision.
    pub confirmed_at: Timestamp,
}
/// An explicit approval that cannot transfer to another row value, context or release scope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewerObservationApproval {
    stored: StoredNewerObservationApproval,
}
impl NewerObservationApproval {
    /// Returns the stable row whose exact values were approved.
    #[must_use]
    pub const fn row(&self) -> RowId {
        self.stored.row
    }
    /// Returns the newest accepted observation covered by the decision.
    #[must_use]
    pub const fn observed_at(&self) -> Timestamp {
        self.stored.evidence.observed_at
    }
    /// Borrows the exact account and mode scope of the decision.
    #[must_use]
    pub const fn evidence(&self) -> &NewerObservationEvidence {
        &self.stored.evidence
    }
    pub(super) fn stored(&self) -> StoredNewerObservationApproval {
        self.stored.clone()
    }
    pub(super) fn restore(
        stored: StoredNewerObservationApproval,
        report: &Report,
    ) -> Result<Self, InvalidReport> {
        let row = report
            .rows()
            .iter()
            .find(|row| row.id() == stored.row)
            .ok_or(InvalidReport)?;
        if stored.key != report.key()
            || stored.observed_at != report.observed_at()
            || stored.commodity != row.commodity()
            || Some(stored.price) != row.price()
            || Some(stored.stock) != row.stock()
            || Some(stored.status) != row.status()
            || stored.evidence.observed_at <= report.observed_at()
            || stored.confirmed_at < report.observed_at()
        {
            return Err(InvalidReport);
        }
        Ok(Self { stored })
    }
}
impl Report {
    /// Borrows decisions bound to exact unchanged row values and their account/mode scope.
    #[must_use]
    pub fn newer_observation_confirmations(&self) -> &[NewerObservationApproval] {
        &self.lifecycle.newer_observations
    }
    /// Records an explicit exact-row decision against committed newer accepted history.
    ///
    /// # Errors
    /// Refuses released work, missing mandatory values and an observation that is not newer.
    pub fn confirm_newer_observation(
        &self,
        row: RowId,
        evidence: NewerObservationEvidence,
        now: Timestamp,
    ) -> Result<ReportChange, InvalidReport> {
        self.ensure_draft()?;
        let field = self
            .rows()
            .iter()
            .find(|field| field.id() == row)
            .ok_or(InvalidReport)?;
        let approval = NewerObservationApproval::restore(
            StoredNewerObservationApproval {
                row,
                key: self.key(),
                observed_at: self.observed_at(),
                commodity: field.commodity(),
                price: field.price().ok_or(InvalidReport)?,
                stock: field.stock().ok_or(InvalidReport)?,
                status: field.status().ok_or(InvalidReport)?,
                evidence,
                confirmed_at: now,
            },
            self,
        )?;
        if self.newer_observation_confirmations().iter().any(|old| {
            old.row() == row
                && old.evidence().account() == approval.evidence().account()
                && old.evidence().mode() == approval.evidence().mode()
                && old.evidence().environment() == approval.evidence().environment()
                && old.observed_at() >= approval.observed_at()
        }) {
            return ReportChange::new(self.version(), self.clone(), None);
        }
        let mut next = self.advanced()?;
        next.lifecycle.newer_observations.retain(|old| {
            old.row() != row
                || old.evidence().account() != approval.evidence().account()
                || old.evidence().mode() != approval.evidence().mode()
                || old.evidence().environment() != approval.evidence().environment()
        });
        next.lifecycle.newer_observations.push(approval);
        ReportChange::new(self.version(), next, None)
    }
    pub(super) fn revoke_newer_observation(&mut self, row: RowId) {
        self.lifecycle
            .newer_observations
            .retain(|approval| approval.row() != row);
    }
    pub(super) fn revoke_all_newer_observations(&mut self) {
        self.lifecycle.newer_observations.clear();
    }
}
