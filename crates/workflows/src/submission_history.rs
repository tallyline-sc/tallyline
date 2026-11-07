// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Real committed Submission history assessed for Reporting's exact release snapshot.

use tallyline_reporting::api::model::{
    ObservationHistoryAssessment, ObservationHistoryFinding, SubmissionGate,
};
use tallyline_submission::api::ObservationOrder;
use tallyline_submission::api::model::{JobPhase, ObservationOrderBlock, SubmissionJob};

/// Supplies exact history facts without confusing unavailable history with an empty queue.
#[must_use]
pub fn assess_release_observation_history(
    gate: &SubmissionGate,
    history: Option<&[SubmissionJob]>,
) -> SubmissionGate {
    let Some(history) = history else {
        return gate.with_observation_history(None);
    };
    let Some(account) = gate.accepted_account() else {
        return gate.with_observation_history(None);
    };
    let environment = gate.snapshot().environment();
    let Ok(request) = crate::submission_request::manual_submission_request(
        gate.report(),
        environment,
        account,
        gate.mode(),
    ) else {
        return gate.with_observation_history(None);
    };
    let mut findings = ObservationOrder::assess(&request, history)
        .into_iter()
        .map(|block| match block {
            ObservationOrderBlock::ResolvedDuplicateOrigin { submission } => {
                ObservationHistoryFinding::ResolvedDuplicateOrigin { submission }
            }
            ObservationOrderBlock::NeedsConfirmation { row, observed_at } => {
                ObservationHistoryFinding::NewerAccepted { row, observed_at }
            }
            ObservationOrderBlock::Unresolved {
                row,
                submission,
                observed_at,
            } => ObservationHistoryFinding::Unresolved {
                row,
                submission,
                observed_at,
            },
        })
        .collect::<Vec<_>>();
    for approval in request.newer_observation_confirmations() {
        let Some(row) = request.rows().iter().find(|row| row.id() == approval.row()) else {
            return gate.with_observation_history(None);
        };
        let matched = history.iter().any(|job| {
            let old = job.request();
            let header = old.header();
            job.phase() == JobPhase::Succeeded
                && header.account == account
                && header.mode == gate.mode()
                && header.terminal == request.header().terminal
                && header.side == request.header().side
                && header.observed_at == approval.observed_at()
                && old
                    .rows()
                    .iter()
                    .any(|old_row| old_row.commodity() == row.commodity())
        });
        if !matched {
            findings.push(ObservationHistoryFinding::StaleDecision {
                row: approval.row(),
            });
        }
    }
    gate.with_observation_history(Some(ObservationHistoryAssessment::new(
        gate.report(),
        account,
        gate.mode(),
        environment,
        findings,
    )))
}

/// Confirms one exact older row only against its newest actual committed accepted observation.
/// Existing confirmations cannot hide newer uncertain history from this decision.
///
/// # Errors
/// Refuses absent, changed or unresolved evidence, future evidence and invalid Draft values.
pub fn confirm_older_observation(
    report: &tallyline_reporting::api::model::Report,
    row: tallyline_shared_kernel::api::model::RowId,
    evidence: tallyline_reporting::api::model::NewerObservationEvidence,
    history: &[SubmissionJob],
    now: tallyline_shared_kernel::api::model::Timestamp,
) -> Result<tallyline_reporting::api::model::Report, crate::api::ManualReleaseError> {
    use crate::api::ManualReleaseError;
    use tallyline_reporting::api::model::ReleaseBlockReason;
    if evidence.observed_at() > now {
        return Err(ManualReleaseError::Conflict);
    }
    let scoped = crate::submission_request::manual_submission_request(
        report,
        evidence.environment(),
        evidence.account(),
        evidence.mode(),
    )
    .map_err(|_| ManualReleaseError::Invalid)?;
    let unconfirmed = tallyline_submission::api::model::SubmissionRequest::new(
        scoped.header().clone(),
        scoped.rows().to_vec(),
    )
    .map_err(|_| ManualReleaseError::Invalid)?;
    let mut matched = false;
    for finding in ObservationOrder::assess(&unconfirmed, history) {
        match finding {
            ObservationOrderBlock::ResolvedDuplicateOrigin { submission } => {
                return Err(ManualReleaseError::Blocked(vec![
                    ReleaseBlockReason::ResolvedDuplicateOrigin { submission },
                ]));
            }
            ObservationOrderBlock::NeedsConfirmation {
                row: candidate,
                observed_at,
            } => {
                if candidate == row {
                    matched = observed_at == evidence.observed_at();
                }
            }
            ObservationOrderBlock::Unresolved {
                row: candidate,
                submission,
                observed_at,
            } => {
                if candidate == row {
                    return Err(ManualReleaseError::Blocked(vec![
                        ReleaseBlockReason::UnresolvedNewerObservation {
                            row,
                            submission,
                            observed_at,
                        },
                    ]));
                }
            }
        }
    }
    if !matched {
        return Err(ManualReleaseError::Conflict);
    }
    report
        .confirm_newer_observation(row, evidence, now)
        .map(tallyline_reporting::api::model::ReportChange::into_state)
        .map_err(|_| ManualReleaseError::Invalid)
}
