// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Durable quota progress and shared admission publication without HTTP replay.

use super::SubmissionQueueController;
use crate::api::{HttpClient, SubmissionControlError};
use tallyline_adapter_storage::api::SqliteReferenceCache;
use tallyline_submission::api::model::{SubmissionServiceCause, TokenQuotaObservation};
use tallyline_submission::api::{SubmissionQuotaRepository, SubmissionServiceFenceRepository};

pub(super) fn publish(
    store: &mut SqliteReferenceCache,
    client: &HttpClient,
) -> Result<(), SubmissionControlError> {
    let mut holds = store
        .authenticated_quotas()
        .map_err(SubmissionControlError::Repository)?;
    let post_fences = store
        .service_fences()
        .map_err(SubmissionControlError::Repository)?
        .into_iter()
        .filter(|fence| fence.cause() == SubmissionServiceCause::DailyQuota)
        .collect::<Vec<_>>();
    holds.extend(
        post_fences
            .iter()
            .map(|fence| TokenQuotaObservation::new(fence.token_scope(), None)),
    );
    client.replace_quota_holds(holds, &post_fences)
}
impl SubmissionQueueController {
    pub(super) fn advance_quotas(&mut self) -> Result<bool, SubmissionControlError> {
        if self.pending_quotas.is_empty() {
            self.pending_quotas = self.client.take_quota_observations()?;
        }
        let mut changed = !self.pending_quotas.is_empty();
        for observation in &self.pending_quotas {
            self.store
                .record_authenticated_quota(*observation)
                .map_err(SubmissionControlError::Repository)?;
        }
        self.pending_quotas.clear();
        let before_read = self.authenticated_quotas()?;
        let before_service = self.service_fences()?;
        if let Some(now) = self.client.quota_time() {
            let resumed = self
                .store
                .expire_quotas(now)
                .map_err(SubmissionControlError::Repository)?;
            changed |= !resumed.is_empty();
            for job in resumed {
                self.queue.update(job);
            }
        }
        changed |= before_read != self.authenticated_quotas()?
            || before_service != self.service_fences()?;
        publish(&mut self.store, &self.client)?;
        Ok(changed)
    }
}
