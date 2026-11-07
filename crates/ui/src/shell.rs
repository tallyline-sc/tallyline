// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Persistent desktop shell composition.

use crate::api::model::SettingsCommand;
use crate::api::{Catalogue, FanKitUnit, ShellState};
use crate::shell_metrics as metrics;
use tallyline_egui::TypeStyle;

/// Draws the shell from owned state and returns user-requested worker commands.
pub fn show_shell(
    ui: &mut egui::Ui,
    state: &mut ShellState,
    catalogue: &Catalogue,
    unit: &FanKitUnit,
) -> Vec<SettingsCommand> {
    let expanded = ui.available_width()
        >= metrics::SIDEBAR
            + metrics::MIN_CONTENT_WIDTH
            + 2.0 * f32::from(metrics::CONTENT_PADDING);
    let theme = crate::api::resolve_theme(state.settings().theme(), ui.ctx().system_theme());
    let palette = theme.palette();
    let frame = egui::Frame::new()
        .fill(
            palette
                .color("surface-1")
                .unwrap_or(ui.visuals().panel_fill),
        )
        .inner_margin(egui::Margin::symmetric(
            metrics::CONTENT_PADDING,
            metrics::BAR_PADDING,
        ));
    egui::Panel::top("shell-header")
        .min_size(metrics::TOP_BAR)
        .frame(frame)
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(TypeStyle::HEADING_3.text(catalogue.text("app.name")));
                ui.separator();
                crate::api::show_submission_mode(ui, state.submission_mode(), catalogue, theme);
                let environment = state
                    .game_witness()
                    .settings
                    .environment_mapping
                    .environment(state.references().channel());
                ui.label(TypeStyle::UI_SMALL.text(format!(
                    "{} {}",
                    catalogue.text("reference.environment"),
                    crate::reference_views::environment_name(environment, catalogue)
                )));
            });
        });
    egui::Panel::bottom("shell-status")
        .min_size(metrics::STATUS_BAR)
        .frame(frame)
        .show(ui, |ui| {
            crate::shell_status::summary(ui, state, catalogue);
        });
    egui::Panel::left("shell-navigation")
        .exact_size(if expanded {
            metrics::SIDEBAR
        } else {
            metrics::RAIL
        })
        .resizable(false)
        .frame(
            egui::Frame::new()
                .fill(
                    palette
                        .color("surface-1")
                        .unwrap_or(ui.visuals().panel_fill),
                )
                .inner_margin(metrics::NAV_PADDING),
        )
        .show(ui, |ui| {
            crate::shell_navigation::show(ui, state, catalogue, expanded);
        });
    let commands = egui::CentralPanel::default()
        .frame(
            egui::Frame::new()
                .fill(
                    palette
                        .color("surface-0")
                        .unwrap_or(ui.visuals().faint_bg_color),
                )
                .inner_margin(metrics::CONTENT_PADDING),
        )
        .show(ui, |ui| {
            let mut commands = crate::shell_status::notices(ui, state, catalogue);
            commands.extend(
                ui.push_id("shell-page", |ui| {
                    crate::shell_content::show(ui, state, catalogue, unit)
                })
                .inner,
            );
            commands
        })
        .inner;
    state.show_dialogs(ui.ctx(), catalogue, unit);
    commands
}

/// Wakes the UI once after a channel result arrives.
pub fn wake_up(context: &egui::Context) {
    context.request_repaint();
}
