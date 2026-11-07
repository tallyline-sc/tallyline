// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Non-authorising queue wait projection over committed job identities and revisions.

use crate::api::model::{JobId, SendHoldReason};
use tallyline_shared_kernel::api::model::Timestamp;

/// Why a complete queued Report currently waits without changing its released snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubmissionWaitReason {
    /// This session has not revalidated the released account.
    AccountUnavailable,
    /// The accepted session belongs to another account.
    AccountChanged,
    /// The one global submission slot is occupied.
    InFlight,
    /// At least one exact row key remains in cooldown until this time.
    CooldownUntil(Timestamp),
    /// An older queued observation sharing a row key precedes this job.
    EarlierObservationPending,
    /// The complete Report does not fit the production row window yet.
    RowBudget,
    /// Independently dated account, target or service facts currently hold dispatch.
    Context(SendHoldReason),
    /// A newer accepted observation requires renewed review.
    NewerObservationReview,
    /// A newer possibly processed observation remains unresolved.
    UnresolvedNewerObservation,
    /// Required local state is unavailable; no send is authorised.
    Unavailable,
}
/// A passive exact-version wait, separate from the durable aggregate phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SubmissionWait {
    /// Committed local job identity.
    pub job: JobId,
    /// Committed revision for which this projection was assessed.
    pub revision: u64,
    /// The current whole-Report wait reason.
    pub reason: SubmissionWaitReason,
    /// Scheduled context recheck, when one exists; never a promised send time.
    pub next_check: Option<Timestamp>,
}
