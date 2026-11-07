// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact-attempt token bindings and atomic due quota expiry.

use crate::api::SqliteReferenceCache;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior};
use tallyline_shared_kernel::api::model::Timestamp;
use tallyline_submission::api::SubmissionQuotaRepository;
use tallyline_submission::api::model::{
    JobId, JobPhase, SubmissionCommand, SubmissionJob, SubmissionRepositoryError,
    SubmissionServiceCause, TokenScope,
};

fn unavailable(_: rusqlite::Error) -> SubmissionRepositoryError {
    SubmissionRepositoryError::Unavailable
}
fn integer<T: TryInto<i64>>(value: T) -> Result<i64, SubmissionRepositoryError> {
    value
        .try_into()
        .map_err(|_| SubmissionRepositoryError::Invalid)
}
pub(crate) fn scope(
    bytes: Option<Vec<u8>>,
) -> Result<Option<TokenScope>, SubmissionRepositoryError> {
    bytes
        .map(|bytes| {
            TokenScope::new(
                bytes
                    .try_into()
                    .map_err(|_| SubmissionRepositoryError::Invalid)?,
            )
            .ok_or(SubmissionRepositoryError::Invalid)
        })
        .transpose()
}
pub(crate) fn bind(
    connection: &Connection,
    job: &SubmissionJob,
    scope: Option<TokenScope>,
) -> Result<(), SubmissionRepositoryError> {
    connection
        .execute(
            "INSERT INTO submission_token_bindings(job_id,attempt,token_scope) VALUES(?1,?2,?3)",
            (
                integer(job.id().0)?,
                integer(
                    job.attempts()
                        .len()
                        .checked_add(1)
                        .ok_or(SubmissionRepositoryError::Invalid)?,
                )?,
                scope.as_ref().map(|scope| scope.bytes().as_slice()),
            ),
        )
        .map_err(unavailable)?;
    Ok(())
}
pub(crate) fn binding(
    connection: &Connection,
    job: JobId,
    attempt: usize,
) -> Result<Option<TokenScope>, SubmissionRepositoryError> {
    let bytes: Option<Option<Vec<u8>>> = connection
        .query_row(
            "SELECT token_scope FROM submission_token_bindings WHERE job_id=?1 AND attempt=?2",
            (integer(job.0)?, integer(attempt)?),
            |row| row.get(0),
        )
        .optional()
        .map_err(unavailable)?;
    scope(bytes.flatten())
}
impl SubmissionQuotaRepository for SqliteReferenceCache {
    fn record_authenticated_quota(
        &mut self,
        observation: tallyline_submission::api::model::TokenQuotaObservation,
    ) -> Result<(), SubmissionRepositoryError> {
        crate::authenticated_quota::record(&self.connection, observation)
    }
    fn authenticated_quotas(
        &mut self,
    ) -> Result<
        Vec<tallyline_submission::api::model::TokenQuotaObservation>,
        SubmissionRepositoryError,
    > {
        crate::authenticated_quota::load(&self.connection)
    }
    fn quota_allows(
        &mut self,
        scope: Option<TokenScope>,
    ) -> Result<bool, SubmissionRepositoryError> {
        crate::authenticated_quota::allows(
            &self.connection,
            &tallyline_submission::api::model::TokenAdmission::from_scope(scope),
        )
    }

    fn quota_allows_admission(
        &mut self,
        proof: &tallyline_submission::api::model::TokenAdmission,
    ) -> Result<bool, SubmissionRepositoryError> {
        crate::authenticated_quota::allows(&self.connection, proof)
    }

    fn expire_quotas(
        &mut self,
        now: Timestamp,
    ) -> Result<Vec<SubmissionJob>, SubmissionRepositoryError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(unavailable)?;
        let fences = crate::submission_service_fences::load(&transaction)?;
        let mut resumed = Vec::new();
        for fence in fences {
            if fence.cause() != SubmissionServiceCause::DailyQuota
                || fence.quota_reset().is_none_or(|reset| reset > now)
            {
                continue;
            }
            let reset = fence
                .quota_reset()
                .ok_or(SubmissionRepositoryError::Invalid)?;
            let job = crate::queue::load_job(&transaction, fence.source())?
                .ok_or(SubmissionRepositoryError::Invalid)?;
            let changed=transaction.execute("DELETE FROM submission_service_fences WHERE cause='daily_quota' AND source_job=?1 AND source_revision=?2",(integer(fence.source().0)?,integer(fence.source_revision())?)).map_err(unavailable)?;
            if changed != 1 {
                return Err(SubmissionRepositoryError::Conflict);
            }
            transaction.execute("INSERT INTO submission_quota_expiries(source_job,source_revision,expired_at,token_scope,reset_at) VALUES(?1,?2,?3,?4,?5)",(integer(fence.source().0)?,integer(fence.source_revision())?,now.millis().to_string(),fence.token_scope().as_ref().map(|scope|scope.bytes().as_slice()),reset.millis().to_string())).map_err(unavailable)?;
            if job.phase() == JobPhase::Held
                && tallyline_submission::api::model::SubmissionRetrySettings::default()
                    .quota_retry_at(&job)
                    .is_some()
            {
                let change = job
                    .apply(SubmissionCommand::ResumeQuota { at: now })
                    .map_err(|_| SubmissionRepositoryError::Invalid)?;
                crate::queue::save_change(&transaction, &change)?;
                resumed.push(change.into_state());
            }
        }
        crate::authenticated_quota::expire(&transaction, now)?;
        transaction.commit().map_err(unavailable)?;
        Ok(resumed)
    }
}
