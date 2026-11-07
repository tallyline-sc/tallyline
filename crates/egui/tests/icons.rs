// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Actual state glyph rendering at the supported display scales and themes.

use egui::{FontData, FontDefinitions, FontFamily, FontId, Pos2, RawInput, Rect, Shape, vec2};
use tallyline_egui::{Density, Icon, Motion, Theme};

const SCALES: [f32; 3] = [1.0, 1.25, 1.5];
const CODEPOINTS: [char; 9] = [
    '\u{e8fd}', '\u{f184}', '\u{f88d}', '\u{f097}', '\u{f0be}', '\u{e877}', '\u{e915}', '\u{ebe9}',
    '\u{e05e}',
];

#[test]
fn state_icons_render_the_approved_named_glyphs_without_missing_boxes() {
    for theme in [Theme::Dark, Theme::Light, Theme::HighContrast] {
        for (scale, size) in SCALES
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
            let mut output = context.run_ui(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(260.0, 90.0))),
                    ..Default::default()
                },
                |ui| {
                    for (index, icon) in Icon::STATE_ICONS.iter().copied().enumerate() {
                        icon.paint(
                            ui.painter(),
                            Rect::from_min_size(
                                Pos2::new(8.0 + index as f32 * 26.0, 8.0),
                                vec2(size, size),
                            ),
                            ui.visuals().text_color(),
                        );
                    }
                },
            );
            output.textures_delta.clear();
            let glyphs = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    Shape::Text(text) => Some(text),
                    Shape::Noop
                    | Shape::Vec(_)
                    | Shape::Circle(_)
                    | Shape::Ellipse(_)
                    | Shape::LineSegment { .. }
                    | Shape::Path(_)
                    | Shape::Band(_)
                    | Shape::Rect(_)
                    | Shape::Mesh(_)
                    | Shape::QuadraticBezier(_)
                    | Shape::CubicBezier(_)
                    | Shape::Callback(_) => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(
                glyphs.len(),
                CODEPOINTS.len(),
                "theme={theme:?} scale={scale}"
            );
            let missing = context.fonts_mut(|fonts| {
                fonts.layout_no_wrap(
                    "\u{10ffff}".into(),
                    FontId::new(size, family.clone()),
                    egui::Color32::WHITE,
                )
            });
            let missing_uv = missing.rows[0].glyphs[0].uv_rect;
            for (text, expected) in glyphs.iter().zip(CODEPOINTS) {
                assert_eq!(text.galley.text(), expected.to_string());
                let uv = text.galley.rows[0].glyphs[0].uv_rect;
                assert!(!uv.is_nothing(), "{expected:?}: empty atlas rectangle");
                assert_ne!(
                    (uv.min, uv.max),
                    (missing_uv.min, missing_uv.max),
                    "{expected:?}: missing glyph"
                );
                assert_eq!(text.galley.job.sections[0].format.font_id.family, family);
            }
        }
    }
}
