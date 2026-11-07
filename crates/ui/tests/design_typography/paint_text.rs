// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Text observed through public egui paint output.

/// Returns the text painted by this clipped shape.
pub fn shape(shape: &egui::epaint::ClippedShape) -> Option<&egui::epaint::TextShape> {
    if let egui::Shape::Text(text) = &shape.shape {
        Some(text)
    } else {
        None
    }
}
