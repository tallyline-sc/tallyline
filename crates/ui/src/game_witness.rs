// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Local witness preferences and privacy-safe presentation state.

use tallyline_game::api::model::{GameSettings, WitnessSnapshot};

/// An owned witness result and local channel preference for the settings view.
pub struct GameWitnessView {
    /// Durable game preferences.
    pub settings: GameSettings,
    /// Local-only channel selection, excluded from debug output.
    pub channel_directory: Option<String>,
    /// Sanitised witness facts or a closed unavailability reason.
    pub snapshot: WitnessSnapshot,
}
impl Default for GameWitnessView {
    fn default() -> Self {
        Self {
            settings: GameSettings::default(),
            channel_directory: None,
            snapshot: WitnessSnapshot::Off,
        }
    }
}
impl std::fmt::Debug for GameWitnessView {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GameWitnessView")
            .field("settings", &self.settings)
            .field("snapshot", &self.snapshot)
            .finish_non_exhaustive()
    }
}

pub(crate) fn show(
    ui: &mut egui::Ui,
    state: &mut crate::state::ShellState,
    catalogue: &crate::api::Catalogue,
) -> Vec<crate::state::SettingsCommand> {
    use crate::state::UserAction;
    use tallyline_game::api::model::{WitnessMode, WitnessUnavailable};
    let mut commands = Vec::new();
    ui.heading(tallyline_egui::TypeStyle::HEADING_2.text(catalogue.text("witness.title")));
    ui.label(catalogue.text("witness.explanation"));
    let channel_label = ui.label(catalogue.text("witness.channel"));
    let enabled = state.ready() && !state.busy();
    let directory = ui
        .add_enabled(
            enabled,
            egui::TextEdit::singleline(state.game_channel_draft())
                .desired_width(ui.available_width())
                .char_limit(4096),
        )
        .labelled_by(channel_label.id);
    crate::control_focus::reveal(ui, &directory);
    let action = ui.add_enabled(
        enabled,
        egui::Button::new(tallyline_egui::button_text(
            catalogue.text("witness.select_channel"),
        )),
    );
    crate::control_focus::reveal(ui, &action);
    if action.clicked() {
        let channel = state.game_channel_draft().clone();
        commands.extend(state.act(UserAction::ChooseGameChannel(channel)));
    }
    let mut selected = state.game_witness().settings.witness == WitnessMode::On;
    let action = ui.add_enabled(
        enabled,
        egui::Checkbox::new(&mut selected, catalogue.text("witness.enable")),
    );
    crate::control_focus::reveal(ui, &action);
    if action.changed() {
        commands.extend(state.act(UserAction::ChangeWitnessMode(if selected {
            WitnessMode::On
        } else {
            WitnessMode::Off
        })));
    }
    let action = ui.add_enabled(
        enabled,
        egui::Button::new(tallyline_egui::button_text(
            catalogue.text("witness.refresh"),
        )),
    );
    crate::control_focus::reveal(ui, &action);
    if action.clicked() {
        commands.extend(state.act(UserAction::RefreshWitness));
    }
    let key = match &state.game_witness().snapshot {
        WitnessSnapshot::Off => "witness.off",
        WitnessSnapshot::Available(_) => "witness.on",
        WitnessSnapshot::Unavailable(reason) => match reason {
            WitnessUnavailable::UnvalidatedBuild => "witness.unvalidated_build",
            WitnessUnavailable::ReadFailure => "witness.read_failure",
            WitnessUnavailable::UnvalidatedFormat => "witness.unvalidated_format",
            WitnessUnavailable::NotConfigured => "witness.not_configured",
            WitnessUnavailable::InputLimit => "witness.input_limit",
        },
    };
    ui.label(catalogue.text(key));
    if let WitnessSnapshot::Available(facts) = &state.game_witness().snapshot {
        let (major, minor) = facts.major_minor();
        ui.label(format!(
            "{} {major}.{minor}",
            catalogue.text("witness.major_minor")
        ));
        ui.label(format!(
            "{} {}",
            catalogue.text("witness.row_count"),
            facts.rows().len()
        ));
        ui.collapsing(catalogue.text("witness.facts"), |ui| {
            for row in facts.rows().iter().rev().take(50) {
                ui.label(format!(
                    "{} {} · {} {:?} · {} {:?} · {} {:?}",
                    catalogue.text("witness.time"),
                    row.logged_at().millis(),
                    catalogue.text("witness.terminal"),
                    row.terminal().map(|id| id.0),
                    catalogue.text("witness.commodity"),
                    row.commodity().map(|id| id.0),
                    catalogue.text("witness.sizes"),
                    row.container_sizes()
                ));
            }
        });
    }
    commands
}
