// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Keyboard review of exact prices and explicit inventory statuses.

use crate::api::Catalogue;
use crate::api::model::{
    ManualReviewAction, ManualReviewCommand, ManualReviewState, ReviewDirection,
};
use crate::manual_review_details::{price_details, status_details};
use tallyline_egui::TypeStyle;
use tallyline_reporting::api::model::ReviewField;

/// Draws committed manual assessments and returns explicit focus or confirmation intent.
/// Confirmation intent must pass through the user-requested confirmation dialog before dispatch.
pub fn show_manual_review(
    ui: &mut egui::Ui,
    state: &mut ManualReviewState,
    catalogue: &Catalogue,
    theme: tallyline_egui::Theme,
) -> Option<ManualReviewCommand> {
    ui.heading(TypeStyle::HEADING_2.text(catalogue.text("review.title")));
    if state.updating() {
        ui.label(catalogue.text("review.updating"));
    }
    let Some(review) = state.review().cloned() else {
        ui.label(catalogue.text("review.pending"));
        return None;
    };
    let summary = review.summary();
    ui.label(format!(
        "{} {} · {} {} · {} {} · {} {}",
        summary.major,
        catalogue.text("review.major_count"),
        summary.minor,
        catalogue.text("review.minor_count"),
        summary.needs_confirmation,
        catalogue.text("review.required_count"),
        summary.unreviewed,
        catalogue.text("review.unreviewed_count")
    ));
    ui.label(catalogue.text("review.scope"));
    let mut direction = ui.input(|input| {
        input
            .key_pressed(egui::Key::F8)
            .then_some(if input.modifiers.shift {
                ReviewDirection::Previous
            } else {
                ReviewDirection::Next
            })
    });
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(
                summary.needs_confirmation > 0,
                egui::Button::new(tallyline_egui::button_text(
                    catalogue.text("review.previous"),
                )),
            )
            .clicked()
        {
            direction = Some(ReviewDirection::Previous);
        }
        if ui
            .add_enabled(
                summary.needs_confirmation > 0,
                egui::Button::new(tallyline_egui::button_text(catalogue.text("review.next"))),
            )
            .clicked()
        {
            direction = Some(ReviewDirection::Next);
        }
    });
    let mut command = direction.and_then(|direction| state.navigate(direction));
    let focus = command.as_ref().and_then(|command| match command.action {
        ManualReviewAction::Focus(field) => Some(field),
        ManualReviewAction::Reassess | ManualReviewAction::Confirm(_) => None,
    });
    for row in review.report().rows() {
        if let Some(assessment) = review
            .prices()
            .iter()
            .find(|assessment| assessment.row() == row.id())
        {
            ui.push_id(("price-review", row.id().0), |ui| {
                ui.separator();
                ui.label(TypeStyle::NUMERALS_STRONG.text(format!(
                    "{} {}: {}",
                    catalogue.text("review.price"),
                    row.commodity().0,
                    row.price()
                        .map(crate::number_text::price)
                        .unwrap_or_default()
                )));
                price_details(ui, *assessment, catalogue, theme);
                decision(
                    ui,
                    state,
                    Decision {
                        field: ReviewField::Price(row.id()),
                        commodity: row.commodity().0,
                        approved: row.price_confirmation().is_some(),
                        revoked: row.price_confirmation_revoked(),
                        unreviewed: row.price_unreviewed(),
                    },
                    focus,
                    catalogue,
                    &mut command,
                );
            });
        }
        if let Some(assessment) = review
            .stocks()
            .iter()
            .find(|assessment| assessment.row() == row.id())
        {
            ui.push_id(("stock-review", row.id().0), |ui| {
                ui.separator();
                ui.label(TypeStyle::NUMERALS_STRONG.text(format!(
                    "{} {}: {} {}",
                    catalogue.text("review.stock"),
                    row.commodity().0,
                    row.stock()
                        .map(crate::number_text::quantity)
                        .unwrap_or_default(),
                    catalogue.text("review.scu")
                )));
                crate::manual_review_details::stock_details(ui, *assessment, catalogue, theme);
                decision(
                    ui,
                    state,
                    Decision {
                        field: ReviewField::Stock(row.id()),
                        commodity: row.commodity().0,
                        approved: row.stock_confirmation().is_some(),
                        revoked: row.stock_confirmation_revoked(),
                        unreviewed: row.stock_unreviewed(),
                    },
                    focus,
                    catalogue,
                    &mut command,
                );
            });
        }
        if let Some(assessment) = review
            .statuses()
            .iter()
            .find(|assessment| assessment.row() == row.id())
        {
            ui.push_id(("status-review", row.id().0), |ui| {
                ui.separator();
                ui.label(format!(
                    "{} {}: {}",
                    catalogue.text("review.status"),
                    row.commodity().0,
                    row.status()
                        .map(|status| status.to_string())
                        .unwrap_or_default()
                ));
                status_details(ui, *assessment, catalogue, theme);
                decision(
                    ui,
                    state,
                    Decision {
                        field: ReviewField::Status(row.id()),
                        commodity: row.commodity().0,
                        approved: row.status_confirmation().is_some(),
                        revoked: row.status_confirmation_revoked(),
                        unreviewed: row.status_unreviewed(),
                    },
                    focus,
                    catalogue,
                    &mut command,
                );
            });
        }
    }
    command
}

struct Decision {
    field: ReviewField,
    commodity: u32,
    approved: bool,
    revoked: bool,
    unreviewed: bool,
}
fn decision(
    ui: &mut egui::Ui,
    state: &mut ManualReviewState,
    decision: Decision,
    focus: Option<ReviewField>,
    catalogue: &Catalogue,
    command: &mut Option<ManualReviewCommand>,
) {
    let Decision {
        field,
        commodity,
        approved,
        revoked,
        unreviewed,
    } = decision;
    if unreviewed {
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(24.0, 24.0), egui::Sense::hover());
            ui.painter().circle_stroke(
                rect.center(),
                6.0,
                egui::Stroke::new(1.5, ui.visuals().text_color()),
            );
            ui.label(catalogue.text("review.unreviewed"));
        });
    }
    if revoked {
        ui.label(catalogue.text("review.revoked"));
    }
    if approved {
        ui.label(catalogue.text("review.confirmed"));
    }
    if state.needs_confirmation(field) {
        ui.label(TypeStyle::BODY_STRONG.text(catalogue.text("review.required")));
        let key = match field {
            ReviewField::Price(_) => "review.confirm_price",
            ReviewField::Status(_) => "review.confirm_status",
            ReviewField::Stock(_) => "review.confirm_stock",
        };
        let Some(report) = state.review().map(|review| review.report().id()) else {
            return;
        };
        let kind = match field {
            ReviewField::Price(_) => "price",
            ReviewField::Status(_) => "status",
            ReviewField::Stock(_) => "stock",
        };
        let response = ui
            .scope_builder(
                egui::UiBuilder::new().scope_id(egui::Id::unique((
                    "manual-review-action",
                    report.0,
                    kind,
                    field.row().0,
                ))),
                |ui| {
                    ui.button(tallyline_egui::button_text(format!(
                        "{} {commodity}",
                        catalogue.text(key)
                    )))
                },
            )
            .inner;
        if focus == Some(field) {
            response.request_focus();
            response.scroll_to_me(Some(egui::Align::Center));
        }
        if response.clicked() {
            *command = state.confirm(field);
        } else if response.gained_focus() && command.is_none() {
            *command = state.focus(field);
        }
    }
}
