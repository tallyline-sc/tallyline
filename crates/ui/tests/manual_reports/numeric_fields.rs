// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Numeric entry observed through public egui paint output.

#[test]
fn numeric_entry_paints_exact_mono_values_and_muted_units_within_narrow_viewports() {
    for (width, zoom) in [(400.0, 1.0), (800.0, 2.0)] {
        for theme in [
            tallyline_egui::Theme::Dark,
            tallyline_egui::Theme::Light,
            tallyline_egui::Theme::HighContrast,
        ] {
            for entered in ["123.125", "12345678901234567890.125", ""] {
                let context = egui::Context::default();
                tallyline_ui::api::configure_fonts(&context, super::font_files().unwrap());
                tallyline_egui::apply_theme(
                    &context,
                    theme,
                    tallyline_egui::Density::Comfortable,
                    tallyline_egui::Motion::Reduced,
                );
                context.set_zoom_factor(zoom);
                let mut setup = context.run_ui(egui::RawInput::default(), |_| {});
                setup.textures_delta.clear();
                let mut value = entered.to_owned();
                let mut field = egui::Rect::NOTHING;
                let mut content = egui::Rect::NOTHING;
                let mut output = context.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width / zoom, 600.0 / zoom),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        field = ui
                            .add(tallyline_egui::NumericField {
                                id_salt: "numeric-typography",
                                label: "Exact price",
                                value: &mut value,
                                unit: tallyline_egui::NumericUnit::AuecPerScu,
                            })
                            .rect;
                        content = ui.min_rect();
                    },
                );
                output.textures_delta.clear();
                assert_eq!(context.pixels_per_point(), zoom);
                assert_eq!(value, entered);
                assert!(content.right() <= width / zoom);
                let text: Vec<_> = output
                    .shapes
                    .iter()
                    .filter_map(|shape| {
                        if let egui::Shape::Text(text) = &shape.shape {
                            Some(text)
                        } else {
                            None
                        }
                    })
                    .collect();
                let suffix = text
                    .iter()
                    .find(|text| text.galley.job.text == "aUEC/SCU")
                    .unwrap();
                assert_eq!(
                    suffix.galley.job.sections[0].format.color,
                    theme.palette().color("text-muted").unwrap()
                );
                assert!(suffix.pos.x >= field.right());
                if !entered.is_empty() {
                    let number = text
                        .iter()
                        .find(|text| text.galley.job.text == entered)
                        .unwrap();
                    assert!(
                        number.pos.x + number.galley.rect.left() >= field.left(),
                        "complete exact value must fit: {field:?}, {:?}, {:?}",
                        number.pos,
                        number.galley.rect
                    );
                    assert!(number.pos.x + number.galley.rect.right() <= field.right());
                    let format = &number.galley.job.sections[0].format;
                    assert_eq!(
                        format.font_id.family,
                        egui::FontFamily::Name("JetBrains Mono NL".into())
                    );
                    assert_eq!(format.font_id.size, 14.0);
                    assert_eq!(format.line_height, Some(20.0));
                    if entered == "123.125" {
                        let right = number.pos.x + number.galley.rect.right();
                        assert!(
                            (field.right() - right - 12.0).abs() < 2.0,
                            "field={field:?} text={:?} pos={:?} right={right}",
                            number.galley.rect,
                            number.pos
                        );
                    }
                }
            }
        }
    }
}
#[test]
fn numeric_groups_wrap_whole_labels_inputs_and_units_at_400_logical_points() {
    for (width, zoom) in [(400.0, 1.0), (800.0, 2.0)] {
        for theme in [
            tallyline_egui::Theme::Dark,
            tallyline_egui::Theme::Light,
            tallyline_egui::Theme::HighContrast,
        ] {
            let context = egui::Context::default();
            tallyline_ui::api::configure_fonts(&context, super::font_files().unwrap());
            tallyline_egui::apply_theme(
                &context,
                theme,
                tallyline_egui::Density::Comfortable,
                tallyline_egui::Motion::Reduced,
            );
            context.set_zoom_factor(zoom);
            let mut setup = context.run_ui(egui::RawInput::default(), |_| {});
            setup.textures_delta.clear();
            let mut values = ["123.125".to_owned(), "12.5".to_owned(), "20".to_owned()];
            let mut fields = Vec::new();
            let mut content = egui::Rect::NOTHING;
            let mut output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width / zoom, 600.0 / zoom),
                    )),
                    ..Default::default()
                },
                |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for ((label, unit), value) in [
                            (
                                "Price per SCU for commodity 42",
                                tallyline_egui::NumericUnit::AuecPerScu,
                            ),
                            ("SCU for commodity 42", tallyline_egui::NumericUnit::Scu),
                            (
                                "SCU percentage (0 to 20)",
                                tallyline_egui::NumericUnit::Percent,
                            ),
                        ]
                        .into_iter()
                        .zip(&mut values)
                        {
                            fields.push(
                                ui.add(tallyline_egui::NumericField {
                                    id_salt: label,
                                    label,
                                    value,
                                    unit,
                                })
                                .rect,
                            );
                        }
                    });
                    content = ui.min_rect();
                },
            );
            output.textures_delta.clear();
            assert_eq!(context.pixels_per_point(), zoom);
            assert!(content.right() <= width / zoom, "{content:?}, zoom={zoom}");
            assert!(
                fields[2].top() > fields[0].bottom(),
                "{fields:?}, zoom={zoom}"
            );
            assert_eq!(values, ["123.125", "12.5", "20"]);
            for suffix in ["aUEC/SCU", "SCU", "%"] {
                let painted = output
                    .shapes
                    .iter()
                    .find_map(|shape| {
                        if let egui::Shape::Text(text) = &shape.shape {
                            (text.galley.job.text == suffix).then_some(text)
                        } else {
                            None
                        }
                    })
                    .unwrap();
                assert!(painted.pos.x + painted.galley.rect.right() <= width / zoom);
                assert_eq!(
                    painted.galley.job.sections[0].format.color,
                    theme.palette().color("text-muted").unwrap()
                );
            }
        }
    }
}
