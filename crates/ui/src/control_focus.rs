// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Explicit focus reveals clipped controls without following passive layout changes.

pub(crate) fn reveal(ui: &egui::Ui, response: &egui::Response) {
    if response.gained_focus() && !ui.clip_rect().contains_rect(response.rect) {
        ui.scroll_to_rect_animation(response.rect, None, egui::style::ScrollAnimation::none());
    }
}
