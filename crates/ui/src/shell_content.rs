// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Existing page contents and command gathering.

use crate::api::model::{SettingsCommand, View};
use crate::api::{Catalogue, FanKitUnit, ShellState};

pub(crate) fn show(
    ui: &mut egui::Ui,
    state: &mut ShellState,
    catalogue: &Catalogue,
    unit: &FanKitUnit,
) -> Vec<SettingsCommand> {
    let mut commands = Vec::new();
    egui::ScrollArea::vertical()
        .animated(false)
        .id_salt("shell-page-scroll")
        .show(ui, |ui| match state.view() {
            View::Onboarding => {
                if let Some(action) = crate::api::show_onboarding_view(ui, state, catalogue, unit) {
                    commands.extend(state.act(action));
                }
                let theme =
                    crate::api::resolve_theme(state.settings().theme(), ui.ctx().system_theme());
                crate::credentials::show(ui, state.credentials_mut(), catalogue, theme);
                ui.label(catalogue.text("release.onboarding_test"));
                if ui
                    .add_enabled(
                        state.ready(),
                        egui::Button::new(tallyline_egui::button_text(
                            catalogue.text("release.prepare_test"),
                        )),
                    )
                    .clicked()
                {
                    state.show_manual();
                }
            }
            View::Queue => {
                if let Some(feedback) = &state.submission.feedback {
                    crate::submission_feedback::show(ui, feedback, catalogue);
                }
                let theme =
                    crate::api::resolve_theme(state.settings().theme(), ui.ctx().system_theme());
                let command = ui
                    .add_enabled_ui(!state.submission_busy(), |ui| {
                        crate::api::show_submission_queue(
                            ui,
                            &mut state.submission.queue,
                            catalogue,
                            theme,
                        )
                    })
                    .inner;
                if let Some(command) = command {
                    state.queue_cancel_intent(command);
                }
            }
            View::Manual => crate::manual_views::show_manual_editor(ui, state, catalogue),
            View::Reference => crate::reference_views::show_browser_contents(ui, state, catalogue),
            View::Home => {
                ui.heading(catalogue.text("home.title"));
                ui.label(catalogue.text("home.empty"));
            }
            View::Settings => {
                let theme =
                    crate::api::resolve_theme(state.settings().theme(), ui.ctx().system_theme());
                commands.extend(crate::settings_page::show(ui, state, catalogue, theme));
            }
        });
    commands
}
