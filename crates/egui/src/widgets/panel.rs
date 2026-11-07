// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Tallyline panel widget.

use crate::style::Theme;
use egui::{Color32, Response, Stroke, Ui, Widget};

/// A surface container with a subtle border.
pub struct Panel<'a> {
    /// Child content.
    pub content: &'a mut dyn FnMut(&mut Ui),
    /// Theme used to resolve panel tokens.
    pub theme: Theme,
}

impl Widget for Panel<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let palette = self.theme.palette();
        egui::Frame::new()
            .fill(
                palette
                    .color("surface-2")
                    .unwrap_or(ui.visuals().panel_fill),
            )
            .stroke(Stroke::new(
                1.0,
                palette.color("border-default").unwrap_or(Color32::GRAY),
            ))
            .corner_radius(2)
            .inner_margin(egui::Margin::symmetric(16, 12))
            .show(ui, |ui| (self.content)(ui))
            .response
    }
}
