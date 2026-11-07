// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Ready POST slots before CPU preparation and durable write-ahead persistence.

use crate::api::model::HttpAdmissionFailure;
use tallyline_adapter_uex::api::model::PreparedUexSubmission;
use tallyline_submission::api::model::{CredentialValues, SubmissionJob, SubmissionResult};
use tokio::sync::oneshot;

pub(crate) struct SubmissionDispatch {
    pub(crate) prepared: PreparedUexSubmission,
    pub(crate) values: CredentialValues,
    pub(crate) token: Option<String>,
    pub(crate) quota_source: Option<crate::http_quota::PostQuotaSource>,
}
/// A pending slot request containing neither credentials nor encoded payload data.
pub struct PendingSubmissionReservation {
    reply: Option<oneshot::Receiver<SubmissionSlot>>,
}
impl PendingSubmissionReservation {
    pub(crate) fn new(reply: oneshot::Receiver<SubmissionSlot>) -> Self {
        Self { reply: Some(reply) }
    }
    /// Takes the sole ready POST slot after the owned HTTP route acquired its permit.
    ///
    /// # Errors
    /// Returns a stopped or consumed reservation without performing an HTTP request.
    pub fn poll_slot(&mut self) -> Result<Option<SubmissionSlot>, HttpAdmissionFailure> {
        match self
            .reply
            .as_mut()
            .ok_or(HttpAdmissionFailure::Stopped)?
            .try_recv()
        {
            Ok(slot) => {
                self.reply = None;
                if slot.dispatch.is_closed() {
                    Err(HttpAdmissionFailure::Stopped)
                } else {
                    Ok(Some(slot))
                }
            }
            Err(oneshot::error::TryRecvError::Empty) => Ok(None),
            Err(oneshot::error::TryRecvError::Closed) => {
                self.reply = None;
                Err(HttpAdmissionFailure::Stopped)
            }
        }
    }
}
/// The sole active POST route, waiting directly for a fully checked committed attempt.
/// Dropping this slot sends no request and releases the driver permit.
pub struct SubmissionSlot {
    dispatch: oneshot::Sender<SubmissionDispatch>,
    reply: oneshot::Receiver<SubmissionResult>,
    quota_source: Option<crate::http_quota::PostQuotaSource>,
}
impl SubmissionSlot {
    pub(crate) fn channel() -> (
        Self,
        oneshot::Receiver<SubmissionDispatch>,
        oneshot::Sender<SubmissionResult>,
    ) {
        let (dispatch, request) = oneshot::channel();
        let (result, reply) = oneshot::channel();
        (
            Self {
                dispatch,
                reply,
                quota_source: None,
            },
            request,
            result,
        )
    }
    pub(crate) fn bind_origin(mut self, sending: &SubmissionJob) -> Self {
        self.quota_source = Some(crate::http_quota::PostQuotaSource::new(sending));
        self
    }
    /// Dispatches once directly to the ready route after the caller committed write-ahead state.
    /// Fresh credentials must still match the immutable released account at this point.
    #[must_use]
    pub fn dispatch(
        self,
        prepared: PreparedUexSubmission,
        values: &CredentialValues,
        release_token: Option<&str>,
    ) -> PendingSubmission {
        let local_abort = self
            .dispatch
            .send(SubmissionDispatch {
                prepared,
                values: values.clone(),
                token: release_token.map(str::to_owned),
                quota_source: self.quota_source,
            })
            .is_err();
        PendingSubmission {
            reply: Some(self.reply),
            local_abort,
        }
    }
}
/// One network completion retained by its owner until the known result can be committed.
pub struct PendingSubmission {
    reply: Option<oneshot::Receiver<SubmissionResult>>,
    local_abort: bool,
}
impl PendingSubmission {
    /// Takes one completed result; a stopped route is uncertain and never authorises replay.
    pub fn poll_result(&mut self) -> Option<SubmissionResult> {
        if std::mem::take(&mut self.local_abort) {
            self.reply = None;
            return Some(SubmissionResult::Returned(
                tallyline_submission::api::model::SubmissionReturnReason::PreflightChanged,
            ));
        }
        let result = match self.reply.as_mut()?.try_recv() {
            Ok(result) => result,
            Err(oneshot::error::TryRecvError::Empty) => return None,
            Err(oneshot::error::TryRecvError::Closed) => SubmissionResult::Unknown,
        };
        self.reply = None;
        Some(result)
    }
}
