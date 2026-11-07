// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Explicit numeric review navigation and command binding.

use tallyline_ui::api::model::{ManualReviewState, ReviewDirection};

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_12_q_manual__empty_review_never_selects_or_confirms_a_field() {
    let mut state = ManualReviewState::default();
    assert!(state.review().is_none());
    assert!(state.focused().is_none());
    assert!(state.navigate(ReviewDirection::Next).is_none());
    assert!(state.navigate(ReviewDirection::Previous).is_none());
    assert!(state.confirm_focused().is_none());
}

#[path = "manual_review/support.rs"]
mod support;

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_12_q_manual__explicit_navigation_wraps_required_fields_and_commands_bind_the_displayed_version()
 {
    use tallyline_reporting::api::model::ReviewField;
    use tallyline_ui::api::model::ManualReviewAction;
    let (review, snapshot) = support::review().unwrap();
    let id = review.report().id();
    let version = review.report().version();
    let row = review.report().rows()[0].id();
    let mut state = ManualReviewState::default();
    state.received(review.clone(), snapshot.clone());
    assert!(state.focused().is_none());
    let first = state.navigate(ReviewDirection::Next).unwrap();
    assert_eq!(first.id, id);
    assert_eq!(first.expected, version);
    assert_eq!(first.snapshot, snapshot);
    assert_eq!(
        first.action,
        ManualReviewAction::Focus(ReviewField::Price(row))
    );
    assert_eq!(
        state.confirm_focused().unwrap().action,
        ManualReviewAction::Confirm(ReviewField::Price(row))
    );
    assert_eq!(
        state.navigate(ReviewDirection::Next).unwrap().action,
        ManualReviewAction::Focus(ReviewField::Status(row))
    );
    assert_eq!(
        state.navigate(ReviewDirection::Next).unwrap().action,
        ManualReviewAction::Focus(ReviewField::Price(row))
    );
    assert_eq!(
        state.navigate(ReviewDirection::Previous).unwrap().action,
        ManualReviewAction::Focus(ReviewField::Status(row))
    );
    state.received(review.clone(), snapshot);
    assert_eq!(state.focused(), Some(ReviewField::Status(row)));
    assert_eq!(state.review(), Some(&review));
    assert!(
        state.review().unwrap().report().rows()[0]
            .price_confirmation()
            .is_none()
    );
    state.invalidate();
    assert!(state.review().is_none());
    assert!(state.confirm_focused().is_none());
    assert!(state.navigate(ReviewDirection::Next).is_none());
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_12_q_manual__direct_keyboard_focus_records_review_without_approving_or_accepting_unknown_fields()
 {
    use tallyline_reporting::api::model::ReviewField;
    use tallyline_shared_kernel::api::model::RowId;
    use tallyline_ui::api::model::ManualReviewAction;
    let (review, snapshot) = support::review().unwrap();
    let row = review.report().rows()[0].id();
    let mut state = ManualReviewState::default();
    state.received(review, snapshot);
    assert!(!state.needs_confirmation(ReviewField::Price(RowId(99))));
    assert!(state.needs_confirmation(ReviewField::Price(row)));
    assert!(state.focus(ReviewField::Price(RowId(99))).is_none());
    assert!(state.focused().is_none());
    let focused = state.focus(ReviewField::Price(row)).unwrap();
    assert_eq!(
        focused.action,
        ManualReviewAction::Focus(ReviewField::Price(row))
    );
    assert_eq!(state.review().unwrap().summary().needs_confirmation, 2);
    assert!(
        state.review().unwrap().report().rows()[0]
            .price_confirmation()
            .is_none()
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_11_q_manual_r_ui_12_q_manual__drawn_review_shows_exact_reference_deltas_and_keyboard_confirmation_intent()
 {
    use egui_kittest::{Harness, kittest::Queryable};
    use tallyline_reporting::api::model::ReviewField;
    use tallyline_ui::api::model::{ManualReviewAction, ManualReviewCommand};
    use tallyline_ui::api::{Catalogue, show_manual_review};
    let (review, snapshot) = support::review().unwrap();
    let row = review.report().rows()[0].id();
    let mut state = ManualReviewState::default();
    state.received(review, snapshot);
    let catalogue = Catalogue::english().unwrap();
    let mut fonts = Some(tallyline_ui::api::model::FontFiles {
        material_symbols: include_bytes!(
            "../assets/fonts/material-symbols/MaterialSymbolsRounded-Tallyline.ttf"
        )
        .to_vec()
        .try_into()
        .unwrap(),
        plex_regular: include_bytes!("../assets/fonts/ibm-plex/IBMPlexSans-Regular.ttf").to_vec(),
        plex_semibold: include_bytes!("../assets/fonts/ibm-plex/IBMPlexSans-SemiBold.ttf").to_vec(),
        plex_bold: include_bytes!("../assets/fonts/ibm-plex/IBMPlexSans-Bold.ttf").to_vec(),
        mono_regular: include_bytes!("../assets/fonts/jetbrains-mono/JetBrainsMonoNL-Regular.ttf")
            .to_vec(),
        mono_bold: include_bytes!("../assets/fonts/jetbrains-mono/JetBrainsMonoNL-Bold.ttf")
            .to_vec(),
    });
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1000.0, 850.0))
        .build_ui_state(
            move |ui, state: &mut (ManualReviewState, Vec<ManualReviewCommand>)| {
                if let Some(fonts) = fonts.take() {
                    tallyline_ui::api::configure_fonts(ui.ctx(), fonts);
                }
                if let Some(command) =
                    show_manual_review(ui, &mut state.0, &catalogue, tallyline_egui::Theme::Dark)
                {
                    state.1.push(command);
                }
            },
            (state, Vec::new()),
        );
    harness.run();
    assert!(
        harness
            .query_by_label("Price, stock and status review")
            .is_some()
    );
    assert!(
        harness
            .query_by_label(
                "2 major · 0 within tolerance · 2 confirmations required · 2 unreviewed"
            )
            .is_some()
    );
    assert!(
        harness
            .query_by_label("Price · commodity 42: 200")
            .is_some()
    );
    assert!(
        harness
            .query_by_label("UEX reference: 100 · age at review: 100 seconds")
            .is_some()
    );
    assert!(
        harness
            .query_by_label("Difference: +100 per SCU · +100.00%")
            .is_some()
    );
    assert!(harness.state().1.is_empty());
    harness.key_press(egui::Key::F8);
    harness.run();
    assert_eq!(
        harness.state().1.last().unwrap().action,
        ManualReviewAction::Focus(ReviewField::Price(row))
    );
    harness.key_press(egui::Key::Enter);
    harness.run();
    assert_eq!(
        harness.state().1.last().unwrap().action,
        ManualReviewAction::Confirm(ReviewField::Price(row))
    );
    assert!(
        harness.state().0.review().unwrap().report().rows()[0]
            .price_confirmation()
            .is_none()
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_11_q_manual_r_ui_12_q_manual__confirmation_dialog_requires_a_separate_explicit_approval_of_the_displayed_value()
 {
    use egui_kittest::{Harness, kittest::Queryable};
    use tallyline_reporting::api::model::ReviewField;
    use tallyline_ui::api::model::ManualReviewCommand;
    use tallyline_ui::api::{Catalogue, DialogPresenter};
    for stock in [false, true] {
        let (review, snapshot) = if stock {
            support::stock_review().unwrap()
        } else {
            support::review().unwrap()
        };
        let row = review.report().rows()[0].id();
        let mut state = ManualReviewState::default();
        state.received(review, snapshot);
        let mut presenter = DialogPresenter::default();
        assert!(!presenter.manual_confirmation_open());
        let field = if stock {
            ReviewField::Stock(row)
        } else {
            ReviewField::Price(row)
        };
        presenter.open_manual_confirmation(&state, field);
        assert!(presenter.manual_confirmation_open());
        let catalogue = Catalogue::english().unwrap();
        let mut fonts = Some(support::fonts().unwrap());
        let mut harness =
            Harness::builder()
                .with_size(egui::vec2(1000.0, 850.0))
                .build_ui_state(
                    move |ui,
                          state: &mut (
                        ManualReviewState,
                        DialogPresenter,
                        Vec<ManualReviewCommand>,
                    )| {
                        if let Some(fonts) = fonts.take() {
                            tallyline_ui::api::configure_fonts(ui.ctx(), fonts);
                        }
                        if let Some(command) = state.1.show_manual_confirmation(
                            ui.ctx(),
                            &state.0,
                            &catalogue,
                            tallyline_egui::Theme::Dark,
                            |ui| {
                                ui.label("No screenshot attached");
                            },
                        ) {
                            state.2.push(command);
                        }
                    },
                    (state, presenter, Vec::new()),
                );
        harness.run();
        assert!(harness.query_by_label("Confirm this exact value").is_some());
        let value = if stock { "100 SCU" } else { "200 per SCU" };
        assert!(harness.query_by_label(value).is_some());
        assert!(
            harness
                .query_by_label(if stock {
                    "Difference: +100 SCU · Percentage unavailable"
                } else {
                    "Difference: +100 per SCU · +100.00%"
                })
                .is_some()
        );
        assert!(harness.state().2.is_empty());
        harness.get_by_label("Approve this exact value").focus();
        harness.run();
        harness.key_press(egui::Key::Enter);
        harness.run();
        assert_eq!(harness.state().2.len(), 1);
        assert!(!harness.state().1.manual_confirmation_open());
        assert_eq!(
            harness.state().2[0].action,
            tallyline_ui::api::model::ManualReviewAction::Confirm(field)
        );
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_12_q_manual_r_ui_16__cancel_or_invalidated_review_closes_the_dialog_without_dispatching_approval()
 {
    use egui_kittest::{Harness, kittest::Queryable};
    use tallyline_reporting::api::model::ReviewField;
    use tallyline_ui::api::model::ManualReviewCommand;
    use tallyline_ui::api::{Catalogue, DialogPresenter};
    let (review, snapshot) = support::review().unwrap();
    let field = ReviewField::Price(review.report().rows()[0].id());
    let mut state = ManualReviewState::default();
    state.received(review, snapshot);
    let mut presenter = DialogPresenter::default();
    presenter.open_manual_confirmation(&state, field);
    let catalogue = Catalogue::english().unwrap();
    let mut fonts = Some(support::fonts().unwrap());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1000.0, 850.0))
        .build_ui_state(
        move |ui, state: &mut (ManualReviewState, DialogPresenter, Vec<ManualReviewCommand>)| {
            if let Some(fonts) = fonts.take() {
                tallyline_ui::api::configure_fonts(ui.ctx(), fonts);
            }
            if let Some(command) = state.1.show_manual_confirmation(
                ui.ctx(),
                &state.0,
                &catalogue,
                tallyline_egui::Theme::Dark,
                |ui| {
                    ui.label("No screenshot attached");
                },
            ) {
                state.2.push(command);
            }
        },
        (state, presenter, Vec::new()),
    );
    harness.run();
    harness.get_by_label("Cancel").click();
    harness.run();
    assert!(!harness.state().1.manual_confirmation_open());
    assert!(harness.state().2.is_empty());
    let (state, presenter, _) = harness.state_mut();
    presenter.open_manual_confirmation(state, field);
    state.invalidate();
    harness.run();
    assert!(!harness.state().1.manual_confirmation_open());
    assert!(harness.query_by_label("Approve this exact value").is_none());
    assert!(harness.state().2.is_empty());
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_2a_q_manual_r_ui_11_q_manual__missing_prior_is_informational_and_never_presented_as_precision_failure()
 {
    use egui_kittest::{Harness, kittest::Queryable};
    use tallyline_ui::api::{Catalogue, show_manual_review};
    let (review, snapshot) = support::without_prior().unwrap();
    let mut state = ManualReviewState::default();
    state.received(review, snapshot);
    let catalogue = Catalogue::english().unwrap();
    let mut fonts = Some(support::fonts().unwrap());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1000.0, 850.0))
        .build_ui_state(
            move |ui, state| {
                if let Some(fonts) = fonts.take() {
                    tallyline_ui::api::configure_fonts(ui.ctx(), fonts);
                }
                assert!(
                    show_manual_review(ui, state, &catalogue, tallyline_egui::Theme::Dark)
                        .is_none()
                );
            },
            state,
        );
    harness.run();
    assert!(
        harness
            .query_all_by_label("No reference value")
            .next()
            .is_some()
    );
    assert!(
        harness
            .query_by_label(
                "Difference: Exact comparison unavailable per SCU · Percentage unavailable"
            )
            .is_none()
    );
    assert!(
        harness
            .query_by_label("Confirm price for commodity 42")
            .is_none()
    );
}
#[path = "manual_review/integration.rs"]
mod integration;

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_11_q_manual_r_ui_12_q_manual__drawn_stock_review_shows_bands_and_navigation_without_implicit_dialog()
 {
    use egui_kittest::{Harness, kittest::Queryable};
    use tallyline_reporting::api::model::ReviewField;
    use tallyline_ui::api::{Catalogue, show_manual_review};
    let (review, snapshot) = support::stock_review().unwrap();
    let row = review.report().rows()[0].id();
    let mut state = ManualReviewState::default();
    state.received(review, snapshot);
    let catalogue = Catalogue::english().unwrap();
    let mut fonts = Some(support::fonts().unwrap());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1100.0, 1400.0))
        .build_ui_state(
            move |ui, state: &mut ManualReviewState| {
                if let Some(fonts) = fonts.take() {
                    tallyline_ui::api::configure_fonts(ui.ctx(), fonts);
                }
                let _intent =
                    show_manual_review(ui, state, &catalogue, tallyline_egui::Theme::Dark);
            },
            state,
        );
    harness.run();
    assert!(
        harness
            .query_by_label("Stock · commodity 42: 100 SCU")
            .is_some()
    );
    assert!(
        harness
            .query_by_label("Effective tolerance: 20% and 16 SCU")
            .is_some()
    );
    assert!(
        harness
            .query_by_label("Difference: +100 SCU · Percentage unavailable")
            .is_some()
    );
    assert!(harness.query_by_label("Confirm this value").is_none());
    assert!(harness.state().focused().is_none());
    harness.key_press(egui::Key::F8);
    harness.run();
    harness.key_press(egui::Key::F8);
    harness.run();
    assert_eq!(harness.state().focused(), Some(ReviewField::Stock(row)));
}

#[test]
fn drawn_price_details_preserve_large_exact_decimals_and_signed_display_conventions() {
    for (value, reference, expected_reference, expected_difference) in [
        (
            "12345678901234567890.125",
            "12345678901234567900.250",
            "UEX reference: 12,345,678,901,234,567,900.25 · age at review: 100 seconds",
            "Difference: \u{2212}10.125 per SCU · \u{2212}0.00%",
        ),
        (
            "200",
            "100",
            "UEX reference: 100 · age at review: 100 seconds",
            "Difference: +100 per SCU · +100.00%",
        ),
        (
            "18014398509481986.25",
            "9007199254740993.125",
            "UEX reference: 9,007,199,254,740,993.125 · age at review: 100 seconds",
            "Difference: +9,007,199,254,740,993.125 per SCU · +100.00%",
        ),
        (
            "100",
            "100",
            "UEX reference: 100 · age at review: 100 seconds",
            "Difference: 0 per SCU · 0.00%",
        ),
    ] {
        let context = egui::Context::default();
        tallyline_ui::api::configure_fonts(&context, support::fonts().unwrap());
        tallyline_egui::apply_theme(
            &context,
            tallyline_egui::Theme::Dark,
            tallyline_egui::Density::Compact,
            tallyline_egui::Motion::Reduced,
        );
        let (review, snapshot) = support::review_prices(value, reference).unwrap();
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
        for expected in [expected_reference, expected_difference] {
            let text = output
                .shapes
                .iter()
                .filter_map(paint_text::shape)
                .find(|text| text.galley.job.text == expected)
                .unwrap_or_else(|| {
                    panic!(
                        "Missing exact displayed details: {expected}; drawn: {:?}",
                        output
                            .shapes
                            .iter()
                            .filter_map(paint_text::shape)
                            .map(|text| &text.galley.job.text)
                            .collect::<Vec<_>>()
                    )
                });
            assert_eq!(
                text.galley.job.sections[0].format.font_id.family,
                egui::FontFamily::Name("JetBrains Mono NL".into())
            );
            assert_eq!(text.galley.job.sections[0].format.font_id.size, 14.0);
            assert_eq!(text.galley.job.sections[0].format.line_height, Some(20.0));
        }
    }
}
#[test]
fn requested_value_confirmation_paints_the_theme_scrim_without_dispatching_approval() {
    for (theme, palette) in [
        (tallyline_egui::Theme::Dark, tallyline_egui::tokens::DARK),
        (tallyline_egui::Theme::Light, tallyline_egui::tokens::LIGHT),
        (
            tallyline_egui::Theme::HighContrast,
            tallyline_egui::tokens::HIGH_CONTRAST,
        ),
    ] {
        let context = egui::Context::default();
        tallyline_ui::api::configure_fonts(&context, support::fonts().unwrap());
        tallyline_egui::apply_theme(
            &context,
            theme,
            tallyline_egui::Density::Compact,
            tallyline_egui::Motion::Reduced,
        );
        let (review, snapshot) = support::review().unwrap();
        let field =
            tallyline_reporting::api::model::ReviewField::Price(review.report().rows()[0].id());
        let mut state = ManualReviewState::default();
        state.received(review, snapshot);
        let mut presenter = tallyline_ui::api::DialogPresenter::default();
        presenter.open_manual_confirmation(&state, field);
        let catalogue = tallyline_ui::api::Catalogue::english().unwrap();
        let mut command = None;
        let mut settled = None;
        for _ in 0..3 {
            let mut output = context.run_ui(egui::RawInput::default(), |ui| {
                command =
                    presenter.show_manual_confirmation(ui.ctx(), &state, &catalogue, theme, |ui| {
                        ui.label("No screenshot attached");
                    });
            });
            output.textures_delta.clear();
            assert!(command.is_none());
            settled = Some(output);
        }
        let output = settled.unwrap();
        assert!(output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Rect(rectangle) if rectangle.fill == palette.color("scrim").unwrap())), "Missing theme scrim: {theme:?}");
    }
}

#[path = "design_typography/paint_text.rs"]
mod paint_text;

#[path = "manual_review/typography.rs"]
mod typography;
