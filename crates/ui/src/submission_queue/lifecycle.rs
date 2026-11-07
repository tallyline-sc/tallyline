// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Committed lifecycle projection and account-throttled explicit refresh presentation.

use super::{JobId, QueueCommand, SubmissionJob, SubmissionQueue, SubmissionResult};
use crate::api::Catalogue;
use tallyline_submission::api::model::{
    RowEditability, SubmissionLifecycle, SubmissionLifecycleSettings, UexReportStatus,
};

impl SubmissionQueue {
    /// Retains only monotonic committed lifecycle evidence for the exact current source.
    pub fn received_lifecycle(&mut self, ledger: SubmissionLifecycle) -> bool {
        if !self.jobs.iter().any(|job| job == ledger.source()) {
            return false;
        }
        if let Some(current) = self
            .lifecycles
            .iter_mut()
            .find(|current| current.job() == ledger.job())
        {
            if ledger.revision() < current.revision()
                || (ledger.revision() == current.revision()
                    && *current != ledger
                    && !preserves_evidence(current, &ledger))
            {
                return false;
            }
            *current = ledger;
        } else {
            self.lifecycles.push(ledger);
        }
        true
    }
    /// Publishes whether the owner retains a lifecycle read or a save-only retry.
    pub fn received_lifecycle_busy(&mut self, busy: bool) {
        self.lifecycle_busy = busy;
    }
    /// Publishes the current validated account allowed to request authenticated lifecycle reads.
    pub fn received_lifecycle_account(&mut self, account: Option<String>) {
        self.lifecycle_account = account;
    }
    /// Borrows committed lifecycle evidence only while its source remains current.
    #[must_use]
    pub fn lifecycle(&self, id: JobId) -> Option<&SubmissionLifecycle> {
        self.lifecycles
            .iter()
            .find(|ledger| ledger.job() == id && self.jobs.iter().any(|job| job == ledger.source()))
    }
    /// Reports passive manual eligibility without granting HTTP admission.
    #[must_use]
    pub fn can_refresh_lifecycle(&self, id: JobId) -> bool {
        if self.busy.is_some() || self.lifecycle_busy || !self.authenticated_quotas.is_empty() {
            return false;
        }
        let Some(job) = self.jobs.iter().find(|job| job.id() == id) else {
            return false;
        };
        if self.lifecycle_account.as_deref() != Some(job.request().header().account.as_str()) {
            return false;
        }
        let Some(at) = self.assessed_at else {
            return false;
        };
        let Some(sent) = job.attempts().last().map(|attempt| attempt.started_at()) else {
            return false;
        };
        let settings = SubmissionLifecycleSettings::default();
        if !settings.within_age(sent, at) {
            return false;
        }
        let account = &job.request().header().account;
        let latest = self
            .lifecycles
            .iter()
            .filter(|ledger| &ledger.source().request().header().account == account)
            .filter_map(SubmissionLifecycle::manual_admitted_at)
            .max();
        if settings
            .manual_due_at(sent, latest)
            .is_none_or(|due| at < due)
        {
            return false;
        }
        if let Some(ledger) = self.lifecycle(id) {
            return ledger.receipts().iter().any(|proof| {
                !ledger
                    .observation(proof.row())
                    .is_some_and(|observation| observation.status().is_final())
            });
        }
        job.request().header().mode
            == tallyline_shared_kernel::api::model::SubmissionMode::Production
            && (self.verified_receipts(id).is_some()
                || job.verified_row_receipts().next().is_some()
                || matches!(job.result().map(SubmissionResult::final_result), Some(SubmissionResult::ProductionSucceeded(receipts)) if !receipts.is_empty()))
    }
    /// Queues one exact manual lifecycle intent without changing observed status.
    pub fn request_lifecycle_refresh(&mut self, id: JobId) -> bool {
        if !self.can_refresh_lifecycle(id) {
            return false;
        }
        let Some(job) = self.jobs.iter().find(|job| job.id() == id) else {
            return false;
        };
        let command = QueueCommand::RefreshLifecycle {
            id,
            expected_revision: job.revision(),
            expected_lifecycle: self.lifecycle(id).map(SubmissionLifecycle::revision),
        };
        self.pending = Some(command);
        self.busy = Some(command);
        self.failure = None;
        true
    }
}

fn preserves_evidence(previous: &SubmissionLifecycle, current: &SubmissionLifecycle) -> bool {
    current.source().revision() > previous.source().revision()
        && current.source().request() == previous.source().request()
        && current.source().attempts() == previous.source().attempts()
        && current.admitted_at() == previous.admitted_at()
        && current.manual_admitted_at() == previous.manual_admitted_at()
        && current.observations() == previous.observations()
        && current.row_admissions().eq(previous.row_admissions())
        && previous
            .receipts()
            .iter()
            .all(|receipt| current.receipts().contains(receipt))
}
fn status_key(status: UexReportStatus) -> &'static str {
    match status {
        UexReportStatus::Pending => "queue.lifecycle.pending",
        UexReportStatus::UnderReview => "queue.lifecycle.under_review",
        UexReportStatus::Queued => "queue.lifecycle.queued",
        UexReportStatus::Approved => "queue.lifecycle.approved",
        UexReportStatus::Consolidated => "queue.lifecycle.consolidated",
        UexReportStatus::Declined => "queue.lifecycle.declined",
        UexReportStatus::Expired => "queue.lifecycle.expired",
    }
}
pub(super) fn show(
    ui: &mut egui::Ui,
    job: &SubmissionJob,
    ledger: Option<&SubmissionLifecycle>,
    ready: bool,
    catalogue: &Catalogue,
) -> bool {
    if let Some(ledger) = ledger {
        for observation in ledger.observations() {
            ui.label(format!(
                "{} {}",
                catalogue.text("queue.lifecycle.row"),
                observation.receipt().row().0
            ));
            ui.label(catalogue.text(status_key(observation.status())));
            ui.label(catalogue.text(match observation.editability() {
                RowEditability::Editable => "queue.lifecycle.editable",
                RowEditability::NotEditable => "queue.lifecycle.not_editable",
            }));
            ui.label(format!(
                "{} {}",
                catalogue.text("queue.lifecycle.checked"),
                crate::attachment_views::format_time(observation.checked_at())
            ));
        }
    }
    if job.request().header().mode
        != tallyline_shared_kernel::api::model::SubmissionMode::Production
    {
        return false;
    }
    ui.add_enabled(
        ready,
        egui::Button::new(tallyline_egui::button_text(format!(
            "{} {}",
            catalogue.text("queue.lifecycle.refresh"),
            job.id().0
        ))),
    )
    .clicked()
}
