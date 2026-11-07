// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact copied-row proofs remain distinct from other accepted rows.

use tallyline_shared_kernel::api::model::{CommodityId, ReportId, RowId, TerminalId, Timestamp};
use tallyline_submission::api::model::{
    JobId, PayloadHash, ReceiptDecision, ScheduleDecision, SubmissionJob,
    SubmissionResolutionAction, SubmissionResult, VerifiedUexReport,
};
use tallyline_submission::api::{SubmissionEvidence, SubmissionSchedule};

fn partial_copy() -> Option<SubmissionJob> {
    let partial = super::duplicate_ancestry::queued(88, 9)?
        .begin_prepared(
            Timestamp::from_seconds(110),
            PayloadHash::new([8; 32]),
            Timestamp::from_seconds(95),
        )
        .ok()?
        .complete(
            SubmissionResult::Partial(vec![]),
            Timestamp::from_seconds(120),
        )
        .ok()?;
    let mut decided = partial;
    for (row, decision) in [
        (RowId(1), ReceiptDecision::Received),
        (RowId(2), ReceiptDecision::NotReceived),
    ] {
        decided = decided
            .resolve_delivery(SubmissionResolutionAction::RowDecision {
                row,
                decision,
                at: Timestamp::from_seconds(130),
            })
            .ok()?;
    }
    decided
        .resolve_delivery(SubmissionResolutionAction::RowsDuplicated {
            report: ReportId(99),
            rows: vec![RowId(2)],
            at: Timestamp::from_seconds(140),
        })
        .ok()
}

fn prove(job: &SubmissionJob, id: u64, commodity: CommodityId) -> Option<SubmissionJob> {
    let receipt = VerifiedUexReport::from_lookup_at(
        id,
        TerminalId(89),
        commodity,
        Timestamp::from_seconds(95),
        job.request(),
        Timestamp::from_seconds(95),
    )
    .ok()?;
    job.resolve_delivery(SubmissionResolutionAction::Verified {
        receipts: vec![receipt],
        at: Timestamp::from_seconds(150),
    })
    .ok()
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_4_r_sub_9__only_a_positive_proof_for_a_copied_row_blocks_its_split_descendant() {
    let copied = partial_copy().unwrap();
    let outside = prove(&copied, 718, CommodityId(7)).unwrap();
    let intersecting = prove(&outside, 719, CommodityId(8)).unwrap();
    let candidate = super::duplicate_ancestry::queued(101, 101).unwrap();
    let parent = super::duplicate_ancestry::split(99, 99, 100).unwrap();
    let child = super::duplicate_ancestry::split(100, 100, 101).unwrap();
    for (origin, expected, decision) in [
        (copied, None, ScheduleDecision::Ready),
        (outside, None, ScheduleDecision::Ready),
        (intersecting, Some(JobId(88)), ScheduleDecision::Unavailable),
    ] {
        let history = [origin, parent.clone(), child.clone()];
        assert_eq!(
            SubmissionEvidence::resolved_duplicate_origin(ReportId(101), &history),
            expected
        );
        assert_eq!(
            SubmissionSchedule::assess(&candidate, &history, Timestamp::from_seconds(1000)),
            decision
        );
    }
}
