// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Passive inspection of the immutable target, values and attempted observation date.

use crate::api::Catalogue;
use egui::{CollapsingHeader, Ui};
use tallyline_shared_kernel::api::model::{TradeSide, UexEnvironment};
use tallyline_submission::api::model::{SubmissionJob, SubmissionResolutionAction};

pub(crate) fn show(ui: &mut Ui, job: &SubmissionJob, catalogue: &Catalogue) {
    let details = CollapsingHeader::new(catalogue.text("queue.unknown_details")).show(ui, |ui| {
        let header = job.request().header();
        let target = match header.environment {
            UexEnvironment::Live => "reference.uex_live",
            UexEnvironment::Ptu => "reference.uex_ptu",
        };
        let side = match header.side {
            TradeSide::Buy => "manual.buy",
            TradeSide::Sell => "manual.sell",
        };
        ui.label(format!(
            "{} {} · {} · {} · {}",
            catalogue.text("queue.terminal"),
            header.terminal.0,
            catalogue.text(target),
            catalogue.text(side),
            header.game_version
        ));
        for row in job.request().rows() {
            ui.label(format!(
                "{} {} · {} aUEC/SCU · {} SCU · {} {}",
                catalogue.text("queue.commodity"),
                row.commodity().0,
                row.price(),
                row.stock(),
                catalogue.text("queue.row_status"),
                row.status()
            ));
        }
        if let Some(attempt) = job.attempts().last() {
            ui.label(format!(
                "{} {}",
                catalogue.text("queue.attempt_date"),
                crate::attachment_views::format_time(attempt.date_added())
            ));
            ui.label(format!(
                "{} {}",
                catalogue.text("queue.attempt_started"),
                crate::attachment_views::format_time(attempt.started_at())
            ));
            if let Some(ended) = attempt.ended_at() {
                ui.label(format!(
                    "{} {}",
                    catalogue.text("queue.attempt_ended"),
                    crate::attachment_views::format_time(ended)
                ));
            }
        }
        for action in job.resolution_actions() {
            if let SubmissionResolutionAction::Decision { at, .. } = action {
                ui.label(format!(
                    "{} {}",
                    catalogue.text("queue.decision_recorded"),
                    crate::attachment_views::format_time(*at)
                ));
            }
        }
    });
    crate::control_focus::reveal(ui, &details.header_response);
}
