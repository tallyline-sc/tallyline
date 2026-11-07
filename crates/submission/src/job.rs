// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Immutable submission jobs and persistence-only state.

mod client_update;
mod completion;
mod receipt_recovery;
mod reconstitution;
mod storage;
mod submission_job;
mod submission_result;

pub use storage::{StoredSubmissionAttempt, StoredSubmissionJob};
pub use submission_job::{
    AttemptOutcome, JobId, JobPhase, PayloadHash, QueueRefusal, SubmissionAttempt, SubmissionJob,
};
pub use submission_result::{
    NotProcessed, SubmissionHoldReason, SubmissionPauseReason, SubmissionResult,
    SubmissionReturnReason,
};

mod account_check;
