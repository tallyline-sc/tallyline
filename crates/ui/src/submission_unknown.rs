// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Explicit unknown-resolution controls and user-decision provenance.

use crate::api::Catalogue;
use crate::resolution_queue_action::ResolutionQueueAction;
use egui::{Button, Ui};
use tallyline_submission::api::model::{
    JobPhase, ReceiptDecision, SubmissionJob, SubmissionResolutionAction, SubmissionResult,
};

pub(crate) fn show(
    ui: &mut Ui,
    job: &SubmissionJob,
    history: &[SubmissionJob],
    catalogue: &Catalogue,
    enabled: bool,
) -> Option<ResolutionQueueAction> {
    if job.phase() != JobPhase::OutcomeUnknown {
        return None;
    }
    crate::unknown_attempt::show(ui, job, catalogue);
    if let Some(decision) = job.unknown_decision() {
        ui.label(catalogue.text(match decision {
            ReceiptDecision::Received => "queue.unknown_received",
            ReceiptDecision::NotReceived => "queue.unknown_not_received",
        }));
    }
    let verified = job.verified_row_receipts().count();
    if verified > 0 {
        ui.label(catalogue.text(if verified == job.request().rows().len() {
            "queue.unknown_verified_full"
        } else {
            "queue.unknown_partial"
        }));
        for receipt in job.verified_row_receipts() {
            ui.label(format!(
                "{} {} · {} {} · {}",
                catalogue.text("queue.verified_row"),
                receipt.row().0,
                catalogue.text("queue.uex_report"),
                receipt.id(),
                crate::attachment_views::format_time(receipt.date_added())
            ));
        }
    }
    ui.label(catalogue.text("queue.unknown_withdrawal_unavailable"));
    for action in job.resolution_actions() {
        if let SubmissionResolutionAction::Duplicated { report, .. } = action {
            ui.label(format!(
                "{} {}",
                catalogue.text("queue.unknown_new_draft"),
                report.0
            ));
        }
    }
    for candidate in history {
        if matches!(candidate.result().map(SubmissionResult::final_result), Some(SubmissionResult::DuplicateEvidence(previous)) if *previous == job.id())
        {
            ui.label(format!(
                "{} {}",
                catalogue.text("queue.unknown_duplicate_evidence"),
                candidate.id().0
            ));
        }
    }
    for (key, action) in [
        (
            "queue.mark_received",
            ResolutionQueueAction::Decide(ReceiptDecision::Received),
        ),
        (
            "queue.mark_not_received",
            ResolutionQueueAction::Decide(ReceiptDecision::NotReceived),
        ),
        ("queue.check_unknown", ResolutionQueueAction::CheckReceipt),
        (
            "queue.duplicate_unknown",
            ResolutionQueueAction::DuplicateAsDraft,
        ),
    ] {
        if action.eligible(job)
            && ui
                .add_enabled(
                    enabled,
                    Button::new(tallyline_egui::button_text(format!(
                        "{} {}",
                        catalogue.text(key),
                        job.id().0
                    ))),
                )
                .clicked()
        {
            return Some(action);
        }
    }
    None
}
