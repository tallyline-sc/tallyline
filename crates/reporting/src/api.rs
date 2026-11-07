// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Manual Report application boundary.

pub mod model;

use model::{Report, ReportId, ReportKey};
use tallyline_shared_kernel::api::model::Timestamp;

/// A privacy-safe repository failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepositoryError {
    /// A stale edit would overwrite a newer Draft.
    Conflict,
    /// Persistent state violates the Report contract.
    Invalid,
    /// An exact price comparison cannot be represented safely.
    AssessmentUnavailable,
    /// Storage could not complete within its bounded operation.
    Unavailable,
}
impl std::fmt::Display for RepositoryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Conflict => "Draft changed; reopen it before editing",
            Self::Invalid => "Draft value, context or stored state is invalid",
            Self::AssessmentUnavailable => {
                "Exact price comparison unavailable; cross-check the entered value and reference"
            }
            Self::Unavailable => "Draft storage is unavailable",
        })
    }
}
impl std::error::Error for RepositoryError {}

/// Durable manual Draft operations owned by Reporting.
pub trait ReportRepository {
    /// Allocates and persists an empty Draft before publishing it.
    ///
    /// # Errors
    /// Returns unavailable storage or an invalid key/version.
    fn create_manual(
        &mut self,
        key: ReportKey,
        observed_at: Timestamp,
        game_version: Option<&str>,
    ) -> Result<Report, RepositoryError>;
    /// Loads a Draft by its stable identity.
    ///
    /// # Errors
    /// Returns unavailable storage or invalid persisted state.
    fn load_draft(&mut self, id: ReportId) -> Result<Option<Report>, RepositoryError>;
    /// Lists persisted Drafts with most recently changed first.
    ///
    /// # Errors
    /// Returns unavailable storage or invalid persisted state.
    fn drafts(&mut self) -> Result<Vec<Report>, RepositoryError>;
    /// Atomically saves against the previously observed concurrency version.
    ///
    /// # Errors
    /// Requires an advancing version; returns a conflict, invalid state or unavailable storage.
    fn save_draft(&mut self, report: &Report, expected_version: u64)
    -> Result<(), RepositoryError>;
}
pub use crate::application::{ManualEdit, ReportingApplication};
pub use crate::attachment::AttachmentEdit;

pub use crate::manual_reviewer::ManualReviewer;

/// Reads retained Reports, including immutable delivery history.
pub trait ReportHistoryRepository {
    /// Loads a retained Report without requiring it to remain editable.
    ///
    /// # Errors
    /// Refuses unavailable storage and invalid persisted state.
    fn retained_report(&mut self, id: ReportId) -> Result<Option<Report>, RepositoryError>;
}
