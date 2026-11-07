// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Persistence-only reconstruction through the aggregate's validated command path.

use crate::api::model::SubmissionResolutionAction;
use crate::api::model::{
    AttemptOutcome, JobId, JobPhase, PayloadHash, SubmissionRequest, SubmissionResult,
};
use crate::api::model::{SendReturnReason, SubmissionAccountEvidence, SubmissionPauseReason};
use tallyline_shared_kernel::api::model::Timestamp;

/// Persistence-only state of an attempt, without credentials or request bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredSubmissionAttempt {
    /// The committed start of the request.
    pub started_at: Timestamp,
    /// The complete untrimmed row count reserved before dispatch.
    pub rows: usize,
    /// Digest of the exact attempted bytes.
    pub payload_hash: PayloadHash,
    /// Whole-second observation time actually encoded in those bytes.
    pub date_added: Timestamp,
    /// Absent while sending; retained for every closed attempt.
    pub ended_at: Option<Timestamp>,
    /// Absent while sending; consistent with the retained result when closed.
    pub outcome: Option<AttemptOutcome>,
    /// A later read-only safety check resolved the stored refusal.
    pub receipt_checked: bool,
    /// Changed account-check actions; absent only in the legacy single-check format.
    pub account_checks: Option<Vec<SubmissionPauseReason>>,
    /// Original account refusal and independent account-read progress.
    pub account_evidence: Option<SubmissionAccountEvidence>,
    /// The compiled newer release that authorised this retried attempt.
    pub client_update: Option<tallyline_shared_kernel::api::model::ReleaseVersion>,
}

/// Persistence-only state of one immutable released submission.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredSubmissionJob {
    /// Stable local job identity.
    pub id: JobId,
    /// Exact optimistic version from the committed row.
    pub revision: u64,
    /// Account, mode and values fixed at manual release.
    pub request: SubmissionRequest,
    /// Durable phase; sending is never converted back to queued here.
    pub phase: JobPhase,
    /// Explicit unknown-resolution actions retained independently of attempts.
    pub resolution_actions: Vec<SubmissionResolutionAction>,
    /// Every committed write-ahead attempt.
    pub attempts: Vec<StoredSubmissionAttempt>,
    /// Retained closed outcome, absent before completion.
    pub result: Option<SubmissionResult>,
    /// A preflight refusal retained independently of transport outcomes.
    pub review_reason: Option<SendReturnReason>,
    /// Queue-only acknowledgement that never authorises replay of this job.
    pub service_pause_acknowledged: bool,
    /// The most recent explicit newer-release recovery decision.
    pub client_update: Option<tallyline_shared_kernel::api::model::ReleaseVersion>,
}
