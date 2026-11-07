// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Single-column topic panels for existing desktop preferences.

use crate::api::model::{SettingsCommand, UserAction};
use crate::api::{Catalogue, ShellState};
use tallyline_egui::TypeStyle;
use tallyline_workflows::api::model::Theme;

type Section = fn(&mut egui::Ui, &mut ShellState, &Catalogue) -> Vec<SettingsCommand>;

pub(crate) fn show(
    ui: &mut egui::Ui,
    state: &mut ShellState,
    catalogue: &Catalogue,
    theme: tallyline_egui::Theme,
) -> Vec<SettingsCommand> {
    let mut commands = Vec::new();
    ui.heading(TypeStyle::HEADING_1.text(catalogue.text("settings.title")));
    let sections: [(&str, Section); 6] = [
        ("settings.theme", self::show_theme),
        ("settings.stock_title", crate::stock_settings::show),
        ("settings.submission", crate::api::show_submission_settings),
        ("update.title", crate::updates::settings),
        ("reference.mapping", crate::reference_views::show_mapping),
        ("witness.title", crate::game_witness::show),
    ];
    for (key, show) in sections {
        ui.scope_builder(
            egui::UiBuilder::new()
                .id_salt(key)
                .accessibility_role(egui::accesskit::Role::Group)
                .accessibility_label(catalogue.text(key)),
            |ui| {
                ui.add(tallyline_egui::Panel {
                    theme,
                    content: &mut |ui| {
                        ui.set_width(ui.available_width());
                        commands.extend(show(ui, state, catalogue));
                    },
                });
            },
        );
        ui.add_space(12.0);
    }
    let done = ui.button(tallyline_egui::button_text(catalogue.text("action.done")));
    crate::control_focus::reveal(ui, &done);
    if done.clicked() {
        state.leave_settings();
    }
    let account = egui::CollapsingHeader::new(catalogue.text("credentials.title")).show(ui, |ui| {
        crate::credentials::show(ui, state.credentials_mut(), catalogue, theme);
    });
    crate::control_focus::reveal(ui, &account.header_response);
    commands
}

fn show_theme(
    ui: &mut egui::Ui,
    state: &mut ShellState,
    catalogue: &Catalogue,
) -> Vec<SettingsCommand> {
    let mut commands = Vec::new();
    ui.heading(TypeStyle::HEADING_2.text(catalogue.text("settings.theme")));
    for (theme, key) in [
        (Theme::System, "settings.system"),
        (Theme::Dark, "settings.dark"),
        (Theme::Light, "settings.light"),
        (Theme::HighContrast, "settings.high_contrast"),
    ] {
        let choice = ui.add_enabled(
            state.ready() && !state.busy(),
            egui::RadioButton::new(state.settings().theme() == theme, catalogue.text(key)),
        );
        crate::control_focus::reveal(ui, &choice);
        if choice.clicked() {
            commands.extend(state.act(UserAction::ChooseTheme(theme)));
        }
    }
    commands
}
