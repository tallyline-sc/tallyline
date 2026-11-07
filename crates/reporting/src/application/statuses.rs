// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Durable explicit status review decisions.

use super::{
    Clock, ReferenceSnapshot, Report, ReportId, ReportRepository, ReportingApplication,
    RepositoryError, RowId,
};
impl<R: ReportRepository, C: Clock> ReportingApplication<R, C> {
    /// Assesses entered statuses against an immutable reference without changing Draft values.
    ///
    /// # Errors
    /// Refuses an unavailable Draft, mismatched environment or repository failure.
    pub fn review_statuses(
        &mut self,
        id: ReportId,
        snapshot: &ReferenceSnapshot,
    ) -> Result<Vec<crate::api::model::StatusAssessment>, RepositoryError> {
        let report = self.draft_for_reference(id, snapshot)?;
        Ok(self.status_assessments(&report, snapshot))
    }
    pub(super) fn status_assessments(
        &self,
        report: &Report,
        snapshot: &ReferenceSnapshot,
    ) -> Vec<crate::api::model::StatusAssessment> {
        self.manual_reviewer()
            .status_assessments(report, snapshot, self.clock.now())
    }
}

impl<R: ReportRepository, C: Clock> ReportingApplication<R, C> {
    /// Persists an explicit status approval against its current unlowered deviation.
    ///
    /// # Errors
    /// Refuses a stale version, absent status, mismatched reference or failed storage.
    pub fn confirm_status(
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
            .status_assessments(&report, snapshot)
            .into_iter()
            .find(|item| item.row() == row)
            .ok_or(RepositoryError::Invalid)?;
        let confirmed = report
            .approve_status(row, assessment.deviation(), snapshot.environment())
            .map_err(|_| RepositoryError::Invalid)?;
        self.repository.save_draft(&confirmed, expected)?;
        Ok(confirmed)
    }
}

impl<R: ReportRepository, C: Clock> ReportingApplication<R, C> {
    /// Persists status reassessment and revokes approvals whose deviation worsened.
    ///
    /// # Errors
    /// Refuses a stale version, mismatched reference or failed storage.
    pub fn reassess_statuses(
        &mut self,
        id: ReportId,
        expected: u64,
        snapshot: &ReferenceSnapshot,
    ) -> Result<Report, RepositoryError> {
        let report = self.draft_for_reference(id, snapshot)?;
        if report.version() != expected {
            return Err(RepositoryError::Conflict);
        }
        let assessments = self.status_assessments(&report, snapshot);
        let next = report
            .reassess_status(&assessments, snapshot.environment())
            .map_err(|_| RepositoryError::Invalid)?;
        if next.version() != expected {
            self.repository.save_draft(&next, expected)?;
        }
        Ok(next)
    }
}

impl<R: ReportRepository, C: Clock> ReportingApplication<R, C> {
    /// Records keyboard focus without approving the status value.
    ///
    /// # Errors
    /// Refuses a stale version, absent status or failed storage.
    pub fn focus_status(
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
            .focus_status(row)
            .map_err(|_| RepositoryError::Invalid)?;
        if next.version() != expected {
            self.repository.save_draft(&next, expected)?;
        }
        Ok(next)
    }
}
