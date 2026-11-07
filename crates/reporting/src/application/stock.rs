// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Parameterised SCU assessment without an implicit calibrated default.

use super::ReportingApplication;
use crate::api::model::{Report, ReportId, StockAssessment};
use crate::api::{ReportRepository, RepositoryError};
use tallyline_reference_data::api::model::{ReferenceSnapshot, ScuTolerance};
use tallyline_shared_kernel::api::Clock;

impl<R: ReportRepository, C: Clock> ReportingApplication<R, C> {
    /// Compares entered quantities using explicit bands without changing Drafts or approvals.
    ///
    /// # Errors
    /// Refuses an unavailable Draft, mismatched environment or repository failure.
    pub fn review_stock(
        &mut self,
        id: ReportId,
        snapshot: &ReferenceSnapshot,
        tolerance: ScuTolerance,
    ) -> Result<Vec<StockAssessment>, RepositoryError> {
        let report = self.draft_for_reference(id, snapshot)?;
        Ok(self.stock_assessments(&report, snapshot, tolerance))
    }
    pub(super) fn stock_assessments(
        &self,
        report: &Report,
        snapshot: &ReferenceSnapshot,
        tolerance: ScuTolerance,
    ) -> Vec<StockAssessment> {
        self.manual_reviewer()
            .stock_assessments(report, snapshot, self.clock.now(), tolerance)
    }
}

impl<R: ReportRepository, C: Clock> ReportingApplication<R, C> {
    /// Persists approval of the entered stock at its current exact deviation.
    ///
    /// # Errors
    /// Refuses stale versions, absent stocks, unavailable exact assessment or storage failure.
    pub fn confirm_stock(
        &mut self,
        id: ReportId,
        expected: u64,
        row: crate::api::model::RowId,
        snapshot: &ReferenceSnapshot,
    ) -> Result<Report, RepositoryError> {
        let report = self.draft_for_reference(id, snapshot)?;
        if report.version() != expected {
            return Err(RepositoryError::Conflict);
        }
        let assessment = self
            .stock_assessments(&report, snapshot, self.stock_settings.tolerance())
            .into_iter()
            .find(|assessment| assessment.row() == row)
            .ok_or(RepositoryError::Invalid)?;
        let deviation = assessment
            .deviation()
            .ok_or(RepositoryError::AssessmentUnavailable)?;
        let next = report
            .approve_stock(row, deviation, snapshot.environment())
            .map_err(|_| RepositoryError::Invalid)?;
        self.repository.save_draft(&next, expected)?;
        Ok(next)
    }
}

impl<R: ReportRepository, C: Clock> ReportingApplication<R, C> {
    /// Persists stock reassessment without changing entered values or status decisions.
    ///
    /// # Errors
    /// Refuses stale versions, unavailable exact assessments, mismatched references or storage failure.
    pub fn reassess_stocks(
        &mut self,
        id: ReportId,
        expected: u64,
        snapshot: &ReferenceSnapshot,
    ) -> Result<Report, RepositoryError> {
        let report = self.draft_for_reference(id, snapshot)?;
        if report.version() != expected {
            return Err(RepositoryError::Conflict);
        }
        let assessments =
            self.stock_assessments(&report, snapshot, self.stock_settings.tolerance());
        if assessments
            .iter()
            .any(|assessment| assessment.unavailable().is_some())
        {
            return Err(RepositoryError::AssessmentUnavailable);
        }
        let next = report
            .reassess_stocks(&assessments, snapshot.environment())
            .map_err(|_| RepositoryError::Invalid)?;
        if next.version() != expected {
            self.repository.save_draft(&next, expected)?;
        }
        Ok(next)
    }
    /// Records keyboard focus without approving a stock.
    ///
    /// # Errors
    /// Refuses stale versions, absent stocks or storage failure.
    pub fn focus_stock(
        &mut self,
        id: ReportId,
        expected: u64,
        row: crate::api::model::RowId,
    ) -> Result<Report, RepositoryError> {
        let report = self
            .repository
            .load_draft(id)?
            .ok_or(RepositoryError::Invalid)?;
        if report.version() != expected {
            return Err(RepositoryError::Conflict);
        }
        let next = report
            .focus_stock(row)
            .map_err(|_| RepositoryError::Invalid)?;
        if next.version() != expected {
            self.repository.save_draft(&next, expected)?;
        }
        Ok(next)
    }
}
