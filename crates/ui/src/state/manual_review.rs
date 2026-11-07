// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Manual review view and dialog composition on owned shell fields.

use super::ShellState;
use crate::api::Catalogue;
use crate::api::model::{ManualCommand, ManualReviewAction};
impl ShellState {
    pub(crate) fn show_review(&mut self, ui: &mut egui::Ui, catalogue: &Catalogue) {
        let Some(report) = self.manual.selected() else {
            return;
        };
        let environment = self
            .game_witness
            .settings
            .environment_mapping
            .environment(report.key().environment());
        let snapshot = self.references.snapshot(environment).cloned();
        if self.manual.inputs_pending() {
            self.manual.review.invalidate();
        }
        if !self.manual.busy
            && let Some(command) = self.manual.review.take_deferred_focus()
        {
            self.manual.queue(ManualCommand::Review(command));
        }
        if let Some(snapshot) = snapshot.as_ref() {
            self.manual.request_review(snapshot);
        }
        let theme = crate::api::resolve_theme(self.settings.theme(), ui.ctx().system_theme());
        ui.add_enabled_ui(!self.manual.inputs_pending(), |ui| {
            if let Some(command) = crate::manual_review_views::show_manual_review(
                ui,
                &mut self.manual.review,
                catalogue,
                theme,
            ) {
                if self.manual.busy {
                    if let ManualReviewAction::Focus(field) = command.action {
                        self.manual.review.defer_focus(field);
                    }
                    return;
                }
                match command.action {
                    ManualReviewAction::Confirm(field) => self
                        .dialogs
                        .open_manual_confirmation(&self.manual.review, field),
                    ManualReviewAction::Focus(_) | ManualReviewAction::Reassess => {
                        self.manual.queue(ManualCommand::Review(command))
                    }
                }
            }
        });
        let entered = self.manual.selected().is_some_and(|report| {
            report
                .rows()
                .iter()
                .any(|row| row.price().is_some() || row.stock().is_some() || row.status().is_some())
        });
        if ui
            .add_enabled(
                snapshot.is_some() && entered && !self.manual.busy && !self.manual.inputs_pending(),
                egui::Button::new(tallyline_egui::button_text(catalogue.text("review.retry"))),
            )
            .clicked()
            && let Some(snapshot) = snapshot.as_ref()
        {
            self.manual.retry_review(snapshot);
        }
        let selected_matches = self.manual.review.review().is_some_and(|review| {
            self.manual.selected().is_some_and(|report| {
                report.id() == review.report().id() && report.version() == review.report().version()
            })
        });
        if !selected_matches || self.manual.busy {
            self.dialogs.manual_confirmation = None;
        }
        let texture = self.manual.attachment.texture.clone();
        let preview_valid = self
            .manual
            .attachment
            .preview
            .as_ref()
            .is_some_and(|preview| {
                self.manual.selected().is_some_and(|report| {
                    self.manual.attachment.preview_report == Some(report.id())
                        && report.attachment().is_some_and(|attachment| {
                            attachment.preview_confirmed()
                                && attachment.binding() == preview.binding()
                        })
                })
            });
        if let Some(command) = self.dialogs.show_manual_confirmation(
            ui.ctx(),
            &self.manual.review,
            catalogue,
            theme,
            |ui| {
                if preview_valid {
                    if let Some(texture) = texture {
                        ui.label(catalogue.text("attachment.full_size"));
                        egui::ScrollArea::both()
                            .id_salt("review-private-preview")
                            .max_height(260.0)
                            .show(ui, |ui| {
                                ui.add(
                                    egui::Image::new((texture.id(), texture.size_vec2()))
                                        .alt_text(catalogue.text("attachment.preview_alt")),
                                );
                            });
                    } else {
                        ui.label(catalogue.text("review.view_attachment"));
                    }
                } else {
                    ui.label(catalogue.text("review.no_preview"));
                }
            },
        ) {
            self.manual.queue(ManualCommand::Review(command));
        }
    }
}
