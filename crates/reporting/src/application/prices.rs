// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Read-only manual price review application commands.

use super::ReportingApplication;
use crate::api::model::{PriceAssessment, Report, ReportId, RowId};
use crate::api::{ReportRepository, RepositoryError};
use tallyline_reference_data::api::model::ReferenceSnapshot;
use tallyline_shared_kernel::api::Clock;

impl<R: ReportRepository, C: Clock> ReportingApplication<R, C> {
    /// Assesses entered prices without rounding tolerance decisions or changing Draft values.
    ///
    /// # Errors
    /// Refuses an unavailable Draft, mismatched environment or repository failure.
    pub fn review_prices(
        &mut self,
        id: ReportId,
        snapshot: &ReferenceSnapshot,
    ) -> Result<Vec<PriceAssessment>, RepositoryError> {
        let report = self.draft_for_reference(id, snapshot)?;
        Ok(self.price_assessments(&report, snapshot))
    }
    pub(super) fn price_assessments(
        &self,
        report: &Report,
        snapshot: &ReferenceSnapshot,
    ) -> Vec<PriceAssessment> {
        self.manual_reviewer()
            .price_assessments(report, snapshot, self.clock.now())
    }
}

impl<R: ReportRepository, C: Clock> ReportingApplication<R, C> {
    /// Persists approval of the entered price at its current exact deviation.
    ///
    /// # Errors
    /// Refuses stale versions, absent prices, unavailable exact assessment or storage failure.
    pub fn confirm_price(
        &mut self,
        id: ReportId,
        expected: u64,
        row: RowId,
        snapshot: &ReferenceSnapshot,
    ) -> Result<Report, RepositoryError> {
        let report = self.draft_for_reference(id, snapshot)?;
        if report.version() != expected {
            return Err(RepositoryError::Conflict);
        }
        let assessment = self
            .price_assessments(&report, snapshot)
            .into_iter()
            .find(|assessment| assessment.row() == row)
            .ok_or(RepositoryError::Invalid)?;
        let deviation = assessment
            .deviation()
            .ok_or(RepositoryError::AssessmentUnavailable)?;
        let next = report
            .approve_price(row, deviation, snapshot.environment())
            .map_err(|_| RepositoryError::Invalid)?;
        self.repository.save_draft(&next, expected)?;
        Ok(next)
    }
}

impl<R: ReportRepository, C: Clock> ReportingApplication<R, C> {
    /// Persists price reassessment without changing entered values or status decisions.
    ///
    /// # Errors
    /// Refuses stale versions, unavailable exact assessments, mismatched references or storage failure.
    pub fn reassess_prices(
        &mut self,
        id: ReportId,
        expected: u64,
        snapshot: &ReferenceSnapshot,
    ) -> Result<Report, RepositoryError> {
        let report = self.draft_for_reference(id, snapshot)?;
        if report.version() != expected {
            return Err(RepositoryError::Conflict);
        }
        let assessments = self.price_assessments(&report, snapshot);
        if assessments
            .iter()
            .any(|assessment| assessment.unavailable().is_some())
        {
            return Err(RepositoryError::AssessmentUnavailable);
        }
        let next = report
            .reassess_prices(&assessments, snapshot.environment())
            .map_err(|_| RepositoryError::Invalid)?;
        if next.version() != expected {
            self.repository.save_draft(&next, expected)?;
        }
        Ok(next)
    }
    /// Records keyboard focus without approving a price.
    ///
    /// # Errors
    /// Refuses stale versions, absent prices or storage failure.
    pub fn focus_price(
        &mut self,
        id: ReportId,
        expected: u64,
        row: RowId,
    ) -> Result<Report, RepositoryError> {
        let report = self
            .repository
            .load_draft(id)?
            .ok_or(RepositoryError::Invalid)?;
        if report.version() != expected {
            return Err(RepositoryError::Conflict);
        }
        let next = report
            .focus_price(row)
            .map_err(|_| RepositoryError::Invalid)?;
        if next.version() != expected {
            self.repository.save_draft(&next, expected)?;
        }
        Ok(next)
    }
}
