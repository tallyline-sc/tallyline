// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Passive release notices and daily-check preferences.

use crate::api::{
    Catalogue, ShellState,
    model::{SettingsCommand, UserAction},
};
use tallyline_workflows::api::model::{UpdateChecks, UpdateStatus};

pub(crate) fn notice(
    ui: &mut egui::Ui,
    state: &mut ShellState,
    catalogue: &Catalogue,
) -> Vec<SettingsCommand> {
    let mut commands = Vec::new();
    if let UpdateStatus::Available(version) = state.update_status() {
        let url = version.release_url();
        let version = version.to_string();
        ui.group(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(catalogue.text("update.available"));
                ui.label(version);
            });
            ui.hyperlink_to(catalogue.text("update.open"), url);
            if ui
                .button(tallyline_egui::button_text(
                    catalogue.text("update.dismiss"),
                ))
                .clicked()
            {
                commands.extend(state.act(UserAction::DismissUpdate));
            }
        });
    }
    commands
}

pub(crate) fn settings(
    ui: &mut egui::Ui,
    state: &mut ShellState,
    catalogue: &Catalogue,
) -> Vec<SettingsCommand> {
    ui.heading(tallyline_egui::TypeStyle::HEADING_2.text(catalogue.text("update.title")));
    ui.label(catalogue.text("update.explanation"));
    let mut enabled = state.settings().update_checks() == UpdateChecks::Enabled;
    let mut commands = Vec::new();
    let toggle = ui.add_enabled(
        state.ready() && !state.busy(),
        egui::Checkbox::new(&mut enabled, catalogue.text("update.enable")),
    );
    crate::control_focus::reveal(ui, &toggle);
    if toggle.changed() {
        commands.extend(state.act(UserAction::ChangeUpdateChecks(if enabled {
            UpdateChecks::Enabled
        } else {
            UpdateChecks::Disabled
        })));
    }
    let key = match state.update_status() {
        UpdateStatus::NotChecked => "update.not_checked",
        UpdateStatus::Current => "update.current",
        UpdateStatus::Dismissed => "update.dismissed",
        UpdateStatus::Available(_) => "update.available",
        UpdateStatus::Unavailable => "update.unavailable",
    };
    ui.label(catalogue.text(key));
    commands
}
