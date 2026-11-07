// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Explicit independently tighten-only stock preferences.

use crate::api::model::{SettingsCommand, UserAction};
use crate::api::{Catalogue, ShellState};
use tallyline_reporting::api::model::StockSettings;
use tallyline_shared_kernel::api::model::Percent;

pub(crate) struct StockSettingsEditor {
    percent: String,
    minimum: String,
    invalid: bool,
}
impl Default for StockSettingsEditor {
    fn default() -> Self {
        Self::from(StockSettings::default())
    }
}
impl From<StockSettings> for StockSettingsEditor {
    fn from(settings: StockSettings) -> Self {
        Self {
            percent: settings.tolerance().percent().get().to_string(),
            minimum: settings.tolerance().minimum().to_string(),
            invalid: false,
        }
    }
}
pub(crate) fn show(
    ui: &mut egui::Ui,
    state: &mut ShellState,
    catalogue: &Catalogue,
) -> Vec<SettingsCommand> {
    ui.heading(tallyline_egui::TypeStyle::HEADING_2.text(catalogue.text("settings.stock_title")));
    ui.label(catalogue.text("review.policy_scu"));
    ui.label(catalogue.text("settings.stock_explanation"));
    let enabled = state.ready() && !state.busy();
    let editor = &mut state.stock_settings_editor;
    let mut save_pressed = false;
    let candidate = ui
        .add_enabled_ui(enabled, |ui| {
            for (key, salt, unit, value) in [
                (
                    "settings.stock_percent",
                    "settings-stock-percent",
                    tallyline_egui::NumericUnit::Percent,
                    &mut editor.percent,
                ),
                (
                    "settings.stock_minimum",
                    "settings-stock-minimum",
                    tallyline_egui::NumericUnit::Scu,
                    &mut editor.minimum,
                ),
            ] {
                let response = ui.add(tallyline_egui::NumericField {
                    id_salt: salt,
                    label: catalogue.text(key),
                    value,
                    unit,
                });
                crate::control_focus::reveal(ui, &response);
            }
            let save = ui.button(tallyline_egui::button_text(
                catalogue.text("settings.stock_save"),
            ));
            crate::control_focus::reveal(ui, &save);
            if save.clicked() {
                save_pressed = true;
                let settings = editor
                    .percent
                    .trim()
                    .parse::<u32>()
                    .ok()
                    .zip(editor.minimum.trim().parse().ok())
                    .and_then(|(percent, minimum)| {
                        StockSettings::new(Percent::whole(percent), minimum).ok()
                    });
                editor.invalid = settings.is_none();
                settings
            } else {
                None
            }
        })
        .inner;
    if editor.invalid {
        let feedback = ui.label(catalogue.text("settings.stock_invalid"));
        if save_pressed {
            feedback.scroll_to_me_animation(None, egui::style::ScrollAnimation::none());
        }
    }
    candidate
        .and_then(|settings| state.act(UserAction::ChangeStockSettings(settings)))
        .into_iter()
        .collect()
}
