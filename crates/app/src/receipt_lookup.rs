// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Nonblocking optional receipt replies without transferring session credentials to another owner.

use tallyline_submission::api::model::{SubmissionJob, VerifiedReceiptMapping, VerifiedUexReport};
use tokio::sync::oneshot;

/// A refused optional HTTP command; known production acceptance remains unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HttpAdmissionFailure {
    /// The bounded shared command queue is full.
    Busy,
    /// The owned HTTP driver has stopped.
    Stopped,
    /// The supplied job is not a complete production acceptance awaiting verification.
    InvalidJob,
}

/// A completed optional lookup, still requiring persistence before UI publication.
pub enum ReceiptLookupOutcome {
    /// Every accepted identity has an exact released-row proof.
    Verified(Box<VerifiedReceiptMapping>),
    /// The read failed, stopped or could not establish a complete consistent mapping.
    Unavailable,
}

/// One bounded receipt read whose reply never blocks the credential or database owner.
pub struct PendingReceiptVerification {
    accepted: SubmissionJob,
    reply: Option<oneshot::Receiver<Option<Vec<VerifiedUexReport>>>>,
}
impl PendingReceiptVerification {
    pub(crate) fn new(
        accepted: SubmissionJob,
        reply: oneshot::Receiver<Option<Vec<VerifiedUexReport>>>,
    ) -> Self {
        Self {
            accepted,
            reply: Some(reply),
        }
    }
    /// Takes one completed proof or safe failure; pending and already consumed replies return absence.
    /// A proof must be committed through the workflow transaction before publishing it to the UI.
    pub fn poll_result(&mut self) -> Option<ReceiptLookupOutcome> {
        let receipts = match self.reply.as_mut()?.try_recv() {
            Ok(receipts) => receipts,
            Err(oneshot::error::TryRecvError::Empty) => return None,
            Err(oneshot::error::TryRecvError::Closed) => None,
        };
        self.reply = None;
        Some(
            match receipts
                .and_then(|receipts| VerifiedReceiptMapping::new(&self.accepted, receipts).ok())
            {
                Some(mapping) => ReceiptLookupOutcome::Verified(Box::new(mapping)),
                None => ReceiptLookupOutcome::Unavailable,
            },
        )
    }
}
