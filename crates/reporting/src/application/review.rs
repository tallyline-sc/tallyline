// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Atomic manual reassessment and immutable review preparation.

use super::{
    Clock, ReferenceSnapshot, ReportId, ReportRepository, ReportingApplication, RepositoryError,
};
use crate::api::model::{ManualReview, Report};

impl<R: ReportRepository, C: Clock> ReportingApplication<R, C> {
    /// Reassesses prices, stocks and statuses atomically and returns the committed Draft with its assessments.
    ///
    /// # Errors
    /// Refuses stale versions, unavailable exact comparisons, mismatched references or storage failure.
    pub fn review_manual(
        &mut self,
        id: ReportId,
        expected: u64,
        snapshot: &ReferenceSnapshot,
    ) -> Result<ManualReview, RepositoryError> {
        let report = self.draft_for_reference(id, snapshot)?;
        if report.version() != expected {
            return Err(RepositoryError::Conflict);
        }
        let review = self.reassess_report(report, snapshot)?;
        if review.report().version() != expected {
            self.repository.save_draft(review.report(), expected)?;
        }
        Ok(review)
    }

    /// Prepares fresh numeric decisions for this exact Report without accessing its repository.
    /// The owning workflow must atomically persist the returned Report before publishing readiness.
    ///
    /// # Errors
    /// Refuses mismatched reference context or unavailable exact arithmetic without changing source values.
    pub fn reassess_report(
        &self,
        report: Report,
        snapshot: &ReferenceSnapshot,
    ) -> Result<ManualReview, RepositoryError> {
        self.manual_reviewer()
            .reassess(report, snapshot, self.clock.now())
    }
}

impl<R: ReportRepository, C: Clock> ReportingApplication<R, C> {
    /// Captures the current immutable review policy for repository-independent workflow preparation.
    #[must_use]
    pub fn manual_reviewer(&self) -> crate::api::ManualReviewer {
        crate::api::ManualReviewer::new(
            self.mapping.clone(),
            self.stock_settings,
            self.price_tolerance_override,
        )
    }
}
