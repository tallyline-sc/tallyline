// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Keyboard focus and complete paint under scaled partial-result updates.

use super::super::font_files;
use super::{partial, partial_with_result};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use tallyline_shared_kernel::api::model::{RowId, Timestamp};
use tallyline_ui::api::model::SubmissionQueue;
use tallyline_ui::api::{self, Catalogue};
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9_r_ui_1_q_reports_r_ui_17__partial_controls_keep_focus_text_and_frame_inside_minimum_scaled_viewport()
 {
    use egui::{Key, Shape};
    use tallyline_egui::{Density, Motion, Theme};
    use tallyline_submission::api::model::{ReceiptDecision, SubmissionResolutionAction};
    use tallyline_ui::api::model::{QueueCommand, ResolutionQueueAction};
    for theme in [Theme::Dark, Theme::Light, Theme::HighContrast] {
        for zoom in [1.0, 2.0] {
            for copied in [false, true] {
                let original = partial().unwrap();
                let job = if copied {
                    original
                        .resolve_delivery(SubmissionResolutionAction::RowDecision {
                            row: RowId(31),
                            decision: ReceiptDecision::NotReceived,
                            at: Timestamp::from_seconds(130),
                        })
                        .unwrap()
                } else {
                    original
                };
                let label = if copied {
                    "Copy not-accepted rows to new Draft 89"
                } else {
                    "Mark row 31 received 89"
                };
                let expected = if copied {
                    ResolutionQueueAction::DuplicateAsDraft
                } else {
                    ResolutionQueueAction::DecideRow {
                        row: RowId(31),
                        decision: ReceiptDecision::Received,
                    }
                };
                let mut state = SubmissionQueue::default();
                state.received(vec![job.clone()]);
                let catalogue = Catalogue::english().unwrap();
                let mut fonts = Some(font_files().unwrap());
                let mut harness = Harness::builder()
                    .with_size(egui::vec2(800.0, 640.0))
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
                                ui.ctx().set_zoom_factor(zoom);
                            }
                            egui::ScrollArea::vertical().show(ui, |ui| {
                                if let Some(command) =
                                    api::show_submission_queue(ui, &mut state.0, &catalogue, theme)
                                {
                                    state.1.push(command);
                                }
                            });
                        },
                        (state, Vec::new()),
                    );
                harness.run();
                harness.get_by_label(label).focus();
                harness.run();
                let button = harness.get_by_label(label);
                assert!(button.is_focused());
                let bounds = button.rect();
                assert!(harness.output().shapes.iter().any(|shape|matches!(&shape.shape,Shape::Rect(frame) if frame.rect==bounds && frame.stroke.width>0.0 && shape.clip_rect.contains_rect(frame.rect))),"focused partial action frame must be completely painted: {label}, {theme:?}, zoom {zoom}");
                assert!(harness.output().shapes.iter().any(|shape|matches!(&shape.shape,Shape::Text(text) if text.galley.job.text==label && shape.clip_rect.contains_rect(text.galley.rect.translate(text.pos.to_vec2())))),"partial action text must be completely painted: {label}, {theme:?}, zoom {zoom}");
                harness.key_press(Key::Enter);
                harness.run();
                assert_eq!(
                    harness.state().1,
                    vec![QueueCommand::Resolve {
                        id: job.id(),
                        expected_revision: job.revision(),
                        action: expected
                    }]
                );
                harness.key_press(Key::Enter);
                harness.run();
                assert_eq!(harness.state().1.len(), 1);
                assert!(
                    harness
                        .output()
                        .viewport_output
                        .values()
                        .all(|output| output.commands.is_empty())
                );
            }
        }
    }
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9_r_ui_1_q_reports_r_ui_16__passive_proof_for_another_row_cannot_retarget_focused_decision()
 {
    use tallyline_shared_kernel::api::model::{CommodityId, TerminalId};
    use tallyline_submission::api::model::{
        ReceiptDecision, SubmissionResolutionAction, SubmissionResult, VerifiedUexReport,
    };
    use tallyline_ui::api::model::{QueueCommand, ResolutionQueueAction};
    let job = partial_with_result(SubmissionResult::Partial(vec![])).unwrap();
    let proof = VerifiedUexReport::from_lookup_at(
        718,
        TerminalId(89),
        CommodityId(7),
        Timestamp::from_seconds(100),
        job.request(),
        Timestamp::from_seconds(100),
    )
    .unwrap();
    let proved = job
        .resolve_delivery(SubmissionResolutionAction::Verified {
            receipts: vec![proof],
            at: Timestamp::from_seconds(130),
        })
        .unwrap();
    let revision = proved.revision();
    let mut state = SubmissionQueue::default();
    state.received(vec![job]);
    let catalogue = Catalogue::english().unwrap();
    let mut fonts = Some(font_files().unwrap());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1200.0, 1000.0))
        .build_ui_state(
            move |ui, state: &mut (SubmissionQueue, Vec<QueueCommand>)| {
                if let Some(fonts) = fonts.take() {
                    api::configure_fonts(ui.ctx(), fonts);
                }
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if let Some(command) = api::show_submission_queue(
                        ui,
                        &mut state.0,
                        &catalogue,
                        tallyline_egui::Theme::HighContrast,
                    ) {
                        state.1.push(command);
                    }
                });
            },
            (state, Vec::new()),
        );
    harness.run();
    harness.get_by_label("Mark row 42 received 89").focus();
    harness.run();
    harness.state_mut().0.received(vec![proved]);
    harness.run();
    assert!(
        harness.get_by_label("Mark row 42 received 89").is_focused(),
        "a passive proof removing another row's controls must preserve the focused row/action identity"
    );
    assert!(harness.state().1.is_empty());
    harness.key_press(egui::Key::Enter);
    harness.run();
    assert_eq!(
        harness.state().1,
        vec![QueueCommand::Resolve {
            id: tallyline_submission::api::model::JobId(89),
            expected_revision: revision,
            action: ResolutionQueueAction::DecideRow {
                row: RowId(42),
                decision: ReceiptDecision::Received
            }
        }]
    );
    assert!(
        harness
            .output()
            .viewport_output
            .values()
            .all(|output| output.commands.is_empty())
    );
}
