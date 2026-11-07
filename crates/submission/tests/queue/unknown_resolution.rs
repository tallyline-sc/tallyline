// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Positive evidence remains attributable after an explicit not-received decision.

use tallyline_shared_kernel::api::model::{
    CommodityId, ReportId, SubmissionMode, TerminalId, Timestamp,
};
use tallyline_submission::api::model::{
    JobId, JobPhase, PayloadHash, ReceiptDecision, SubmissionJob, SubmissionRequest,
    SubmissionResolutionAction, SubmissionResult, VerifiedUexReport,
};

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9__late_positive_receipt_after_not_received_and_duplication_keeps_both_histories() {
    let mut production = super::header();
    production.mode = SubmissionMode::Production;
    let request = SubmissionRequest::new(production, vec![super::row().unwrap()]).unwrap();
    let unknown = SubmissionJob::queued(JobId(88), request)
        .unwrap()
        .begin_prepared(
            Timestamp::from_seconds(110),
            PayloadHash::new([8; 32]),
            Timestamp::from_seconds(95),
        )
        .unwrap()
        .recover(Timestamp::from_seconds(120))
        .unwrap();
    let decided = unknown
        .resolve_delivery(SubmissionResolutionAction::Decision {
            decision: ReceiptDecision::NotReceived,
            at: Timestamp::from_seconds(130),
        })
        .unwrap();
    let duplicated = decided
        .resolve_delivery(SubmissionResolutionAction::Duplicated {
            report: ReportId(99),
            at: Timestamp::from_seconds(140),
        })
        .unwrap();
    let proof = VerifiedUexReport::from_lookup_at(
        718,
        TerminalId(89),
        CommodityId(7),
        Timestamp::from_seconds(95),
        duplicated.request(),
        Timestamp::from_seconds(95),
    )
    .unwrap();
    let late = duplicated.resolve_delivery(SubmissionResolutionAction::Verified {
        receipts: vec![proof],
        at: Timestamp::from_seconds(150),
    });
    assert!(
        late.is_ok(),
        "late positive proof must remain attributable to the original unknown attempt: {late:?}"
    );
    let late = late.unwrap();
    assert_eq!(late.resolution_actions().len(), 3);
    assert_eq!(late.unknown_decision(), Some(ReceiptDecision::NotReceived));
    assert_eq!(late.phase(), JobPhase::OutcomeUnknown);
    assert_eq!(late.attempts(), unknown.attempts());
    assert_eq!(late.result(), Some(&SubmissionResult::Unknown));
    assert!(!late.can_duplicate_unknown());
    assert!(late.retry().is_err());
    use tallyline_submission::api::SubmissionSchedule;
    use tallyline_submission::api::model::{ScheduleDecision, SubmissionCommand};
    let mut copied = late.request().header().clone();
    copied.report = ReportId(99);
    let first = super::row().unwrap();
    let second = tallyline_submission::api::model::SubmissionRow::new(
        tallyline_shared_kernel::api::model::RowId(2),
        CommodityId(8),
        first.price(),
        first.stock(),
        first.status(),
    )
    .unwrap();
    let third = tallyline_submission::api::model::SubmissionRow::new(
        tallyline_shared_kernel::api::model::RowId(3),
        CommodityId(9),
        first.price(),
        first.stock(),
        first.status(),
    )
    .unwrap();
    let split_source = SubmissionJob::queued(
        JobId(99),
        SubmissionRequest::new(copied.clone(), vec![first.clone(), second.clone(), third]).unwrap(),
    )
    .unwrap()
    .begin(Timestamp::from_seconds(141), PayloadHash::new([4; 32]))
    .unwrap()
    .complete(
        SubmissionResult::NotProcessed(
            tallyline_submission::api::model::NotProcessed::ConnectFailure,
        ),
        Timestamp::from_seconds(142),
    )
    .unwrap()
    .retry()
    .unwrap();
    let cancelled = split_source
        .apply(SubmissionCommand::CancelForSplit {
            report: ReportId(100),
            at: Timestamp::from_seconds(145),
        })
        .unwrap()
        .into_state();
    assert_eq!(cancelled.attempts(), split_source.attempts());
    assert_eq!(cancelled.attempts().len(), 1);
    copied.report = ReportId(100);
    let child = SubmissionJob::queued(
        JobId(100),
        SubmissionRequest::new(copied.clone(), vec![first, second]).unwrap(),
    )
    .unwrap();
    assert_eq!(
        SubmissionSchedule::assess(
            &child,
            &[late.clone(), cancelled.clone()],
            Timestamp::from_seconds(1000)
        ),
        ScheduleDecision::Unavailable
    );
    let chained = child
        .apply(SubmissionCommand::CancelForSplit {
            report: ReportId(101),
            at: Timestamp::from_seconds(146),
        })
        .unwrap()
        .into_state();
    copied.report = ReportId(101);
    let grandchild = SubmissionJob::queued(
        JobId(101),
        SubmissionRequest::new(copied, vec![super::row().unwrap()]).unwrap(),
    )
    .unwrap();
    assert_eq!(cancelled.verified_row_receipts().count(), 0);
    assert_eq!(chained.verified_row_receipts().count(), 0);
    assert_eq!(
        SubmissionSchedule::assess(
            &grandchild,
            &[duplicated, cancelled.clone(), chained.clone()],
            Timestamp::from_seconds(1000)
        ),
        ScheduleDecision::Ready
    );
    assert_eq!(
        SubmissionSchedule::assess(
            &grandchild,
            &[late, cancelled, chained],
            Timestamp::from_seconds(1000)
        ),
        ScheduleDecision::Unavailable
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9_r_sub_4__resolution_preserves_provisional_cooldown_and_rejects_empty_or_foreign_proof() {
    use tallyline_submission::api::SubmissionSchedule;
    use tallyline_submission::api::model::{ScheduleDecision, SubmissionRow};
    let mut production = super::header();
    production.mode = SubmissionMode::Production;
    let request = SubmissionRequest::new(production.clone(), vec![super::row().unwrap()]).unwrap();
    let unknown = SubmissionJob::queued(JobId(88), request)
        .unwrap()
        .begin_prepared(
            Timestamp::from_seconds(110),
            PayloadHash::new([8; 32]),
            Timestamp::from_seconds(95),
        )
        .unwrap()
        .recover(Timestamp::from_seconds(120))
        .unwrap();
    assert!(
        unknown
            .resolve_delivery(SubmissionResolutionAction::Verified {
                receipts: vec![],
                at: Timestamp::from_seconds(130)
            })
            .is_err()
    );
    let foreign_row = SubmissionRow::new(
        super::row().unwrap().id(),
        CommodityId(7),
        "121".parse().unwrap(),
        super::row().unwrap().stock(),
        1,
    )
    .unwrap();
    let foreign_request = SubmissionRequest::new(production.clone(), vec![foreign_row]).unwrap();
    let foreign = VerifiedUexReport::from_lookup_at(
        718,
        TerminalId(89),
        CommodityId(7),
        Timestamp::from_seconds(95),
        &foreign_request,
        Timestamp::from_seconds(95),
    )
    .unwrap();
    assert!(
        unknown
            .resolve_delivery(SubmissionResolutionAction::Verified {
                receipts: vec![foreign],
                at: Timestamp::from_seconds(130)
            })
            .is_err()
    );
    let received = unknown
        .resolve_delivery(SubmissionResolutionAction::Decision {
            decision: ReceiptDecision::Received,
            at: Timestamp::from_seconds(130),
        })
        .unwrap();
    production.report = ReportId(100);
    production.observed_at = Timestamp::from_seconds(101);
    let candidate = SubmissionJob::queued(
        JobId(89),
        SubmissionRequest::new(production, vec![super::row().unwrap()]).unwrap(),
    )
    .unwrap();
    assert_eq!(
        SubmissionSchedule::assess(&candidate, &[received], Timestamp::from_seconds(150)),
        ScheduleDecision::WaitingUntil(Timestamp::from_seconds(420))
    );
}
