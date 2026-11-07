// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact-snapshot observation-history facts supplied by the coordinating workflow.

use crate::api::model::Report;
use tallyline_shared_kernel::api::model::{
    RowId, SubmissionJobId, SubmissionMode, Timestamp, UexEnvironment,
};

/// A durable-history finding that independently prevents release.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObservationHistoryFinding {
    /// Positive origin evidence prevents release of its existing explicit duplicate.
    ResolvedDuplicateOrigin {
        /// The original uncertain submission with positive evidence.
        submission: SubmissionJobId,
    },
    /// This exact older row lacks a decision against the newest accepted observation.
    NewerAccepted {
        /// Candidate local row.
        row: RowId,
        /// Newest accepted original observation.
        observed_at: Timestamp,
    },
    /// A newer sending, partial or unknown row cannot be bypassed by a decision.
    Unresolved {
        /// Candidate local row.
        row: RowId,
        /// Newer uncertain job.
        submission: SubmissionJobId,
        /// Newer original observation.
        observed_at: Timestamp,
    },
    /// A retained decision no longer has matching committed accepted evidence.
    StaleDecision {
        /// Candidate local row.
        row: RowId,
    },
}
/// One workflow assessment bound to the exact reassessed Report and release scope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObservationHistoryAssessment {
    report: Report,
    account: String,
    mode: SubmissionMode,
    environment: UexEnvironment,
    findings: Vec<ObservationHistoryFinding>,
}
impl ObservationHistoryAssessment {
    /// Captures findings derived from complete validated committed job history.
    ///
    /// Unavailable history must remain absent; it must never be represented by empty findings.
    #[must_use]
    pub fn new(
        report: &Report,
        account: &str,
        mode: SubmissionMode,
        environment: UexEnvironment,
        findings: Vec<ObservationHistoryFinding>,
    ) -> Self {
        Self {
            report: report.clone(),
            account: account.to_owned(),
            mode,
            environment,
            findings,
        }
    }
    pub(crate) fn matches(
        &self,
        report: &Report,
        account: Option<&str>,
        mode: SubmissionMode,
        environment: UexEnvironment,
    ) -> bool {
        &self.report == report
            && account == Some(self.account.as_str())
            && self.mode == mode
            && self.environment == environment
    }
    pub(crate) fn findings(&self) -> &[ObservationHistoryFinding] {
        &self.findings
    }
}
