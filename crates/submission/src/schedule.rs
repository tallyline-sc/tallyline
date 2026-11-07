// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Whole-report cooldown and observation-order checks over durable jobs.

use crate::api::model::{JobPhase, SubmissionJob, SubmissionResult};
use crate::duplicate_evidence::SubmissionEvidence;
use std::time::Duration;
use tallyline_shared_kernel::api::model::{RowId, Timestamp};

const COOLDOWN: Duration = Duration::from_secs(5 * 60);

/// A closed scheduling decision without trimming any released row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScheduleDecision {
    /// Every row can proceed to the remaining fresh release checks.
    Ready,
    /// Another durable attempt is already in flight, irrespective of account or mode.
    InFlight,
    /// At least one row still has a cooldown; the complete Report waits until this time.
    WaitingUntil(Timestamp),
    /// An older queued observation of a shared row key must be considered first.
    EarlierObservationPending,
    /// A newer observation of a shared row key was already accepted.
    NewerObservationSent,
    /// A newer possibly processed observation still needs explicit resolution.
    UnresolvedNewerObservation,
    /// The candidate phase or durable attempt metadata cannot authorise dispatch.
    Unavailable,
}

/// One exact row key still held by retained cooldown evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RowCooldown {
    row: RowId,
    until: Timestamp,
}
impl RowCooldown {
    /// Returns the stable row in the candidate release snapshot.
    #[must_use]
    pub const fn row(self) -> RowId {
        self.row
    }
    /// Returns the latest active cooldown deadline for this row.
    #[must_use]
    pub const fn until(self) -> Timestamp {
        self.until
    }
}

/// Ordered complete row groups assessed from the same durable scheduling policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CooldownPartition {
    ready: Vec<RowId>,
    cooling: Vec<RowCooldown>,
}
impl CooldownPartition {
    /// Borrows rows with no active cooldown in their original release order.
    #[must_use]
    pub fn ready_rows(&self) -> &[RowId] {
        &self.ready
    }
    /// Borrows cooling rows and exact deadlines in their original release order.
    #[must_use]
    pub fn cooling_rows(&self) -> &[RowCooldown] {
        &self.cooling
    }
}

/// Decides scheduling from committed history without performing I/O or mutating a job.
pub struct SubmissionSchedule;
impl SubmissionSchedule {
    /// Checks global in-flight work, row cooldowns and oldest-observation ordering.
    ///
    /// The persistence port must supply all unfinished jobs and relevant retained history;
    /// unavailable history must fail at that port rather than become an empty history.
    #[must_use]
    pub fn assess(
        candidate: &SubmissionJob,
        history: &[SubmissionJob],
        now: Timestamp,
    ) -> ScheduleDecision {
        match Self::cooldown_partition(candidate, history, now) {
            Ok(partition) => partition
                .cooling_rows()
                .iter()
                .map(|row| row.until())
                .max()
                .map_or(ScheduleDecision::Ready, ScheduleDecision::WaitingUntil),
            Err(reason) => reason,
        }
    }
    /// Assesses every released row without granting authority to send either group.
    ///
    /// # Errors
    /// Refuses non-queued work, in-flight requests and unresolved observation-order evidence.
    /// The caller must supply complete durable history and repeat all independent send checks.
    pub fn cooldown_partition(
        candidate: &SubmissionJob,
        history: &[SubmissionJob],
        now: Timestamp,
    ) -> Result<CooldownPartition, ScheduleDecision> {
        if candidate.phase() != JobPhase::Queued {
            return Err(ScheduleDecision::Unavailable);
        }
        if SubmissionEvidence::resolved_duplicate_origin(
            candidate.request().header().report,
            history,
        )
        .is_some()
        {
            return Err(ScheduleDecision::Unavailable);
        }
        if history
            .iter()
            .any(|job| job.id() != candidate.id() && job.phase() == JobPhase::Sending)
        {
            return Err(ScheduleDecision::InFlight);
        }
        if let Some(attempt) = candidate.attempts().last()
            && attempt.outcome()
                == Some(crate::api::model::AttemptOutcome::Held(
                    crate::api::model::SubmissionHoldReason::Cooldown,
                ))
            && let Some(until) = attempt.ended_at().and_then(|at| at.checked_add(COOLDOWN))
            && until > now
        {
            return Ok(CooldownPartition {
                ready: Vec::new(),
                cooling: candidate
                    .request()
                    .rows()
                    .iter()
                    .map(|row| RowCooldown {
                        row: row.id(),
                        until,
                    })
                    .collect(),
            });
        }
        let request = candidate.request();
        let key = request.header();
        let mut deadlines = std::collections::BTreeMap::<u64, Timestamp>::new();
        let mut earlier = false;
        let blocks = crate::observation_order::ObservationOrder::assess(request, history);
        if blocks.iter().any(|block| {
            matches!(
                block,
                crate::api::model::ObservationOrderBlock::NeedsConfirmation { .. }
            )
        }) {
            return Err(ScheduleDecision::NewerObservationSent);
        }
        let unresolved_newer = blocks.iter().any(|block| {
            matches!(
                block,
                crate::api::model::ObservationOrderBlock::Unresolved { .. }
            )
        });
        for job in history.iter().filter(|job| job.id() != candidate.id()) {
            let phase = job.observation_order_phase();
            let previous = job.request();
            let old = previous.header();
            if old.account != key.account || old.mode != key.mode || old.terminal != key.terminal {
                continue;
            }
            let shared = previous.rows().iter().any(|old_row| {
                request
                    .rows()
                    .iter()
                    .any(|row| row.commodity() == old_row.commodity())
            });
            if !shared {
                continue;
            }
            if old.side == key.side
                && matches!(phase, JobPhase::Queued | JobPhase::Held)
                && (old.observed_at, job.id()) < (key.observed_at, candidate.id())
            {
                earlier = true;
            }
            let mut cooling_rows = request
                .rows()
                .iter()
                .filter(|row| {
                    previous.rows().iter().any(|old_row| {
                        row.commodity() == old_row.commodity()
                            && job
                                .row_result(old_row.id())
                                .is_none_or(|result| result.retains_cooldown())
                    })
                })
                .peekable();
            if cooling_rows.peek().is_some()
                && (matches!(
                    phase,
                    JobPhase::Succeeded | JobPhase::PartiallyAccepted | JobPhase::OutcomeUnknown
                ) || matches!(
                    job.result()
                        .map(crate::api::model::SubmissionResult::final_result),
                    Some(
                        SubmissionResult::Held(crate::api::model::SubmissionHoldReason::Cooldown)
                            | SubmissionResult::DuplicateEvidence(_)
                    )
                ))
            {
                let Some(attempt) = job.attempts().last() else {
                    return Err(ScheduleDecision::Unavailable);
                };
                let Some(ended_at) = attempt.ended_at() else {
                    return Err(ScheduleDecision::Unavailable);
                };
                let origin = match job.result() {
                    Some(
                        SubmissionResult::ProductionSucceeded(_)
                        | SubmissionResult::ProductionAccepted(_),
                    ) => attempt.date_added(),
                    Some(
                        SubmissionResult::AccountRefusal { .. }
                        | SubmissionResult::DryRunSucceeded
                        | SubmissionResult::Unknown
                        | SubmissionResult::Partial(_)
                        | SubmissionResult::NotProcessed(_)
                        | SubmissionResult::Returned(_)
                        | SubmissionResult::Paused(_)
                        | SubmissionResult::Held(_)
                        | SubmissionResult::Rejected(_)
                        | SubmissionResult::NeedsOutcomeCheck { .. }
                        | SubmissionResult::PartialError { .. }
                        | SubmissionResult::Rechecked { .. }
                        | SubmissionResult::RateLimitedCode(_)
                        | SubmissionResult::DuplicateEvidence(_)
                        | SubmissionResult::PausedCode { .. },
                    )
                    | Some(SubmissionResult::VersionLockedOut(_))
                    | None => ended_at,
                };
                let Some(until) = origin.checked_add(COOLDOWN) else {
                    return Err(ScheduleDecision::Unavailable);
                };
                if until > now {
                    for row in cooling_rows {
                        deadlines
                            .entry(row.id().0)
                            .and_modify(|current| *current = (*current).max(until))
                            .or_insert(until);
                    }
                }
            }
        }
        if unresolved_newer {
            Err(ScheduleDecision::UnresolvedNewerObservation)
        } else if earlier {
            Err(ScheduleDecision::EarlierObservationPending)
        } else {
            let mut ready = Vec::new();
            let mut cooling = Vec::new();
            for row in request.rows() {
                match deadlines.get(&row.id().0) {
                    Some(until) => cooling.push(RowCooldown {
                        row: row.id(),
                        until: *until,
                    }),
                    None => ready.push(row.id()),
                }
            }
            Ok(CooldownPartition { ready, cooling })
        }
    }
}
