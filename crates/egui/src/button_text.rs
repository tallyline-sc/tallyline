// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Explicit weighted labels for built-in egui buttons.

use egui::RichText;

use crate::TypeStyle;

/// Creates a button label in the theme's weighted button style.
#[must_use]
pub fn button_text(text: impl Into<String>) -> RichText {
    TypeStyle::BUTTON.text(text)
}
