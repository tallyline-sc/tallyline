// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Explicit account-switch choice over immutable committed queued jobs.

use crate::api::{Catalogue, ShellState};
use tallyline_submission::api::model::{AccountState, JobPhase, SubmissionJob};

#[derive(Default)]
pub(crate) struct SubmissionAccountChoice {
    acknowledged: Option<String>,
    prompt: Option<String>,
}
impl SubmissionAccountChoice {
    pub(crate) fn changed(&mut self, previous: &AccountState, current: &AccountState) {
        if username(previous) != username(current) {
            self.acknowledged = None;
            self.prompt = None;
        }
    }
    pub(crate) fn refresh(&mut self, account: &AccountState, jobs: &[SubmissionJob]) {
        let Some(current) = username(account) else {
            self.prompt = None;
            return;
        };
        if self.acknowledged.as_deref() != Some(current)
            && jobs.iter().any(|job| {
                matches!(
                    job.phase(),
                    JobPhase::Queued | JobPhase::Held | JobPhase::Paused
                ) && job.request().header().account != current
            })
        {
            self.prompt = Some(current.to_owned());
        } else {
            self.prompt = None;
        }
    }
}
fn username(account: &AccountState) -> Option<&str> {
    match account {
        AccountState::Validated { username, .. } => Some(username),
        AccountState::Unconfigured | AccountState::Rejected(_) => None,
    }
}
impl ShellState {
    /// Borrows committed job views without granting authority to alter their released accounts.
    #[must_use]
    pub fn submission_jobs(&self) -> &[SubmissionJob] {
        self.submission.queue.jobs()
    }
}
/// Offers explicit keep/review choices while preserving jobs, view and focus on background changes.
pub fn show_submission_account_choice(
    ui: &mut egui::Ui,
    state: &mut ShellState,
    catalogue: &Catalogue,
) {
    if state.submission.accounts.prompt.is_none() {
        return;
    }
    ui.label(catalogue.text("queue.account_choice"));
    ui.horizontal_wrapped(|ui| {
        if ui
            .button(tallyline_egui::button_text(
                catalogue.text("queue.review_accounts"),
            ))
            .clicked()
        {
            state.show_submission_queue();
        }
        if ui
            .button(tallyline_egui::button_text(
                catalogue.text("queue.keep_accounts"),
            ))
            .clicked()
        {
            state.submission.accounts.acknowledged = state.submission.accounts.prompt.take();
        }
    });
}
