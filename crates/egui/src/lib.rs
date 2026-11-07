// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tallyline design tokens and reusable widgets for egui.

#![forbid(unsafe_code)]

mod button_text;
mod font_face;
mod icon;
mod numeric_field;
mod semantic_tone;
mod style;
mod type_style;
mod widgets;

pub mod symbols;
pub mod tokens;

pub use button_text::button_text;
pub use font_face::FontFace;
pub use icon::{Icon, IconGlyph};
pub use numeric_field::{NumericField, NumericUnit};
pub use semantic_tone::SemanticTone;
pub use style::{Density, Motion, Theme, apply_theme};
pub use type_style::TypeStyle;
pub use widgets::{Chip, DeltaBadge, Field, IconButton, Panel, Toast};
