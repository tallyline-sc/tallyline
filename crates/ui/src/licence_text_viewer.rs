// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared wrapped, selectable and immutable legal text.

use std::sync::Arc;
use tallyline_egui::TypeStyle;

const LICENCE_LINE_HEIGHT: f32 = 22.0;

#[derive(PartialEq)]
struct LayoutKey {
    context: egui::Context,
    font: egui::FontId,
    line_height_bits: u32,
    width_bits: u32,
    pixels_per_point_bits: u32,
    colour: egui::Color32,
}

#[derive(Default)]
pub(crate) struct LicenceTextViewer {
    layout: Option<(LayoutKey, Arc<egui::Galley>)>,
}

impl LicenceTextViewer {
    pub(crate) fn show(
        &mut self,
        ui: &mut egui::Ui,
        messages: &crate::api::Catalogue,
        mut text: &str,
    ) -> egui::Response {
        let id = ui.make_persistent_id("licence_text");
        scroll_from_keyboard(ui, id);
        let label = ui.label(TypeStyle::BODY_STRONG.text(messages.text("about.licence_text")));
        let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, width: f32| {
            let key = LayoutKey {
                context: ui.ctx().clone(),
                font: TypeStyle::BODY.font_id(),
                line_height_bits: LICENCE_LINE_HEIGHT.to_bits(),
                width_bits: width.to_bits(),
                pixels_per_point_bits: ui.ctx().pixels_per_point().to_bits(),
                colour: ui.visuals().text_color(),
            };
            if let Some((cached_key, galley)) = &self.layout
                && *cached_key == key
                && galley.job.text == text.as_str()
            {
                return Arc::clone(galley);
            }
            let mut job = egui::text::LayoutJob::simple(
                text.as_str().to_owned(),
                key.font.clone(),
                key.colour,
                width,
            );
            for section in &mut job.sections {
                section.format.line_height = Some(LICENCE_LINE_HEIGHT);
            }
            let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
            self.layout = Some((key, Arc::clone(&galley)));
            galley
        };
        let response = ui
            .add(
                egui::TextEdit::multiline(&mut text)
                    .id(id)
                    .font(TypeStyle::BODY.font_id())
                    .desired_width(f32::INFINITY)
                    .desired_rows(1)
                    .layouter(&mut layouter),
            )
            .labelled_by(label.id);
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_role(egui::accesskit::Role::Document);
        });
        response
    }
}

fn scroll_from_keyboard(ui: &egui::Ui, id: egui::Id) {
    if !ui.memory(|memory| memory.has_focus(id)) {
        return;
    }
    let page = ui.clip_rect().height().max(LICENCE_LINE_HEIGHT);
    let delta = ui.input_mut(|input| {
        if input.modifiers != egui::Modifiers::NONE {
            return 0.0;
        }
        let mut delta = 0.0;
        for (key, distance) in [
            (egui::Key::ArrowUp, LICENCE_LINE_HEIGHT),
            (egui::Key::ArrowDown, -LICENCE_LINE_HEIGHT),
            (egui::Key::PageUp, page),
            (egui::Key::PageDown, -page),
        ] {
            if input.consume_key(egui::Modifiers::NONE, key) {
                delta += distance;
            }
        }
        delta
    });
    if delta != 0.0 {
        ui.scroll_with_delta(egui::vec2(0.0, delta));
    }
}
