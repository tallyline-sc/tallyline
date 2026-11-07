// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Typography through the installed desktop fonts and drawn controls.

#[path = "resources/common.rs"]
mod resources;

use egui::{FontFamily, RawInput};
use tallyline_egui::{Density, Motion, Theme};

#[test]
fn drawn_heading_and_button_use_the_approved_weighted_faces() {
    let context = egui::Context::default();
    let files = resources::font_files().unwrap();
    let bold = files.plex_bold.clone();
    let semibold = files.plex_semibold.clone();
    tallyline_ui::api::configure_fonts(&context, files);
    tallyline_egui::apply_theme(&context, Theme::Dark, Density::Compact, Motion::Reduced);
    let mut output = context.run_ui(RawInput::default(), |ui| {
        ui.heading("Review prices");
        assert!(!ui.button(tallyline_egui::button_text("Confirm")).clicked());
    });
    output.textures_delta.clear();
    let text: Vec<_> = output.shapes.iter().filter_map(paint_text::shape).collect();
    let heading = text
        .iter()
        .find(|text| text.galley.job.text == "Review prices")
        .unwrap();
    let button = text
        .iter()
        .find(|text| text.galley.job.text == "Confirm")
        .unwrap();
    assert_eq!(heading.galley.job.sections[0].format.font_id.size, 22.0);
    assert_eq!(
        heading.galley.job.sections[0].format.font_id.family,
        FontFamily::Name("IBM Plex Sans Bold".into())
    );
    assert_eq!(
        button.galley.job.sections[0].format.font_id.family,
        FontFamily::Name("IBM Plex Sans SemiBold".into())
    );
    context.fonts(|fonts| {
        let definitions = fonts.definitions();
        for (family, bytes) in [
            ("IBM Plex Sans Bold", &bold),
            ("IBM Plex Sans SemiBold", &semibold),
        ] {
            let faces = &definitions.families[&FontFamily::Name(family.into())];
            assert_eq!(faces, &[family]);
            assert_eq!(
                definitions.font_data[family].font.as_ref().as_ref(),
                bytes.as_slice()
            );
        }
    });
    assert!(heading.galley.rows.iter().any(|row| !row.glyphs.is_empty()));
}
#[test]
fn every_named_style_shapes_with_its_dtcg_size_and_exact_face() {
    let source: serde_json::Value =
        serde_json::from_str(include_str!("../../../ui/design-tokens/base.tokens.json")).unwrap();
    let typography = source["typography"].as_object().unwrap();
    let context = egui::Context::default();
    tallyline_ui::api::configure_fonts(&context, resources::font_files().unwrap());
    tallyline_egui::apply_theme(&context, Theme::Dark, Density::Compact, Motion::Reduced);
    for (name, token) in typography.iter().filter(|(name, _)| !name.starts_with('$')) {
        let style = egui::TextStyle::Name(name.as_str().into());
        assert!(
            context.global_style().text_styles.contains_key(&style),
            "Missing {name}"
        );
        let mut output = context.run_ui(RawInput::default(), |ui| {
            ui.label(egui::RichText::new(name).text_style(style.clone()));
        });
        output.textures_delta.clear();
        let shaped = output
            .shapes
            .iter()
            .filter_map(paint_text::shape)
            .find(|text| text.galley.job.text == *name)
            .unwrap();
        let actual = &shaped.galley.job.sections[0].format.font_id;
        assert_eq!(
            f64::from(actual.size),
            token["$value"]["fontSize"]["value"].as_f64().unwrap(),
            "{name}"
        );
        let expected = match (
            token["$value"]["fontFamily"].as_str().unwrap(),
            token["$value"]["fontWeight"].as_u64().unwrap(),
        ) {
            ("{font-family.sans}", 400) => "IBM Plex Sans",
            ("{font-family.sans}", 600) => "IBM Plex Sans SemiBold",
            ("{font-family.sans}", 700) => "IBM Plex Sans Bold",
            ("{font-family.mono}", 400) => "JetBrains Mono NL",
            ("{font-family.mono}", 700) => "JetBrains Mono NL Bold",
            other => panic!("Unregistered approved face: {other:?}"),
        };
        assert_eq!(actual.family, FontFamily::Name(expected.into()), "{name}");
    }
}
#[path = "design_typography/widgets.rs"]
mod widgets;

#[test]
fn onboarding_continue_uses_the_weighted_button_face_in_the_actual_app_view() {
    let context = egui::Context::default();
    tallyline_ui::api::configure_fonts(&context, resources::font_files().unwrap());
    tallyline_egui::apply_theme(&context, Theme::Dark, Density::Comfortable, Motion::Reduced);
    let catalogue = tallyline_ui::api::Catalogue::english().unwrap();
    let pixels = resources::decode_logo(include_bytes!(
        "../../../docs/images/fankit/MadeByTheCommunity_White.png"
    ))
    .unwrap();
    let unit = tallyline_ui::api::FanKitUnit::new(&context, &catalogue, pixels);
    let state = tallyline_ui::api::ShellState::default();
    let mut output = context.run_ui(RawInput::default(), |ui| {
        assert!(tallyline_ui::api::show_onboarding_view(ui, &state, &catalogue, &unit).is_none());
    });
    output.textures_delta.clear();
    let label = output
        .shapes
        .iter()
        .filter_map(paint_text::shape)
        .find(|text| text.galley.job.text == "Continue")
        .unwrap();
    assert_eq!(
        label.galley.job.sections[0].format.font_id.family,
        FontFamily::Name("IBM Plex Sans SemiBold".into())
    );
}

#[path = "design_typography/paint_text.rs"]
mod paint_text;

#[test]
fn reference_browser_draws_screen_and_section_roles_with_explicit_type_metrics() {
    let context = egui::Context::default();
    tallyline_ui::api::configure_fonts(&context, resources::font_files().unwrap());
    tallyline_egui::apply_theme(&context, Theme::Dark, Density::Comfortable, Motion::Reduced);
    let catalogue = tallyline_ui::api::Catalogue::english().unwrap();
    let mut state = tallyline_ui::api::ShellState::default();
    let mut output = context.run_ui(RawInput::default(), |ui| {
        tallyline_ui::api::show_reference_browser(ui, &mut state, &catalogue);
    });
    output.textures_delta.clear();
    for (key, face, size, line_height) in [
        ("reference.title", "IBM Plex Sans Bold", 22.0, 28.0),
        ("reference.terminals", "IBM Plex Sans SemiBold", 15.0, 20.0),
        (
            "reference.commodities",
            "IBM Plex Sans SemiBold",
            15.0,
            20.0,
        ),
        ("reference.channel", "IBM Plex Sans", 14.0, 20.0),
        ("reference.refresh", "IBM Plex Sans SemiBold", 14.0, 20.0),
    ] {
        let text = output
            .shapes
            .iter()
            .filter_map(paint_text::shape)
            .find(|text| text.galley.job.text == catalogue.text(key))
            .unwrap();
        let format = &text.galley.job.sections[0].format;
        assert_eq!(
            format.font_id.family,
            FontFamily::Name(face.into()),
            "{key}"
        );
        assert_eq!(format.font_id.size, size, "{key}");
        assert_eq!(format.line_height, Some(line_height), "{key}");
    }
}
