// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Typography observed in the actual manual review and confirmation views.

use super::{ManualReviewState, paint_text, support};

#[test]
fn actual_review_and_confirmation_draw_semantic_heading_emphasis_and_value_roles() {
    let context = egui::Context::default();
    tallyline_ui::api::configure_fonts(&context, support::fonts().unwrap());
    tallyline_egui::apply_theme(
        &context,
        tallyline_egui::Theme::Dark,
        tallyline_egui::Density::Compact,
        tallyline_egui::Motion::Reduced,
    );
    let (review, snapshot) = support::review().unwrap();
    let field = tallyline_reporting::api::model::ReviewField::Price(review.report().rows()[0].id());
    let mut state = ManualReviewState::default();
    state.received(review, snapshot);
    let catalogue = tallyline_ui::api::Catalogue::english().unwrap();
    let mut output = context.run_ui(egui::RawInput::default(), |ui| {
        let _ = tallyline_ui::api::show_manual_review(
            ui,
            &mut state,
            &catalogue,
            tallyline_egui::Theme::Dark,
        );
    });
    output.textures_delta.clear();
    assert_painted_type(
        &output,
        catalogue.text("review.title"),
        "IBM Plex Sans SemiBold",
        18.0,
        24.0,
    )
    .unwrap();
    assert_painted_type(
        &output,
        catalogue.text("review.required"),
        "IBM Plex Sans SemiBold",
        14.0,
        20.0,
    )
    .unwrap();
    let mut presenter = tallyline_ui::api::DialogPresenter::default();
    presenter.open_manual_confirmation(&state, field);
    for _ in 0..3 {
        output = context.run_ui(egui::RawInput::default(), |ui| {
            assert!(
                presenter
                    .show_manual_confirmation(
                        ui.ctx(),
                        &state,
                        &catalogue,
                        tallyline_egui::Theme::Dark,
                        |_| {}
                    )
                    .is_none()
            );
        });
        output.textures_delta.clear();
    }
    assert_painted_type(
        &output,
        catalogue.text("review.confirm_title"),
        "IBM Plex Sans SemiBold",
        18.0,
        24.0,
    )
    .unwrap();
    assert_painted_type(&output, "200 per SCU", "JetBrains Mono NL Bold", 18.0, 24.0).unwrap();
    assert_painted_type(
        &output,
        catalogue.text("review.approval_scope"),
        "IBM Plex Sans SemiBold",
        14.0,
        20.0,
    )
    .unwrap();
}

fn assert_painted_type(
    output: &egui::FullOutput,
    expected: &str,
    face: &str,
    size: f32,
    line_height: f32,
) -> Result<(), &'static str> {
    let text = output
        .shapes
        .iter()
        .filter_map(paint_text::shape)
        .find(|text| text.galley.job.text == expected)
        .ok_or("Expected text was not painted.")?;
    let format = &text
        .galley
        .job
        .sections
        .first()
        .ok_or("Painted text has no font section.")?
        .format;
    assert_eq!(
        format.font_id.family,
        egui::FontFamily::Name(face.into()),
        "{expected}"
    );
    assert_eq!(format.font_id.size, size, "{expected}");
    assert_eq!(format.line_height, Some(line_height), "{expected}");
    Ok(())
}
#[test]
fn current_price_and_stock_draw_exact_values_in_the_strong_mono_role() {
    for (review, snapshot, expected) in [
        {
            let (review, snapshot) =
                support::review_prices("12345678901234567890.125", "12345678901234567900.250")
                    .unwrap();
            (
                review,
                snapshot,
                "Price · commodity 42: 12,345,678,901,234,567,890.125",
            )
        },
        {
            let (review, snapshot) = support::stock_review().unwrap();
            (review, snapshot, "Stock · commodity 42: 100 SCU")
        },
    ] {
        let context = egui::Context::default();
        tallyline_ui::api::configure_fonts(&context, support::fonts().unwrap());
        tallyline_egui::apply_theme(
            &context,
            tallyline_egui::Theme::Dark,
            tallyline_egui::Density::Compact,
            tallyline_egui::Motion::Reduced,
        );
        let mut state = ManualReviewState::default();
        state.received(review, snapshot);
        let catalogue = tallyline_ui::api::Catalogue::english().unwrap();
        let mut output = context.run_ui(egui::RawInput::default(), |ui| {
            let _ = tallyline_ui::api::show_manual_review(
                ui,
                &mut state,
                &catalogue,
                tallyline_egui::Theme::Dark,
            );
        });
        output.textures_delta.clear();
        assert_painted_type(&output, expected, "JetBrains Mono NL Bold", 14.0, 20.0).unwrap();
    }
}
