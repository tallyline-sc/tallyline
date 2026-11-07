// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Tallyline icon button widget.

use crate::Icon;
use egui::{Response, Sense, Ui, Widget, vec2};

/// A compact icon with a visible text label.
pub struct IconButton<'a> {
    /// Icon to display.
    pub icon: Icon,
    /// Button label.
    pub label: &'a str,
}

impl Widget for IconButton<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
            self.icon
                .paint(ui.painter(), rect, ui.visuals().text_color());
            ui.button(crate::button_text(self.label))
        })
        .inner
    }
}
