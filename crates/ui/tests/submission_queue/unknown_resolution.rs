// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact-version unknown controls observed through keyboard focus and paint output.

use egui::Key;
use egui::Shape;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use tallyline_egui::Density;
use tallyline_egui::Motion;
use tallyline_egui::Theme;
use tallyline_shared_kernel::api::model::{SubmissionMode, Timestamp};
use tallyline_submission::api::model::ReceiptDecision;
use tallyline_submission::api::model::{JobId, PayloadHash};
use tallyline_ui::api::model::ResolutionQueueAction;
use tallyline_ui::api::model::{QueueCommand, SubmissionQueue};
use tallyline_ui::api::{self, Catalogue};

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9_r_ui_16__unknown_received_action_is_drawn_and_keyboard_operable() {
    let unknown = super::queued(88, SubmissionMode::Production)
        .unwrap()
        .begin(Timestamp::from_seconds(110), PayloadHash::new([8; 32]))
        .unwrap()
        .recover(Timestamp::from_seconds(120))
        .unwrap();
    let mut state = SubmissionQueue::default();
    state.received(vec![unknown.clone()]);
    let catalogue = Catalogue::english().unwrap();
    let mut fonts = Some(super::font_files().unwrap());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(900.0, 700.0))
        .build_ui_state(
            move |ui, state: &mut (SubmissionQueue, Vec<QueueCommand>)| {
                if let Some(fonts) = fonts.take() {
                    tallyline_ui::api::configure_fonts(ui.ctx(), fonts);
                }
                if let Some(command) =
                    api::show_submission_queue(ui, &mut state.0, &catalogue, Theme::HighContrast)
                {
                    state.1.push(command);
                }
            },
            (state, Vec::new()),
        );
    harness.run();
    let button = harness.query_by_label("Mark received 88");
    assert!(
        button.is_some(),
        "an unknown origin must offer the explicit received decision"
    );
    button.unwrap().focus();
    harness.run();
    assert!(harness.get_by_label("Mark received 88").is_focused());
    harness.key_press(Key::Enter);
    harness.run();
    assert_eq!(
        harness.state().1,
        vec![QueueCommand::Resolve {
            id: JobId(88),
            expected_revision: unknown.revision(),
            action: ResolutionQueueAction::Decide(ReceiptDecision::Received)
        }]
    );
    assert_eq!(harness.state().0.jobs(), &[unknown]);
    assert!(
        harness
            .output()
            .viewport_output
            .values()
            .all(|output| output.commands.is_empty())
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9_r_ui_16__not_received_history_offers_only_explicit_new_draft_and_read_only_evidence() {
    use tallyline_submission::api::model::{ReceiptDecision, SubmissionResolutionAction};
    use tallyline_ui::api::model::ResolutionQueueAction;
    for theme in [Theme::Dark, Theme::Light, Theme::HighContrast] {
        let unknown = super::queued(88, SubmissionMode::Production)
            .unwrap()
            .begin(Timestamp::from_seconds(110), PayloadHash::new([8; 32]))
            .unwrap()
            .recover(Timestamp::from_seconds(120))
            .unwrap()
            .resolve_delivery(SubmissionResolutionAction::Decision {
                decision: ReceiptDecision::NotReceived,
                at: Timestamp::from_seconds(130),
            })
            .unwrap();
        let mut state = SubmissionQueue::default();
        state.received(vec![unknown.clone()]);
        let catalogue = Catalogue::english().unwrap();
        let mut fonts = Some(super::font_files().unwrap());
        let mut harness = Harness::builder()
            .with_size(egui::vec2(1200.0, 1000.0))
            .build_ui_state(
                move |ui, state: &mut (SubmissionQueue, Vec<QueueCommand>)| {
                    if let Some(fonts) = fonts.take() {
                        tallyline_ui::api::configure_fonts(ui.ctx(), fonts);
                        tallyline_egui::apply_theme(
                            ui.ctx(),
                            theme,
                            Density::Comfortable,
                            Motion::Reduced,
                        );
                        ui.ctx().set_zoom_factor(2.0);
                    }
                    if let Some(command) =
                        api::show_submission_queue(ui, &mut state.0, &catalogue, theme)
                    {
                        state.1.push(command);
                    }
                },
                (state, Vec::new()),
            );
        harness.run();
        assert!(
            harness
                .query_by_label("Marked not received by you. The original outcome remains unknown.")
                .is_some(),
            "the durable user decision must remain distinct from UEX proof"
        );
        harness.get_by_label("Check UEX evidence 88");
        assert!(harness.query_by_label("Mark received 88").is_none());
        harness.get_by_label("Duplicate as new Draft 88").focus();
        harness.run();
        let button = harness.get_by_label("Duplicate as new Draft 88");
        assert!(button.is_focused());
        let bounds = button.rect();
        assert!(
            harness.output().shapes.iter().any(|shape| {
                let Shape::Rect(frame) = &shape.shape else {
                    return false;
                };
                frame.rect == bounds
                    && frame.stroke.width > 0.0
                    && shape.clip_rect.contains_rect(frame.rect)
            }),
            "the focused action must have a completely visible painted frame"
        );
        for label in ["Duplicate as new Draft 88", "Check UEX evidence 88"] {
            assert!(
                harness.output().shapes.iter().any(|shape| {
                    let Shape::Text(text) = &shape.shape else {
                        return false;
                    };
                    text.galley.job.text == label
                        && shape
                            .clip_rect
                            .contains_rect(text.galley.rect.translate(text.pos.to_vec2()))
                }),
                "the complete action text must fit its paint clip: {label}"
            );
        }
        harness.key_press(Key::Enter);
        harness.run();
        assert_eq!(
            harness.state().1,
            vec![QueueCommand::Resolve {
                id: JobId(88),
                expected_revision: unknown.revision(),
                action: ResolutionQueueAction::DuplicateAsDraft
            }]
        );
        assert_eq!(harness.state().0.jobs(), &[unknown]);
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9_r_ui_1_q_reports__unknown_inspection_exposes_fixed_target_values_and_actual_wire_date() {
    let unknown = super::queued(88, SubmissionMode::Production)
        .unwrap()
        .begin_prepared(
            Timestamp::from_seconds(110),
            PayloadHash::new([8; 32]),
            Timestamp::from_seconds(95),
        )
        .unwrap()
        .recover(Timestamp::from_seconds(120))
        .unwrap();
    let mut state = SubmissionQueue::default();
    state.received(vec![unknown.clone()]);
    let catalogue = Catalogue::english().unwrap();
    let mut fonts = Some(super::font_files().unwrap());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1000.0, 900.0))
        .build_ui_state(
            move |ui, state: &mut (SubmissionQueue, Vec<QueueCommand>)| {
                if let Some(fonts) = fonts.take() {
                    tallyline_ui::api::configure_fonts(ui.ctx(), fonts);
                }
                if let Some(command) =
                    api::show_submission_queue(ui, &mut state.0, &catalogue, Theme::Light)
                {
                    state.1.push(command);
                }
            },
            (state, Vec::new()),
        );
    harness.run();
    assert!(
        harness.query_by_label("Inspect original attempt").is_some(),
        "unknown inspection must expose the immutable target and values before a decision"
    );
    harness.get_by_label("Inspect original attempt").focus();
    harness.key_press(Key::Enter);
    harness.run();
    harness.get_by_label("Terminal 89 · UEX LIVE · Sell · 4.10.2");
    harness.get_by_label("Commodity 7 · 120 aUEC/SCU · 0 SCU · Status 1");
    harness.get_by_label("Attempted observation time: 1970-01-01 00:01:35 UTC");
    assert!(harness.state().1.is_empty());
    assert_eq!(harness.state().0.jobs(), &[unknown]);
}
