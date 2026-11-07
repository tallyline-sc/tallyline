// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Stable released-value and wire-attempt binding across append-only resolution evidence.

use crate::submission_request_document::{HeaderDocument, RowDocument};
use serde::Serialize;
use sha2::Sha256;
use tallyline_submission::api::model::{SubmissionJob, SubmissionRepositoryError};

#[derive(Serialize)]
struct LifecycleBinding {
    job: u64,
    header: HeaderDocument,
    rows: Vec<RowDocument>,
    attempts: usize,
    started_at: String,
    date_added: String,
    ended_at: Option<String>,
    payload_hash: [u8; 32],
    attempt_rows: usize,
}
pub(crate) fn hash(job: &SubmissionJob) -> Result<[u8; 32], SubmissionRepositoryError> {
    use sha2::Digest;
    let attempt = job
        .attempts()
        .last()
        .ok_or(SubmissionRepositoryError::Invalid)?;
    let document = LifecycleBinding {
        job: job.id().0,
        header: job.request().header().into(),
        rows: job.request().rows().iter().map(Into::into).collect(),
        attempts: job.attempts().len(),
        started_at: attempt.started_at().millis().to_string(),
        date_added: attempt.date_added().millis().to_string(),
        ended_at: attempt.ended_at().map(|at| at.millis().to_string()),
        payload_hash: attempt.payload_hash().bytes(),
        attempt_rows: attempt.rows(),
    };
    let encoded = serde_json::to_vec(&document).map_err(|_| SubmissionRepositoryError::Invalid)?;
    Ok(Sha256::digest(encoded).into())
}
