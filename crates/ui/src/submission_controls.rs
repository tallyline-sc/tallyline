// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Explicit full-gate release controls; the worker owns all readiness decisions.

use crate::api::model::QueueCommand;
use crate::api::model::{SubmissionControl, SubmissionIntent};
use crate::api::{Catalogue, ShellState};
use tallyline_shared_kernel::api::model::SubmissionMode;
use tallyline_submission::api::model::AccountState;

#[derive(Default)]
pub(crate) struct SubmissionControlState {
    pub(crate) accounts: crate::submission_accounts::SubmissionAccountChoice,
    pub(crate) pending: Option<SubmissionControl>,
    pub(crate) busy: bool,
    pub(crate) cancel: Option<(tallyline_submission::api::model::JobId, u64)>,
    pub(crate) release_build: bool,
    pub(crate) scope: Option<(String, SubmissionMode)>,
    pub(crate) queue: crate::api::model::SubmissionQueue,
    pub(crate) feedback: Option<crate::api::model::SubmissionFeedback>,
    pub(crate) background_feedback: Option<crate::api::model::SubmissionFeedback>,
}

impl ShellState {
    /// Resolves visible global mode from confirmed settings and injected build presentation.
    #[must_use]
    pub fn submission_mode(&self) -> SubmissionMode {
        let fallback = if self.submission.release_build && self.settings().onboarding_complete() {
            SubmissionMode::Production
        } else {
            SubmissionMode::DryRun
        };
        self.settings().submission_mode(fallback)
    }
    /// Reports whether an explicit queue control is waiting for its own completion.
    #[must_use]
    pub const fn submission_busy(&self) -> bool {
        self.submission.busy
    }
    /// Drains the single immutable user intent for the bounded session-owner channel.
    pub fn take_submission_command(&mut self) -> Option<SubmissionControl> {
        self.submission.pending.take()
    }
    /// Retains a passive actionable notice after failed bounded channel delivery.
    pub fn submission_delivery_failed(&mut self) {
        self.submission.busy = false;
        self.submission.pending = None;
        self.manual_mut().busy = false;
        if let Some((id, expected)) = self.submission.cancel.take() {
            self.submission.queue.failed(
                id,
                expected,
                tallyline_submission::api::model::SubmissionApplicationError::Repository(
                    tallyline_submission::api::model::SubmissionRepositoryError::Unavailable,
                ),
            );
        }
        self.submission.feedback = Some(crate::api::model::SubmissionFeedback::Unavailable);
    }
    /// Opens committed submission history only in response to a user action.
    pub fn show_submission_queue(&mut self) {
        self.show_queue_view();
    }
    pub(crate) fn queue_cancel_intent(&mut self, command: crate::api::model::QueueCommand) {
        if self.submission.busy {
            return;
        }
        let (id, expected_revision, control) = match command {
            QueueCommand::RefreshLifecycle {
                id,
                expected_revision,
                expected_lifecycle,
            } => (
                id,
                expected_revision,
                SubmissionControl::RefreshLifecycle {
                    id,
                    expected: expected_revision,
                    expected_lifecycle,
                },
            ),
            QueueCommand::Split {
                id,
                expected_revision,
            } => (
                id,
                expected_revision,
                SubmissionControl::Split {
                    id,
                    expected: expected_revision,
                },
            ),
            QueueCommand::Resolve {
                id,
                expected_revision,
                action,
            } => (
                id,
                expected_revision,
                SubmissionControl::Resolve {
                    id,
                    expected: expected_revision,
                    action,
                },
            ),
            crate::api::model::QueueCommand::Cancel {
                id,
                expected_revision,
            } => (
                id,
                expected_revision,
                SubmissionControl::Cancel {
                    id,
                    expected: expected_revision,
                },
            ),
            crate::api::model::QueueCommand::Retry {
                id,
                expected_revision,
            } => (
                id,
                expected_revision,
                SubmissionControl::Retry {
                    id,
                    expected: expected_revision,
                },
            ),
        };
        self.submission.pending = Some(control);
        self.submission.cancel = Some((id, expected_revision));
        self.submission.busy = true;
        self.submission.feedback = None;
    }
    fn release_intent(&mut self, intent: SubmissionIntent) {
        if !self.can_release() {
            return;
        }
        let Some(report) = self.manual().selected() else {
            return;
        };
        let AccountState::Validated { username, .. } = self.credentials().account() else {
            return;
        };
        let command = SubmissionControl::Release {
            id: report.id(),
            expected: report.version(),
            account: username.clone(),
            intent,
        };
        let mode = match intent {
            SubmissionIntent::Manual(mode) => mode,
            SubmissionIntent::OnboardingTest => SubmissionMode::DryRun,
        };
        self.submission.scope = Some((username.clone(), mode));
        self.submission.pending = Some(command);
        self.submission.busy = true;
        self.manual_mut().busy = true;
        self.submission.feedback = None;
    }
    fn can_release(&self) -> bool {
        !self.submission.busy
            && self.ready()
            && !self.manual().busy
            && self.manual().selected().is_some()
            && !self.manual().inputs_pending()
            && matches!(self.credentials().account(), AccountState::Validated { .. })
    }
}
/// Draws fixed-source release and test actions without claiming numerical or historical readiness.
pub fn show_submission_controls(ui: &mut egui::Ui, state: &mut ShellState, catalogue: &Catalogue) {
    ui.separator();
    ui.heading(catalogue.text("release.title"));
    let theme = crate::api::resolve_theme(state.settings().theme(), ui.ctx().system_theme());
    crate::api::show_submission_mode(ui, state.submission_mode(), catalogue, theme);
    ui.label(catalogue.text("release.full_gate"));
    if let Some(feedback) = &state.submission.feedback {
        crate::submission_feedback::show(ui, feedback, catalogue);
    }
    let Some(report) = state.manual().selected().cloned() else {
        return;
    };
    ui.label(format!(
        "{} {}",
        catalogue.text("manual.observed"),
        crate::attachment_views::format_time(report.observed_at())
    ));
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(
                state.can_release(),
                egui::Button::new(tallyline_egui::button_text(
                    catalogue.text("release.manual"),
                )),
            )
            .clicked()
        {
            state.release_intent(SubmissionIntent::Manual(state.submission_mode()));
        }
        if ui
            .add_enabled(
                state.can_release(),
                egui::Button::new(tallyline_egui::button_text(catalogue.text("release.test"))),
            )
            .clicked()
        {
            state.release_intent(SubmissionIntent::OnboardingTest);
        }
    });
    show_newer_choices(ui, state, &report, catalogue);
    let needs_age = matches!(&state.submission.feedback, Some(crate::api::model::SubmissionFeedback::Blocked(blocks)) if blocks.contains(&tallyline_reporting::api::model::ReleaseBlockReason::ObservationAgeUnconfirmed));
    if needs_age
        && ui
            .add_enabled(
                !state.submission.busy && !state.manual().busy && !state.manual().inputs_pending(),
                egui::Button::new(tallyline_egui::button_text(
                    catalogue.text("release.confirm_age"),
                )),
            )
            .clicked()
    {
        state.submission.pending = Some(SubmissionControl::ConfirmAge {
            id: report.id(),
            expected: report.version(),
        });
        state.submission.busy = true;
        state.manual_mut().busy = true;
    }
}

fn show_newer_choices(
    ui: &mut egui::Ui,
    state: &mut ShellState,
    report: &tallyline_reporting::api::model::Report,
    catalogue: &Catalogue,
) {
    let Some((account, mode)) = state.submission.scope.clone() else {
        return;
    };
    let Some(crate::api::model::SubmissionFeedback::Blocked(blocks)) = &state.submission.feedback
    else {
        return;
    };
    let choices = blocks
        .iter()
        .filter_map(|block| {
            if let tallyline_reporting::api::model::ReleaseBlockReason::NewerObservationSent {
                row,
                observed_at,
            } = block
            {
                Some((*row, *observed_at))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    for (id, observed_at) in choices {
        let Some(row) = report.rows().iter().find(|row| row.id() == id) else {
            continue;
        };
        let (Some(price), Some(stock), Some(status)) = (row.price(), row.stock(), row.status())
        else {
            continue;
        };
        ui.label(format!(
            "{} {} \u{b7} {} {} \u{b7} {} {} \u{b7} {} {} \u{b7} {} {}",
            catalogue.text("queue.row"),
            id.0,
            catalogue.text("release.commodity"),
            row.commodity().0,
            catalogue.text("release.price"),
            price,
            catalogue.text("release.stock_value"),
            stock,
            catalogue.text("release.status"),
            status
        ));
        ui.label(format!(
            "{} {}",
            catalogue.text("release.newest_observation"),
            crate::attachment_views::format_time(observed_at)
        ));
        ui.label(format!(
            "{} {}",
            catalogue.text("release.account_value"),
            account
        ));
        let theme = crate::api::resolve_theme(state.settings().theme(), ui.ctx().system_theme());
        crate::api::show_submission_mode(ui, mode, catalogue, theme);
        let same_account = matches!(state.credentials().account(),AccountState::Validated {username,..} if username == &account);
        if ui
            .add_enabled(
                state.can_release() && same_account,
                egui::Button::new(tallyline_egui::button_text(format!(
                    "{} {} {}",
                    catalogue.text("release.older_prefix"),
                    id.0,
                    catalogue.text("release.older_suffix")
                ))),
            )
            .clicked()
        {
            let environment = state
                .game_witness()
                .settings
                .environment_mapping
                .environment(report.key().environment());
            if let Ok(evidence) = tallyline_reporting::api::model::NewerObservationEvidence::new(
                &account,
                mode,
                environment,
                observed_at,
            ) {
                state.submission.pending = Some(SubmissionControl::ConfirmNewer {
                    id: report.id(),
                    expected: report.version(),
                    row: id,
                    evidence,
                });
                state.submission.busy = true;
                state.manual_mut().busy = true;
            }
        }
    }
}
