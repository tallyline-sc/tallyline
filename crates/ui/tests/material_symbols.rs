// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact approved icon font acceptance at the desktop resource boundary.

use tallyline_ui::api::model::MaterialSymbolsFont;

const APPROVED: &[u8] =
    include_bytes!("../assets/fonts/material-symbols/MaterialSymbolsRounded-Tallyline.ttf");

#[test]
fn accepts_only_the_historically_approved_icon_font() {
    assert!(MaterialSymbolsFont::try_from(APPROVED.to_vec()).is_ok());
    let mut changed = APPROVED.to_vec();
    changed[100] ^= 1;
    assert!(MaterialSymbolsFont::try_from(changed).is_err());
    assert!(MaterialSymbolsFont::try_from(APPROVED[..APPROVED.len() - 1].to_vec()).is_err());
    assert!(MaterialSymbolsFont::try_from(Vec::new()).is_err());
}

#[test]
fn desktop_fonts_register_a_dedicated_symbol_family_without_text_fallback() {
    use egui::{Color32, FontFamily, FontId, RawInput};
    use tallyline_ui::api::model::FontFiles;

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/fonts");
    let read = |name: &str| std::fs::read(root.join(name)).unwrap();
    let context = egui::Context::default();
    tallyline_ui::api::configure_fonts(
        &context,
        FontFiles {
            material_symbols: APPROVED.to_vec().try_into().unwrap(),
            plex_regular: read("ibm-plex/IBMPlexSans-Regular.ttf"),
            plex_semibold: read("ibm-plex/IBMPlexSans-SemiBold.ttf"),
            plex_bold: read("ibm-plex/IBMPlexSans-Bold.ttf"),
            mono_regular: read("jetbrains-mono/JetBrainsMonoNL-Regular.ttf"),
            mono_bold: read("jetbrains-mono/JetBrainsMonoNL-Bold.ttf"),
        },
    );
    let mut output = context.run_ui(RawInput::default(), |ui| {
        ui.fonts_mut(|fonts| {
            let family = FontFamily::Name(tallyline_egui::symbols::FAMILY.into());
            let icon = fonts.layout_no_wrap(
                tallyline_egui::symbols::HELP.into(),
                FontId::new(20.0, family.clone()),
                Color32::WHITE,
            );
            let missing = fonts.layout_no_wrap(
                "\u{10ffff}".into(),
                FontId::new(20.0, family),
                Color32::WHITE,
            );
            let text =
                fonts.layout_no_wrap("State".into(), FontId::proportional(14.0), Color32::WHITE);
            let icon_uv = icon.rows[0].glyphs[0].uv_rect;
            let missing_uv = missing.rows[0].glyphs[0].uv_rect;
            assert!(!icon_uv.is_nothing());
            assert_ne!((icon_uv.min, icon_uv.max), (missing_uv.min, missing_uv.max));
            assert_eq!(text.rows[0].glyphs.len(), 5);
            assert!(
                text.rows[0]
                    .glyphs
                    .iter()
                    .all(|glyph| !glyph.uv_rect.is_nothing())
            );
        });
    });
    output.textures_delta.clear();
}
