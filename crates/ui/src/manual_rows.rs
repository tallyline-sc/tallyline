// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Keyboard entry with aged UEX references beside empty manual inputs.

use crate::api::{Catalogue, ShellState};
use crate::manual_frame;
use crate::manual_frame::ManualFrame;
use tallyline_reference_data::api::model::ReferenceSnapshot;
use tallyline_reporting::api::ManualEdit;
use tallyline_shared_kernel::api::model::TradeSide;

const FIELD_WIDTH: f32 = 130.0;

pub(crate) fn show(
    ui: &mut egui::Ui,
    state: &mut ShellState,
    catalogue: &Catalogue,
    snapshot: &ReferenceSnapshot,
    mut actions: Vec<ManualEdit>,
) {
    let Some(report) = state.manual().selected().cloned() else {
        return;
    };
    ui.separator();
    ui.label(format!(
        "{} {}",
        catalogue.text("reference.version"),
        report
            .game_version()
            .unwrap_or(catalogue.text("reference.no_version"))
    ));
    ui.label(format!(
        "{} {}",
        catalogue.text("manual.observed"),
        crate::attachment_views::format_time(report.observed_at())
    ));
    let terminal = state.manual().terminal.unwrap_or(report.key().terminal());
    let side = state.manual().side.unwrap_or(report.key().side());
    let priors = snapshot.terminal_priors(terminal);
    let mut add = None;
    if let Some(priors) = priors {
        for prior in priors.rows().iter().filter(|prior| prior.side() == side) {
            if !report
                .rows()
                .iter()
                .any(|row| row.commodity() == prior.commodity())
                && ui
                    .button(tallyline_egui::button_text(format!(
                        "{} {}",
                        catalogue.text("manual.add"),
                        name(snapshot, prior.commodity())
                    )))
                    .clicked()
            {
                add = Some(prior.commodity());
            }
        }
    }
    ui.add(
        egui::TextEdit::singleline(&mut state.manual_mut().commodity_query)
            .hint_text(catalogue.text("manual.add_search"))
            .id_salt("manual-commodity"),
    )
    .widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::Role::TextInput,
            true,
            catalogue.text("manual.add_search"),
        )
    });
    if !state.manual().commodity_query.is_empty() {
        for commodity in snapshot
            .commodities(&state.manual().commodity_query)
            .into_iter()
        {
            if !report
                .rows()
                .iter()
                .any(|row| row.commodity() == commodity.id())
                && ui
                    .button(tallyline_egui::button_text(format!(
                        "{} {} · UEX ID {}",
                        catalogue.text("manual.add"),
                        commodity.name(),
                        commodity.id().0
                    )))
                    .clicked()
            {
                add = Some(commodity.id());
            }
        }
    }
    if let Some(commodity) = add {
        actions.push(ManualEdit::AddCommodity(commodity));
    }
    let ages: std::collections::BTreeMap<_, _> = priors
        .into_iter()
        .flat_map(|priors| priors.rows())
        .map(|prior| {
            (
                prior.commodity(),
                state
                    .references()
                    .age_seconds(prior.provenance().modified_at),
            )
        })
        .collect();
    let mut frame = ManualFrame::new(actions);
    if field(
        ui,
        &mut state.manual_mut().faction_affinity,
        catalogue.text("manual.affinity").to_owned(),
    ) {
        match manual_frame::optional(&state.manual().faction_affinity) {
            Ok(value) => frame.edits.push(ManualEdit::FactionAffinity(value)),
            Err(()) => frame.invalid = true,
        }
    }
    ui.label(catalogue.text("manual.affinity_help"));
    for row in &mut state.manual_mut().rows {
        ui.push_id(row.id.0, |ui| {
            ui.separator();
            ui.label(name(snapshot, row.commodity));
            let prior = priors.and_then(|priors| {
                priors
                    .rows()
                    .iter()
                    .find(|prior| prior.commodity() == row.commodity && prior.side() == side)
            });
            if let Some(prior) = prior {
                let values = prior.values();
                let age = ages.get(&row.commodity).copied().flatten().map_or_else(
                    || catalogue.text("reference.age_unknown").to_owned(),
                    |age| {
                        format!(
                            "{} {age} {}",
                            catalogue.text("reference.age"),
                            catalogue.text("reference.seconds")
                        )
                    },
                );
                ui.label(format!(
                    "{}: {} / {} SCU / {} · {}",
                    catalogue.text("manual.reference"),
                    values.price,
                    values.stock,
                    values
                        .status
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "?".to_owned()),
                    age
                ));
                if ui
                    .button(tallyline_egui::button_text(
                        catalogue.text("manual.takeover"),
                    ))
                    .clicked()
                {
                    frame.actions.push(ManualEdit::TakeOver(row.id));
                }
            } else {
                ui.label(catalogue.text("manual.no_reference"));
            }
            ui.horizontal_wrapped(|ui| {
                if numeric_field(
                    ui,
                    &mut row.price,
                    tallyline_egui::NumericUnit::AuecPerScu,
                    format!("{} {}", catalogue.text("manual.price_for"), row.commodity.0),
                ) {
                    match manual_frame::optional(&row.price) {
                        Ok(value) => frame.edits.push(ManualEdit::Price(row.id, value)),
                        Err(()) => frame.invalid = true,
                    }
                }
                if numeric_field(
                    ui,
                    &mut row.stock,
                    tallyline_egui::NumericUnit::Scu,
                    format!("{} {}", catalogue.text("manual.scu_for"), row.commodity.0),
                ) {
                    match manual_frame::optional(&row.stock) {
                        Ok(value) => frame.edits.push(ManualEdit::Stock(row.id, value)),
                        Err(()) => frame.invalid = true,
                    }
                }
                if field(
                    ui,
                    &mut row.status,
                    format!(
                        "{} {}",
                        catalogue.text("manual.status_for"),
                        row.commodity.0
                    ),
                ) {
                    match manual_frame::optional(&row.status) {
                        Ok(value) => frame.edits.push(ManualEdit::Status(row.id, value)),
                        Err(()) => frame.invalid = true,
                    }
                }
            });
        });
    }
    frame.finish(
        state.manual_mut(),
        &report,
        snapshot,
        catalogue.text("manual.invalid"),
    );
    if let Some(levels) = snapshot.status_levels() {
        let levels = match side {
            TradeSide::Buy => &levels.buy,
            TradeSide::Sell => &levels.sell,
        };
        ui.label(
            levels
                .iter()
                .map(|level| format!("{}: {}", level.code, level.name))
                .collect::<Vec<_>>()
                .join(" · "),
        );
    }
}
fn name(
    snapshot: &ReferenceSnapshot,
    commodity: tallyline_shared_kernel::api::model::CommodityId,
) -> String {
    snapshot
        .commodities("")
        .iter()
        .find(|item| item.id() == commodity)
        .map_or_else(
            || format!("UEX ID {}", commodity.0),
            |item| format!("{} · UEX ID {}", item.name(), commodity.0),
        )
}
fn field(ui: &mut egui::Ui, input: &mut String, label: String) -> bool {
    ui.allocate_ui_with_layout(
        egui::vec2(FIELD_WIDTH, 0.0),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            ui.label(&label);
            let response = ui.add(
                egui::TextEdit::singleline(input)
                    .desired_width(FIELD_WIDTH)
                    .id_salt(&label),
            );
            response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, &label));
            crate::control_focus::reveal(ui, &response);
            response.lost_focus() && ui.is_enabled()
        },
    )
    .inner
}
fn numeric_field(
    ui: &mut egui::Ui,
    input: &mut String,
    unit: tallyline_egui::NumericUnit,
    label: String,
) -> bool {
    let response = ui.add(tallyline_egui::NumericField {
        id_salt: &label,
        label: &label,
        value: input,
        unit,
    });
    crate::control_focus::reveal(ui, &response);
    response.lost_focus() && ui.is_enabled()
}
