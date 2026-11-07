// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! The complete DTCG type ramp using approved static font faces.

use egui::{FontId, RichText, TextStyle};

use crate::FontFace;

/// A named type style with an exact face, size and line height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TypeStyle {
    name: &'static str,
    face: FontFace,
    size: f32,
    line_height: f32,
}

impl TypeStyle {
    /// The `display-name` type style.
    pub const DISPLAY_NAME: Self = Self {
        name: "display-name",
        face: FontFace::SansBold,
        size: 40.0,
        line_height: 44.0,
    };

    /// The `heading-1` type style.
    pub const HEADING_1: Self = Self {
        name: "heading-1",
        face: FontFace::SansBold,
        size: 22.0,
        line_height: 28.0,
    };

    /// The `heading-2` type style.
    pub const HEADING_2: Self = Self {
        name: "heading-2",
        face: FontFace::SansSemibold,
        size: 18.0,
        line_height: 24.0,
    };

    /// The `heading-3` type style.
    pub const HEADING_3: Self = Self {
        name: "heading-3",
        face: FontFace::SansSemibold,
        size: 15.0,
        line_height: 20.0,
    };

    /// The `label-caps` type style.
    pub const LABEL_CAPS: Self = Self {
        name: "label-caps",
        face: FontFace::SansSemibold,
        size: 12.0,
        line_height: 16.0,
    };

    /// The `body` type style.
    pub const BODY: Self = Self {
        name: "body",
        face: FontFace::SansRegular,
        size: 14.0,
        line_height: 20.0,
    };

    /// The `body-strong` type style.
    pub const BODY_STRONG: Self = Self {
        name: "body-strong",
        face: FontFace::SansSemibold,
        size: 14.0,
        line_height: 20.0,
    };

    /// The `ui-small` type style.
    pub const UI_SMALL: Self = Self {
        name: "ui-small",
        face: FontFace::SansRegular,
        size: 13.0,
        line_height: 18.0,
    };

    /// The `caption` type style.
    pub const CAPTION: Self = Self {
        name: "caption",
        face: FontFace::SansRegular,
        size: 12.0,
        line_height: 16.0,
    };

    /// The `button` type style.
    pub const BUTTON: Self = Self {
        name: "button",
        face: FontFace::SansSemibold,
        size: 14.0,
        line_height: 20.0,
    };

    /// The `chip` type style.
    pub const CHIP: Self = Self {
        name: "chip",
        face: FontFace::SansSemibold,
        size: 13.0,
        line_height: 18.0,
    };

    /// The `table-header` type style.
    pub const TABLE_HEADER: Self = Self {
        name: "table-header",
        face: FontFace::SansSemibold,
        size: 12.0,
        line_height: 16.0,
    };

    /// The `table-name` type style.
    pub const TABLE_NAME: Self = Self {
        name: "table-name",
        face: FontFace::SansSemibold,
        size: 14.0,
        line_height: 20.0,
    };

    /// The `table-text` type style.
    pub const TABLE_TEXT: Self = Self {
        name: "table-text",
        face: FontFace::SansRegular,
        size: 14.0,
        line_height: 20.0,
    };

    /// The `table-meta` type style.
    pub const TABLE_META: Self = Self {
        name: "table-meta",
        face: FontFace::SansRegular,
        size: 13.0,
        line_height: 18.0,
    };

    /// The `numerals` type style.
    pub const NUMERALS: Self = Self {
        name: "numerals",
        face: FontFace::MonoRegular,
        size: 14.0,
        line_height: 20.0,
    };

    /// The `numerals-strong` type style.
    pub const NUMERALS_STRONG: Self = Self {
        name: "numerals-strong",
        face: FontFace::MonoBold,
        size: 14.0,
        line_height: 20.0,
    };

    /// The `numerals-large` type style.
    pub const NUMERALS_LARGE: Self = Self {
        name: "numerals-large",
        face: FontFace::MonoBold,
        size: 18.0,
        line_height: 24.0,
    };

    /// The `mono-log` type style.
    pub const MONO_LOG: Self = Self {
        name: "mono-log",
        face: FontFace::MonoRegular,
        size: 13.0,
        line_height: 18.0,
    };

    /// The `keycap` type style.
    pub const KEYCAP: Self = Self {
        name: "keycap",
        face: FontFace::MonoRegular,
        size: 12.0,
        line_height: 16.0,
    };

    /// Every type style in the canonical ramp.
    pub const ALL: [Self; 20] = [
        Self::DISPLAY_NAME,
        Self::HEADING_1,
        Self::HEADING_2,
        Self::HEADING_3,
        Self::LABEL_CAPS,
        Self::BODY,
        Self::BODY_STRONG,
        Self::UI_SMALL,
        Self::CAPTION,
        Self::BUTTON,
        Self::CHIP,
        Self::TABLE_HEADER,
        Self::TABLE_NAME,
        Self::TABLE_TEXT,
        Self::TABLE_META,
        Self::NUMERALS,
        Self::NUMERALS_STRONG,
        Self::NUMERALS_LARGE,
        Self::MONO_LOG,
        Self::KEYCAP,
    ];

    /// Returns the canonical token name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// Returns the exact approved face and size.
    #[must_use]
    pub fn font_id(self) -> FontId {
        FontId::new(self.size, self.face.family())
    }

    /// Returns the named egui style installed by the theme.
    #[must_use]
    pub fn text_style(self) -> TextStyle {
        TextStyle::Name(self.name.into())
    }

    /// Formats text with the specified face, size and explicit line height.
    #[must_use]
    pub fn text(self, text: impl Into<String>) -> RichText {
        RichText::new(text)
            .font(self.font_id())
            .line_height(Some(self.line_height))
    }
}
