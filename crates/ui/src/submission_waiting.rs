// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Plain per-job waits with absolute cooldown and context-check times.

use crate::api::Catalogue;
use tallyline_submission::api::model::{SendHoldReason, SubmissionWait, SubmissionWaitReason};

pub(crate) fn show(ui: &mut egui::Ui, wait: &SubmissionWait, catalogue: &Catalogue) {
    let key = match wait.reason {
        SubmissionWaitReason::AccountUnavailable => "queue.wait_account",
        SubmissionWaitReason::AccountChanged => "queue.wait_account_changed",
        SubmissionWaitReason::InFlight => "queue.wait_inflight",
        SubmissionWaitReason::CooldownUntil(until) => {
            ui.label(format!(
                "{} {}",
                catalogue.text("queue.wait_cooldown"),
                crate::attachment_views::format_time(until)
            ));
            return;
        }
        SubmissionWaitReason::EarlierObservationPending => "queue.wait_earlier",
        SubmissionWaitReason::RowBudget => "queue.wait_budget",
        SubmissionWaitReason::Context(SendHoldReason::AccountUnavailable) => "queue.wait_account",
        SubmissionWaitReason::Context(SendHoldReason::AccountChanged) => {
            "queue.wait_account_changed"
        }
        SubmissionWaitReason::Context(SendHoldReason::ReferenceUnavailable) => "release.reference",
        SubmissionWaitReason::Context(SendHoldReason::AcceptanceClosed) => "release.acceptance",
        SubmissionWaitReason::NewerObservationReview => "release.newer",
        SubmissionWaitReason::UnresolvedNewerObservation => "release.unknown_newer",
        SubmissionWaitReason::Unavailable => "queue.unavailable",
    };
    ui.label(catalogue.text(key));
    if let Some(next) = wait.next_check {
        ui.label(format!(
            "{} {}",
            catalogue.text("queue.next_check"),
            crate::attachment_views::format_time(next)
        ));
    }
}
