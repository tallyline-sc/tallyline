// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Duplicate evidence bound to an earlier uncertain same-account row key.

use std::collections::{BTreeMap, BTreeSet};

use crate::api::model::SubmissionResolutionAction;
use crate::api::model::{JobId, JobPhase, SubmissionJob};
use tallyline_shared_kernel::api::model::{ReportId, Timestamp};

const EVIDENCE_MILLIS: u128 = 2 * 5 * 60 * 1000;

/// Assesses evidence without resolving or replaying any uncertain request.
pub struct SubmissionEvidence;
impl SubmissionEvidence {
    /// Finds the newest recent unknown attempt sharing at least one exact cooldown key.
    #[must_use]
    pub fn duplicate_after_unknown(
        candidate: &SubmissionJob,
        history: &[SubmissionJob],
        now: Timestamp,
    ) -> Option<JobId> {
        let current = candidate.request();
        history
            .iter()
            .filter_map(|job| {
                let previous = job.request();
                if job.id() == candidate.id()
                    || job.phase() != JobPhase::OutcomeUnknown
                    || previous.header().account != current.header().account
                    || previous.header().mode != current.header().mode
                    || previous.header().terminal != current.header().terminal
                    || !previous.rows().iter().any(|old| {
                        current
                            .rows()
                            .iter()
                            .any(|row| row.commodity() == old.commodity())
                    })
                {
                    return None;
                }
                let ended = job.attempts().last()?.ended_at()?;
                (ended <= now && now.millis() - ended.millis() <= EVIDENCE_MILLIS)
                    .then_some((ended, job.id()))
            })
            .max()
            .map(|(_, id)| id)
    }
}

impl SubmissionEvidence {
    /// Finds positive origin evidence that closes release of its already created duplicate.
    #[must_use]
    pub fn resolved_duplicate_origin(report: ReportId, history: &[SubmissionJob]) -> Option<JobId> {
        let mut parents_by_child = BTreeMap::<ReportId, Vec<ReportId>>::new();
        for source in history
            .iter()
            .filter(|job| job.phase() == JobPhase::Cancelled)
        {
            for action in source.resolution_actions() {
                match action {
                    SubmissionResolutionAction::Split { report: child, .. } => {
                        parents_by_child
                            .entry(*child)
                            .or_default()
                            .push(source.request().header().report);
                    }
                    SubmissionResolutionAction::RowsDuplicated { .. }
                    | SubmissionResolutionAction::RowDecision { .. }
                    | SubmissionResolutionAction::Decision { .. }
                    | SubmissionResolutionAction::Duplicated { .. }
                    | SubmissionResolutionAction::Verified { .. } => {}
                }
            }
        }
        let mut reports = BTreeSet::from([report]);
        let mut pending = vec![report];
        while let Some(child) = pending.pop() {
            if let Some(parents) = parents_by_child.remove(&child) {
                for parent in parents {
                    if reports.insert(parent) {
                        pending.push(parent);
                    }
                }
            }
        }

        history
            .iter()
            .find(|job| {
                job.verified_row_receipts().next().is_some()
                    && job.resolution_actions().iter().any(|action| match action {
                        SubmissionResolutionAction::Duplicated {
                            report: duplicate, ..
                        } => reports.contains(duplicate),
                        SubmissionResolutionAction::RowsDuplicated {
                            report: duplicate,
                            rows,
                            ..
                        } => {
                            reports.contains(duplicate)
                                && job
                                    .verified_row_receipts()
                                    .any(|proof| rows.contains(&proof.row()))
                        }
                        SubmissionResolutionAction::Split { .. }
                        | SubmissionResolutionAction::Decision { .. }
                        | SubmissionResolutionAction::Verified { .. }
                        | SubmissionResolutionAction::RowDecision { .. } => false,
                    })
            })
            .map(SubmissionJob::id)
    }
}
