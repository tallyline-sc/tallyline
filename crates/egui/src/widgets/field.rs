// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Tallyline field widget.

use crate::TypeStyle;
use crate::style::Theme;
use egui::{Color32, Response, Stroke, Ui, Widget};

/// A bordered data field with optional supporting text.
pub struct Field<'a> {
    /// Field label.
    pub label: &'a str,
    /// Current field value.
    pub value: &'a str,
    /// Optional state explanation.
    pub reason: Option<&'a str>,
    /// Theme used to resolve field tokens.
    pub theme: Theme,
}

impl Widget for Field<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let colors = self.theme.palette();
        egui::Frame::new()
            .fill(
                colors
                    .color("surface-sunken")
                    .unwrap_or(Color32::TRANSPARENT),
            )
            .stroke(Stroke::new(
                1.0,
                colors.color("border-strong").unwrap_or(Color32::GRAY),
            ))
            .corner_radius(2)
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                ui.label(TypeStyle::UI_SMALL.text(self.label).weak());
                ui.label(TypeStyle::BODY.text(self.value));
                if let Some(reason) = self.reason {
                    ui.label(TypeStyle::CAPTION.text(reason));
                }
            })
            .response
    }
}
