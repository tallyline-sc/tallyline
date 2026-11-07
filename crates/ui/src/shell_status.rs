// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Persistent status summary and actionable notices.

use crate::api::model::SettingsCommand;
use crate::api::{Catalogue, ShellState};
use tallyline_egui::TypeStyle;
use tallyline_submission::api::model::JobPhase;

pub(crate) fn summary(ui: &mut egui::Ui, state: &mut ShellState, catalogue: &Catalogue) {
    ui.horizontal_wrapped(|ui| {
        ui.label(TypeStyle::UI_SMALL.text(format!(
            "{} {}",
            catalogue.text("queue.count"),
            state.submission_jobs().len()
        )));
        let held = state
            .submission_jobs()
            .iter()
            .filter(|job| job.phase() == JobPhase::Held)
            .count();
        if held > 0
            && ui
                .button(tallyline_egui::button_text(format!(
                    "{}: {held}",
                    catalogue.text("queue.held")
                )))
                .clicked()
        {
            state.show_submission_queue();
        }
        crate::reference_views::show_reference_summary(ui, state, catalogue);
        crate::reference_views::show_reference_age(ui, state, catalogue);
        if !state.ready() && state.notice().is_none() {
            ui.label(catalogue.text("status.starting"));
        }
        if state.busy() {
            ui.label(catalogue.text("status.saving"));
        }
    });
}

pub(crate) fn notices(
    ui: &mut egui::Ui,
    state: &mut ShellState,
    catalogue: &Catalogue,
) -> Vec<SettingsCommand> {
    let mut commands = Vec::new();
    egui::ScrollArea::vertical()
        .id_salt("shell-notices")
        .max_height(ui.available_height() * crate::shell_metrics::NOTICE_HEIGHT_FRACTION)
        .show(ui, |ui| {
            crate::api::show_submission_account_choice(ui, state, catalogue);
            if let Some(feedback) = &state.submission.background_feedback {
                ui.label(catalogue.text("queue.action_required"));
                crate::submission_feedback::show(ui, feedback, catalogue);
            }
            commands.extend(crate::updates::notice(ui, state, catalogue));
            if let Some(notice) = state.notice() {
                ui.label(catalogue.text("status.action_required"));
                ui.label(notice);
            }
            egui::CollapsingHeader::new(catalogue.text("shell.reference_details")).show(ui, |ui| {
                crate::reference_views::show_reference_details(ui, state, catalogue);
            });
            crate::reference_views::show_reference_warnings(ui, state, catalogue);
        });
    ui.separator();
    commands
}
