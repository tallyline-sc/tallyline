// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Manual Report entry and keyboard commands.

use crate::api::{Catalogue, ShellState};
use crate::manual_editor::ManualCommand;
use tallyline_reference_data::api::model::ReferenceSnapshot;
use tallyline_reporting::api::ManualEdit;
use tallyline_reporting::api::model::ReportKey;
use tallyline_shared_kernel::api::model::TradeSide;

/// Draws persistent manual Draft entry without requiring account credentials.
pub fn show_manual_editor(ui: &mut egui::Ui, state: &mut ShellState, catalogue: &Catalogue) {
    ui.add_enabled_ui(!state.submission_busy(), |ui| {
        show_contents(ui, state, catalogue)
    });
    crate::api::show_submission_controls(ui, state, catalogue);
}
fn show_contents(ui: &mut egui::Ui, state: &mut ShellState, catalogue: &Catalogue) {
    ui.heading(catalogue.text("manual.title"));
    ui.label(catalogue.text("manual.instructions"));
    if let Some(review) = state.manual().review() {
        let summary = review.summary();
        ui.label(format!(
            "{} {} {} · {} {} · {} {} · {} {}",
            catalogue.text("review.summary_prefix"),
            summary.major,
            catalogue.text("review.major_count"),
            summary.minor,
            catalogue.text("review.minor_count"),
            summary.needs_confirmation,
            catalogue.text("review.required_count"),
            summary.unreviewed,
            catalogue.text("review.unreviewed_count")
        ));
    }
    if let Some(notice) = &state.manual().notice {
        ui.label(notice);
    }
    if state.manual().busy {
        ui.label(catalogue.text("manual.working"));
    }
    let reports: Vec<_> = state
        .manual()
        .reports
        .iter()
        .map(|report| (report.id(), report.key()))
        .collect();
    ui.horizontal_wrapped(|ui| {
        if ui
            .button(tallyline_egui::button_text(catalogue.text("manual.new")))
            .clicked()
        {
            state.manual_mut().new_draft();
        }
        for (id, key) in reports {
            if ui
                .button(tallyline_egui::button_text(format!(
                    "{} {} · {} · {:?}",
                    catalogue.text("manual.resume"),
                    id.0,
                    key.terminal().0,
                    key.environment()
                )))
                .clicked()
            {
                state.manual_mut().select(id);
            }
        }
    });
    ui.add_enabled_ui(!state.manual().busy && state.manual().ready, |ui| {
        let mut actions = Vec::new();
        context(ui, state, catalogue, &mut actions);
        let snapshot = state
            .manual()
            .environment
            .and_then(|channel| {
                state.references().snapshot(
                    state
                        .game_witness()
                        .settings
                        .environment_mapping
                        .environment(channel),
                )
            })
            .cloned();
        if let Some(snapshot) = snapshot {
            terminals(ui, state, catalogue, &snapshot, &mut actions);
            if state.manual().selected().is_some() {
                crate::manual_rows::show(ui, state, catalogue, &snapshot, actions);
            } else {
                create(ui, state, catalogue, &snapshot);
            }
        } else {
            ui.label(catalogue.text("reference.no_data"));
        }
    });
    state.show_review(ui, catalogue);
    let account = state.credentials().account().clone();
    crate::attachment_views::show_manual_attachment_editor(ui, state, catalogue, &account);
}
fn context(
    ui: &mut egui::Ui,
    state: &mut ShellState,
    catalogue: &Catalogue,
    actions: &mut Vec<ManualEdit>,
) {
    let old = state.manual().selected().map(|report| report.key());
    let mut changed = false;
    ui.label(catalogue.text("manual.environment"));
    ui.horizontal_wrapped(|ui| {
        for (channel, key) in crate::reference_views::CHANNELS {
            changed |= ui
                .selectable_value(
                    &mut state.manual_mut().environment,
                    Some(channel),
                    catalogue.text(key),
                )
                .changed();
        }
    });
    ui.horizontal(|ui| {
        ui.label(catalogue.text("manual.side"));
        changed |= ui
            .selectable_value(
                &mut state.manual_mut().side,
                Some(TradeSide::Buy),
                catalogue.text("manual.buy"),
            )
            .changed();
        changed |= ui
            .selectable_value(
                &mut state.manual_mut().side,
                Some(TradeSide::Sell),
                catalogue.text("manual.sell"),
            )
            .changed();
    });
    if changed
        && let Some(old) = old
        && let Some(environment) = state.manual().environment
        && let Some(side) = state.manual().side
        && (old.environment() != environment || old.side() != side)
        && state
            .references()
            .snapshot(
                state
                    .game_witness()
                    .settings
                    .environment_mapping
                    .environment(environment),
            )
            .is_some()
        && let Ok(key) = ReportKey::new(old.terminal(), side, environment)
    {
        actions.push(ManualEdit::ChangeKey(key));
    }
}
fn terminals(
    ui: &mut egui::Ui,
    state: &mut ShellState,
    catalogue: &Catalogue,
    snapshot: &ReferenceSnapshot,
    actions: &mut Vec<ManualEdit>,
) {
    ui.label(catalogue.text("manual.terminal"));
    ui.add(
        egui::TextEdit::singleline(&mut state.manual_mut().terminal_query)
            .hint_text(catalogue.text("reference.terminal_search"))
            .id_salt("manual-terminal"),
    )
    .widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::Role::TextInput,
            true,
            catalogue.text("reference.terminal_search"),
        )
    });
    ui.add(
        egui::TextEdit::singleline(&mut state.manual_mut().location_query)
            .hint_text(catalogue.text("manual.location_filter"))
            .id_salt("manual-location"),
    )
    .widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::Role::TextInput,
            true,
            catalogue.text("manual.location_filter"),
        )
    });
    let recents: Vec<_> = state
        .manual()
        .reports
        .iter()
        .map(|report| report.key().terminal())
        .collect();
    let mut terminals = snapshot.terminals(&state.manual().terminal_query);
    terminals.retain(|terminal| {
        terminal
            .text()
            .location
            .to_lowercase()
            .contains(&state.manual().location_query.to_lowercase())
    });
    terminals.sort_by_key(|terminal| {
        recents
            .iter()
            .position(|id| *id == terminal.id())
            .unwrap_or(usize::MAX)
    });
    egui::ScrollArea::vertical()
        .id_salt("manual-terminal-results")
        .max_height(180.0)
        .show(ui, |ui| {
            for terminal in terminals {
                let label = format!(
                    "{} · {} · UEX ID {}",
                    terminal.text().name,
                    terminal.text().location,
                    terminal.id().0
                );
                if ui
                    .selectable_label(state.manual().terminal == Some(terminal.id()), label)
                    .clicked()
                {
                    state.manual_mut().terminal = Some(terminal.id());
                    state.references_mut().open_terminal(terminal.id());
                    if let Some(report) = state.manual().selected()
                        && let Ok(key) = ReportKey::new(
                            terminal.id(),
                            state.manual().side.unwrap_or(report.key().side()),
                            state
                                .manual()
                                .environment
                                .unwrap_or(report.key().environment()),
                        )
                    {
                        actions.push(ManualEdit::ChangeKey(key));
                    }
                }
            }
        });
}
fn create(
    ui: &mut egui::Ui,
    state: &mut ShellState,
    catalogue: &Catalogue,
    snapshot: &ReferenceSnapshot,
) {
    let key = state
        .manual()
        .terminal
        .zip(state.manual().side)
        .zip(state.manual().environment)
        .and_then(|((terminal, side), environment)| {
            ReportKey::new(terminal, side, environment).ok()
        });
    if ui
        .add_enabled(
            key.is_some(),
            egui::Button::new(tallyline_egui::button_text(catalogue.text("manual.create"))),
        )
        .clicked()
        && let Some(key) = key
    {
        state.manual_mut().queue(ManualCommand::Create {
            key,
            snapshot: snapshot.clone(),
        });
    }
}
