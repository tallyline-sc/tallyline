// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Drawn lifecycle values and exact manual actions come only from committed projections.

use super::*;
use tallyline_submission::api::model::{
    LifecycleRefresh, RowEditability, SubmissionLifecycle, UexLifecycleObservation,
    UexReportStatus, VerifiedUexReport,
};

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_4_r_ui_16__committed_lifecycle_is_drawn_and_manual_refresh_binds_both_versions_and_account_interval()
 {
    let queued = queued(1, SubmissionMode::Production).unwrap();
    let proof = VerifiedUexReport::from_lookup_at(
        81,
        TerminalId(89),
        CommodityId(7),
        Timestamp::from_seconds(100),
        queued.request(),
        Timestamp::from_seconds(100),
    )
    .unwrap();
    let accepted = queued
        .begin(Timestamp::from_seconds(110), PayloadHash::new([1; 32]))
        .unwrap()
        .complete(
            SubmissionResult::ProductionSucceeded(vec![proof.clone()]),
            Timestamp::from_seconds(115),
        )
        .unwrap();
    let claim = SubmissionLifecycle::try_from_job(&accepted)
        .unwrap()
        .claim(LifecycleRefresh::Manual, Timestamp::from_seconds(120))
        .unwrap()
        .unwrap();
    let ledger = claim
        .observed(&[UexLifecycleObservation::new(
            proof,
            UexReportStatus::Pending,
            RowEditability::Editable,
            Timestamp::from_seconds(125),
        )])
        .unwrap();
    let mut state = SubmissionQueue::default();
    state.received(vec![accepted.clone()]);
    state.received_schedule_time(Timestamp::from_seconds(179));
    assert!(state.received_lifecycle(ledger.clone()));
    assert!(!state.can_refresh_lifecycle(accepted.id()));
    assert!(!state.received_lifecycle(claim.lifecycle().clone()));
    assert_eq!(state.lifecycle(accepted.id()), Some(&ledger));
    state.received_schedule_time(Timestamp::from_seconds(180));
    assert!(
        !state.can_refresh_lifecycle(accepted.id()),
        "history without current account authority cannot enable refresh"
    );
    state.received_lifecycle_account(Some("different-account".to_owned()));
    assert!(!state.can_refresh_lifecycle(accepted.id()));
    state.received_lifecycle_account(Some("test-account".to_owned()));
    assert!(state.can_refresh_lifecycle(accepted.id()));
    let catalogue = Catalogue::english().unwrap();
    let mut fonts = Some(font_files().unwrap());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1000.0, 1000.0))
        .build_ui_state(
            move |ui, state: &mut (SubmissionQueue, Vec<QueueCommand>)| {
                if let Some(fonts) = fonts.take() {
                    tallyline_ui::api::configure_fonts(ui.ctx(), fonts);
                }
                if let Some(command) = show_submission_queue(
                    ui,
                    &mut state.0,
                    &catalogue,
                    tallyline_egui::Theme::Light,
                ) {
                    state.1.push(command);
                }
            },
            (state, Vec::new()),
        );
    harness.run();
    harness.get_by_label("Pending");
    harness.get_by_label("Editable at UEX");
    harness.get_by_label("Refresh lifecycle 1").click();
    harness.run();
    assert_eq!(
        harness.state().1,
        vec![QueueCommand::RefreshLifecycle {
            id: accepted.id(),
            expected_revision: accepted.revision(),
            expected_lifecycle: Some(ledger.revision())
        }]
    );
    assert!(!harness.state().0.can_refresh_lifecycle(accepted.id()));
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_4_r_sub_9_r_ui_16__append_only_source_rebinding_preserves_final_evidence_and_exposes_new_verified_targets()
 {
    use tallyline_submission::api::model::{
        StoredSubmissionLifecycle, SubmissionCommand, SubmissionResolutionAction,
    };
    let partial =
        super::partial_resolution::partial_with_result(SubmissionResult::Partial(Vec::new()))
            .unwrap();
    let request = partial.request();
    let proofs = [7, 8]
        .into_iter()
        .enumerate()
        .map(|(index, commodity)| {
            VerifiedUexReport::from_lookup_at(
                81 + u64::try_from(index).unwrap(),
                TerminalId(89),
                CommodityId(commodity),
                Timestamp::from_seconds(100),
                request,
                Timestamp::from_seconds(100),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let first = partial
        .apply(SubmissionCommand::ResolveDelivery(
            SubmissionResolutionAction::Verified {
                receipts: vec![proofs[0].clone()],
                at: Timestamp::from_seconds(121),
            },
        ))
        .unwrap()
        .into_state();
    let claim = SubmissionLifecycle::try_from_job(&first)
        .unwrap()
        .claim(LifecycleRefresh::Manual, Timestamp::from_seconds(122))
        .unwrap()
        .unwrap();
    let ledger = claim
        .observed(&[UexLifecycleObservation::new(
            proofs[0].clone(),
            UexReportStatus::Declined,
            RowEditability::NotEditable,
            Timestamp::from_seconds(123),
        )])
        .unwrap();
    let second = first
        .apply(SubmissionCommand::ResolveDelivery(
            SubmissionResolutionAction::Verified {
                receipts: vec![proofs[1].clone()],
                at: Timestamp::from_seconds(150),
            },
        ))
        .unwrap()
        .into_state();
    let rebound = SubmissionLifecycle::reconstitute(
        &SubmissionLifecycle::try_from_job(&second).unwrap(),
        StoredSubmissionLifecycle {
            revision: ledger.revision(),
            admitted_at: ledger.admitted_at(),
            manual_admitted_at: ledger.manual_admitted_at(),
            observations: ledger.observations().to_vec(),
            row_admissions: ledger.row_admissions().collect(),
        },
    )
    .unwrap();
    let mut state = SubmissionQueue::default();
    state.received(vec![first]);
    state.received_lifecycle_account(Some("test-account".to_owned()));
    state.received_schedule_time(Timestamp::from_seconds(182));
    assert!(state.received_lifecycle(ledger.clone()));
    assert!(!state.can_refresh_lifecycle(partial.id()));
    state.received(vec![second.clone()]);
    assert!(
        state.received_lifecycle(rebound.clone()),
        "same ledger revision with validated append-only source growth must preserve its committed evidence"
    );
    assert_eq!(
        state.lifecycle(partial.id()).unwrap().observations(),
        ledger.observations()
    );
    assert!(state.can_refresh_lifecycle(partial.id()));
    assert!(state.request_lifecycle_refresh(partial.id()));
    assert_eq!(
        state.take_command(),
        Some(QueueCommand::RefreshLifecycle {
            id: partial.id(),
            expected_revision: second.revision(),
            expected_lifecycle: Some(rebound.revision())
        })
    );
    assert!(!state.received_lifecycle(ledger));
}
