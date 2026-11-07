// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Explicit cooldown partition after the ordinary fresh submission checks.

use super::{DispatchPreparation, SubmissionControlError, SubmissionQueueController};
use tallyline_submission::api::model::{JobId, ReleaseToken};
use tallyline_submission::api::{CredentialApplication, SubmissionJobRepository};
use tallyline_workflows::api::{CooldownSplit, CooldownSplitStore};

impl SubmissionQueueController {
    /// Splits the exact displayed mixed-cooldown release after all fresh non-scheduling checks.
    ///
    /// # Errors
    /// Refuses stale revisions, independent account/reference/quota/age holds and failed atomic saves.
    /// Both committed replacement jobs re-enter every ordinary send-time guard.
    pub fn split_cooldown(
        &mut self,
        id: JobId,
        expected: u64,
        credentials: &CredentialApplication,
        token: Option<&ReleaseToken>,
    ) -> Result<CooldownSplit, SubmissionControlError> {
        let job = SubmissionJobRepository::load(&mut self.store, id)
            .map_err(SubmissionControlError::Repository)?
            .ok_or(SubmissionControlError::Invalid)?;
        if job.revision() != expected {
            return Err(SubmissionControlError::Conflict);
        }
        let mut preparation = DispatchPreparation::new(
            &mut self.store,
            &self.client,
            &mut *self.preferences,
            &mut self.copies,
            &*self.clock,
            &mut self.queue,
        );
        let prepared = preparation
            .prepare_reserved(id, credentials, token)?
            .ok_or(SubmissionControlError::Conflict)?;
        if prepared.plan().source() != &job {
            return Err(SubmissionControlError::Conflict);
        }
        let history = tallyline_workflows::api::SubmissionQueueStore::queue_jobs(&mut self.store)
            .map_err(SubmissionControlError::Repository)?;
        prepared
            .plan()
            .recheck(self.clock.now(), &history)
            .map_err(SubmissionControlError::Dispatch)?;
        let split = self
            .store
            .split_cooldown(id, expected, self.clock.now())
            .map_err(SubmissionControlError::Repository)?;
        self.dispatch.cancel_reservation(id);
        for job in [split.origin(), split.ready().job(), split.cooling().job()] {
            self.queue.update(job.clone());
        }
        Ok(split)
    }
}
