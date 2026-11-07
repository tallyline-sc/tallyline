// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Lifecycle read admission is independent of delivery and observation age.

use tallyline_shared_kernel::api::model::{
    CommodityId, ReportId, RowId, SubmissionMode, TerminalId, Timestamp, TradeSide, UexEnvironment,
    UexReportId,
};
use tallyline_submission::api::model::{
    JobId, LifecycleRefresh, PayloadHash, SubmissionHeader, SubmissionJob, SubmissionLifecycle,
    SubmissionRequest, SubmissionResult, SubmissionRow, VerifiedReceiptMapping, VerifiedUexReport,
};

fn lifecycle() -> Option<SubmissionLifecycle> {
    let request = SubmissionRequest::new(
        SubmissionHeader {
            report: ReportId(4),
            report_version: 3,
            account: "dummy-a".to_owned(),
            mode: SubmissionMode::Production,
            terminal: TerminalId(89),
            side: TradeSide::Sell,
            environment: UexEnvironment::Live,
            game_version: "4.10.2".to_owned(),
            faction_affinity: None,
            observed_at: Timestamp::from_seconds(100),
        },
        vec![
            SubmissionRow::new(
                RowId(2),
                CommodityId(7),
                "120".parse().ok()?,
                "0".parse().ok()?,
                1,
            )
            .ok()?,
        ],
    )
    .ok()?;
    let job = SubmissionJob::queued(JobId(7), request)
        .ok()?
        .begin_prepared(
            Timestamp::from_seconds(110),
            PayloadHash::new([1; 32]),
            Timestamp::from_seconds(95),
        )
        .ok()?
        .complete(
            SubmissionResult::ProductionAccepted(vec![UexReportId::new(
                42,
                Timestamp::from_seconds(95),
            )?]),
            Timestamp::from_seconds(115),
        )
        .ok()?;
    let receipt = VerifiedUexReport::from_lookup_at(
        42,
        TerminalId(89),
        CommodityId(7),
        Timestamp::from_seconds(95),
        job.request(),
        Timestamp::from_seconds(95),
    )
    .ok()?;
    let mapping = VerifiedReceiptMapping::new(&job, vec![receipt]).ok()?;
    SubmissionLifecycle::try_new(&job, &mapping).ok()
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_4__automatic_reads_wait_five_minutes_after_sending_and_slow_after_the_first_hour() {
    let initial = lifecycle().unwrap();
    assert!(
        initial
            .claim(LifecycleRefresh::Automatic, Timestamp::from_seconds(409))
            .unwrap()
            .is_none()
    );
    let first = initial
        .claim(LifecycleRefresh::Automatic, Timestamp::from_seconds(410))
        .unwrap()
        .unwrap();
    assert!(
        first
            .lifecycle()
            .claim(LifecycleRefresh::Automatic, Timestamp::from_seconds(709))
            .unwrap()
            .is_none()
    );
    let next = first
        .lifecycle()
        .claim(LifecycleRefresh::Automatic, Timestamp::from_seconds(710))
        .unwrap()
        .unwrap();
    let last_first_hour = next
        .lifecycle()
        .claim(LifecycleRefresh::Automatic, Timestamp::from_seconds(3410))
        .unwrap()
        .unwrap();
    assert!(
        last_first_hour
            .lifecycle()
            .claim(LifecycleRefresh::Automatic, Timestamp::from_seconds(3709))
            .unwrap()
            .is_none()
    );
    let hour = last_first_hour
        .lifecycle()
        .claim(LifecycleRefresh::Automatic, Timestamp::from_seconds(3710))
        .unwrap()
        .unwrap();
    assert!(
        hour.lifecycle()
            .claim(LifecycleRefresh::Automatic, Timestamp::from_seconds(5509))
            .unwrap()
            .is_none()
    );
    assert!(
        hour.lifecycle()
            .claim(LifecycleRefresh::Automatic, Timestamp::from_seconds(5510))
            .unwrap()
            .is_some()
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_4__manual_refresh_starts_on_first_verified_proof_and_waits_a_minute_between_admissions() {
    let initial = lifecycle().unwrap();
    let first = initial
        .claim(LifecycleRefresh::Manual, Timestamp::from_seconds(115))
        .unwrap()
        .unwrap();
    assert!(
        first
            .lifecycle()
            .claim(LifecycleRefresh::Manual, Timestamp::from_seconds(174))
            .unwrap()
            .is_none()
    );
    let minute = first
        .lifecycle()
        .claim(LifecycleRefresh::Manual, Timestamp::from_seconds(175))
        .unwrap()
        .unwrap();
    assert!(
        minute
            .lifecycle()
            .claim(LifecycleRefresh::Manual, Timestamp::from_seconds(234))
            .unwrap()
            .is_none()
    );
    assert!(
        minute
            .lifecycle()
            .claim(LifecycleRefresh::Manual, Timestamp::from_seconds(235))
            .unwrap()
            .is_some()
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_4__final_status_and_fourteen_days_stop_both_automatic_and_manual_reads() {
    use tallyline_submission::api::model::{
        RowEditability, UexLifecycleObservation, UexReportStatus,
    };
    let initial = lifecycle().unwrap();
    for refresh in [LifecycleRefresh::Automatic, LifecycleRefresh::Manual] {
        assert!(
            initial
                .claim(refresh, Timestamp::from_seconds(1_209_709))
                .unwrap()
                .is_some()
        );
        assert!(
            initial
                .claim(refresh, Timestamp::from_seconds(1_209_710))
                .unwrap()
                .is_none()
        );
    }
    for status in [
        UexReportStatus::Consolidated,
        UexReportStatus::Declined,
        UexReportStatus::Expired,
    ] {
        let claim = initial
            .claim(LifecycleRefresh::Manual, Timestamp::from_seconds(115))
            .unwrap()
            .unwrap();
        let observation = UexLifecycleObservation::new(
            initial.receipts()[0].clone(),
            status,
            RowEditability::NotEditable,
            Timestamp::from_seconds(116),
        );
        let final_state = claim.observed(&[observation]).unwrap();
        for refresh in [LifecycleRefresh::Automatic, LifecycleRefresh::Manual] {
            assert!(
                final_state
                    .claim(refresh, Timestamp::from_seconds(410))
                    .unwrap()
                    .is_none()
            );
        }
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_4_r_sub_9__only_positive_verified_delivery_evidence_can_seed_lifecycle_targets() {
    use tallyline_submission::api::model::{ReceiptDecision, SubmissionResolutionAction};
    let initial = lifecycle().unwrap();
    assert!(SubmissionLifecycle::try_from_job(initial.source()).is_err());
    let sending = SubmissionJob::queued(JobId(8), initial.source().request().clone())
        .unwrap()
        .begin_prepared(
            Timestamp::from_seconds(110),
            PayloadHash::new([6; 32]),
            Timestamp::from_seconds(95),
        )
        .unwrap();
    let unknown = sending
        .complete(SubmissionResult::Unknown, Timestamp::from_seconds(115))
        .unwrap();
    let statement = unknown
        .resolve_delivery(SubmissionResolutionAction::Decision {
            decision: ReceiptDecision::Received,
            at: Timestamp::from_seconds(120),
        })
        .unwrap();
    assert!(SubmissionLifecycle::try_from_job(&statement).is_err());
    let receipt = initial.receipts()[0].clone();
    let verified = unknown
        .resolve_delivery(SubmissionResolutionAction::Verified {
            receipts: vec![receipt.clone()],
            at: Timestamp::from_seconds(120),
        })
        .unwrap();
    let partial_lifecycle = SubmissionLifecycle::try_from_job(&verified).unwrap();
    assert_eq!(
        partial_lifecycle.source().result(),
        Some(&SubmissionResult::Unknown)
    );
    assert_eq!(partial_lifecycle.source().attempts(), unknown.attempts());
    assert_eq!(partial_lifecycle.receipts(), std::slice::from_ref(&receipt));
    let succeeded = sending
        .complete(
            SubmissionResult::ProductionSucceeded(vec![receipt.clone()]),
            Timestamp::from_seconds(115),
        )
        .unwrap();
    assert_eq!(
        SubmissionLifecycle::try_from_job(&succeeded)
            .unwrap()
            .receipts(),
        &[receipt]
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_4__a_final_row_leaves_the_batch_while_other_verified_rows_keep_polling() {
    use tallyline_submission::api::model::{
        RowEditability, UexLifecycleObservation, UexReportStatus,
    };
    let single = lifecycle().unwrap();
    let request = SubmissionRequest::new(
        single.source().request().header().clone(),
        vec![
            single.source().request().rows()[0].clone(),
            SubmissionRow::new(
                RowId(3),
                CommodityId(8),
                "121".parse().unwrap(),
                "0".parse().unwrap(),
                1,
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let first = VerifiedUexReport::from_lookup_at(
        42,
        TerminalId(89),
        CommodityId(7),
        Timestamp::from_seconds(95),
        &request,
        Timestamp::from_seconds(95),
    )
    .unwrap();
    let second = VerifiedUexReport::from_lookup_at(
        43,
        TerminalId(89),
        CommodityId(8),
        Timestamp::from_seconds(95),
        &request,
        Timestamp::from_seconds(95),
    )
    .unwrap();
    let accepted = SubmissionJob::queued(JobId(9), request)
        .unwrap()
        .begin_prepared(
            Timestamp::from_seconds(110),
            PayloadHash::new([7; 32]),
            Timestamp::from_seconds(95),
        )
        .unwrap()
        .complete(
            SubmissionResult::ProductionSucceeded(vec![first.clone(), second.clone()]),
            Timestamp::from_seconds(115),
        )
        .unwrap();
    let initial = SubmissionLifecycle::try_from_job(&accepted).unwrap();
    let claim = initial
        .claim(LifecycleRefresh::Manual, Timestamp::from_seconds(115))
        .unwrap()
        .unwrap();
    assert_eq!(claim.targets().collect::<Vec<_>>(), vec![&first, &second]);
    let observed = claim
        .observed(&[
            UexLifecycleObservation::new(
                first.clone(),
                UexReportStatus::Declined,
                RowEditability::NotEditable,
                Timestamp::from_seconds(116),
            ),
            UexLifecycleObservation::new(
                second.clone(),
                UexReportStatus::Pending,
                RowEditability::Editable,
                Timestamp::from_seconds(116),
            ),
        ])
        .unwrap();
    let next = observed
        .claim(LifecycleRefresh::Manual, Timestamp::from_seconds(175))
        .unwrap()
        .unwrap();
    assert_eq!(next.targets().collect::<Vec<_>>(), vec![&second]);
    assert!(
        next.observed(&[UexLifecycleObservation::new(
            first,
            UexReportStatus::Pending,
            RowEditability::Editable,
            Timestamp::from_seconds(176)
        )])
        .is_err()
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_4_r_sub_9__lifecycle_observations_require_the_exact_claimed_id_date_row_and_completion_time()
 {
    use tallyline_submission::api::model::{
        RowEditability, UexLifecycleObservation, UexReportStatus,
    };
    let initial = lifecycle().unwrap();
    let claim = initial
        .claim(LifecycleRefresh::Manual, Timestamp::from_seconds(115))
        .unwrap()
        .unwrap();
    let request = initial.source().request();
    let wrong_id = VerifiedUexReport::from_lookup_at(
        99,
        TerminalId(89),
        CommodityId(7),
        Timestamp::from_seconds(95),
        request,
        Timestamp::from_seconds(95),
    )
    .unwrap();
    let wrong_partition = VerifiedUexReport::from_lookup_at(
        42,
        TerminalId(89),
        CommodityId(7),
        Timestamp::from_seconds(94),
        request,
        Timestamp::from_seconds(94),
    )
    .unwrap();
    for receipt in [wrong_id, wrong_partition] {
        assert!(
            claim
                .observed(&[UexLifecycleObservation::new(
                    receipt,
                    UexReportStatus::Pending,
                    RowEditability::Editable,
                    Timestamp::from_seconds(116)
                )])
                .is_err()
        );
    }
    let positive = UexLifecycleObservation::new(
        initial.receipts()[0].clone(),
        UexReportStatus::Pending,
        RowEditability::Editable,
        Timestamp::from_seconds(116),
    );
    assert!(claim.observed(&[positive.clone(), positive]).is_err());
    let premature = UexLifecycleObservation::new(
        initial.receipts()[0].clone(),
        UexReportStatus::Pending,
        RowEditability::Editable,
        Timestamp::from_seconds(114),
    );
    assert!(claim.observed(&[premature]).is_err());
}

#[path = "submission_lifecycle/batch_bounds.rs"]
mod batch_bounds;
