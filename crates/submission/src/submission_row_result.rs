// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact-row delivery evidence independent of whole-job transport phase.

use crate::api::model::{
    JobPhase, ReceiptDecision, RowFailureKind, SubmissionJob, SubmissionResolutionAction,
    SubmissionResult, UexErrorCode, VerifiedUexReport,
};
use tallyline_shared_kernel::api::model::{RowId, SubmissionMode};

/// The established delivery result of one immutable released row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubmissionRowResult<'a> {
    /// Storage may have occurred without a verified remote identity.
    Unmapped,
    /// A positive remote identity proves this exact released row.
    Accepted(&'a VerifiedUexReport),
    /// Full-count transport success establishes acceptance without a verified identity.
    AcceptedUnmapped,
    /// An explicit user statement establishes receipt without a remote identity.
    ReceivedByUser,
    /// An explicit user statement establishes non-receipt for this partial row.
    NotReceivedByUser,
    /// UEX refused the named row before storing it.
    Rejected(&'a UexErrorCode),
    /// Ordered processing stopped before this row was sent.
    NotSent,
}
impl SubmissionRowResult<'_> {
    /// Reports whether this row is resolved as accepted without granting withdrawal authority.
    #[must_use]
    pub const fn is_accepted(self) -> bool {
        matches!(
            self,
            Self::Accepted(_) | Self::AcceptedUnmapped | Self::ReceivedByUser
        )
    }
    /// Retains uncertainty and duplicate cooldowns while lifting only resolved non-acceptance.
    #[must_use]
    pub fn retains_cooldown(self) -> bool {
        !self.is_not_accepted()
            || matches!(self,Self::Rejected(code) if code.row_failure()==RowFailureKind::Duplicate)
    }
    /// Reports whether this row is resolved as not accepted.
    #[must_use]
    pub const fn is_not_accepted(self) -> bool {
        matches!(
            self,
            Self::NotReceivedByUser | Self::Rejected(_) | Self::NotSent
        )
    }
}
impl SubmissionJob {
    /// Returns row evidence without inferring acceptance from a prefix or response position.
    #[must_use]
    pub fn row_result(&self, row: RowId) -> Option<SubmissionRowResult<'_>> {
        let index = self
            .request()
            .rows()
            .iter()
            .position(|candidate| candidate.id() == row)?;
        if let Some(proof) = self
            .verified_row_receipts()
            .find(|receipt| receipt.row() == row)
        {
            return Some(SubmissionRowResult::Accepted(proof));
        }
        for action in self.resolution_actions() {
            if let SubmissionResolutionAction::RowDecision {
                row: decided,
                decision,
                ..
            } = action
                && *decided == row
            {
                return Some(match decision {
                    ReceiptDecision::Received => SubmissionRowResult::ReceivedByUser,
                    ReceiptDecision::NotReceived => SubmissionRowResult::NotReceivedByUser,
                });
            }
        }
        match self.phase() {
            JobPhase::Succeeded => return Some(SubmissionRowResult::AcceptedUnmapped),
            JobPhase::OutcomeUnknown => {
                return Some(
                    if self.unknown_decision() == Some(ReceiptDecision::Received) {
                        SubmissionRowResult::ReceivedByUser
                    } else {
                        SubmissionRowResult::Unmapped
                    },
                );
            }
            JobPhase::PartiallyAccepted => {}
            JobPhase::Queued
            | JobPhase::Sending
            | JobPhase::Rejected
            | JobPhase::Returned
            | JobPhase::Cancelled
            | JobPhase::Paused
            | JobPhase::Held => return None,
        }
        if let SubmissionResult::PartialError {
            code,
            row: Some(failed),
            ..
        } = self.result()?.final_result()
            && *failed < self.request().rows().len()
        {
            return Some(match index.cmp(failed) {
                std::cmp::Ordering::Less => SubmissionRowResult::Unmapped,
                std::cmp::Ordering::Equal => SubmissionRowResult::Rejected(code),
                std::cmp::Ordering::Greater => SubmissionRowResult::NotSent,
            });
        }
        Some(SubmissionRowResult::Unmapped)
    }
}
impl SubmissionJob {
    /// Returns the complete eligible subset only after every partial row is resolved.
    #[must_use]
    pub fn partial_copy_rows(&self) -> Option<impl Iterator<Item = RowId> + '_> {
        if self.phase() != JobPhase::PartiallyAccepted
            || self
                .resolution_actions()
                .iter()
                .any(|action| matches!(action, SubmissionResolutionAction::RowsDuplicated { .. }))
        {
            return None;
        }
        if !self.request().rows().iter().all(|row| {
            self.row_result(row.id())
                .is_some_and(|result| result.is_accepted() || result.is_not_accepted())
        }) || !self.request().rows().iter().any(|row| {
            self.row_result(row.id())
                .is_some_and(|result| result.is_not_accepted())
        }) {
            return None;
        }
        Some(
            self.request()
                .rows()
                .iter()
                .filter(|row| {
                    self.row_result(row.id())
                        .is_some_and(|result| result.is_not_accepted())
                })
                .map(|row| row.id()),
        )
    }
}
impl SubmissionJob {
    /// Reports whether a whole unknown-origin statement can be recorded.
    #[must_use]
    pub fn can_decide_unknown(&self) -> bool {
        self.phase() == JobPhase::OutcomeUnknown && self.resolution_actions().is_empty()
    }
    /// Reports whether one exact unresolved partial row can receive a user statement.
    #[must_use]
    pub fn can_decide_row(&self, row: RowId) -> bool {
        self.phase() == JobPhase::PartiallyAccepted
            && matches!(self.row_result(row), Some(SubmissionRowResult::Unmapped))
    }
    /// Reports whether positive identities may be read for this immutable production attempt.
    #[must_use]
    pub fn can_check_receipts(&self) -> bool {
        matches!(
            self.phase(),
            JobPhase::OutcomeUnknown | JobPhase::PartiallyAccepted
        ) && self.request().header().mode == SubmissionMode::Production
    }
}
