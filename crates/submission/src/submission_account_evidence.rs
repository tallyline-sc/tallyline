// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Immutable account-refusal provenance retained per submission attempt.

use crate::api::model::{SubmissionPauseReason, SubmissionResult, UexErrorCode};

/// Progress of the separate read-only account check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubmissionAccountCheck {
    /// A token refusal needs no account-key read.
    NotRequired,
    /// The account read must follow durable refusal persistence.
    Pending,
    /// The account read completed with this closed recovery action.
    Completed(SubmissionPauseReason),
}
/// Original account refusal and independent account-check evidence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubmissionAccountEvidence {
    code: Option<UexErrorCode>,
    http_status: u16,
    original_action: SubmissionPauseReason,
    check: SubmissionAccountCheck,
    origin_revision: Option<u64>,
}
impl SubmissionAccountEvidence {
    /// Records bounded provenance without attesting receipt certainty or send authority.
    #[must_use]
    pub const fn new(
        code: Option<UexErrorCode>,
        http_status: u16,
        original_action: SubmissionPauseReason,
        check: SubmissionAccountCheck,
        origin_revision: Option<u64>,
    ) -> Self {
        Self {
            code,
            http_status,
            original_action,
            check,
            origin_revision,
        }
    }
    /// Borrows the original safe code; unsafe identifiers remain absent.
    #[must_use]
    pub const fn code(&self) -> Option<&UexErrorCode> {
        self.code.as_ref()
    }
    /// Returns the original HTTP status.
    #[must_use]
    pub const fn http_status(&self) -> u16 {
        self.http_status
    }
    /// Returns the original account action independently of later reads.
    #[must_use]
    pub const fn original_action(&self) -> SubmissionPauseReason {
        self.original_action
    }
    /// Returns account-check progress without implying a receipt check.
    #[must_use]
    pub const fn check(&self) -> SubmissionAccountCheck {
        self.check
    }
    /// Returns the committed revision of the originating HTTP refusal.
    #[must_use]
    pub const fn origin_revision(&self) -> Option<u64> {
        self.origin_revision
    }
    pub(crate) fn at_revision(mut self, revision: u64) -> Self {
        self.origin_revision = Some(revision);
        self
    }
    pub(crate) fn pending(&self) -> Self {
        let mut next = self.clone();
        if matches!(next.check, SubmissionAccountCheck::Completed(_)) {
            next.check = SubmissionAccountCheck::Pending;
        }
        next
    }
    pub(crate) fn wrap(&self, outcome: SubmissionResult) -> SubmissionResult {
        SubmissionResult::AccountRefusal {
            code: self.code.clone(),
            http_status: self.http_status,
            reason: self.original_action,
            check: self.check,
            outcome: Box::new(outcome),
        }
    }
}
