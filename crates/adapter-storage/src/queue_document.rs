// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Validated queue persistence without credentials or encoded request bodies.

use crate::queue_account_evidence::{AccountCheckDocument, AccountEvidenceDocument};
use crate::queue_client_version::ClientVersionDocument;
use crate::queue_outcome_document::{OutcomeDocument, ReceiptDocument, ResultDocument};
use crate::resolution_action_document::ResolutionActionDocument;
pub(crate) use crate::submission_request_document::{EnvironmentDocument, SideDocument};
use crate::submission_request_document::{HeaderDocument, RowDocument};
use serde::{Deserialize, Serialize};
use tallyline_shared_kernel::api::model::{CommodityId, ReportId, RowId, TerminalId, Timestamp};
use tallyline_submission::api::model::SubmissionRepositoryError;
use tallyline_submission::api::model::{
    JobId, JobPhase, PayloadHash, SendReturnReason, StoredSubmissionAttempt, StoredSubmissionJob,
    SubmissionHeader, SubmissionJob, SubmissionRequest, SubmissionResult, SubmissionRow,
};
use tallyline_submission::api::model::{SubmissionAccountCheck, VerifiedReceiptMapping};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct JobDocument {
    #[serde(default, alias = "unknown_actions")]
    resolution_actions: Vec<ResolutionActionDocument>,
    header: HeaderDocument,
    rows: Vec<RowDocument>,
    newer_approvals: Vec<(u64, String)>,
    phase: PhaseDocument,
    attempts: Vec<AttemptDocument>,
    result: Option<ResultDocument>,
    review_reason: Option<ReturnDocument>,
    #[serde(default)]
    service_pause_acknowledged: bool,
    #[serde(default)]
    client_update: Option<ClientVersionDocument>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AttemptDocument {
    started_at: String,
    rows: usize,
    payload_hash: [u8; 32],
    date_added: String,
    ended_at: Option<String>,
    outcome: Option<OutcomeDocument>,
    #[serde(default)]
    receipt_checked: bool,
    #[serde(default)]
    account_evidence: Option<AccountEvidenceDocument>,
    #[serde(default)]
    account_checks: Option<Vec<AccountCheckDocument>>,
    #[serde(default)]
    client_update: Option<ClientVersionDocument>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PhaseDocument {
    Rejected,
    Queued,
    Succeeded,
    PartiallyAccepted,
    Sending,
    OutcomeUnknown,
    Returned,
    Cancelled,
    Held,
    Paused,
}
impl From<JobPhase> for PhaseDocument {
    fn from(value: JobPhase) -> Self {
        match value {
            JobPhase::Queued => Self::Queued,
            JobPhase::Rejected => Self::Rejected,
            JobPhase::Succeeded => Self::Succeeded,
            JobPhase::PartiallyAccepted => Self::PartiallyAccepted,
            JobPhase::Sending => Self::Sending,
            JobPhase::OutcomeUnknown => Self::OutcomeUnknown,
            JobPhase::Returned => Self::Returned,
            JobPhase::Cancelled => Self::Cancelled,
            JobPhase::Held => Self::Held,
            JobPhase::Paused => Self::Paused,
        }
    }
}
impl From<PhaseDocument> for JobPhase {
    fn from(value: PhaseDocument) -> Self {
        match value {
            PhaseDocument::Queued => Self::Queued,
            PhaseDocument::Rejected => Self::Rejected,
            PhaseDocument::Succeeded => Self::Succeeded,
            PhaseDocument::PartiallyAccepted => Self::PartiallyAccepted,
            PhaseDocument::Sending => Self::Sending,
            PhaseDocument::OutcomeUnknown => Self::OutcomeUnknown,
            PhaseDocument::Returned => Self::Returned,
            PhaseDocument::Cancelled => Self::Cancelled,
            PhaseDocument::Held => Self::Held,
            PhaseDocument::Paused => Self::Paused,
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ReturnDocument {
    ReviewRequired,
    GameVersionChanged,
    ObservationAgeUnconfirmed,
    ObservationTooOld,
    ObservationInFuture,
}
impl From<SendReturnReason> for ReturnDocument {
    fn from(value: SendReturnReason) -> Self {
        match value {
            SendReturnReason::ReviewRequired => Self::ReviewRequired,
            SendReturnReason::GameVersionChanged => Self::GameVersionChanged,
            SendReturnReason::ObservationAgeUnconfirmed => Self::ObservationAgeUnconfirmed,
            SendReturnReason::ObservationTooOld => Self::ObservationTooOld,
            SendReturnReason::ObservationInFuture => Self::ObservationInFuture,
        }
    }
}
impl From<ReturnDocument> for SendReturnReason {
    fn from(value: ReturnDocument) -> Self {
        match value {
            ReturnDocument::ReviewRequired => Self::ReviewRequired,
            ReturnDocument::GameVersionChanged => Self::GameVersionChanged,
            ReturnDocument::ObservationAgeUnconfirmed => Self::ObservationAgeUnconfirmed,
            ReturnDocument::ObservationTooOld => Self::ObservationTooOld,
            ReturnDocument::ObservationInFuture => Self::ObservationInFuture,
        }
    }
}

fn invalid<T>(_: T) -> SubmissionRepositoryError {
    SubmissionRepositoryError::Invalid
}
fn timestamp(value: &str) -> Result<Timestamp, SubmissionRepositoryError> {
    crate::reference_cache::timestamp(value).map_err(invalid)
}
pub(crate) fn encode(job: &SubmissionJob) -> Result<String, SubmissionRepositoryError> {
    let request = job.request();
    let header = request.header();
    let document = JobDocument {
        resolution_actions: job.resolution_actions().iter().map(Into::into).collect(),
        header: header.into(),
        rows: request.rows().iter().map(Into::into).collect(),
        newer_approvals: request
            .newer_observation_confirmations()
            .iter()
            .map(|approval| {
                (
                    approval.row().0,
                    approval.observed_at().millis().to_string(),
                )
            })
            .collect(),
        phase: job.phase().into(),
        attempts: job
            .attempts()
            .iter()
            .map(|attempt| AttemptDocument {
                started_at: attempt.started_at().millis().to_string(),
                rows: attempt.rows(),
                payload_hash: attempt.payload_hash().bytes(),
                date_added: attempt.date_added().millis().to_string(),
                ended_at: attempt.ended_at().map(|at| at.millis().to_string()),
                outcome: attempt.outcome().map(Into::into),
                receipt_checked: attempt.receipt_checked(),
                account_evidence: attempt.account_evidence().map(Into::into),
                account_checks: Some(
                    attempt
                        .account_checks()
                        .iter()
                        .map(|reason| {
                            AccountCheckDocument::from(SubmissionAccountCheck::Completed(*reason))
                        })
                        .collect(),
                ),
                client_update: attempt.client_update().map(ClientVersionDocument),
            })
            .collect(),
        result: job.result().map(Into::into),
        review_reason: job.review_reason().map(Into::into),
        service_pause_acknowledged: job.service_pause_acknowledged(),
        client_update: job.client_update().map(ClientVersionDocument),
    };
    let payload = serde_json::to_string(&document).map_err(invalid)?;
    if decode(job.id(), job.revision(), &payload)? != *job {
        return Err(SubmissionRepositoryError::Invalid);
    }
    Ok(payload)
}
pub(crate) fn decode(
    id: JobId,
    revision: u64,
    payload: &str,
) -> Result<SubmissionJob, SubmissionRepositoryError> {
    let document: JobDocument = serde_json::from_str(payload).map_err(invalid)?;
    let header = document.header;
    let rows = document
        .rows
        .into_iter()
        .map(|row| {
            SubmissionRow::new(
                RowId(row.id),
                CommodityId(row.commodity),
                row.price.parse().map_err(invalid)?,
                row.stock.parse().map_err(invalid)?,
                row.status,
            )
            .map_err(invalid)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut request = SubmissionRequest::new(
        SubmissionHeader {
            report: ReportId(header.report),
            report_version: header.report_version,
            account: header.account,
            mode: header.mode.into(),
            terminal: TerminalId(header.terminal),
            side: header.side.into(),
            environment: header.environment.into(),
            game_version: header.game_version,
            faction_affinity: header
                .faction_affinity
                .map(tallyline_shared_kernel::api::model::FactionAffinity::new)
                .transpose()
                .map_err(invalid)?,
            observed_at: timestamp(&header.observed_at)?,
        },
        rows,
    )
    .map_err(invalid)?;
    for (row, observed) in document.newer_approvals {
        request = request
            .with_confirmed_newer_observation(RowId(row), timestamp(&observed)?)
            .map_err(invalid)?;
    }
    let attempts = document
        .attempts
        .into_iter()
        .map(|attempt| {
            Ok(StoredSubmissionAttempt {
                started_at: timestamp(&attempt.started_at)?,
                rows: attempt.rows,
                payload_hash: PayloadHash::new(attempt.payload_hash),
                date_added: timestamp(&attempt.date_added)?,
                ended_at: attempt.ended_at.map(|at| timestamp(&at)).transpose()?,
                outcome: attempt.outcome.map(Into::into),
                receipt_checked: attempt.receipt_checked,
                account_checks: attempt
                    .account_checks
                    .map(|checks| {
                        checks
                            .into_iter()
                            .map(|check| {
                                match check
                                    .restore(&request, Some(timestamp(&attempt.date_added)?))?
                                {
                                    SubmissionAccountCheck::Completed(reason) => Ok(reason),
                                    SubmissionAccountCheck::NotRequired
                                    | SubmissionAccountCheck::Pending => {
                                        Err(SubmissionRepositoryError::Invalid)
                                    }
                                }
                            })
                            .collect::<Result<Vec<_>, _>>()
                    })
                    .transpose()?,
                account_evidence: attempt
                    .account_evidence
                    .map(|evidence| {
                        evidence.restore(&request, Some(timestamp(&attempt.date_added)?))
                    })
                    .transpose()?,
                client_update: attempt.client_update.map(|version| version.0),
            })
        })
        .collect::<Result<Vec<_>, SubmissionRepositoryError>>()?;
    let result = document
        .result
        .map(|result| result.restore(&request, attempts.last().map(|attempt| attempt.date_added)))
        .transpose()?;
    let resolution_actions = document
        .resolution_actions
        .into_iter()
        .map(|action| action.restore(&request, attempts.last().map(|attempt| attempt.date_added)))
        .collect::<Result<Vec<_>, _>>()?;
    SubmissionJob::reconstitute(StoredSubmissionJob {
        resolution_actions,
        id,
        revision,
        request,
        phase: document.phase.into(),
        attempts,
        result,
        review_reason: document.review_reason.map(Into::into),
        service_pause_acknowledged: document.service_pause_acknowledged,
        client_update: document.client_update.map(|version| version.0),
    })
    .map_err(invalid)
}
pub(crate) const fn phase_name(phase: JobPhase) -> &'static str {
    match phase {
        JobPhase::Queued => "queued",
        JobPhase::Rejected => "rejected",
        JobPhase::Succeeded => "succeeded",
        JobPhase::PartiallyAccepted => "partially_accepted",
        JobPhase::Sending => "sending",
        JobPhase::OutcomeUnknown => "outcome_unknown",
        JobPhase::Returned => "returned",
        JobPhase::Cancelled => "cancelled",
        JobPhase::Held => "held",
        JobPhase::Paused => "paused",
    }
}

pub(crate) fn encode_mapping(
    mapping: &VerifiedReceiptMapping,
) -> Result<String, SubmissionRepositoryError> {
    let document: Vec<_> = mapping
        .receipts()
        .iter()
        .map(|receipt| ReceiptDocument {
            id: receipt.id(),
            row: receipt.row().0,
            date_added: receipt.date_added().millis().to_string(),
        })
        .collect();
    serde_json::to_string(&document).map_err(invalid)
}
pub(crate) fn decode_mapping(
    job: &SubmissionJob,
    payload: &str,
) -> Result<VerifiedReceiptMapping, SubmissionRepositoryError> {
    let documents: Vec<ReceiptDocument> = serde_json::from_str(payload).map_err(invalid)?;
    let restored = ResultDocument::ProductionSucceeded(documents).restore(
        job.request(),
        job.attempts().last().map(|attempt| attempt.date_added()),
    )?;
    let SubmissionResult::ProductionSucceeded(receipts) = restored else {
        return Err(SubmissionRepositoryError::Invalid);
    };
    VerifiedReceiptMapping::new(job, receipts).map_err(invalid)
}
