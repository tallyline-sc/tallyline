// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Drawn exact-row outcomes retain the immutable partial transport result.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use std::slice;
use tallyline_shared_kernel::api::model::{CommodityId, RowId, SubmissionMode, Timestamp};
use tallyline_submission::api::model::{
    PayloadHash, RowFailureKind, RowFailureSource, SubmissionJob, SubmissionRequest,
    SubmissionResult, SubmissionRow, UexErrorCode,
};
use tallyline_ui::api::model::SubmissionQueue;
use tallyline_ui::api::{self, Catalogue};

fn partial() -> Option<SubmissionJob> {
    partial_with_code("invalid_status_sell", RowFailureKind::InvalidSellStatus)
}
fn partial_with_code(code: &str, kind: RowFailureKind) -> Option<SubmissionJob> {
    partial_with_result(SubmissionResult::PartialError {
        code: UexErrorCode::classified(code.to_owned(), kind)?,
        row: Some(1),
        source: RowFailureSource::Index,
        receipts: Vec::new(),
    })
}
pub(super) fn partial_with_result(result: SubmissionResult) -> Option<SubmissionJob> {
    let base = super::queued(89, SubmissionMode::Production)?;
    let rows = [(31, 7), (42, 8), (53, 9)]
        .into_iter()
        .map(|(row, commodity)| {
            SubmissionRow::new(
                RowId(row),
                CommodityId(commodity),
                "120".parse().ok()?,
                "0".parse().ok()?,
                1,
            )
            .ok()
        })
        .collect::<Option<Vec<_>>>()?;
    let request = SubmissionRequest::new(base.request().header().clone(), rows).ok()?;
    SubmissionJob::queued(base.id(), request)
        .ok()?
        .begin(Timestamp::from_seconds(110), PayloadHash::new([8; 32]))
        .ok()?
        .complete(result, Timestamp::from_seconds(120))
        .ok()
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9_r_sub_11_r_ui_1_q_reports__named_failure_displays_each_actual_row_without_claiming_prefix_success()
 {
    let partial = partial().unwrap();
    let mut state = SubmissionQueue::default();
    state.received(vec![partial.clone()]);
    let catalogue = Catalogue::english().unwrap();
    let mut fonts = Some(super::font_files().unwrap());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1000.0, 900.0))
        .build_ui_state(
            move |ui, state| {
                if let Some(fonts) = fonts.take() {
                    api::configure_fonts(ui.ctx(), fonts);
                }
                api::show_submission_queue(
                    ui,
                    state,
                    &catalogue,
                    tallyline_egui::Theme::HighContrast,
                );
            },
            state,
        );
    harness.run();
    for label in [
        "Row 31 · Commodity 7 · Unmapped",
        "Row 42 · Commodity 8 · Rejected · invalid_status_sell",
        "Row 53 · Commodity 9 · Not sent",
    ] {
        assert!(
            harness.query_by_label(label).is_some(),
            "missing exact-row outcome: {label}"
        );
    }
    assert_eq!(harness.state().jobs(), &[partial]);
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9_r_ui_1_q_reports__unmapped_partial_row_offers_explicit_keyboard_decision_without_replay()
{
    use tallyline_ui::api::model::QueueCommand;
    let partial = partial().unwrap();
    let mut state = SubmissionQueue::default();
    state.received(vec![partial.clone()]);
    let catalogue = Catalogue::english().unwrap();
    let mut fonts = Some(super::font_files().unwrap());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1200.0, 1000.0))
        .build_ui_state(
            move |ui, state: &mut (SubmissionQueue, Vec<QueueCommand>)| {
                if let Some(fonts) = fonts.take() {
                    api::configure_fonts(ui.ctx(), fonts);
                }
                if let Some(command) = api::show_submission_queue(
                    ui,
                    &mut state.0,
                    &catalogue,
                    tallyline_egui::Theme::HighContrast,
                ) {
                    state.1.push(command);
                }
            },
            (state, Vec::new()),
        );
    harness.run();
    let button = harness.query_by_label("Mark row 31 received 89");
    assert!(
        button.is_some(),
        "an unmapped row must offer an explicit received decision"
    );
    assert!(
        harness
            .query_by_label("Mark row 31 not received 89")
            .is_some()
    );
    assert!(harness.query_by_label("Mark row 42 received 89").is_none());
    assert!(harness.query_by_label("Mark row 53 received 89").is_none());
    button.unwrap().focus();
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();
    assert_eq!(harness.state().1.len(), 1);
    assert_eq!(harness.state().0.jobs(), &[partial]);
    assert!(harness.query_by_label("Retry now 89").is_none());
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9_r_ui_1_q_reports__resolved_partial_offers_explicit_new_draft_without_resending_original()
{
    use tallyline_submission::api::model::{ReceiptDecision, SubmissionResolutionAction};
    use tallyline_ui::api::model::{QueueCommand, ResolutionQueueAction};
    let partial = partial()
        .unwrap()
        .resolve_delivery(SubmissionResolutionAction::RowDecision {
            row: RowId(31),
            decision: ReceiptDecision::NotReceived,
            at: Timestamp::from_seconds(130),
        })
        .unwrap();
    let mut state = SubmissionQueue::default();
    state.received(vec![partial.clone()]);
    let catalogue = Catalogue::english().unwrap();
    let mut fonts = Some(super::font_files().unwrap());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1200.0, 1000.0))
        .build_ui_state(
            move |ui, state: &mut (SubmissionQueue, Vec<QueueCommand>)| {
                if let Some(fonts) = fonts.take() {
                    api::configure_fonts(ui.ctx(), fonts);
                }
                if let Some(command) = api::show_submission_queue(
                    ui,
                    &mut state.0,
                    &catalogue,
                    tallyline_egui::Theme::HighContrast,
                ) {
                    state.1.push(command);
                }
            },
            (state, Vec::new()),
        );
    harness.run();
    let button = harness.query_by_label("Copy not-accepted rows to new Draft 89");
    assert!(
        button.is_some(),
        "resolved partial rows must offer only an explicit separate-Draft action"
    );
    button.unwrap().focus();
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();
    assert_eq!(
        harness.state().1,
        vec![QueueCommand::Resolve {
            id: partial.id(),
            expected_revision: partial.revision(),
            action: ResolutionQueueAction::DuplicateAsDraft
        }]
    );
    assert_eq!(harness.state().0.jobs(), &[partial]);
    assert!(harness.query_by_label("Retry now 89").is_none());
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9_r_sub_11_r_ui_1_q_reports__named_database_failure_shows_not_stored_without_permanent_rejection()
 {
    let partial = partial_with_code("database_error", RowFailureKind::Database).unwrap();
    let mut state = SubmissionQueue::default();
    state.received(vec![partial]);
    let catalogue = Catalogue::english().unwrap();
    let mut fonts = Some(super::font_files().unwrap());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1200.0, 1000.0))
        .build_ui_state(
            move |ui, state| {
                if let Some(fonts) = fonts.take() {
                    api::configure_fonts(ui.ctx(), fonts);
                }
                api::show_submission_queue(
                    ui,
                    state,
                    &catalogue,
                    tallyline_egui::Theme::HighContrast,
                );
            },
            state,
        );
    harness.run();
    assert!(
        harness
            .query_by_label("Row 42 · Commodity 8 · Not stored · database_error")
            .is_some(),
        "a failed database row was not stored and must not be presented as a permanent validation rejection"
    );
    harness.get_by_label("Row 31 · Commodity 7 · Unmapped");
    harness.get_by_label("Row 53 · Commodity 9 · Not sent");
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9_r_ui_1_q_reports__late_positive_proof_keeps_prior_row_decision_and_copy_link_visible() {
    use tallyline_shared_kernel::api::model::{ReportId, TerminalId};
    use tallyline_submission::api::model::{
        ReceiptDecision, SubmissionResolutionAction, VerifiedUexReport,
    };
    let decided = partial()
        .unwrap()
        .resolve_delivery(SubmissionResolutionAction::RowDecision {
            row: RowId(31),
            decision: ReceiptDecision::NotReceived,
            at: Timestamp::from_seconds(130),
        })
        .unwrap();
    let copied = decided
        .resolve_delivery(SubmissionResolutionAction::RowsDuplicated {
            report: ReportId(999),
            rows: vec![RowId(31), RowId(42), RowId(53)],
            at: Timestamp::from_seconds(140),
        })
        .unwrap();
    let proof = VerifiedUexReport::from_lookup_at(
        718,
        TerminalId(89),
        CommodityId(7),
        Timestamp::from_seconds(100),
        copied.request(),
        Timestamp::from_seconds(100),
    )
    .unwrap();
    let proved = copied
        .resolve_delivery(SubmissionResolutionAction::Verified {
            receipts: vec![proof],
            at: Timestamp::from_seconds(150),
        })
        .unwrap();
    let mut state = SubmissionQueue::default();
    state.received(vec![proved]);
    let catalogue = Catalogue::english().unwrap();
    let mut fonts = Some(super::font_files().unwrap());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1200.0, 1000.0))
        .build_ui_state(
            move |ui, state| {
                if let Some(fonts) = fonts.take() {
                    api::configure_fonts(ui.ctx(), fonts);
                }
                api::show_submission_queue(
                    ui,
                    state,
                    &catalogue,
                    tallyline_egui::Theme::HighContrast,
                );
            },
            state,
        );
    harness.run();
    assert!(
        harness
            .query_by_label("Row 31 · Your decision: Not received")
            .is_some(),
        "later positive proof must not hide the earlier explicit row statement"
    );
    harness.get_by_label("Copied to Draft 999 · 3 rows");
    assert!(
        harness
            .query_by_label("Copy not-accepted rows to new Draft 89")
            .is_none()
    );
}

#[path = "partial_focus.rs"]
mod focus;

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9_r_ui_1_q_reports__partial_inspection_exposes_original_values_and_actual_wire_date() {
    use egui::Key;
    use tallyline_egui::Theme;
    use tallyline_ui::api::model::QueueCommand;
    let original = partial().unwrap();
    let partial = SubmissionJob::queued(original.id(), original.request().clone())
        .unwrap()
        .begin_prepared(
            Timestamp::from_seconds(110),
            PayloadHash::new([8; 32]),
            Timestamp::from_seconds(95),
        )
        .unwrap()
        .complete(
            original.result().unwrap().clone(),
            Timestamp::from_seconds(120),
        )
        .unwrap();
    for theme in [Theme::Dark, Theme::Light, Theme::HighContrast] {
        let mut state = SubmissionQueue::default();
        state.received(vec![partial.clone()]);
        let catalogue = Catalogue::english().unwrap();
        let mut fonts = Some(super::font_files().unwrap());
        let mut harness = Harness::builder()
            .with_size(egui::vec2(1000.0, 900.0))
            .build_ui_state(
                move |ui, state: &mut (SubmissionQueue, Vec<QueueCommand>)| {
                    if let Some(fonts) = fonts.take() {
                        api::configure_fonts(ui.ctx(), fonts);
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
            harness.query_by_label("Inspect original attempt").is_some(),
            "partial decisions must expose the immutable attempted values first"
        );
        harness.get_by_label("Inspect original attempt").focus();
        harness.key_press(Key::Enter);
        harness.run();
        for label in [
            "Terminal 89 · UEX LIVE · Sell · 4.10.2",
            "Commodity 7 · 120 aUEC/SCU · 0 SCU · Status 1",
            "Commodity 8 · 120 aUEC/SCU · 0 SCU · Status 1",
            "Commodity 9 · 120 aUEC/SCU · 0 SCU · Status 1",
            "Attempted observation time: 1970-01-01 00:01:35 UTC",
            "Attempt started: 1970-01-01 00:01:50 UTC",
        ] {
            harness.get_by_label(label);
        }
        assert!(harness.state().1.is_empty());
        assert_eq!(harness.state().0.jobs(), slice::from_ref(&partial));
    }
}
