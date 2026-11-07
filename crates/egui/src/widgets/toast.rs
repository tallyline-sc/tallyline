// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Tallyline toast widget.

use crate::SemanticTone;
use crate::TypeStyle;
use crate::style::Theme;
use egui::{Color32, Response, Stroke, Ui, Widget};

/// A short-lived message with semantic tone.
pub struct Toast<'a> {
    /// Message content.
    pub text: &'a str,
    /// Semantic message tone.
    pub tone: SemanticTone,
    /// Theme used to resolve toast tokens.
    pub theme: Theme,
}

impl Widget for Toast<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let palette = self.theme.palette();
        let (foreground_token, background_token, border_token) = self.tone.tokens();
        let background = palette
            .color(background_token)
            .unwrap_or(Color32::TRANSPARENT);
        let foreground = palette
            .color(foreground_token)
            .unwrap_or_else(|| ui.visuals().text_color());
        let border = palette.color(border_token).unwrap_or(foreground);
        egui::Frame::new()
            .fill(background)
            .stroke(Stroke::new(1.0, border))
            .corner_radius(2)
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                ui.label(TypeStyle::BODY.text(self.text).color(foreground))
            })
            .response
    }
}
