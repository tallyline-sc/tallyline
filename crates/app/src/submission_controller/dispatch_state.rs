// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Sole reserved POST and active completion state, retained across persistence failures.

use super::SubmissionControlError;
use super::dispatch_preparation::{DispatchPreparation, PreparedDispatch};
use super::queue_state::QueueState;
use crate::api::{PendingSubmission, PendingSubmissionReservation, SubmissionSlot};
use tallyline_adapter_storage::api::SqliteReferenceCache;
use tallyline_shared_kernel::api::Clock;
use tallyline_submission::api::model::{
    JobId, ReleaseToken, SubmissionCommand, SubmissionJob, SubmissionResult,
};
use tallyline_submission::api::{CredentialApplication, SubmissionJobRepository};

struct ActiveSubmission {
    sending: SubmissionJob,
    pending: Option<PendingSubmission>,
    completion: Option<(
        SubmissionResult,
        tallyline_shared_kernel::api::model::Timestamp,
    )>,
}

#[derive(Default)]
pub(super) struct DispatchState {
    reservation: Option<(JobId, PendingSubmissionReservation)>,
    active: Option<ActiveSubmission>,
    client_version: Option<tallyline_shared_kernel::api::model::ReleaseVersion>,
}
pub(super) enum DispatchProgress {
    Idle,
    Started,
    Scheduled,
}
impl DispatchState {
    pub(super) fn new(
        client_version: Option<tallyline_shared_kernel::api::model::ReleaseVersion>,
    ) -> Self {
        Self {
            client_version,
            ..Self::default()
        }
    }
    pub(super) fn is_active(&self) -> bool {
        self.active.is_some()
    }
    pub(super) fn cancel_reservation(&mut self, id: JobId) {
        if self
            .reservation
            .as_ref()
            .is_some_and(|(reserved, _)| *reserved == id)
        {
            self.reservation = None;
        }
    }
    pub(super) fn advance(
        &mut self,
        mut context: DispatchPreparation<'_>,
        credentials: &CredentialApplication,
        token: Option<&ReleaseToken>,
    ) -> Result<DispatchProgress, SubmissionControlError> {
        if let Some((id, mut reservation)) = self.reservation.take() {
            let slot = match reservation.poll_slot() {
                Ok(Some(slot)) => slot,
                Ok(None) => {
                    self.reservation = Some((id, reservation));
                    return Ok(DispatchProgress::Idle);
                }
                Err(_) => return Err(SubmissionControlError::Unavailable),
            };
            let Some(prepared) = context.prepare_reserved(id, credentials, token)? else {
                return Ok(DispatchProgress::Idle);
            };
            self.dispatch_prepared(&mut context, prepared, slot, token)?;
            return Ok(DispatchProgress::Started);
        }
        self.reservation = context.reserve_candidate(credentials, token)?;
        Ok(DispatchProgress::Scheduled)
    }
    fn dispatch_prepared(
        &mut self,
        context: &mut DispatchPreparation<'_>,
        work: PreparedDispatch<'_>,
        slot: SubmissionSlot,
        token: Option<&ReleaseToken>,
    ) -> Result<(), SubmissionControlError> {
        let sending = context.begin_attempt(&work)?;
        self.active = Some(ActiveSubmission {
            sending: sending.clone(),
            pending: None,
            completion: Some((
                SubmissionResult::Returned(
                    tallyline_submission::api::model::SubmissionReturnReason::PreflightChanged,
                ),
                context.now(),
            )),
        });
        let pending = context.dispatch_if_current(work, slot.bind_origin(&sending), token)?;
        let completion = if pending.is_none() {
            Some((
                SubmissionResult::Returned(
                    tallyline_submission::api::model::SubmissionReturnReason::PreflightChanged,
                ),
                context.now(),
            ))
        } else {
            None
        };
        context.publish_started(sending.clone());
        self.active = Some(ActiveSubmission {
            sending,
            pending,
            completion,
        });
        Ok(())
    }
    pub(super) fn commit_completion(
        &mut self,
        store: &mut SqliteReferenceCache,
        clock: &dyn Clock,
        queue: &mut QueueState,
    ) -> Result<Option<SubmissionJob>, SubmissionControlError> {
        let Some(mut active) = self.active.take() else {
            return Ok(None);
        };
        if active.completion.is_none() {
            active.completion = active
                .pending
                .as_mut()
                .and_then(PendingSubmission::poll_result)
                .map(|result| (result, clock.now()));
        }
        let Some((result, at)) = active.completion.as_ref() else {
            self.active = Some(active);
            return Ok(None);
        };
        let result = if matches!(
            result,
            SubmissionResult::Paused(
                tallyline_submission::api::model::SubmissionPauseReason::UpdateRequired
            )
        ) {
            SubmissionResult::VersionLockedOut(self.client_version)
        } else if matches!(
            result,
            SubmissionResult::Held(
                tallyline_submission::api::model::SubmissionHoldReason::Cooldown
            )
        ) {
            tallyline_submission::api::SubmissionEvidence::duplicate_after_unknown(
                &active.sending,
                queue.jobs(),
                *at,
            )
            .map_or_else(|| result.clone(), SubmissionResult::DuplicateEvidence)
        } else {
            result.clone()
        };
        let change = active
            .sending
            .apply(SubmissionCommand::Complete { result, at: *at })
            .map_err(|_| SubmissionControlError::Invalid)?;
        if let Err(error) = SubmissionJobRepository::save(store, &change) {
            self.active = Some(active);
            return Err(SubmissionControlError::Repository(error));
        }
        let committed = change.into_state();
        super::failure_diagnostics::record(&committed);
        queue.update(committed.clone());
        Ok(Some(committed))
    }
    pub(super) fn shutdown(
        &mut self,
        store: &mut SqliteReferenceCache,
        clock: &dyn Clock,
        queue: &mut QueueState,
    ) -> Result<(), SubmissionControlError> {
        self.reservation = None;
        if let Some(active) = &mut self.active
            && active.completion.is_none()
        {
            let result = active
                .pending
                .as_mut()
                .and_then(PendingSubmission::poll_result)
                .unwrap_or(SubmissionResult::Unknown);
            active.completion = Some((result, clock.now()));
        }
        self.commit_completion(store, clock, queue)?;
        Ok(())
    }
}
