// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Durable future-release mode and tighten-only observation-age choices.

use crate::api::model::{SettingsCommand, UserAction};
use crate::api::{Catalogue, ShellState};
use tallyline_shared_kernel::api::model::{ObservationAgeSettings, SubmissionMode};

/// Draws explicit preferences; the visible saved settings change only after worker acknowledgement.
pub fn show_submission_settings(
    ui: &mut egui::Ui,
    state: &mut ShellState,
    catalogue: &Catalogue,
) -> Vec<SettingsCommand> {
    let mut commands = Vec::new();
    ui.heading(tallyline_egui::TypeStyle::HEADING_2.text(catalogue.text("settings.submission")));
    ui.label(catalogue.text("settings.submission_explanation"));
    for (mode, key) in [
        (None, "settings.mode_default"),
        (Some(SubmissionMode::DryRun), "settings.mode_dry"),
        (Some(SubmissionMode::Production), "settings.mode_production"),
    ] {
        let choice = ui.add_enabled(
            state.ready() && !state.busy(),
            egui::RadioButton::new(
                state.settings().submission_mode_override() == mode,
                catalogue.text(key),
            ),
        );
        crate::control_focus::reveal(ui, &choice);
        if choice.clicked() {
            commands.extend(state.act(UserAction::ChangeSubmissionMode(mode)));
        }
    }
    ui.label(catalogue.text("settings.age_explanation"));
    let milliseconds = state.settings().observation_age().maximum().as_millis();
    ui.label(format!(
        "{} {}.{:03} {}",
        catalogue.text("settings.current_age"),
        milliseconds / 1000,
        milliseconds % 1000,
        catalogue.text("settings.seconds")
    ));
    for minutes in [1, 5, 15, 30, 60] {
        let Ok(age) = ObservationAgeSettings::new(std::time::Duration::from_secs(minutes * 60))
        else {
            continue;
        };
        let choice = ui.add_enabled(
            state.ready() && !state.busy(),
            egui::RadioButton::new(
                state.settings().observation_age() == age,
                format!("{minutes} {}", catalogue.text("settings.minutes")),
            ),
        );
        crate::control_focus::reveal(ui, &choice);
        if choice.clicked() {
            commands.extend(state.act(UserAction::ChangeObservationAge(age)));
        }
    }
    commands
}
