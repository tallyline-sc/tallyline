// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Complete supplied symbol interface rendered through the approved font.

use egui::{Color32, FontData, FontDefinitions, FontFamily, FontId, RawInput};
use tallyline_egui::{Density, Motion, Theme, symbols};

#[test]
fn all_supplied_named_symbols_have_distinct_nonempty_atlas_glyphs() {
    assert_eq!(symbols::ALL.len(), 156);
    for theme in [Theme::Dark, Theme::Light, Theme::HighContrast] {
        for (scale, size) in [1.0, 1.25, 1.5]
            .into_iter()
            .flat_map(|scale| [16.0, 20.0].map(|size| (scale, size)))
        {
            let context = egui::Context::default();
            context.set_pixels_per_point(scale);
            tallyline_egui::apply_theme(&context, theme, Density::Comfortable, Motion::Reduced);
            let family = FontFamily::Name("Material Symbols Rounded".into());
            let mut definitions = FontDefinitions::empty();
            definitions.font_data.insert(
                "symbols".into(),
                FontData::from_static(include_bytes!(
                    "../../ui/assets/fonts/material-symbols/MaterialSymbolsRounded-Tallyline.ttf"
                ))
                .into(),
            );
            definitions
                .families
                .insert(family.clone(), vec!["symbols".into()]);
            definitions
                .families
                .insert(FontFamily::Proportional, vec!["symbols".into()]);
            context.set_fonts(definitions);
            let mut output = context.run_ui(RawInput::default(), |ui| {
                ui.fonts_mut(|fonts| {
                    let font = FontId::new(size, family.clone());
                    let missing =
                        fonts.layout_no_wrap("\u{10ffff}".into(), font.clone(), Color32::WHITE);
                    let missing_uv = missing.rows[0].glyphs[0].uv_rect;
                    let mut atlas_areas = std::collections::BTreeSet::new();
                    for &(name, symbol) in symbols::ALL {
                        let galley =
                            fonts.layout_no_wrap(symbol.into(), font.clone(), Color32::WHITE);
                        let uv = galley.rows[0].glyphs[0].uv_rect;
                        assert!(
                            !uv.is_nothing(),
                            "{name} ({symbol:?}) at {scale}: empty atlas area"
                        );
                        assert_ne!(
                            (uv.min, uv.max),
                            (missing_uv.min, missing_uv.max),
                            "{name} ({symbol:?}) at {scale}: missing glyph"
                        );
                        assert!(
                            atlas_areas.insert((uv.min, uv.max)),
                            "{name} ({symbol:?}) at {scale}: duplicated atlas glyph"
                        );
                    }
                });
            });
            output.textures_delta.clear();
        }
    }
}
