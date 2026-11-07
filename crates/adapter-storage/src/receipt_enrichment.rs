// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Durable single-admission receipt proofs without altering known production acceptance.

use crate::api::SqliteReferenceCache;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior};
use tallyline_submission::api::model::{
    JobId, JobPhase, SubmissionJob, SubmissionRepositoryError, SubmissionResult,
    VerifiedReceiptMapping,
};
use tallyline_workflows::api::ReceiptEnrichmentStore;

fn invalid<T>(_: T) -> SubmissionRepositoryError {
    SubmissionRepositoryError::Invalid
}
fn unavailable(_: rusqlite::Error) -> SubmissionRepositoryError {
    SubmissionRepositoryError::Unavailable
}
fn integer(value: u64) -> Result<i64, SubmissionRepositoryError> {
    value.try_into().map_err(invalid)
}

impl ReceiptEnrichmentStore for SqliteReferenceCache {
    fn claim_receipt_enrichment(
        &mut self,
        accepted: &SubmissionJob,
    ) -> Result<bool, SubmissionRepositoryError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(unavailable)?;
        let current = crate::queue::load_job(&transaction, accepted.id())?
            .ok_or(SubmissionRepositoryError::Invalid)?;
        if &current != accepted {
            return Err(SubmissionRepositoryError::Conflict);
        }
        if current.phase() != JobPhase::Succeeded
            || !matches!(
                current.result(),
                Some(SubmissionResult::ProductionAccepted(_))
            )
        {
            return Err(SubmissionRepositoryError::Invalid);
        }
        let changed=transaction.execute("INSERT INTO submission_receipt_enrichment(job_id,job_revision,payload) VALUES(?1,?2,NULL) ON CONFLICT(job_id) DO NOTHING",(integer(current.id().0)?,integer(current.revision())?)).map_err(unavailable)?;
        let revision: i64 = transaction
            .query_row(
                "SELECT job_revision FROM submission_receipt_enrichment WHERE job_id=?1",
                [integer(current.id().0)?],
                |row| row.get(0),
            )
            .map_err(unavailable)?;
        if revision != integer(current.revision())? {
            return Err(SubmissionRepositoryError::Invalid);
        }
        transaction.commit().map_err(unavailable)?;
        Ok(changed == 1)
    }
    fn save_receipt_mapping(
        &mut self,
        mapping: &VerifiedReceiptMapping,
    ) -> Result<(), SubmissionRepositoryError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(unavailable)?;
        let current = crate::queue::load_job(&transaction, mapping.job())?
            .ok_or(SubmissionRepositoryError::Invalid)?;
        if !mapping.matches_job(&current) {
            return Err(SubmissionRepositoryError::Conflict);
        }
        let record: Option<(i64, Option<String>)> = transaction
            .query_row(
                "SELECT job_revision,payload FROM submission_receipt_enrichment WHERE job_id=?1",
                [integer(mapping.job().0)?],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(unavailable)?;
        let (revision, payload) = record.ok_or(SubmissionRepositoryError::Invalid)?;
        if revision != integer(mapping.revision())? {
            return Err(SubmissionRepositoryError::Conflict);
        }
        if let Some(payload) = payload {
            if crate::queue_document::decode_mapping(&current, &payload)? != *mapping {
                return Err(SubmissionRepositoryError::Conflict);
            }
            return Ok(());
        }
        let payload = crate::queue_document::encode_mapping(mapping)?;
        let changed = transaction.execute("UPDATE submission_receipt_enrichment SET payload=?1 WHERE job_id=?2 AND payload IS NULL",(payload,integer(mapping.job().0)?)).map_err(unavailable)?;
        if changed != 1 {
            return Err(SubmissionRepositoryError::Conflict);
        }
        transaction.commit().map_err(unavailable)
    }
    fn receipt_mapping(
        &mut self,
        job: JobId,
    ) -> Result<Option<VerifiedReceiptMapping>, SubmissionRepositoryError> {
        load_mapping(&self.connection, job)
    }
}
pub(crate) fn load_mapping(
    connection: &Connection,
    job: JobId,
) -> Result<Option<VerifiedReceiptMapping>, SubmissionRepositoryError> {
    let record: Option<(i64, Option<String>)> = connection
        .query_row(
            "SELECT job_revision,payload FROM submission_receipt_enrichment WHERE job_id=?1",
            [integer(job.0)?],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(unavailable)?;
    let Some((revision, Some(payload))) = record else {
        return Ok(None);
    };
    let current =
        crate::queue::load_job(connection, job)?.ok_or(SubmissionRepositoryError::Invalid)?;
    if revision != integer(current.revision())? {
        return Err(SubmissionRepositoryError::Invalid);
    }
    crate::queue_document::decode_mapping(&current, &payload).map(Some)
}
