// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Tallyline delta badge widget.

use crate::TypeStyle;
use crate::style::Theme;
use egui::{Color32, Response, Stroke, Ui, Widget};

/// A price deviation badge.
pub struct DeltaBadge<'a> {
    /// Signed delta label such as `Δ +12%`.
    pub text: &'a str,
    /// Whether the deviation requires confirmation.
    pub major: bool,
    /// Theme used to resolve badge tokens.
    pub theme: Theme,
}

impl Widget for DeltaBadge<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let palette = self.theme.palette();
        let (background, foreground) = if self.major {
            (
                palette.color("deviation-major-badge-bg"),
                palette.color("deviation-major-badge-fg"),
            )
        } else {
            (None, palette.color("deviation-minor-fg"))
        };
        let foreground = foreground.unwrap_or(Color32::WHITE);
        let mut frame = egui::Frame::new()
            .corner_radius(2)
            .inner_margin(egui::Margin::symmetric(5, 2));
        if !self.major {
            frame = frame.stroke(Stroke::new(
                1.0,
                palette
                    .color("deviation-minor-border")
                    .unwrap_or(foreground),
            ));
        }
        if let Some(fill) = background {
            frame = frame.fill(fill);
        }
        frame
            .show(ui, |ui| {
                ui.label(TypeStyle::CHIP.text(self.text).color(foreground))
            })
            .response
    }
}
