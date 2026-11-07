// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Tallyline chip widget.

use crate::TypeStyle;
use egui::{Color32, Response, Ui, Widget};

/// A semantic status chip.
pub struct Chip<'a> {
    /// Chip content.
    pub text: &'a str,
    /// Foreground colour.
    pub foreground: Color32,
    /// Background colour.
    pub background: Color32,
}

impl Widget for Chip<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let text = TypeStyle::CHIP.text(self.text).color(self.foreground);
        egui::Frame::new()
            .fill(self.background)
            .corner_radius(2)
            .inner_margin(egui::Margin::symmetric(6, 2))
            .show(ui, |ui| ui.label(text))
            .response
    }
}
