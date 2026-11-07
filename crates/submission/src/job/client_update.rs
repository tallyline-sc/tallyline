// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Explicit strictly newer stable-release recovery without startup dispatch authority.

use super::submission_job::{JobPhase, QueueRefusal, SubmissionJob};
use super::submission_result::SubmissionResult;
use tallyline_shared_kernel::api::model::ReleaseVersion;

impl SubmissionJob {
    /// Returns the retained explicit newer-release recovery decision.
    #[must_use]
    pub const fn client_update(&self) -> Option<ReleaseVersion> {
        self.client_update
    }
    /// Reports whether a known strictly newer compiled stable release can recover this lockout.
    #[must_use]
    pub fn can_retry_after_update(&self, version: Option<ReleaseVersion>) -> bool {
        !self.service_pause_acknowledged
            && matches!(
                self.phase,
                JobPhase::Paused | JobPhase::Cancelled | JobPhase::Returned
            )
            && matches!((self.result.as_ref().map(SubmissionResult::final_result), version), (Some(SubmissionResult::VersionLockedOut(Some(origin))), Some(current)) if current > *origin)
    }
    /// Re-enters fresh send checks or acknowledges a final Report without replaying it.
    ///
    /// # Errors
    /// Refuses unknown, development, same or older identities and unrelated outcomes.
    pub fn retry_after_update(&self, version: ReleaseVersion) -> Result<Self, QueueRefusal> {
        if !self.can_retry_after_update(Some(version)) {
            return Err(QueueRefusal::InvalidTransition);
        }
        let mut next = self.advance()?;
        next.client_update = Some(version);
        if self.phase == JobPhase::Paused {
            next.phase = JobPhase::Queued;
            next.result = None;
        } else {
            next.service_pause_acknowledged = true;
        }
        Ok(next)
    }
}
