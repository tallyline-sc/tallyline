// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Append-only resolution evidence beside immutable partial and unknown attempts.

use crate::api::model::{
    JobPhase, QueueRefusal, SubmissionJob, SubmissionResult, SubmissionRowResult, VerifiedUexReport,
};
use std::collections::BTreeSet;
use tallyline_shared_kernel::api::model::{ReportId, RowId, Timestamp};

/// A user's statement about an uncertain submission, without remote proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReceiptDecision {
    /// The user considers the submitted values received.
    Received,
    /// The user considers the submitted values not received.
    NotReceived,
}

/// An explicit resolution action without submission authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubmissionResolutionAction {
    /// Links the separate ready Report while retaining all original evidence ancestry.
    Split {
        /// Fresh ready Report identity; the cooling Report retains the source identity.
        report: ReportId,
        /// Explicit split time.
        at: Timestamp,
    },
    /// Links a separate Draft containing exactly the resolved not-accepted rows.
    RowsDuplicated {
        /// The fresh separate Draft identity.
        report: ReportId,
        /// The exact original local row identities copied.
        rows: Vec<RowId>,
        /// The injected copying time.
        at: Timestamp,
    },
    /// Records a statement about one unresolved row without remote proof.
    RowDecision {
        /// The exact immutable local row.
        row: RowId,
        /// The user's statement.
        decision: ReceiptDecision,
        /// The injected decision time.
        at: Timestamp,
    },
    /// Records a user statement independently of the original attempt result.
    Decision {
        /// The user's statement.
        decision: ReceiptDecision,
        /// The injected decision time.
        at: Timestamp,
    },
    /// Records positive remote identities verified for the exact attempted snapshot.
    Verified {
        /// Positively matched row identities, never inferred from absence.
        receipts: Vec<VerifiedUexReport>,
        /// The injected evidence time.
        at: Timestamp,
    },
    /// Links a separate Draft created by the explicit copying action.
    Duplicated {
        /// The fresh identity of the separate Draft.
        report: ReportId,
        /// The injected duplication time.
        at: Timestamp,
    },
}
impl SubmissionResolutionAction {
    /// Returns the explicit action's recorded time.
    #[must_use]
    pub const fn at(&self) -> Timestamp {
        match self {
            Self::Split { at, .. }
            | Self::RowsDuplicated { at, .. }
            | Self::RowDecision { at, .. }
            | Self::Decision { at, .. }
            | Self::Duplicated { at, .. }
            | Self::Verified { at, .. } => *at,
        }
    }
}

impl SubmissionJob {
    /// Retains an explicit resolution action without changing the original attempt.
    ///
    /// # Errors
    /// Refuses ineligible jobs, rows and conflicting evidence.
    pub fn resolve_delivery(
        &self,
        mut action: SubmissionResolutionAction,
    ) -> Result<Self, QueueRefusal> {
        if !matches!(
            self.phase(),
            JobPhase::OutcomeUnknown | JobPhase::PartiallyAccepted | JobPhase::Rejected
        ) {
            return Err(QueueRefusal::InvalidTransition);
        }
        if self.phase() == JobPhase::Rejected
            && !matches!(action, SubmissionResolutionAction::Duplicated { .. })
        {
            return Err(QueueRefusal::InvalidTransition);
        }
        if self.resolution_actions.last() == Some(&action) {
            return Ok(self.clone());
        }
        match &action {
            SubmissionResolutionAction::RowsDuplicated { report, rows, .. }
                if report.0 != 0
                    && *report != self.request().header().report
                    && self
                        .partial_copy_rows()
                        .is_some_and(|eligible| eligible.eq(rows.iter().copied())) => {}
            SubmissionResolutionAction::RowDecision { row, .. } if self.can_decide_row(*row) => {}
            SubmissionResolutionAction::Decision { .. } if self.can_decide_unknown() => {}
            SubmissionResolutionAction::Verified { receipts, .. } => {
                self.validate_row_receipts(receipts)?
            }
            SubmissionResolutionAction::Duplicated { report, .. }
                if (self.can_duplicate_unknown() || self.can_duplicate_rejected())
                    && report.0 != 0
                    && *report != self.request().header().report => {}
            SubmissionResolutionAction::Split { .. }
            | SubmissionResolutionAction::RowsDuplicated { .. }
            | SubmissionResolutionAction::RowDecision { .. }
            | SubmissionResolutionAction::Decision { .. }
            | SubmissionResolutionAction::Duplicated { .. } => {
                return Err(QueueRefusal::InvalidTransition);
            }
        }
        let previous_at = self
            .resolution_actions
            .last()
            .map(SubmissionResolutionAction::at)
            .or_else(|| {
                self.attempts()
                    .last()
                    .and_then(|attempt| attempt.ended_at())
            })
            .ok_or(QueueRefusal::InvalidTransition)?;
        if action.at() < previous_at {
            return Err(QueueRefusal::InvalidTransition);
        }
        if let SubmissionResolutionAction::Verified { receipts, .. } = &mut action {
            receipts.retain(|receipt| {
                !self
                    .verified_row_receipts()
                    .any(|existing| existing == receipt)
            });
        }
        let mut next = self.advance()?;
        next.resolution_actions.push(action);
        Ok(next)
    }

    /// Borrows explicit evidence without granting sending or withdrawal authority.
    #[must_use]
    pub fn resolution_actions(&self) -> &[SubmissionResolutionAction] {
        &self.resolution_actions
    }
}

impl SubmissionJob {
    /// Returns a user statement without treating it as remote receipt proof.
    #[must_use]
    pub fn unknown_decision(&self) -> Option<ReceiptDecision> {
        self.resolution_actions
            .iter()
            .find_map(|action| match action {
                SubmissionResolutionAction::Decision { decision, .. } => Some(*decision),
                SubmissionResolutionAction::Split { .. }
                | SubmissionResolutionAction::RowsDuplicated { .. }
                | SubmissionResolutionAction::RowDecision { .. }
                | SubmissionResolutionAction::Duplicated { .. }
                | SubmissionResolutionAction::Verified { .. } => None,
            })
    }

    /// Reports eligibility to copy a permanent client-review refusal into a new Draft.
    #[must_use]
    pub fn can_duplicate_rejected(&self) -> bool {
        use crate::api::model::SubmissionCorrection;
        self.phase() == JobPhase::Rejected
            && matches!(self.result().map(SubmissionResult::final_result),
                Some(SubmissionResult::Rejected(rejection))
                    if rejection.correction() == SubmissionCorrection::ReviewClient)
            && !self
                .resolution_actions
                .iter()
                .any(|action| matches!(action, SubmissionResolutionAction::Duplicated { .. }))
    }
    /// Reports eligibility for the separate explicit new-Draft action.
    #[must_use]
    pub fn can_duplicate_unknown(&self) -> bool {
        self.phase() == JobPhase::OutcomeUnknown
            && self.unknown_decision() == Some(ReceiptDecision::NotReceived)
            && self.verified_row_receipts().next().is_none()
            && !self
                .resolution_actions
                .iter()
                .any(|action| matches!(action, SubmissionResolutionAction::Duplicated { .. }))
    }
}

impl SubmissionJob {
    /// Borrows positive row identities while retaining their returned dates.
    pub fn verified_row_receipts(&self) -> impl Iterator<Item = &VerifiedUexReport> {
        let original = match self.result().map(SubmissionResult::final_result) {
            Some(SubmissionResult::PartialError { receipts, .. }) => receipts.as_slice(),
            _ => &[],
        };
        original.iter().chain(
            self.resolution_actions
                .iter()
                .flat_map(|action| match action {
                    SubmissionResolutionAction::Verified { receipts, .. } => receipts.as_slice(),
                    SubmissionResolutionAction::Split { .. }
                    | SubmissionResolutionAction::RowsDuplicated { .. }
                    | SubmissionResolutionAction::RowDecision { .. }
                    | SubmissionResolutionAction::Decision { .. }
                    | SubmissionResolutionAction::Duplicated { .. } => &[],
                }),
        )
    }

    pub(crate) fn validate_row_receipts(
        &self,
        receipts: &[VerifiedUexReport],
    ) -> Result<(), QueueRefusal> {
        let sent_at = self
            .attempts()
            .last()
            .ok_or(QueueRefusal::InvalidTransition)?
            .date_added();
        let mut rows = BTreeSet::new();
        let mut identities = BTreeSet::new();
        let mut novel = false;
        for receipt in receipts {
            if self.phase() == JobPhase::PartiallyAccepted
                && matches!(
                    self.row_result(receipt.row()),
                    Some(SubmissionRowResult::Rejected(_) | SubmissionRowResult::NotSent)
                )
            {
                return Err(QueueRefusal::InvalidTransition);
            }
            if !receipt.matches_at(self.request(), sent_at)
                || !rows.insert(receipt.row().0)
                || !identities.insert(receipt.id())
            {
                return Err(QueueRefusal::InvalidTransition);
            }
            for existing in self.verified_row_receipts() {
                if (existing.row() == receipt.row() || existing.id() == receipt.id())
                    && existing != receipt
                {
                    return Err(QueueRefusal::InvalidTransition);
                }
            }
            novel |= !self
                .verified_row_receipts()
                .any(|existing| existing == receipt);
        }
        if !novel {
            return Err(QueueRefusal::InvalidTransition);
        }
        Ok(())
    }
}

impl SubmissionJob {
    pub(crate) fn cancel_for_split(
        &self,
        report: ReportId,
        at: Timestamp,
    ) -> Result<Self, QueueRefusal> {
        if self.phase() != JobPhase::Queued
            || report.0 == 0
            || report == self.request().header().report
            || self.request().rows().len() < 2
            || !self.resolution_actions.is_empty()
            || at < self.request().header().observed_at
            || self
                .attempts()
                .last()
                .and_then(|attempt| attempt.ended_at())
                .is_some_and(|ended| at < ended)
        {
            return Err(QueueRefusal::InvalidTransition);
        }
        let mut next = self.cancel()?;
        next.resolution_actions
            .push(SubmissionResolutionAction::Split { report, at });
        Ok(next)
    }
}
