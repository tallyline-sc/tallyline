// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Bounded retry policy for requests proven not to have been processed.

use crate::api::model::{JobPhase, SubmissionJob, SubmissionResult};
use std::time::Duration;
use tallyline_shared_kernel::api::model::Timestamp;

/// Release-calibrated retry limits and exponential backoff.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SubmissionRetrySettings {
    maximum_attempts: usize,
    send_timeout: Duration,
    initial_backoff: Duration,
    maximum_backoff: Duration,
}
impl Default for SubmissionRetrySettings {
    fn default() -> Self {
        Self {
            send_timeout: Duration::from_secs(90),
            maximum_attempts: 3,
            initial_backoff: Duration::from_secs(5),
            maximum_backoff: Duration::from_secs(60),
        }
    }
}
impl SubmissionRetrySettings {
    /// Returns the maximum automatic attempts for one released job.
    #[must_use]
    pub const fn maximum_attempts(&self) -> usize {
        self.maximum_attempts
    }
    /// Returns the bounded timeout for one already reserved HTTP submission.
    #[must_use]
    pub const fn send_timeout(&self) -> Duration {
        self.send_timeout
    }

    /// Returns a usable token reset only within the bounded automatic attempt limit.
    #[must_use]
    pub fn quota_retry_at(&self, job: &SubmissionJob) -> Option<Timestamp> {
        if job.phase() != JobPhase::Held
            || job.attempts().len() >= self.maximum_attempts
            || job.result()?.account_pause().is_some()
        {
            return None;
        }
        let reset = job.result()?.daily_quota_reset()?;
        (reset > job.result()?.daily_quota_observed_at()?).then_some(reset)
    }

    /// Returns the next automatic attempt time only for bounded proven unprocessed work.
    #[must_use]
    pub fn automatic_retry_at(&self, job: &SubmissionJob) -> Option<Timestamp> {
        if matches!(
            job.result(),
            Some(SubmissionResult::Held(
                crate::api::model::SubmissionHoldReason::Cooldown
            ))
        ) {
            return job
                .attempts()
                .last()?
                .ended_at()?
                .checked_add(Duration::from_secs(5 * 60));
        }
        if matches!(
            job.result(),
            Some(SubmissionResult::Held(
                crate::api::model::SubmissionHoldReason::AcceptanceClosed
            ))
        ) {
            return job
                .attempts()
                .last()?
                .ended_at()?
                .checked_add(Duration::from_secs(15 * 60));
        }
        if job.phase() != JobPhase::Held
            || job.attempts().len() >= self.maximum_attempts
            || !matches!(
                job.result(),
                Some(SubmissionResult::NotProcessed(_) | SubmissionResult::RateLimitedCode(_))
            )
            || matches!(
                job.result(),
                Some(SubmissionResult::NotProcessed(
                    crate::api::model::NotProcessed::DailyQuota
                        | crate::api::model::NotProcessed::DailyQuotaUntil(_)
                ))
            )
        {
            return None;
        }
        let mut backoff = self.initial_backoff;
        for _ in 1..job.attempts().len() {
            backoff = backoff.saturating_mul(2).min(self.maximum_backoff);
        }
        job.attempts().last()?.ended_at()?.checked_add(backoff)
    }
}
