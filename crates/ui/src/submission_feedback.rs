// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Closed actionable release feedback without formatting source or remote values.

use crate::api::Catalogue;
use crate::api::model::SubmissionFeedback;
use tallyline_reporting::api::model::ReleaseBlockReason;

pub(crate) fn show(ui: &mut egui::Ui, feedback: &SubmissionFeedback, catalogue: &Catalogue) {
    match feedback {
        SubmissionFeedback::Split => {
            ui.label(catalogue.text("queue.split_result"));
        }
        SubmissionFeedback::Blocked(blocks) => {
            for block in blocks {
                ui.label(catalogue.text(block_key(*block)));
            }
        }
        SubmissionFeedback::Changed => {
            ui.label(catalogue.text("release.changed"));
        }
        SubmissionFeedback::Refused(_) => {
            ui.label(catalogue.text("queue.cannot_cancel"));
        }
        SubmissionFeedback::Unavailable => {
            ui.label(catalogue.text("queue.unavailable"));
        }
        SubmissionFeedback::AttachmentUnavailable => {
            ui.label(catalogue.text("release.working_copy"));
        }
        SubmissionFeedback::Returned(reason) => {
            ui.label(catalogue.text(crate::submission_queue::review_reason_key(*reason)));
        }
        SubmissionFeedback::Held(reason) => {
            use tallyline_submission::api::model::SendHoldReason;
            let key = match reason {
                SendHoldReason::AccountUnavailable => "release.account",
                SendHoldReason::AccountChanged => "release.changed",
                SendHoldReason::ReferenceUnavailable => "release.reference",
                SendHoldReason::AcceptanceClosed => "release.acceptance",
            };
            ui.label(catalogue.text(key));
        }
    }
}
fn block_key(block: ReleaseBlockReason) -> &'static str {
    match block {
        ReleaseBlockReason::StaleReview | ReleaseBlockReason::InvalidState => "release.changed",
        ReleaseBlockReason::Numeric(_) => "release.numeric",
        ReleaseBlockReason::TooManyRows => "release.rows",
        ReleaseBlockReason::ReferenceMismatch
        | ReleaseBlockReason::AssessmentUnavailable
        | ReleaseBlockReason::UnknownTerminal
        | ReleaseBlockReason::UnknownCommodity(_)
        | ReleaseBlockReason::StatusVocabularyUnavailable
        | ReleaseBlockReason::UnknownStatus(_) => "release.reference",
        ReleaseBlockReason::UnsupportedStock(_) => "release.stock",
        ReleaseBlockReason::AccountUnavailable => "release.account",
        ReleaseBlockReason::ScreenshotPolicyUnavailable => "release.policy",
        ReleaseBlockReason::ImageRequired => "queue.screenshot_required",
        ReleaseBlockReason::PreviewUnconfirmed => "release.preview",
        ReleaseBlockReason::ObservationTimeUncertain => "release.time",
        ReleaseBlockReason::WorkingCopyUnavailable => "release.working_copy",
        ReleaseBlockReason::GameVersionUnavailable => "release.version",
        ReleaseBlockReason::VersionHistoryUnavailable => "release.version_history",
        ReleaseBlockReason::GameVersionChanged => "queue.version_changed",
        ReleaseBlockReason::ObservationAgeUnconfirmed => "queue.age_unconfirmed",
        ReleaseBlockReason::ObservationTooOld => "queue.observation_too_old",
        ReleaseBlockReason::ObservationInFuture => "queue.observation_in_future",
        ReleaseBlockReason::EnvironmentUnconfirmed => "release.environment",
        ReleaseBlockReason::ObservationHistoryUnavailable => "release.history",
        ReleaseBlockReason::NewerObservationSent { .. } => "release.newer",
        ReleaseBlockReason::ResolvedDuplicateOrigin { .. } => "release.resolved_duplicate_origin",
        ReleaseBlockReason::UnresolvedNewerObservation { .. } => "release.unknown_newer",
        ReleaseBlockReason::StaleObservationDecision(_) => "release.changed",
    }
}
