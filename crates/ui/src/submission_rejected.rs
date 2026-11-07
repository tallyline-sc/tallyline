// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Explicit permanent-refusal copying beside immutable rejection history.

use crate::api::Catalogue;
use crate::resolution_queue_action::ResolutionQueueAction;
use egui::{Button, Ui};
use tallyline_submission::api::model::{JobPhase, SubmissionJob, SubmissionResolutionAction};

pub(crate) fn show(
    ui: &mut Ui,
    job: &SubmissionJob,
    catalogue: &Catalogue,
    enabled: bool,
) -> Option<ResolutionQueueAction> {
    if job.phase() != JobPhase::Rejected {
        return None;
    }
    for action in job.resolution_actions() {
        if let SubmissionResolutionAction::Duplicated { report, .. } = action {
            ui.label(format!(
                "{} {}",
                catalogue.text("queue.unknown_new_draft"),
                report.0
            ));
        }
    }
    let action = ResolutionQueueAction::DuplicateAsDraft;
    if action.eligible(job)
        && ui
            .add_enabled(
                enabled,
                Button::new(tallyline_egui::button_text(format!(
                    "{} {}",
                    catalogue.text("queue.duplicate_unknown"),
                    job.id().0
                ))),
            )
            .clicked()
    {
        Some(action)
    } else {
        None
    }
}
