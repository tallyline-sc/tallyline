// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Per-row delivery evidence and explicit immutable resolution intent.

use crate::api::Catalogue;
use crate::resolution_queue_action::ResolutionQueueAction;
use egui::{Button, Ui, UiBuilder};
use tallyline_submission::api::model::{
    JobPhase, ReceiptDecision, RowFailureKind, SubmissionJob, SubmissionResolutionAction,
    SubmissionRowResult,
};

pub(crate) fn show(
    ui: &mut Ui,
    job: &SubmissionJob,
    catalogue: &Catalogue,
    enabled: bool,
) -> Option<ResolutionQueueAction> {
    if job.phase() != JobPhase::PartiallyAccepted {
        return None;
    }
    crate::unknown_attempt::show(ui, job, catalogue);
    let mut selected = None;
    for row in job.request().rows() {
        let Some(result) = job.row_result(row.id()) else {
            continue;
        };
        let outcome = match result {
            SubmissionRowResult::Accepted(proof) => format!(
                "{} · {} {} · {}",
                catalogue.text("queue.row_accepted"),
                catalogue.text("queue.uex_report"),
                proof.id(),
                crate::attachment_views::format_time(proof.date_added())
            ),
            SubmissionRowResult::AcceptedUnmapped => {
                catalogue.text("queue.row_accepted_unmapped").to_owned()
            }
            SubmissionRowResult::ReceivedByUser => {
                catalogue.text("queue.row_received_user").to_owned()
            }
            SubmissionRowResult::NotReceivedByUser => {
                catalogue.text("queue.row_not_received_user").to_owned()
            }
            SubmissionRowResult::Unmapped => catalogue.text("queue.row_unmapped").to_owned(),
            SubmissionRowResult::NotSent => catalogue.text("queue.row_not_sent").to_owned(),
            SubmissionRowResult::Rejected(code) => format!(
                "{} · {}",
                catalogue.text(if code.row_failure() == RowFailureKind::Database {
                    "queue.row_not_stored"
                } else {
                    "queue.row_rejected"
                }),
                code.as_str()
            ),
        };
        ui.label(format!(
            "{} {} · {} {} · {}",
            catalogue.text("queue.row"),
            row.id().0,
            catalogue.text("queue.commodity"),
            row.commodity().0,
            outcome
        ));
        for (key, decision) in [
            ("queue.mark_row_received", ReceiptDecision::Received),
            ("queue.mark_row_not_received", ReceiptDecision::NotReceived),
        ] {
            let action = ResolutionQueueAction::DecideRow {
                row: row.id(),
                decision,
            };
            if action.eligible(job)
                && action_button(
                    ui,
                    enabled,
                    format!(
                        "{} {} {} {}",
                        catalogue.text("queue.mark_row"),
                        row.id().0,
                        catalogue.text(key),
                        job.id().0
                    ),
                )
            {
                selected = Some(action);
            }
        }
    }
    if ResolutionQueueAction::CheckReceipt.eligible(job)
        && action_button(
            ui,
            enabled,
            format!("{} {}", catalogue.text("queue.check_unknown"), job.id().0),
        )
    {
        selected = Some(ResolutionQueueAction::CheckReceipt);
    }
    let copy = ResolutionQueueAction::DuplicateAsDraft;
    if copy.eligible(job)
        && action_button(
            ui,
            enabled,
            format!("{} {}", catalogue.text("queue.copy_partial"), job.id().0),
        )
    {
        selected = Some(copy);
    }
    for action in job.resolution_actions() {
        if let SubmissionResolutionAction::RowDecision { row, decision, at } = action {
            ui.label(format!(
                "{} {} · {} {}",
                catalogue.text("queue.row"),
                row.0,
                catalogue.text("queue.row_decision"),
                catalogue.text(match decision {
                    ReceiptDecision::Received => "queue.row_decision_received",
                    ReceiptDecision::NotReceived => "queue.row_decision_not_received",
                })
            ));
            ui.label(crate::attachment_views::format_time(*at));
        }
        if let SubmissionResolutionAction::RowsDuplicated { report, rows, .. } = action {
            ui.label(format!(
                "{} {} · {} {}",
                catalogue.text("queue.partial_copied"),
                report.0,
                rows.len(),
                catalogue.text("queue.rows")
            ));
        }
    }
    selected
}

fn action_button(ui: &mut Ui, enabled: bool, label: String) -> bool {
    let identity = ui.make_persistent_id(&label);
    let response = ui
        .scope_builder(UiBuilder::new().scope_id(identity), |ui| {
            ui.add_enabled(enabled, Button::new(tallyline_egui::button_text(label)))
        })
        .inner;
    crate::control_focus::reveal(ui, &response);
    response.clicked()
}
