// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Fixed typed lifecycle cadence independent of submission cooldowns.

use std::time::Duration;
use tallyline_shared_kernel::api::model::Timestamp;

/// Documented automatic lifecycle intervals relative to the original sending time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SubmissionLifecycleSettings {
    first_hour: Duration,
    first_interval: Duration,
    later_interval: Duration,
    manual_interval: Duration,
    maximum_age: Duration,
    max_batch_jobs: usize,
    max_batch_targets: usize,
}
impl Default for SubmissionLifecycleSettings {
    fn default() -> Self {
        Self {
            first_hour: Duration::from_secs(3600),
            first_interval: Duration::from_secs(300),
            later_interval: Duration::from_secs(1800),
            manual_interval: Duration::from_secs(60),
            maximum_age: Duration::from_secs(1_209_600),
            max_batch_jobs: 100,
            max_batch_targets: 500,
        }
    }
}
impl SubmissionLifecycleSettings {
    /// Returns the pending-work bound on independently versioned source jobs.
    #[must_use]
    pub const fn max_batch_jobs(self) -> usize {
        self.max_batch_jobs
    }
    /// Returns the active-target bound that accommodates one maximum-sized submission.
    #[must_use]
    pub const fn max_batch_targets(self) -> usize {
        self.max_batch_targets
    }
    /// Reports whether a nonempty set fits both pending-work bounds.
    #[must_use]
    pub const fn admits_batch(self, jobs: usize, targets: usize) -> bool {
        jobs > 0 && jobs <= self.max_batch_jobs && targets > 0 && targets <= self.max_batch_targets
    }
    /// Reports whether the original send remains inside the lifecycle monitoring period.
    #[must_use]
    pub fn within_age(self, sent_at: Timestamp, at: Timestamp) -> bool {
        at.millis()
            .checked_sub(sent_at.millis())
            .is_some_and(|age| age < self.maximum_age.as_millis())
    }
    /// Returns the next manual admission time or the supplied first eligibility.
    /// Timestamp overflow supplies no deadline.
    #[must_use]
    pub fn manual_due_at(
        self,
        first_eligible: Timestamp,
        admitted_at: Option<Timestamp>,
    ) -> Option<Timestamp> {
        match admitted_at {
            Some(previous) => previous.checked_add(self.manual_interval),
            None => Some(first_eligible),
        }
    }
    pub(crate) fn automatic_due_at(
        self,
        sent_at: Timestamp,
        admitted_at: Option<Timestamp>,
    ) -> Option<Timestamp> {
        let Some(previous) = admitted_at else {
            return sent_at.checked_add(self.first_interval);
        };
        let age = previous.millis().checked_sub(sent_at.millis())?;
        let interval = if age < self.first_hour.as_millis() {
            self.first_interval
        } else {
            self.later_interval
        };
        previous.checked_add(interval)
    }
}
