// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Drawn permanent-refusal copy intent and committed history presentation.

use egui::{Key, Shape};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use tallyline_egui::{Density, Motion, Theme};
use tallyline_shared_kernel::api::model::{ReportId, SubmissionMode, Timestamp};
use tallyline_submission::api::model::{
    JobId, PayloadHash, SubmissionCorrection, SubmissionRejection, SubmissionResolutionAction,
    SubmissionResult, UexErrorCode,
};
use tallyline_ui::api::model::{QueueCommand, ResolutionQueueAction, SubmissionQueue};
use tallyline_ui::api::{self, Catalogue};

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_11_r_ui_16__rejected_copy_is_drawn_focused_once_and_new_capture_has_no_action() {
    for theme in [Theme::Dark, Theme::Light, Theme::HighContrast] {
        let refused = |id, correction| {
            super::queued(id, SubmissionMode::DryRun)
                .unwrap()
                .begin(Timestamp::from_seconds(110), PayloadHash::new([8; 32]))
                .unwrap()
                .complete(
                    SubmissionResult::Rejected(SubmissionRejection::new(
                        UexErrorCode::new("opaque_refusal".to_owned()).unwrap(),
                        correction,
                    )),
                    Timestamp::from_seconds(120),
                )
                .unwrap()
        };
        let origin = refused(88, SubmissionCorrection::ReviewClient);
        let new_capture = refused(87, SubmissionCorrection::NewCapture);
        let mut state = SubmissionQueue::default();
        state.received(vec![origin.clone(), new_capture]);
        let catalogue = Catalogue::english().unwrap();
        let mut fonts = Some(super::font_files().unwrap());
        let mut harness = Harness::builder()
            .with_size(egui::vec2(1600.0, 1500.0))
            .build_ui_state(
                move |ui, state: &mut (SubmissionQueue, Vec<QueueCommand>)| {
                    if let Some(fonts) = fonts.take() {
                        api::configure_fonts(ui.ctx(), fonts);
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
        let button = harness.query_by_label("Duplicate as new Draft 88");
        assert!(
            button.is_some(),
            "a client-review rejection must draw its explicit copy action"
        );
        button.unwrap().focus();
        harness.run();
        let button = harness.get_by_label("Duplicate as new Draft 88");
        assert!(button.is_focused());
        let bounds = button.rect();
        assert!(harness.output().shapes.iter().any(|shape| matches!(&shape.shape,Shape::Rect(frame) if frame.rect==bounds && frame.stroke.width>0.0 && shape.clip_rect.contains_rect(frame.rect))),"the focused action must paint a visible frame");
        assert!(harness.output().shapes.iter().any(|shape| matches!(&shape.shape,Shape::Text(text) if text.galley.job.text=="Duplicate as new Draft 88" && shape.clip_rect.contains_rect(text.galley.rect.translate(text.pos.to_vec2())))),"the complete action text must fit its clip at double zoom");
        let background = harness
            .output()
            .shapes
            .iter()
            .find_map(|shape| {
                if let Shape::Rect(frame) = &shape.shape {
                    (frame.rect == bounds && frame.fill.a() == 255).then_some(frame.fill)
                } else {
                    None
                }
            })
            .expect("the button must have an opaque painted background");
        let luminance = |colour: egui::Color32| {
            let [red, green, blue, _] = egui::Rgba::from(colour).to_array();
            0.2126 * red + 0.7152 * green + 0.0722 * blue
        };
        let background_luminance = luminance(background);
        let text = harness
            .output()
            .shapes
            .iter()
            .find_map(|shape| {
                if let Shape::Text(text) = &shape.shape {
                    (text.galley.job.text == "Duplicate as new Draft 88").then_some(text)
                } else {
                    None
                }
            })
            .unwrap();
        for section in &text.galley.job.sections {
            let colour = text.override_text_color.unwrap_or(
                if section.format.color == egui::Color32::PLACEHOLDER {
                    text.fallback_color
                } else {
                    section.format.color
                },
            );
            let foreground_luminance = luminance(colour);
            let contrast = (foreground_luminance.max(background_luminance) + 0.05)
                / (foreground_luminance.min(background_luminance) + 0.05);
            assert!(
                contrast >= 4.5,
                "{theme:?}: focused copy text contrast must meet WCAG AA, got {contrast}"
            );
        }
        for label in [
            "Duplicate as new Draft 87",
            "Mark received 88",
            "Mark not received 88",
            "Check UEX evidence 88",
            "Retry now 88",
        ] {
            assert!(
                harness.query_by_label(label).is_none(),
                "ineligible action {label}"
            );
        }
        harness.key_press(Key::Enter);
        harness.run();
        assert_eq!(
            harness.state().1,
            vec![QueueCommand::Resolve {
                id: JobId(88),
                expected_revision: origin.revision(),
                action: ResolutionQueueAction::DuplicateAsDraft
            }]
        );
        harness.key_press(Key::Enter);
        harness.run();
        assert_eq!(
            harness.state().1.len(),
            1,
            "busy copying must not emit another command"
        );
        assert_eq!(harness.state().0.jobs()[0], origin);
        let committed = origin
            .resolve_delivery(SubmissionResolutionAction::Duplicated {
                report: ReportId(900),
                at: Timestamp::from_seconds(130),
            })
            .unwrap();
        harness.state_mut().0.received(vec![committed]);
        harness.run();
        harness.get_by_label("Separate new Draft: 900");
        assert!(
            harness
                .query_by_label("Duplicate as new Draft 88")
                .is_none()
        );
    }
}
