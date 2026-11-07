// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Committed queue projection and passive waits with independent context recheck deadlines.

use super::SubmissionControlError;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};
use tallyline_shared_kernel::api::Clock;
use tallyline_shared_kernel::api::model::Timestamp;
use tallyline_submission::api::model::{
    AccountAttempt, AccountState, JobId, JobPhase, RowBudgetDecision, ScheduleDecision,
    SendHoldReason, SubmissionJob, SubmissionRepositoryError, SubmissionWait, SubmissionWaitReason,
};
use tallyline_submission::api::{CredentialApplication, SubmissionBudget, SubmissionSchedule};

const CONTEXT_RECHECK: Duration = Duration::from_secs(1);

pub(super) struct QueueState {
    jobs: Vec<SubmissionJob>,
    waiting: Vec<SubmissionWait>,
    deferred: BTreeMap<JobId, (Instant, SubmissionWait)>,
}
impl QueueState {
    pub(super) fn new(jobs: Vec<SubmissionJob>) -> Self {
        Self {
            jobs,
            waiting: Vec::new(),
            deferred: BTreeMap::new(),
        }
    }
    pub(super) fn jobs(&self) -> &[SubmissionJob] {
        &self.jobs
    }

    pub(super) fn update(&mut self, job: SubmissionJob) {
        if job.phase() != JobPhase::Queued {
            self.deferred.remove(&job.id());
        }
        if let Some(current) = self
            .jobs
            .iter_mut()
            .find(|current| current.id() == job.id())
        {
            *current = job;
        } else {
            self.jobs.push(job);
        }
    }

    /// Borrows passive waits for exact committed job revisions without granting send authority.
    #[must_use]
    pub(super) fn waiting(&self) -> &[SubmissionWait] {
        &self.waiting
    }

    pub(super) fn account_attempts(&self) -> Result<Vec<AccountAttempt>, SubmissionControlError> {
        self.jobs
            .iter()
            .flat_map(|job| job.attempts().iter().map(move |attempt| (job, attempt)))
            .map(|(job, attempt)| {
                AccountAttempt::new(
                    job.request().header().account.clone(),
                    job.request().header().mode,
                    attempt.started_at(),
                    attempt.rows(),
                    attempt.outcome(),
                )
                .map_err(|_| SubmissionControlError::Repository(SubmissionRepositoryError::Invalid))
            })
            .collect()
    }

    pub(super) fn defer_context(
        &mut self,
        job: &SubmissionJob,
        reason: SendHoldReason,
        now: Timestamp,
    ) {
        let wait = SubmissionWait {
            job: job.id(),
            revision: job.revision(),
            reason: SubmissionWaitReason::Context(reason),
            next_check: now.checked_add(CONTEXT_RECHECK),
        };
        self.deferred
            .insert(job.id(), (Instant::now() + CONTEXT_RECHECK, wait));
    }

    pub(super) fn context_deferred(&self, id: JobId) -> bool {
        self.deferred
            .get(&id)
            .is_some_and(|(until, _)| *until > Instant::now())
    }

    pub(super) fn refresh_waiting(
        &mut self,
        credentials: &CredentialApplication,
        clock: &dyn Clock,
    ) -> Result<(), SubmissionControlError> {
        let attempts = self.account_attempts()?;
        let now = clock.now();
        let mut waiting = Vec::new();
        for job in self
            .jobs
            .iter()
            .filter(|job| job.phase() == JobPhase::Queued)
        {
            let reason = if credentials
                .credentials_for(&job.request().header().account)
                .is_none()
            {
                Some(match credentials.state() {
                    AccountState::Validated { username, .. }
                        if username != &job.request().header().account =>
                    {
                        SubmissionWaitReason::AccountChanged
                    }
                    AccountState::Unconfigured
                    | AccountState::Rejected(_)
                    | AccountState::Validated { .. } => SubmissionWaitReason::AccountUnavailable,
                })
            } else {
                schedule_wait(job, &self.jobs, &attempts, now)
            };
            if let Some(reason) = reason {
                waiting.push(SubmissionWait {
                    job: job.id(),
                    revision: job.revision(),
                    reason,
                    next_check: None,
                });
            } else if let Some((_, wait)) = self.deferred.get(&job.id()) {
                waiting.push(*wait);
            }
        }
        self.waiting = waiting;
        Ok(())
    }
}
fn schedule_wait(
    job: &SubmissionJob,
    history: &[SubmissionJob],
    attempts: &[AccountAttempt],
    now: Timestamp,
) -> Option<SubmissionWaitReason> {
    match SubmissionSchedule::assess(job, history, now) {
        ScheduleDecision::Ready => (SubmissionBudget::assess(job.request(), attempts, now)
            == RowBudgetDecision::Held)
            .then_some(SubmissionWaitReason::RowBudget),
        ScheduleDecision::InFlight => Some(SubmissionWaitReason::InFlight),
        ScheduleDecision::WaitingUntil(until) => Some(SubmissionWaitReason::CooldownUntil(until)),
        ScheduleDecision::EarlierObservationPending => {
            Some(SubmissionWaitReason::EarlierObservationPending)
        }
        ScheduleDecision::NewerObservationSent => {
            Some(SubmissionWaitReason::NewerObservationReview)
        }
        ScheduleDecision::UnresolvedNewerObservation => {
            Some(SubmissionWaitReason::UnresolvedNewerObservation)
        }
        ScheduleDecision::Unavailable => Some(SubmissionWaitReason::Unavailable),
    }
}
