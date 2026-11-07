// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Durable-first account and receipt GET recovery with local-only save retries.

use super::SubmissionControlError;
use super::queue_state::QueueState;
use crate::api::HttpClient;
use std::collections::BTreeMap;
use std::time::Duration;
use tallyline_adapter_storage::api::SqliteReferenceCache;
use tallyline_shared_kernel::api::Clock;
use tallyline_shared_kernel::api::model::Timestamp;
use tallyline_submission::api::model::{
    JobId, ReleaseToken, SubmissionAccountCheck, SubmissionCommand, SubmissionJob,
    SubmissionPauseReason, SubmissionResult,
};
use tallyline_submission::api::{CredentialApplication, SubmissionJobRepository};
use tokio::sync::oneshot;

const RECHECK_INTERVAL: Duration = Duration::from_secs(15 * 60);
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ReadKind {
    Account,
    Receipt,
}
struct CheckWork {
    job: SubmissionJob,
    kind: ReadKind,
    pending: oneshot::Receiver<Option<SubmissionResult>>,
    result: Option<SubmissionResult>,
}
#[derive(Default)]
pub(super) struct RefusalRecovery {
    work: Option<CheckWork>,
    retry_at: BTreeMap<(JobId, ReadKind), Timestamp>,
}
fn kind(job: &SubmissionJob) -> Option<ReadKind> {
    let receipt = matches!(
        job.result().map(SubmissionResult::final_result),
        Some(SubmissionResult::NeedsOutcomeCheck { .. })
    );
    match job
        .result()
        .and_then(SubmissionResult::account_evidence)
        .map(|evidence| evidence.check())
    {
        Some(SubmissionAccountCheck::Pending) => Some(ReadKind::Account),
        Some(SubmissionAccountCheck::Completed(SubmissionPauseReason::KeyCheckUnavailable))
            if !receipt =>
        {
            Some(ReadKind::Account)
        }
        Some(SubmissionAccountCheck::NotRequired | SubmissionAccountCheck::Completed(_)) | None => {
            receipt.then_some(ReadKind::Receipt)
        }
    }
}
impl RefusalRecovery {
    pub(super) fn advance(
        &mut self,
        store: &mut SqliteReferenceCache,
        client: &HttpClient,
        queue: &mut QueueState,
        clock: &dyn Clock,
        credentials: &CredentialApplication,
        token: Option<&ReleaseToken>,
    ) -> Result<bool, SubmissionControlError> {
        if let Some(mut work) = self.work.take() {
            if work.result.is_none() {
                match work.pending.try_recv() {
                    Ok(Some(result)) => work.result = Some(result),
                    Ok(None) | Err(oneshot::error::TryRecvError::Closed) => {
                        if let Some(at) = clock.now().checked_add(RECHECK_INTERVAL) {
                            self.retry_at.insert((work.job.id(), work.kind), at);
                        }
                        return Ok(false);
                    }
                    Err(oneshot::error::TryRecvError::Empty) => {
                        self.work = Some(work);
                        return Ok(false);
                    }
                }
            }
            let current = match SubmissionJobRepository::load(store, work.job.id()) {
                Ok(Some(current)) => current,
                Ok(None) => return Err(SubmissionControlError::Invalid),
                Err(error) => {
                    self.work = Some(work);
                    return Err(SubmissionControlError::Repository(error));
                }
            };
            if current.revision() != work.job.revision() {
                if current.request() != work.job.request()
                    || current.attempts() != work.job.attempts()
                    || current.result() != work.job.result()
                    || current.phase() != work.job.phase()
                    || work.job.service_pause_acknowledged()
                    || !current.service_pause_acknowledged()
                    || work.job.revision().checked_add(1) != Some(current.revision())
                {
                    return Err(SubmissionControlError::Conflict);
                }
                work.job = current;
            }
            let result = work
                .result
                .as_ref()
                .ok_or(SubmissionControlError::Invalid)?;
            let command = match work.kind {
                ReadKind::Receipt => SubmissionCommand::ResolveOutcomeCheck {
                    result: result.clone(),
                },
                ReadKind::Account => {
                    let original = work
                        .job
                        .result()
                        .and_then(SubmissionResult::account_evidence)
                        .ok_or(SubmissionControlError::Invalid)?;
                    let checked = result
                        .account_evidence()
                        .ok_or(SubmissionControlError::Invalid)?;
                    if checked.code() != original.code()
                        || checked.http_status() != original.http_status()
                        || checked.original_action() != original.original_action()
                    {
                        return Err(SubmissionControlError::Conflict);
                    }
                    let SubmissionAccountCheck::Completed(reason) = checked.check() else {
                        return Err(SubmissionControlError::Invalid);
                    };
                    SubmissionCommand::ResolveAccountCheck(reason)
                }
            };
            let change = work
                .job
                .apply(command)
                .map_err(SubmissionControlError::Refused)?;
            if let Err(error) = SubmissionJobRepository::save(store, &change) {
                self.work = Some(work);
                return Err(SubmissionControlError::Repository(error));
            }
            if let Some(at) = clock.now().checked_add(RECHECK_INTERVAL) {
                self.retry_at.insert((work.job.id(), work.kind), at);
            }
            let committed = change.into_state();
            super::failure_diagnostics::record(&committed);
            queue.update(committed);
            return Ok(true);
        }
        for job in queue.jobs() {
            let Some(kind) = kind(job) else {
                continue;
            };
            let initial = job
                .result()
                .and_then(SubmissionResult::account_evidence)
                .is_some_and(|evidence| {
                    matches!(evidence.check(), SubmissionAccountCheck::Pending)
                        || kind == ReadKind::Receipt
                });
            let at = self.retry_at.get(&(job.id(), kind)).copied().or_else(|| {
                job.attempts().last()?.ended_at()?.checked_add(if initial {
                    Duration::ZERO
                } else {
                    RECHECK_INTERVAL
                })
            });
            if at.is_none_or(|at| at > clock.now()) {
                continue;
            }
            let Some(values) = credentials.credentials_for(&job.request().header().account) else {
                continue;
            };
            let pending = client
                .begin_refusal_recheck(job, values, token.map(ReleaseToken::for_validation))
                .map_err(|_| SubmissionControlError::Unavailable)?;
            self.work = Some(CheckWork {
                job: job.clone(),
                kind,
                pending,
                result: None,
            });
            break;
        }
        Ok(false)
    }
}
