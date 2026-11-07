// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Main navigation and setup guards.

use crate::api::model::View;
use crate::api::{Catalogue, ShellState};
use crate::shell_metrics as metrics;
use tallyline_egui::symbols;

pub(crate) fn show(
    ui: &mut egui::Ui,
    state: &mut ShellState,
    catalogue: &Catalogue,
    expanded: bool,
) {
    let theme = crate::api::resolve_theme(state.settings().theme(), ui.ctx().system_theme());
    let selected_fill = theme
        .palette()
        .color("accent-subtle-bg")
        .unwrap_or(ui.visuals().selection.bg_fill);
    ui.spacing_mut().button_padding = egui::Vec2::splat(metrics::CONTROL_PADDING);
    egui::ScrollArea::vertical()
        .id_salt("navigation-scroll")
        .show(ui, |ui| {
            for (view, key, glyph) in [
                (View::Home, "menu.home", symbols::INBOX),
                (View::Reference, "reference.title", symbols::VIEW_LIST),
                (View::Manual, "manual.title", symbols::EDIT_NOTE),
                (View::Queue, "queue.title", symbols::HISTORY),
                (View::Settings, "menu.settings", symbols::SETTINGS),
            ] {
                let enabled = view != View::Home || state.settings().onboarding_complete();
                let label = catalogue.text(key);
                let mut control = button(ui, glyph, label, expanded).selected(state.view() == view);
                if state.view() == view {
                    control = control.fill(selected_fill);
                }
                let response = ui.push_id(key, |ui| ui.add_enabled(enabled, control)).inner;
                response.widget_info(|| {
                    egui::WidgetInfo::selected(
                        egui::Role::Button,
                        enabled,
                        state.view() == view,
                        label,
                    )
                });
                if response.on_hover_text(label).clicked() {
                    match view {
                        View::Home => state.leave_settings(),
                        View::Reference => state.show_reference(),
                        View::Manual => state.show_manual(),
                        View::Queue => state.show_submission_queue(),
                        View::Settings => state.show_settings(),
                        View::Onboarding => state.return_to_setup(),
                    }
                }
            }
            if !state.settings().onboarding_complete() && state.view() != View::Onboarding {
                let label = catalogue.text("release.back_setup");
                let response = ui
                    .push_id("nav-setup", |ui| {
                        ui.add(button(ui, symbols::UNDO, label, expanded))
                    })
                    .inner;
                response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
                if response.on_hover_text(label).clicked() {
                    state.return_to_setup();
                }
            }
            ui.push_id("nav-help", |ui| {
                let label = catalogue.text("menu.help");
                let response = ui.add(button(ui, symbols::HELP, label, expanded));
                let menu = egui::Popup::menu(&response);
                let restore_focus =
                    menu.is_open() && ui.input(|input| input.key_pressed(egui::Key::Escape));
                menu.show(|ui| {
                    if ui
                        .button(tallyline_egui::button_text(catalogue.text("menu.about")))
                        .clicked()
                    {
                        state.open_about();
                        ui.close();
                    }
                });
                if restore_focus {
                    response.request_focus();
                }
                response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
                response.on_hover_text(label);
            });
        });
}

fn button<'a>(
    ui: &egui::Ui,
    glyph: &'static str,
    label: &'a str,
    expanded: bool,
) -> egui::Button<'a> {
    let icon = egui::RichText::new(glyph).font(egui::FontId::new(
        metrics::ICON,
        egui::FontFamily::Name(symbols::FAMILY.into()),
    ));
    let button = if expanded {
        egui::Button::new((icon, tallyline_egui::button_text(label))).wrap()
    } else {
        egui::Button::new(icon)
    };
    button.min_size(egui::vec2(ui.available_width(), metrics::CONTROL))
}
