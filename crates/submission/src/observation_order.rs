// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact-row observation ordering independent of global dispatch scheduling.

use crate::api::model::{JobId, JobPhase, SubmissionJob, SubmissionRequest};
use crate::duplicate_evidence::SubmissionEvidence;
use tallyline_shared_kernel::api::model::{RowId, Timestamp};

/// A row's unresolved ordering decision before release or sending.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObservationOrderBlock {
    /// Positive original receipt evidence closes release of this explicit duplicate.
    ResolvedDuplicateOrigin {
        /// The original uncertain submission with positive evidence.
        submission: JobId,
    },
    /// An exact-row decision against this newer accepted observation is missing.
    NeedsConfirmation {
        /// The older candidate's local row identity.
        row: RowId,
        /// The newest accepted original observation, independent of wire-date clamping.
        observed_at: Timestamp,
    },
    /// A newer row may have reached UEX and cannot be bypassed by a confirmation.
    Unresolved {
        /// The older candidate's local row identity.
        row: RowId,
        /// The newer uncertain submission requiring resolution.
        submission: JobId,
        /// The newer original observation.
        observed_at: Timestamp,
    },
}
/// Checks durable same-account/mode/terminal/side history for each exact candidate row.
pub struct ObservationOrder;
impl ObservationOrder {
    /// Returns ordering blocks without treating unrelated in-flight work as approval.
    ///
    /// An unavailable history must fail at the persistence port before this method is called.
    /// Newer unresolved rows remain closed even with a known-row confirmation.
    #[must_use]
    pub fn assess(
        request: &SubmissionRequest,
        history: &[SubmissionJob],
    ) -> Vec<ObservationOrderBlock> {
        let header = request.header();
        if let Some(submission) =
            SubmissionEvidence::resolved_duplicate_origin(header.report, history)
        {
            return vec![ObservationOrderBlock::ResolvedDuplicateOrigin { submission }];
        }
        let mut blocks = Vec::new();
        for row in request.rows() {
            let mut accepted = None;
            let mut unresolved = None;
            for job in history {
                let previous = job.request();
                let old = previous.header();
                if old.account != header.account
                    || old.mode != header.mode
                    || old.terminal != header.terminal
                    || old.side != header.side
                    || old.observed_at <= header.observed_at
                    || !previous
                        .rows()
                        .iter()
                        .any(|old_row| old_row.commodity() == row.commodity())
                {
                    continue;
                }
                let Some(previous_row) = previous
                    .rows()
                    .iter()
                    .find(|old_row| old_row.commodity() == row.commodity())
                else {
                    continue;
                };
                let result = job.row_result(previous_row.id());
                if result.is_some_and(|result| result.is_not_accepted()) {
                    continue;
                }
                if result.is_some_and(|result| result.is_accepted()) {
                    accepted = Some(
                        accepted.map_or(old.observed_at, |at: Timestamp| at.max(old.observed_at)),
                    );
                    continue;
                }
                match job.observation_order_phase() {
                    JobPhase::Succeeded
                    | JobPhase::Sending
                    | JobPhase::PartiallyAccepted
                    | JobPhase::OutcomeUnknown => {
                        let candidate = (old.observed_at, job.id());
                        unresolved = Some(
                            unresolved.map_or(candidate, |previous: (Timestamp, JobId)| {
                                previous.max(candidate)
                            }),
                        );
                    }
                    JobPhase::Queued
                    | JobPhase::Returned
                    | JobPhase::Cancelled
                    | JobPhase::Held
                    | JobPhase::Paused
                    | JobPhase::Rejected => continue,
                }
            }
            if let Some((observed_at, submission)) = unresolved {
                blocks.push(ObservationOrderBlock::Unresolved {
                    row: row.id(),
                    submission,
                    observed_at,
                });
            } else if let Some(observed_at) = accepted
                && !request.newer_observation_confirmed(row.id(), observed_at)
            {
                blocks.push(ObservationOrderBlock::NeedsConfirmation {
                    row: row.id(),
                    observed_at,
                });
            }
        }
        blocks
    }
}
