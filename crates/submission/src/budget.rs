// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Per-account production row accounting over durable attempts.

use crate::api::model::{AttemptOutcome, InvalidSubmission, NotProcessed, SubmissionRequest};
use tallyline_shared_kernel::api::model::{SubmissionMode, Timestamp};

const MAX_WINDOW_ROWS: usize = 1000;
const WINDOW_MILLIS: u128 = 30 * 60 * 1000;

/// A durable attempt projected without credentials or payload content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountAttempt {
    account: String,
    mode: SubmissionMode,
    started_at: Timestamp,
    rows: usize,
    outcome: Option<AttemptOutcome>,
}
impl AccountAttempt {
    /// Validates an attempt's account and bounded complete row count.
    ///
    /// # Errors
    /// Refuses an absent account, empty attempt or more than five hundred rows.
    pub fn new(
        account: String,
        mode: SubmissionMode,
        started_at: Timestamp,
        rows: usize,
        outcome: Option<AttemptOutcome>,
    ) -> Result<Self, InvalidSubmission> {
        if account.trim().is_empty() || rows == 0 || rows > crate::request::MAX_REPORT_ROWS {
            return Err(InvalidSubmission);
        }
        Ok(Self {
            account,
            mode,
            started_at,
            rows,
            outcome,
        })
    }
}

/// Whether the complete untrimmed Report fits the current row window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowBudgetDecision {
    /// The next attempt can fit the fixed row budget.
    Ready,
    /// The complete Report must wait without silently dropping rows.
    Held,
}

/// The fixed one-thousand-row, thirty-minute production budget.
pub struct SubmissionBudget;
impl SubmissionBudget {
    /// Counts every same-account production attempt except verified connect failures.
    #[must_use]
    pub fn assess(
        request: &SubmissionRequest,
        attempts: &[AccountAttempt],
        now: Timestamp,
    ) -> RowBudgetDecision {
        if request.header().mode == SubmissionMode::DryRun {
            return RowBudgetDecision::Ready;
        }
        let used = attempts
            .iter()
            .filter(|attempt| {
                attempt.account == request.header().account
                    && attempt.mode == SubmissionMode::Production
                    && now.millis().saturating_sub(attempt.started_at.millis()) < WINDOW_MILLIS
                    && attempt.outcome
                        != Some(AttemptOutcome::NotProcessed(NotProcessed::ConnectFailure))
            })
            .fold(request.rows().len(), |total, attempt| {
                total.saturating_add(attempt.rows)
            });
        if used <= MAX_WINDOW_ROWS {
            RowBudgetDecision::Ready
        } else {
            RowBudgetDecision::Held
        }
    }
}
