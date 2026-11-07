// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Approved separate font faces shared by desktop loading and typography.

use egui::FontFamily;

/// An approved static font face and weight.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum FontFace {
    /// IBM Plex Sans at weight 400.
    SansRegular,
    /// IBM Plex Sans at weight 600.
    SansSemibold,
    /// IBM Plex Sans at weight 700.
    SansBold,
    /// JetBrains Mono NL at weight 400.
    MonoRegular,
    /// JetBrains Mono NL at weight 700.
    MonoBold,
}

impl FontFace {
    /// Returns the separately registered face name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::SansRegular => "IBM Plex Sans",
            Self::SansSemibold => "IBM Plex Sans SemiBold",
            Self::SansBold => "IBM Plex Sans Bold",
            Self::MonoRegular => "JetBrains Mono NL",
            Self::MonoBold => "JetBrains Mono NL Bold",
        }
    }

    /// Returns the egui family containing this exact face.
    #[must_use]
    pub fn family(self) -> FontFamily {
        FontFamily::Name(self.name().into())
    }
}
