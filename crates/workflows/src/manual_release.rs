// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Complete immutable release coordination for one same-database transaction.

use tallyline_reporting::api::model::{ReleaseBlockReason, Report, ReportEvent, SubmissionGate};
use tallyline_shared_kernel::api::model::SubmissionJobId;
use tallyline_submission::api::model::SubmissionJob;

/// A safe manual release failure without source values or technology diagnostics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ManualReleaseError {
    /// Complete release checks refused the exact reviewed Report.
    Blocked(Vec<ReleaseBlockReason>),
    /// A payload value or identity is outside the supported request contract.
    Invalid,
    /// The source Report or its reference data changed before the transaction.
    Conflict,
    /// The atomic state-and-outbox transaction could not commit.
    Unavailable,
}
/// A real gated Report, exact queued request and release event saved together.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManualRelease {
    source: Report,
    report: Report,
    job: SubmissionJob,
    events: Vec<ReportEvent>,
}
impl ManualRelease {
    /// Coordinates actual Reporting release and Submission snapshot creation.
    ///
    /// This preparation publishes nothing. The transaction port must compare the
    /// exact source and current references before saving all resulting values.
    ///
    /// # Errors
    /// Refuses every complete-gate block, invalid identity and unsupported payload.
    pub fn prepare(
        gate: &SubmissionGate,
        job: SubmissionJobId,
    ) -> Result<Self, ManualReleaseError> {
        let released = gate
            .report()
            .release(gate)
            .map_err(ManualReleaseError::Blocked)?;
        let queued = released
            .state()
            .enqueue(job)
            .map_err(|_| ManualReleaseError::Invalid)?;
        let report = queued.into_state();
        let binding = report
            .release_binding()
            .ok_or(ManualReleaseError::Invalid)?;
        let request = crate::submission_request::manual_submission_request(
            &report,
            binding.environment(),
            binding.account(),
            binding.mode(),
        )
        .map_err(|_| ManualReleaseError::Invalid)?;
        let job = SubmissionJob::queued(job, request).map_err(|_| ManualReleaseError::Invalid)?;
        Ok(Self {
            source: gate.source().clone(),
            report,
            job,
            events: released.events().to_vec(),
        })
    }
    /// Borrows the exact source required by the optimistic transaction comparison.
    #[must_use]
    pub const fn source(&self) -> &Report {
        &self.source
    }
    /// Borrows the final immutable queued Report.
    #[must_use]
    pub const fn report(&self) -> &Report {
        &self.report
    }
    /// Borrows the exact account-, mode- and version-bound job.
    #[must_use]
    pub const fn job(&self) -> &SubmissionJob {
        &self.job
    }
    /// Borrows events requiring the same transaction as both aggregates.
    #[must_use]
    pub fn events(&self) -> &[ReportEvent] {
        &self.events
    }
}
/// Atomically persists Reporting release, its Submission job and durable outbox.
pub trait ManualReleaseStore {
    /// Allocates the job identity and commits one exact gated release atomically.
    ///
    /// # Errors
    /// Refuses changed source/reference state, gate blocks and unavailable storage.
    /// A failure publishes no Report, job or outbox event.
    fn release_manual(
        &mut self,
        gate: &SubmissionGate,
    ) -> Result<ManualRelease, ManualReleaseError>;
}
