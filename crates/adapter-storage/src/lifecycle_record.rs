// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Indexed lifecycle records validated against payload and canonical source.

use rusqlite::{Connection, OptionalExtension};
use tallyline_shared_kernel::api::model::Timestamp;
use tallyline_submission::api::model::{JobId, SubmissionLifecycle, SubmissionRepositoryError};

pub(crate) struct LifecycleRecord {
    account: String,
    admitted_at: Timestamp,
    manual_admitted_at: Option<Timestamp>,
    revision: u64,
    payload: String,
}
fn invalid<T>(_: T) -> SubmissionRepositoryError {
    SubmissionRepositoryError::Invalid
}
fn unavailable(_: rusqlite::Error) -> SubmissionRepositoryError {
    SubmissionRepositoryError::Unavailable
}
fn integer(value: u64) -> Result<i64, SubmissionRepositoryError> {
    value.try_into().map_err(invalid)
}
fn admission(value: &str) -> Result<Timestamp, SubmissionRepositoryError> {
    let at = crate::reference_cache::timestamp(value).map_err(invalid)?;
    if at.millis().to_string() != value {
        return Err(SubmissionRepositoryError::Invalid);
    }
    Ok(at)
}
impl LifecycleRecord {
    pub(crate) fn decode(
        self,
        source: &SubmissionLifecycle,
    ) -> Result<SubmissionLifecycle, SubmissionRepositoryError> {
        let lifecycle = crate::lifecycle_document::decode(source, self.revision, &self.payload)?;
        if self.account != source.source().request().header().account
            || lifecycle.admitted_at() != Some(self.admitted_at)
            || lifecycle.manual_admitted_at() != self.manual_admitted_at
        {
            return Err(SubmissionRepositoryError::Invalid);
        }
        Ok(lifecycle)
    }
}
pub(crate) fn record(
    connection: &Connection,
    id: JobId,
) -> Result<Option<LifecycleRecord>, SubmissionRepositoryError> {
    let row: Option<(String, String, Option<String>, i64, String)> = connection.query_row(
        "SELECT account,admitted_at,manual_admitted_at,revision,payload FROM submission_lifecycles WHERE job_id=?1",
        [integer(id.0)?], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
    ).optional().map_err(unavailable)?;
    row.map(
        |(account, admitted_at, manual_admitted_at, revision, payload)| {
            Ok(LifecycleRecord {
                account,
                admitted_at: admission(&admitted_at)?,
                manual_admitted_at: manual_admitted_at.as_deref().map(admission).transpose()?,
                revision: revision.try_into().map_err(invalid)?,
                payload,
            })
        },
    )
    .transpose()
}
pub(crate) fn load(
    connection: &Connection,
    source: &SubmissionLifecycle,
) -> Result<Option<SubmissionLifecycle>, SubmissionRepositoryError> {
    record(connection, source.job())?
        .map(|record| record.decode(source))
        .transpose()
}
pub(crate) fn latest_manual(
    connection: &Connection,
    account: &str,
) -> Result<Option<Timestamp>, SubmissionRepositoryError> {
    let id: Option<i64> = connection.query_row(
        "SELECT job_id FROM submission_lifecycles WHERE account=?1 AND manual_admitted_at IS NOT NULL ORDER BY length(manual_admitted_at) DESC,manual_admitted_at DESC LIMIT 1",
        [account], |row| row.get(0),
    ).optional().map_err(unavailable)?;
    let Some(id) = id else {
        return Ok(None);
    };
    let id = JobId(id.try_into().map_err(invalid)?);
    let source = crate::lifecycle_source::load(connection, id)?;
    let lifecycle = record(connection, id)?
        .ok_or(SubmissionRepositoryError::Invalid)?
        .decode(&source)?;
    if lifecycle.source().request().header().account != account {
        return Err(SubmissionRepositoryError::Invalid);
    }
    lifecycle
        .manual_admitted_at()
        .map(Some)
        .ok_or(SubmissionRepositoryError::Invalid)
}
pub(crate) fn save(
    connection: &Connection,
    lifecycle: &SubmissionLifecycle,
    expected: Option<u64>,
) -> Result<(), SubmissionRepositoryError> {
    let payload = crate::lifecycle_document::encode(lifecycle)?;
    let account = &lifecycle.source().request().header().account;
    let admitted_at = lifecycle
        .admitted_at()
        .ok_or(SubmissionRepositoryError::Invalid)?
        .millis()
        .to_string();
    let manual_admitted_at = lifecycle
        .manual_admitted_at()
        .map(|at| at.millis().to_string());
    let changed = match expected {
        None => connection.execute(
            "INSERT INTO submission_lifecycles(job_id,account,admitted_at,manual_admitted_at,revision,payload) VALUES(?1,?2,?3,?4,?5,?6)",
            (integer(lifecycle.job().0)?, account, admitted_at, manual_admitted_at, integer(lifecycle.revision())?, payload),
        ),
        Some(revision) => connection.execute(
            "UPDATE submission_lifecycles SET account=?2,admitted_at=?3,manual_admitted_at=?4,revision=?5,payload=?6 WHERE job_id=?1 AND revision=?7",
            (integer(lifecycle.job().0)?, account, admitted_at, manual_admitted_at, integer(lifecycle.revision())?, payload, integer(revision)?),
        ),
    }.map_err(unavailable)?;
    if changed != 1 {
        return Err(SubmissionRepositoryError::Conflict);
    }
    Ok(())
}
