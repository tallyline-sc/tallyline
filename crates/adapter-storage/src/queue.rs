// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Same-database Report, submission and durable-outbox transactions.

use crate::api::SqliteReferenceCache;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior};
use tallyline_reporting::api::model::{
    Report, ReportEvent, ReportEventKind, ReportId, SubmissionGate,
};
use tallyline_shared_kernel::api::model::EventId;
use tallyline_submission::api::model::{
    AccountAttempt, JobId, JobPhase, RowBudgetDecision, ScheduleDecision, SubmissionChange,
    SubmissionEventKind, SubmissionJob, SubmissionRepositoryError,
};
use tallyline_submission::api::{SubmissionBudget, SubmissionJobRepository, SubmissionSchedule};
use tallyline_workflows::api::{ManualRelease, ManualReleaseError, ManualReleaseStore};

fn invalid<T>(_: T) -> SubmissionRepositoryError {
    SubmissionRepositoryError::Invalid
}
fn unavailable(_: rusqlite::Error) -> SubmissionRepositoryError {
    SubmissionRepositoryError::Unavailable
}
fn integer(value: u64) -> Result<i64, SubmissionRepositoryError> {
    value.try_into().map_err(invalid)
}
fn unsigned(value: i64) -> Result<u64, SubmissionRepositoryError> {
    value.try_into().map_err(invalid)
}
fn release_error(error: SubmissionRepositoryError) -> ManualReleaseError {
    match error {
        SubmissionRepositoryError::Invalid => ManualReleaseError::Invalid,
        SubmissionRepositoryError::Conflict => ManualReleaseError::Conflict,
        SubmissionRepositoryError::Unavailable => ManualReleaseError::Unavailable,
    }
}
pub(crate) fn load_job(
    connection: &Connection,
    id: JobId,
) -> Result<Option<SubmissionJob>, SubmissionRepositoryError> {
    let record: Option<QueueRecord> = connection
        .query_row(
            "SELECT revision,phase,payload,report_id,release_report,release_report_version FROM submission_jobs WHERE id=?1",
            [integer(id.0)?],
            |row| Ok(QueueRecord { revision: row.get(0)?, phase: row.get(1)?, payload: row.get(2)?, report: row.get(3)?, release_report: row.get(4)?, release_version: row.get(5)? }),
        )
        .optional()
        .map_err(unavailable)?;
    record
        .map(|record| {
            let job =
                crate::queue_document::decode(id, unsigned(record.revision)?, &record.payload)?;
            if crate::queue_document::phase_name(job.phase()) != record.phase {
                return Err(SubmissionRepositoryError::Invalid);
            }
            let report = crate::reports::decode(
                ReportId(unsigned(record.report)?),
                unsigned(record.release_version)?,
                &record.release_report,
            )
            .map_err(invalid)?;
            let binding = report
                .release_binding()
                .ok_or(SubmissionRepositoryError::Invalid)?;
            if binding.job() != Some(job.id())
                || report.state() != tallyline_reporting::api::model::ReportState::Queued
            {
                return Err(SubmissionRepositoryError::Invalid);
            }
            let request = tallyline_workflows::api::manual_submission_request(
                &report,
                binding.environment(),
                binding.account(),
                binding.mode(),
            )
            .map_err(invalid)?;
            if &request != job.request() {
                return Err(SubmissionRepositoryError::Invalid);
            }
            Ok(job)
        })
        .transpose()
}
struct QueueRecord {
    revision: i64,
    phase: String,
    payload: String,
    report: i64,
    release_report: String,
    release_version: i64,
}

pub(crate) fn load_report(
    connection: &Connection,
    id: ReportId,
) -> Result<Report, SubmissionRepositoryError> {
    load_report_optional(connection, id)?.ok_or(SubmissionRepositoryError::Invalid)
}
pub(crate) fn load_report_optional(
    connection: &Connection,
    id: ReportId,
) -> Result<Option<Report>, SubmissionRepositoryError> {
    let record: Option<(i64, String, String)> = connection
        .query_row(
            "SELECT version,phase,payload FROM reports WHERE id=?1",
            [integer(id.0)?],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(unavailable)?;
    let Some((version, phase, payload)) = record else {
        return Ok(None);
    };
    let report = crate::reports::decode(id, unsigned(version)?, &payload).map_err(invalid)?;
    if crate::report_lifecycle::phase_name(report.state()) != phase {
        return Err(SubmissionRepositoryError::Invalid);
    }
    Ok(Some(report))
}
pub(crate) fn save_report(
    connection: &Connection,
    report: &Report,
    expected: u64,
) -> Result<(), SubmissionRepositoryError> {
    let payload = crate::reports::encode(report).map_err(invalid)?;
    let changed = connection
        .execute(
            "UPDATE reports SET version=?1,phase=?2,payload=?3 WHERE id=?4 AND version=?5",
            (
                integer(report.version())?,
                crate::report_lifecycle::phase_name(report.state()),
                payload,
                integer(report.id().0)?,
                integer(expected)?,
            ),
        )
        .map_err(unavailable)?;
    if changed != 1 {
        return Err(SubmissionRepositoryError::Conflict);
    }
    Ok(())
}
pub(crate) fn outbox(
    connection: &Connection,
    source: &str,
    id: EventId,
    kind: &str,
) -> Result<(), SubmissionRepositoryError> {
    let changed = connection.execute("INSERT INTO domain_outbox(event_source,aggregate_id,revision,event_kind) VALUES(?1,?2,?3,?4)", (source, integer(id.aggregate_id())?, integer(id.version())?, kind)).map_err(unavailable)?;
    if changed != 1 {
        return Err(SubmissionRepositoryError::Conflict);
    }
    Ok(())
}
pub(crate) fn report_events(
    connection: &Connection,
    events: &[ReportEvent],
) -> Result<(), SubmissionRepositoryError> {
    for event in events {
        let kind = match event.kind() {
            ReportEventKind::Released => "released",
            ReportEventKind::Submitted => "submitted",
            ReportEventKind::Discarded => "discarded",
        };
        outbox(connection, "reporting", event.id(), kind)?;
    }
    Ok(())
}
pub(crate) fn history(
    connection: &Connection,
) -> Result<Vec<SubmissionJob>, SubmissionRepositoryError> {
    let mut statement = connection
        .prepare("SELECT id FROM submission_jobs ORDER BY id")
        .map_err(unavailable)?;
    let ids = statement
        .query_map([], |row| row.get::<_, i64>(0))
        .map_err(unavailable)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(unavailable)?;
    ids.into_iter()
        .map(|id| {
            load_job(connection, JobId(unsigned(id)?))?.ok_or(SubmissionRepositoryError::Invalid)
        })
        .collect()
}
impl ManualReleaseStore for SqliteReferenceCache {
    fn release_manual(
        &mut self,
        gate: &SubmissionGate,
    ) -> Result<ManualRelease, ManualReleaseError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| ManualReleaseError::Unavailable)?;
        let current = load_report(&transaction, gate.source().id()).map_err(release_error)?;
        if &current != gate.source() {
            return Err(ManualReleaseError::Conflict);
        }
        crate::reference_consistency::check(&transaction, gate.snapshot())
            .map_err(release_error)?;
        let jobs = history(&transaction).map_err(release_error)?;
        let gate = tallyline_workflows::api::assess_release_observation_history(gate, Some(&jobs));
        transaction.execute("INSERT INTO submission_jobs(report_id,revision,phase,payload,release_report,release_report_version) VALUES(?1,1,'queued','','',1)", [integer(current.id().0).map_err(release_error)?]).map_err(|_| ManualReleaseError::Unavailable)?;
        let id = JobId(unsigned(transaction.last_insert_rowid()).map_err(release_error)?);
        let release = ManualRelease::prepare(&gate, id)?;
        let account = release
            .report()
            .release_binding()
            .ok_or(ManualReleaseError::Invalid)?
            .account();
        if crate::screenshot_policy::required_since(&transaction, account)
            .map_err(|_| ManualReleaseError::Unavailable)?
            .is_some()
            && release.report().attachment().is_none()
        {
            return Err(ManualReleaseError::Blocked(vec![
                tallyline_reporting::api::model::ReleaseBlockReason::ImageRequired,
            ]));
        }
        let payload = crate::queue_document::encode(release.job()).map_err(release_error)?;
        let report_payload =
            crate::reports::encode(release.report()).map_err(|_| ManualReleaseError::Invalid)?;
        transaction
            .execute(
                "UPDATE submission_jobs SET payload=?1,release_report=?2,release_report_version=?3 WHERE id=?4",
                (
                    payload,
                    report_payload,
                    integer(release.report().version()).map_err(release_error)?,
                    integer(id.0).map_err(release_error)?,
                ),
            )
            .map_err(|_| ManualReleaseError::Unavailable)?;
        save_report(&transaction, release.report(), current.version()).map_err(release_error)?;
        report_events(&transaction, release.events()).map_err(release_error)?;
        transaction
            .commit()
            .map_err(|_| ManualReleaseError::Unavailable)?;
        Ok(release)
    }
}
impl SubmissionJobRepository for SqliteReferenceCache {
    fn load(&mut self, id: JobId) -> Result<Option<SubmissionJob>, SubmissionRepositoryError> {
        load_job(&self.connection, id)
    }
    fn save(&mut self, change: &SubmissionChange) -> Result<(), SubmissionRepositoryError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(unavailable)?;
        save_change(&transaction, change)?;
        transaction.commit().map_err(unavailable)
    }
}

pub(crate) fn save_change(
    transaction: &Connection,
    change: &SubmissionChange,
) -> Result<(), SubmissionRepositoryError> {
    save_change_with_admission(transaction, change, None)
}
pub(crate) fn save_change_with_admission(
    transaction: &Connection,
    change: &SubmissionChange,
    admission: Option<&tallyline_submission::api::model::TokenAdmission>,
) -> Result<(), SubmissionRepositoryError> {
    let next = change.state();
    if change.event().is_some_and(|event| {
        matches!(
            event.kind(),
            SubmissionEventKind::Cancelled(
                tallyline_submission::api::model::CancellationReason::Split
            )
        )
    }) {
        return Err(SubmissionRepositoryError::Invalid);
    }
    let current = load_job(transaction, next.id())?.ok_or(SubmissionRepositoryError::Invalid)?;
    if &current == next {
        return Ok(());
    }
    if current.revision() != change.expected_revision() {
        return Err(SubmissionRepositoryError::Conflict);
    }
    if next.revision()
        != current
            .revision()
            .checked_add(1)
            .ok_or(SubmissionRepositoryError::Invalid)?
        || next.request() != current.request()
    {
        return Err(SubmissionRepositoryError::Invalid);
    }
    if next.phase() == JobPhase::Sending {
        let scope = crate::token_quota::binding(transaction, next.id(), next.attempts().len())?;
        let fallback = tallyline_submission::api::model::TokenAdmission::from_scope(scope);
        if !crate::authenticated_quota::allows(transaction, admission.unwrap_or(&fallback))? {
            return Err(SubmissionRepositoryError::Conflict);
        }
        let at = next
            .attempts()
            .last()
            .ok_or(SubmissionRepositoryError::Invalid)?
            .started_at();
        let previous = history(transaction)?;
        if SubmissionSchedule::assess(&current, &previous, at) != ScheduleDecision::Ready {
            return Err(SubmissionRepositoryError::Conflict);
        }
        let mut attempts = Vec::new();
        for job in &previous {
            for attempt in job.attempts() {
                attempts.push(
                    AccountAttempt::new(
                        job.request().header().account.clone(),
                        job.request().header().mode,
                        attempt.started_at(),
                        attempt.rows(),
                        attempt.outcome(),
                    )
                    .map_err(invalid)?,
                );
            }
        }
        if SubmissionBudget::assess(current.request(), &attempts, at) != RowBudgetDecision::Ready {
            return Err(SubmissionRepositoryError::Conflict);
        }
    }
    let payload = crate::queue_document::encode(next)?;
    let changed = transaction.execute("UPDATE submission_jobs SET revision=?1,phase=?2,payload=?3 WHERE id=?4 AND revision=?5", (integer(next.revision())?, crate::queue_document::phase_name(next.phase()), payload, integer(next.id().0)?, integer(current.revision())?)).map_err(unavailable)?;
    if changed != 1 {
        return Err(SubmissionRepositoryError::Conflict);
    }
    if matches!(
        next.result(),
        Some(
            tallyline_submission::api::model::SubmissionResult::Returned(
                tallyline_submission::api::model::SubmissionReturnReason::ScreenshotRequired
            )
        )
    ) {
        let at = next
            .attempts()
            .last()
            .and_then(|attempt| attempt.ended_at())
            .ok_or(SubmissionRepositoryError::Invalid)?;
        crate::screenshot_policy::remember_requirement(
            transaction,
            &next.request().header().account,
            at,
        )
        .map_err(|_| SubmissionRepositoryError::Unavailable)?;
    }
    crate::submission_service_fences::commit(transaction, &current, next)?;
    let accepted = tallyline_workflows::api::accepted_submission_priors(next).map_err(invalid)?;
    crate::accepted_priors::accepted(transaction, &accepted)
        .map_err(|_| SubmissionRepositoryError::Unavailable)?;
    if let Some(event) = change.event() {
        let kind = match event.kind() {
            SubmissionEventKind::Succeeded(_) => "succeeded",
            SubmissionEventKind::PartiallyAccepted(_) => "partially_accepted",
            SubmissionEventKind::OutcomeUnknown => "outcome_unknown",
            SubmissionEventKind::Rejected => "rejected",
            SubmissionEventKind::Deferred(_) => "deferred",
            SubmissionEventKind::Returned(_) => "returned",
            SubmissionEventKind::Cancelled(_) => "cancelled",
        };
        outbox(transaction, "submission", event.id(), kind)?;
        let report = load_report(transaction, event.report())?;
        if let Some(change) =
            tallyline_workflows::api::apply_submission_delivery(&report, next).map_err(invalid)?
        {
            save_report(transaction, change.state(), report.version())?;
            report_events(transaction, change.events())?;
        }
    }

    Ok(())
}

impl tallyline_workflows::api::SubmissionQueueStore for SqliteReferenceCache {
    fn queue_jobs(&mut self) -> Result<Vec<SubmissionJob>, SubmissionRepositoryError> {
        history(&self.connection)
    }
    fn released_report(&mut self, id: JobId) -> Result<Option<Report>, SubmissionRepositoryError> {
        let Some(job) = load_job(&self.connection, id)? else {
            return Ok(None);
        };
        let (report, version, payload): (i64, i64, String) = self.connection.query_row("SELECT report_id,release_report_version,release_report FROM submission_jobs WHERE id=?1", [integer(id.0)?], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))).map_err(unavailable)?;
        let report =
            crate::reports::decode(ReportId(unsigned(report)?), unsigned(version)?, &payload)
                .map_err(invalid)?;
        if report.id() != job.request().header().report
            || report.version() != job.request().header().report_version
        {
            return Err(SubmissionRepositoryError::Invalid);
        }
        Ok(Some(report))
    }
    fn recover_unfinished(
        &mut self,
        now: tallyline_shared_kernel::api::model::Timestamp,
    ) -> Result<Vec<SubmissionJob>, SubmissionRepositoryError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(unavailable)?;
        let mut recovered = Vec::new();
        for job in history(&transaction)? {
            if !matches!(
                job.phase(),
                JobPhase::Queued | JobPhase::Held | JobPhase::Paused | JobPhase::Sending
            ) {
                continue;
            }
            let change = job
                .apply(tallyline_submission::api::model::SubmissionCommand::Recover { at: now })
                .map_err(invalid)?;
            save_change(&transaction, &change)?;
            recovered.push(change.into_state());
        }
        transaction.commit().map_err(unavailable)?;
        Ok(recovered)
    }
}
