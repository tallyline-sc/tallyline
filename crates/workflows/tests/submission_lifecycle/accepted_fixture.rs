// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact accepted and enriched one-row submissions for lifecycle contracts.

use tallyline_adapter_storage::api::SqliteReferenceCache;
use tallyline_shared_kernel::api::model::{Timestamp, UexReportId};
use tallyline_submission::api::SubmissionJobRepository;
use tallyline_submission::api::model::{
    PayloadHash, SubmissionChange, SubmissionCommand, SubmissionJob, SubmissionResult,
    VerifiedReceiptMapping, VerifiedUexReport,
};
use tallyline_workflows::api::ReceiptEnrichmentStore;

pub(super) struct AcceptedAttempt {
    pub started_at: Timestamp,
    pub completed_at: Timestamp,
    pub date_added: Timestamp,
    pub payload_hash: PayloadHash,
}

pub(super) fn accepted_and_enriched(
    store: &mut SqliteReferenceCache,
    job: &SubmissionJob,
    id: u64,
    attempt: AcceptedAttempt,
) -> std::io::Result<(SubmissionChange, VerifiedUexReport)> {
    let sending = job
        .apply(SubmissionCommand::Begin {
            at: attempt.started_at,
            payload_hash: attempt.payload_hash,
            date_added: attempt.date_added,
        })
        .map_err(error)?;
    SubmissionJobRepository::save(store, &sending).map_err(error)?;
    let accepted = sending
        .state()
        .apply(SubmissionCommand::Complete {
            at: attempt.completed_at,
            result: SubmissionResult::ProductionAccepted(vec![
                UexReportId::new(id, attempt.date_added)
                    .ok_or_else(|| error("invalid remote fixture identity"))?,
            ]),
        })
        .map_err(error)?;
    SubmissionJobRepository::save(store, &accepted).map_err(error)?;
    let request = accepted.state().request();
    assert_eq!(request.rows().len(), 1);
    let receipt = VerifiedUexReport::from_lookup_at(
        id,
        request.header().terminal,
        request
            .rows()
            .first()
            .ok_or_else(|| error("missing fixture row"))?
            .commodity(),
        attempt.date_added,
        request,
        attempt.date_added,
    )
    .map_err(error)?;
    let mapping =
        VerifiedReceiptMapping::new(accepted.state(), vec![receipt.clone()]).map_err(error)?;
    assert!(
        store
            .claim_receipt_enrichment(accepted.state())
            .map_err(error)?
    );
    store.save_receipt_mapping(&mapping).map_err(error)?;
    Ok((accepted, receipt))
}

pub(super) fn error(value: impl std::fmt::Debug) -> std::io::Error {
    std::io::Error::other(format!("{value:?}"))
}
