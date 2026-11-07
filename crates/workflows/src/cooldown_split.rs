// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Atomic value-preserving cooldown partition preparation.

use tallyline_reporting::api::model::{Report, ReportEvent};
use tallyline_shared_kernel::api::model::{ReportId, Timestamp};
use tallyline_submission::api::SubmissionSchedule;
use tallyline_submission::api::model::{
    JobId, SubmissionChange, SubmissionCommand, SubmissionJob, SubmissionRepositoryError,
};

/// Fresh local identities for the two replacement owning jobs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CooldownSplitIdentities {
    /// Separate ready Report identity.
    pub ready_report: ReportId,
    /// Owning job for the ready Report.
    pub ready_job: JobId,
    /// Owning job for the retained cooling Report.
    pub cooling_job: JobId,
}
/// One immutable queued Report and its exact replacement job.
///
/// ```compile_fail
/// use tallyline_submission::api::model::SubmissionJob;
/// use tallyline_workflows::api::CooldownSplitPart;
/// fn replace_job(part: &mut CooldownSplitPart, job: SubmissionJob) {
///     part.job = job;
/// }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CooldownSplitPart {
    /// Released values with their preserved review decisions and fresh job binding.
    report: Report,
    /// The exact replacement release snapshot requiring all ordinary dispatch checks.
    job: SubmissionJob,
}
/// A complete partition committed with source cancellation and all outbox events.
///
/// ```compile_fail
/// use tallyline_submission::api::model::SubmissionJob;
/// use tallyline_workflows::api::CooldownSplit;
/// fn replace_source(split: &mut CooldownSplit, job: SubmissionJob) {
///     split.origin = job;
/// }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CooldownSplit {
    /// Original cancelled job with unchanged request and attempts.
    origin: SubmissionJob,
    /// Rows with no current cooldown.
    ready: CooldownSplitPart,
    /// Rows retaining their exact current cooldowns.
    cooling: CooldownSplitPart,
    cancellation: SubmissionChange,
    events: Vec<ReportEvent>,
}
impl CooldownSplitPart {
    /// Borrows the preserved released values and fresh owning-job binding.
    #[must_use]
    pub const fn report(&self) -> &Report {
        &self.report
    }

    /// Borrows the exact replacement job requiring ordinary dispatch checks.
    #[must_use]
    pub const fn job(&self) -> &SubmissionJob {
        &self.job
    }
}
impl CooldownSplit {
    /// Prepares two complete disjoint releases from current durable history.
    ///
    /// # Errors
    /// Refuses stale owning bindings, non-mixed partitions and unavailable ordering evidence.
    pub fn prepare(
        report: &Report,
        origin: &SubmissionJob,
        history: &[SubmissionJob],
        ids: CooldownSplitIdentities,
        now: Timestamp,
    ) -> Result<Self, SubmissionRepositoryError> {
        let binding = report
            .release_binding()
            .ok_or(SubmissionRepositoryError::Invalid)?;
        let request = crate::submission_request::manual_submission_request(
            report,
            binding.environment(),
            binding.account(),
            binding.mode(),
        )
        .map_err(invalid)?;
        let mut header = request.header().clone();
        header.report_version = origin.request().header().report_version;
        if &header != origin.request().header()
            || request.rows() != origin.request().rows()
            || request.newer_observation_confirmations()
                != origin.request().newer_observation_confirmations()
            || binding.job() != Some(origin.id())
            || ids.ready_job == ids.cooling_job
            || ids.ready_job == origin.id()
            || ids.cooling_job == origin.id()
        {
            return Err(SubmissionRepositoryError::Invalid);
        }
        let partition =
            SubmissionSchedule::cooldown_partition(origin, history, now).map_err(invalid)?;
        if partition.ready_rows().is_empty() || partition.cooling_rows().is_empty() {
            return Err(SubmissionRepositoryError::Invalid);
        }
        let rows = partition
            .cooling_rows()
            .iter()
            .map(|row| row.row())
            .collect::<Vec<_>>();
        let reports = report
            .split_off_cooldown_rows(&rows, ids.ready_report)
            .map_err(invalid)?;
        let mut events = reports.cooling().events().to_vec();
        events.extend_from_slice(reports.ready().events());
        let ready = part(reports.ready().state(), ids.ready_job)?;
        let cooling = part(reports.cooling().state(), ids.cooling_job)?;
        let cancellation = origin
            .apply(SubmissionCommand::CancelForSplit {
                report: ids.ready_report,
                at: now,
            })
            .map_err(invalid)?;
        Ok(Self {
            origin: cancellation.state().clone(),
            ready,
            cooling,
            cancellation,
            events,
        })
    }
    /// Borrows the cancelled source with its unchanged request and attempts.
    #[must_use]
    pub const fn origin(&self) -> &SubmissionJob {
        &self.origin
    }

    /// Borrows the replacement containing rows without a current cooldown.
    #[must_use]
    pub const fn ready(&self) -> &CooldownSplitPart {
        &self.ready
    }

    /// Borrows the replacement retaining the exact current cooldowns.
    #[must_use]
    pub const fn cooling(&self) -> &CooldownSplitPart {
        &self.cooling
    }

    /// Borrows the exact cancellation requiring the same transaction as both replacements.
    #[must_use]
    pub const fn cancellation(&self) -> &SubmissionChange {
        &self.cancellation
    }
    /// Borrows replacement release events requiring the same atomic transaction.
    #[must_use]
    pub fn events(&self) -> &[ReportEvent] {
        &self.events
    }
}
fn invalid<T>(_: T) -> SubmissionRepositoryError {
    SubmissionRepositoryError::Invalid
}
fn part(report: &Report, id: JobId) -> Result<CooldownSplitPart, SubmissionRepositoryError> {
    let report = report
        .enqueue(id)
        .map_err(|_| SubmissionRepositoryError::Invalid)?
        .into_state();
    let binding = report
        .release_binding()
        .ok_or(SubmissionRepositoryError::Invalid)?;
    let request = crate::submission_request::manual_submission_request(
        &report,
        binding.environment(),
        binding.account(),
        binding.mode(),
    )
    .map_err(|_| SubmissionRepositoryError::Invalid)?;
    let job = SubmissionJob::queued(id, request).map_err(|_| SubmissionRepositoryError::Invalid)?;
    Ok(CooldownSplitPart { report, job })
}
/// Atomically re-reads exact source and history before creating both replacement jobs.
pub trait CooldownSplitStore {
    /// Commits the explicit mixed-cooldown split for the displayed queued revision.
    ///
    /// # Errors
    /// Refuses stale, uncertain, expired, non-mixed or already split work; failures publish nothing.
    fn split_cooldown(
        &mut self,
        id: JobId,
        expected_revision: u64,
        now: Timestamp,
    ) -> Result<CooldownSplit, SubmissionRepositoryError>;
}
