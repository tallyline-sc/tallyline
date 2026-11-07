// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Validated canonical lifecycle sources from retained submission proofs.

use rusqlite::Connection;
use tallyline_submission::api::model::{JobId, SubmissionLifecycle, SubmissionRepositoryError};

pub(crate) fn load(
    connection: &Connection,
    id: JobId,
) -> Result<SubmissionLifecycle, SubmissionRepositoryError> {
    let job = crate::queue::load_job(connection, id)?.ok_or(SubmissionRepositoryError::Invalid)?;
    match crate::receipt_enrichment::load_mapping(connection, id)? {
        Some(mapping) => SubmissionLifecycle::try_new(&job, &mapping),
        None => SubmissionLifecycle::try_from_job(&job),
    }
    .map_err(|_| SubmissionRepositoryError::Invalid)
}
pub(crate) fn verify(
    connection: &Connection,
    source: &SubmissionLifecycle,
) -> Result<(), SubmissionRepositoryError> {
    if crate::queue::load_job(connection, source.job())?.as_ref() != Some(source.source()) {
        return Err(SubmissionRepositoryError::Conflict);
    }
    Ok(())
}
