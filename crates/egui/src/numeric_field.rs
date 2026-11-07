// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact-string numeric entry with an adjacent unit.

use std::sync::Arc;

use egui::{Align, Align2, Galley, Response, TextEdit, Ui, Widget, WidgetInfo};

use crate::TypeStyle;

const MIN_FIELD_WIDTH: f32 = 170.0;

/// A unit displayed and announced beside a numeric input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumericUnit {
    /// In-game currency per standard cargo unit.
    AuecPerScu,
    /// Standard cargo units.
    Scu,
    /// Percentage points.
    Percent,
}

impl NumericUnit {
    /// Returns the conventional unit suffix.
    #[must_use]
    pub const fn suffix(self) -> &'static str {
        match self {
            Self::AuecPerScu => "aUEC/SCU",
            Self::Scu => "SCU",
            Self::Percent => "%",
        }
    }
}

/// A labelled numeric editor that leaves parsing and commit decisions to its caller.
pub struct NumericField<'a> {
    /// Stable caller-owned text editor identity.
    pub id_salt: &'a str,
    /// Visible field label.
    pub label: &'a str,
    /// Exact editable text, including incomplete input.
    pub value: &'a mut String,
    /// Muted suffix included in the accessible name.
    pub unit: NumericUnit,
}

impl Widget for NumericField<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let number = layout_number(ui, self.value);
        let suffix = ui.fonts_mut(|fonts| {
            fonts.layout_no_wrap(
                self.unit.suffix().to_owned(),
                TypeStyle::UI_SMALL.font_id(),
                ui.visuals().weak_text_color(),
            )
        });
        let unit_width = suffix.size().x + ui.spacing().item_spacing.x;
        let required_width = number.size().x + 2.0 * ui.spacing().button_padding.x + unit_width;
        let width = required_width
            .ceil()
            .max(MIN_FIELD_WIDTH)
            .min(ui.max_rect().width());
        ui.allocate_ui_with_layout(
            egui::vec2(width, 0.0),
            egui::Layout::top_down(Align::Min),
            |ui| {
                ui.set_width(width);
                ui.label(TypeStyle::UI_SMALL.text(self.label));
                ui.horizontal(|ui| {
                    let input_width = (width - unit_width).max(0.0);
                    let mut layouter = |ui: &Ui, text: &dyn egui::TextBuffer, _width: f32| {
                        if text.as_str() == number.job.text {
                            Arc::clone(&number)
                        } else {
                            layout_number(ui, text.as_str())
                        }
                    };
                    let response = ui.add(
                        TextEdit::singleline(self.value)
                            .font(TypeStyle::NUMERALS.font_id())
                            .layouter(&mut layouter)
                            .align(Align2::RIGHT_TOP)
                            .desired_width(input_width)
                            .id_salt(self.id_salt),
                    );
                    ui.label(suffix);
                    response.widget_info(|| {
                        WidgetInfo::labeled(
                            egui::Role::TextInput,
                            ui.is_enabled(),
                            format!("{} ({})", self.label, self.unit.suffix()),
                        )
                    });
                    response
                })
                .inner
            },
        )
        .inner
    }
}

fn layout_number(ui: &Ui, text: &str) -> Arc<Galley> {
    let mut job = egui::text::LayoutJob::default();
    TypeStyle::NUMERALS
        .text(text)
        .color(ui.visuals().text_color())
        .append_to(
            &mut job,
            ui.style(),
            egui::FontSelection::Default,
            Align::Center,
        );
    job.halign = Align::RIGHT;
    job.keep_trailing_whitespace = true;
    ui.fonts_mut(|fonts| fonts.layout_job(job))
}
