// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Transactional service fences independent of Report/job cancellation and return.

use crate::api::SqliteReferenceCache;
use rusqlite::Connection;
use tallyline_submission::api::SubmissionServiceFenceRepository;
use tallyline_submission::api::model::{
    JobId, JobPhase, NotProcessed, SubmissionHoldReason, SubmissionJob, SubmissionRepositoryError,
    SubmissionResult, SubmissionServiceCause, SubmissionServiceFence,
};

fn invalid<T>(_: T) -> SubmissionRepositoryError {
    SubmissionRepositoryError::Invalid
}
fn unavailable(_: rusqlite::Error) -> SubmissionRepositoryError {
    SubmissionRepositoryError::Unavailable
}
fn integer(value: u64) -> Result<i64, SubmissionRepositoryError> {
    i64::try_from(value).map_err(invalid)
}

pub(crate) fn load(
    connection: &Connection,
) -> Result<Vec<SubmissionServiceFence>, SubmissionRepositoryError> {
    let mut statement = connection
        .prepare(
            "SELECT f.cause,f.source_job,f.source_revision,j.revision,j.payload,f.token_scope,f.reset_at FROM submission_service_fences f LEFT JOIN submission_jobs j ON j.id=f.source_job ORDER BY f.cause,f.source_job",
        )
        .map_err(unavailable)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, Option<i64>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<Vec<u8>>>(5)?,
                row.get::<_, Option<String>>(6)?,
            ))
        })
        .map_err(unavailable)?;
    rows.map(|row| {
        let (cause, source, revision, current_revision, payload, scope, reset) =
            row.map_err(unavailable)?;
        let cause = match cause.as_str() {
            "api_changed" => SubmissionServiceCause::ApiChanged,
            "daily_quota" => SubmissionServiceCause::DailyQuota,
            "account_pause" => SubmissionServiceCause::AccountPause,
            _ => return Err(SubmissionRepositoryError::Invalid),
        };
        let fence = SubmissionServiceFence::new(
            cause,
            JobId(u64::try_from(source).map_err(invalid)?),
            u64::try_from(revision).map_err(invalid)?,
        )
        .ok_or(SubmissionRepositoryError::Invalid)?;
        let source = crate::queue_document::decode(
            fence.source(),
            u64::try_from(current_revision.ok_or(SubmissionRepositoryError::Invalid)?)
                .map_err(invalid)?,
            &payload.ok_or(SubmissionRepositoryError::Invalid)?,
        )?;
        if !fence.matches_source(&source) {
            return Err(SubmissionRepositoryError::Invalid);
        }
        let scope = crate::token_quota::scope(scope)?;
        let reset = reset
            .map(|at| crate::reference_cache::timestamp(&at).map_err(invalid))
            .transpose()?;
        if cause == SubmissionServiceCause::DailyQuota {
            if scope
                != crate::token_quota::binding(connection, source.id(), source.attempts().len())?
                || reset.is_some_and(|reset| {
                    source
                        .result()
                        .and_then(SubmissionResult::daily_quota_reset)
                        != Some(reset)
                        || source
                            .result()
                            .and_then(SubmissionResult::daily_quota_observed_at)
                            .is_none_or(|at| reset <= at)
                })
            {
                return Err(SubmissionRepositoryError::Invalid);
            }
            return fence
                .with_quota_metadata(scope, reset)
                .ok_or(SubmissionRepositoryError::Invalid);
        }
        if scope.is_some() || reset.is_some() {
            return Err(SubmissionRepositoryError::Invalid);
        }
        Ok(fence)
    })
    .collect()
}
pub(crate) fn commit(
    connection: &Connection,
    current: &SubmissionJob,
    next: &SubmissionJob,
) -> Result<(), SubmissionRepositoryError> {
    if next
        .result()
        .and_then(SubmissionResult::account_pause)
        .is_some()
        && current.result() != next.result()
        || current.phase() == JobPhase::Sending
        || matches!(
            current.result().map(SubmissionResult::final_result),
            Some(SubmissionResult::NeedsOutcomeCheck { .. })
        )
    {
        let cause = if next
            .result()
            .and_then(SubmissionResult::account_pause)
            .is_some()
        {
            Some("account_pause")
        } else {
            match next.result().map(SubmissionResult::final_result) {
                Some(SubmissionResult::Held(SubmissionHoldReason::ApiChanged)) => {
                    Some("api_changed")
                }
                Some(SubmissionResult::NotProcessed(
                    NotProcessed::DailyQuota | NotProcessed::DailyQuotaUntil(_),
                )) => Some("daily_quota"),
                _ => None,
            }
        };
        if let Some(cause) = cause.filter(|_| !next.service_pause_acknowledged()) {
            connection.execute("INSERT INTO submission_service_fences(cause,source_job,source_revision) VALUES(?1,?2,?3) ON CONFLICT(cause,source_job) DO UPDATE SET source_job=excluded.source_job,source_revision=excluded.source_revision",(cause,integer(next.id().0)?,integer(if cause=="account_pause" {next.attempts().last().and_then(|attempt|attempt.account_evidence()).map_or(Ok(next.revision()),|evidence|evidence.origin_revision().ok_or(SubmissionRepositoryError::Invalid))?} else {next.revision()})?)).map_err(unavailable)?;
        }
        if cause == Some("daily_quota") {
            let scope = crate::token_quota::binding(connection, next.id(), next.attempts().len())?;
            let observed = next
                .result()
                .and_then(SubmissionResult::daily_quota_observed_at);
            let reset = next
                .result()
                .and_then(SubmissionResult::daily_quota_reset)
                .filter(|reset| observed.is_some_and(|at| *reset > at));
            connection.execute("UPDATE submission_service_fences SET token_scope=?1,reset_at=?2 WHERE cause='daily_quota' AND source_job=?3 AND source_revision=?4",(scope.as_ref().map(|scope|scope.bytes().as_slice()),reset.map(|reset|reset.millis().to_string()),integer(next.id().0)?,integer(next.revision())?)).map_err(unavailable)?;
        }
    }
    let explicit_ack = next.service_pause_acknowledged() && !current.service_pause_acknowledged();
    let cause = if current
        .result()
        .and_then(SubmissionResult::account_pause)
        .is_some()
    {
        Some("account_pause")
    } else {
        match current.result().map(SubmissionResult::final_result) {
            Some(SubmissionResult::Held(SubmissionHoldReason::ApiChanged)) => Some("api_changed"),
            _ => None,
        }
    };
    let requeued = matches!(current.phase(), JobPhase::Held | JobPhase::Paused)
        && next.phase() == JobPhase::Queued;
    if let Some(cause) = cause.filter(|_| explicit_ack || requeued) {
        let source_revision = if let Some(evidence) = current
            .attempts()
            .last()
            .and_then(|attempt| attempt.account_evidence())
        {
            evidence
                .origin_revision()
                .ok_or(SubmissionRepositoryError::Invalid)?
        } else if explicit_ack
            && matches!(current.phase(), JobPhase::Cancelled | JobPhase::Returned)
        {
            current
                .revision()
                .checked_sub(1)
                .ok_or(SubmissionRepositoryError::Invalid)?
        } else {
            current.revision()
        };
        let changed = connection.execute("DELETE FROM submission_service_fences WHERE cause=?1 AND source_job=?2 AND source_revision=?3",(cause,integer(current.id().0)?,integer(source_revision)?)).map_err(unavailable)?;
        if changed != 1 {
            return Err(SubmissionRepositoryError::Conflict);
        }
    }
    Ok(())
}
impl SubmissionServiceFenceRepository for SqliteReferenceCache {
    fn service_fences(&mut self) -> Result<Vec<SubmissionServiceFence>, SubmissionRepositoryError> {
        load(&self.connection)
    }
}
