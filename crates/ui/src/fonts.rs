// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Separately shipped font resources approved by ADR-0004 and O-112.

use egui::{FontData, FontDefinitions, FontFamily};
use tallyline_egui::{FontFace, symbols};

use crate::material_symbols_font::MaterialSymbolsFont;

/// The approved font files loaded by the composition root.
pub struct FontFiles {
    /// Exact approved modified Material Symbols Rounded subset.
    pub material_symbols: MaterialSymbolsFont,
    /// IBM Plex Sans Regular.
    pub plex_regular: Vec<u8>,
    /// IBM Plex Sans SemiBold.
    pub plex_semibold: Vec<u8>,
    /// IBM Plex Sans Bold.
    pub plex_bold: Vec<u8>,
    /// JetBrains Mono NL Regular.
    pub mono_regular: Vec<u8>,
    /// JetBrains Mono NL Bold.
    pub mono_bold: Vec<u8>,
}

/// Installs the approved fonts from their separately shipped files.
pub fn configure_fonts(context: &egui::Context, files: FontFiles) {
    let mut fonts = FontDefinitions::empty();
    for (face, bytes) in [
        (FontFace::SansRegular, files.plex_regular),
        (FontFace::SansSemibold, files.plex_semibold),
        (FontFace::SansBold, files.plex_bold),
        (FontFace::MonoRegular, files.mono_regular),
        (FontFace::MonoBold, files.mono_bold),
    ] {
        let name = face.name();
        fonts
            .font_data
            .insert(name.to_owned(), FontData::from_owned(bytes).into());
        fonts.families.insert(face.family(), vec![name.to_owned()]);
    }
    fonts
        .families
        .insert(FontFamily::Proportional, vec!["IBM Plex Sans".to_owned()]);
    fonts
        .families
        .insert(FontFamily::Monospace, vec!["JetBrains Mono NL".to_owned()]);
    fonts.font_data.insert(
        symbols::FAMILY.to_owned(),
        FontData::from_owned(files.material_symbols.into_bytes()).into(),
    );
    fonts.families.insert(
        FontFamily::Name(symbols::FAMILY.into()),
        vec![symbols::FAMILY.to_owned()],
    );
    context.set_fonts(fonts);
}
