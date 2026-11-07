// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Retained immutable Report inspection through the shared validated mapper.

use crate::api::SqliteReferenceCache;
use tallyline_reporting::api::model::{Report, ReportId};
use tallyline_reporting::api::{ReportHistoryRepository, RepositoryError};
use tallyline_submission::api::model::SubmissionRepositoryError;

impl ReportHistoryRepository for SqliteReferenceCache {
    fn retained_report(&mut self, id: ReportId) -> Result<Option<Report>, RepositoryError> {
        crate::queue::load_report_optional(&self.connection, id).map_err(|error| match error {
            SubmissionRepositoryError::Conflict => RepositoryError::Conflict,
            SubmissionRepositoryError::Invalid => RepositoryError::Invalid,
            SubmissionRepositoryError::Unavailable => RepositoryError::Unavailable,
        })
    }
}
