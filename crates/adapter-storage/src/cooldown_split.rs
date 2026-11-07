// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Same-database explicit cooldown split and immutable original release history.

use crate::api::SqliteReferenceCache;
use tallyline_shared_kernel::api::model::{ReportId, Timestamp};
use tallyline_submission::api::model::{JobId, SubmissionRepositoryError};
use tallyline_workflows::api::{CooldownSplit, CooldownSplitIdentities, CooldownSplitStore};

fn unavailable(_: rusqlite::Error) -> SubmissionRepositoryError {
    SubmissionRepositoryError::Unavailable
}
fn integer(value: u64) -> Result<i64, SubmissionRepositoryError> {
    value
        .try_into()
        .map_err(|_| SubmissionRepositoryError::Invalid)
}
impl CooldownSplitStore for SqliteReferenceCache {
    fn split_cooldown(
        &mut self,
        id: JobId,
        expected_revision: u64,
        now: Timestamp,
    ) -> Result<CooldownSplit, SubmissionRepositoryError> {
        let transaction = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(unavailable)?;
        let origin =
            crate::queue::load_job(&transaction, id)?.ok_or(SubmissionRepositoryError::Invalid)?;
        if origin.revision() != expected_revision {
            return Err(SubmissionRepositoryError::Conflict);
        }
        let report = crate::queue::load_report(&transaction, origin.request().header().report)?;
        let history = crate::queue::history(&transaction)?;
        transaction
            .execute("INSERT INTO reports(version,payload) VALUES(0,'')", [])
            .map_err(unavailable)?;
        let ready_report = ReportId(
            transaction
                .last_insert_rowid()
                .try_into()
                .map_err(|_| SubmissionRepositoryError::Invalid)?,
        );
        let allocate = |report: ReportId| -> Result<JobId, SubmissionRepositoryError> {
            transaction.execute("INSERT INTO submission_jobs(report_id,revision,phase,payload,release_report,release_report_version) VALUES(?1,1,'queued','','',1)", [integer(report.0)?]).map_err(unavailable)?;
            Ok(JobId(
                transaction
                    .last_insert_rowid()
                    .try_into()
                    .map_err(|_| SubmissionRepositoryError::Invalid)?,
            ))
        };
        let ready_job = allocate(ready_report)?;
        let cooling_job = allocate(report.id())?;
        let split = CooldownSplit::prepare(
            &report,
            &origin,
            &history,
            CooldownSplitIdentities {
                ready_report,
                ready_job,
                cooling_job,
            },
            now,
        )?;
        let payload = crate::queue_document::encode(split.origin())?;
        let changed = transaction.execute("UPDATE submission_jobs SET revision=?1,phase='cancelled',payload=?2 WHERE id=?3 AND revision=?4", (integer(split.origin().revision())?,payload,integer(id.0)?,integer(expected_revision)?)).map_err(unavailable)?;
        if changed != 1 {
            return Err(SubmissionRepositoryError::Conflict);
        }
        for part in [split.ready(), split.cooling()] {
            let payload = crate::queue_document::encode(part.job())?;
            let release = crate::reports::encode(part.report())
                .map_err(|_| SubmissionRepositoryError::Invalid)?;
            transaction.execute("UPDATE submission_jobs SET payload=?1,release_report=?2,release_report_version=?3 WHERE id=?4", (payload,release,integer(part.report().version())?,integer(part.job().id().0)?)).map_err(unavailable)?;
            let expected = if part.report().id() == report.id() {
                report.version()
            } else {
                0
            };
            crate::queue::save_report(&transaction, part.report(), expected)?;
        }
        transaction.execute("INSERT INTO report_recents(report_id,changed_order) VALUES(?1,(SELECT COALESCE(MAX(changed_order),0)+1 FROM report_recents))", [integer(ready_report.0)?]).map_err(unavailable)?;
        let event = split
            .cancellation()
            .event()
            .ok_or(SubmissionRepositoryError::Invalid)?;
        crate::queue::outbox(&transaction, "submission", event.id(), "cancelled_split")?;
        crate::queue::report_events(&transaction, split.events())?;
        transaction.execute("INSERT INTO submission_splits(source_job,ready_job,cooling_job,created_at) VALUES(?1,?2,?3,?4)", (integer(id.0)?,integer(ready_job.0)?,integer(cooling_job.0)?,now.millis().to_string())).map_err(unavailable)?;
        transaction.commit().map_err(unavailable)?;
        Ok(split)
    }
}
