// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Submission outcomes applied through Reporting without a context dependency reversal.

use tallyline_reporting::api::model::{
    InvalidReport, Report, ReportChange, ReportDelivery, ReportFinding, ReportState,
};
use tallyline_submission::api::model::{
    JobPhase, ReceiptDecision, SendReturnReason, SubmissionJob, SubmissionResult,
    SubmissionReturnReason,
};

/// Translates a committed job outcome for its exact owning Report and release binding.
///
/// # Errors
/// Refuses another release identity or a conflicting terminal Report outcome.
pub fn apply_submission_delivery(
    report: &Report,
    job: &SubmissionJob,
) -> Result<Option<ReportChange>, InvalidReport> {
    let header = job.request().header();
    let binding = report.release_binding().ok_or(InvalidReport)?;
    if header.report != report.id()
        || binding.job() != Some(job.id())
        || header.account != binding.account()
        || header.mode != binding.mode()
        || header.environment != binding.environment()
    {
        return Err(InvalidReport);
    }
    let verified = job.verified_row_receipts().count();
    let received = job.unknown_decision() == Some(ReceiptDecision::Received);
    let partial_received = job.phase() == JobPhase::PartiallyAccepted
        && job.request().rows().iter().all(|row| {
            job.row_result(row.id())
                .is_some_and(|result| result.is_accepted())
        });
    let resolution = if received || partial_received || verified == job.request().rows().len() {
        Some(ReportDelivery::FullAcceptance)
    } else if verified > 0 {
        Some(ReportDelivery::PartialAcceptance)
    } else {
        None
    };
    if let Some(delivery) = resolution {
        return if matches!(
            report.state(),
            ReportState::Queued | ReportState::WaitingForCooldown(_)
        ) {
            report.apply_delivery(job.id(), delivery)
        } else {
            report.resolve_delivery(job.id(), delivery)
        }
        .map(Some);
    }
    let delivery = match job.phase() {
        JobPhase::Queued | JobPhase::Sending | JobPhase::Held | JobPhase::Paused => None,
        JobPhase::Succeeded => Some(ReportDelivery::FullAcceptance),
        JobPhase::PartiallyAccepted => Some(ReportDelivery::PartialAcceptance),
        JobPhase::OutcomeUnknown => Some(ReportDelivery::Unknown),
        JobPhase::Rejected => Some(ReportDelivery::Rejected),
        JobPhase::Cancelled => Some(ReportDelivery::Return(ReportFinding::ReviewRequired)),
        JobPhase::Returned => {
            let finding = match (
                job.review_reason(),
                job.result().map(SubmissionResult::final_result),
            ) {
                (Some(SendReturnReason::ReviewRequired), _) => ReportFinding::ReviewRequired,
                (Some(SendReturnReason::GameVersionChanged), _) => {
                    ReportFinding::GameVersionChanged
                }
                (Some(SendReturnReason::ObservationAgeUnconfirmed), _) => {
                    ReportFinding::ObservationAgeUnconfirmed
                }
                (Some(SendReturnReason::ObservationTooOld), _) => ReportFinding::ObservationTooOld,
                (Some(SendReturnReason::ObservationInFuture), _) => {
                    ReportFinding::ObservationInFuture
                }
                (
                    None,
                    Some(SubmissionResult::Returned(SubmissionReturnReason::ScreenshotRequired)),
                ) => ReportFinding::ReturnedForFix(
                    tallyline_reporting::api::model::FixableReason::ScreenshotRequired,
                ),
                (
                    None,
                    Some(SubmissionResult::Returned(
                        SubmissionReturnReason::InvalidObservationTime,
                    )),
                ) => ReportFinding::ReturnedForFix(
                    tallyline_reporting::api::model::FixableReason::InvalidObservationTime,
                ),
                (
                    None,
                    Some(SubmissionResult::Returned(SubmissionReturnReason::MaxRowsExceeded)),
                ) => ReportFinding::ReturnedForFix(
                    tallyline_reporting::api::model::FixableReason::TooManyRows,
                ),
                (
                    None,
                    Some(SubmissionResult::Returned(SubmissionReturnReason::TerminalNotFound)),
                ) => ReportFinding::ReturnedForFix(
                    tallyline_reporting::api::model::FixableReason::TerminalNotFound,
                ),
                (
                    None,
                    Some(SubmissionResult::Returned(SubmissionReturnReason::InvalidScreenshot)),
                ) => ReportFinding::ReturnedForFix(
                    tallyline_reporting::api::model::FixableReason::InvalidScreenshot,
                ),
                (
                    None,
                    Some(SubmissionResult::Returned(SubmissionReturnReason::ScreenshotTooLarge)),
                ) => ReportFinding::ReturnedForFix(
                    tallyline_reporting::api::model::FixableReason::ScreenshotTooLarge,
                ),
                (
                    None,
                    Some(SubmissionResult::Returned(SubmissionReturnReason::PreflightChanged)),
                ) => ReportFinding::ReviewRequired,
                (
                    None,
                    Some(
                        SubmissionResult::DuplicateEvidence(_)
                        | SubmissionResult::Returned(SubmissionReturnReason::PossiblyAlreadyReceived),
                    ),
                ) => ReportFinding::PossiblyAlreadyReceived,
                (None, _) => ReportFinding::AppRestart,
            };
            Some(ReportDelivery::Return(finding))
        }
    };
    delivery
        .map(|delivery| report.apply_delivery(job.id(), delivery))
        .transpose()
}
