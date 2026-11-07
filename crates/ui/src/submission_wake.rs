// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Visibility-aware queue publication wake-up without native window effects.

/// Wakes a visible queue view once; minimised timer labels wait for the next window event.
pub fn wake_submission(context: &egui::Context) {
    if context.input(|input| input.viewport().minimized != Some(true)) {
        context.request_repaint();
    }
}
