// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! State icons from the approved Material Symbols Rounded subset.

use egui::{Align2, Color32, FontFamily, FontId, Painter, Rect};

use crate::symbols;

/// A state icon used by Tallyline widgets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    /// Help or confirmation needed.
    Help,
    /// Choose between alternatives.
    AltRoute,
    /// Correct a value.
    EditSquare,
    /// User-entered value.
    Edit,
    /// Confirmed value.
    CheckCircle,
    /// Double confirmation.
    DoneAll,
    /// Compare values.
    CompareArrows,
    /// Critical warning.
    CrisisAlert,
    /// New item without a reference.
    FiberNew,
}

/// A drawable glyph and its accessible name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IconGlyph {
    /// The icon kind.
    pub icon: Icon,
    /// Localized or user-facing accessible label.
    pub label: &'static str,
}

impl Icon {
    /// The nine icons used for confidence and deviation states.
    pub const STATE_ICONS: [Self; 9] = [
        Self::Help,
        Self::AltRoute,
        Self::EditSquare,
        Self::Edit,
        Self::CheckCircle,
        Self::DoneAll,
        Self::CompareArrows,
        Self::CrisisAlert,
        Self::FiberNew,
    ];

    /// Draw this icon centered in `rect`.
    pub fn paint(self, painter: &Painter, rect: Rect, color: Color32) {
        let glyph = match self {
            Self::Help => symbols::HELP,
            Self::AltRoute => symbols::ALT_ROUTE,
            Self::EditSquare => symbols::EDIT_SQUARE,
            Self::Edit => symbols::EDIT,
            Self::CheckCircle => symbols::CHECK_CIRCLE,
            Self::DoneAll => symbols::DONE_ALL,
            Self::CompareArrows => symbols::COMPARE_ARROWS,
            Self::CrisisAlert => symbols::CRISIS_ALERT,
            Self::FiberNew => symbols::FIBER_NEW,
        };
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            glyph,
            FontId::new(
                rect.width().min(rect.height()),
                FontFamily::Name(symbols::FAMILY.into()),
            ),
            color,
        );
    }
}
