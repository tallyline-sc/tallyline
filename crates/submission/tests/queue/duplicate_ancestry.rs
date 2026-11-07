// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Public duplicate ancestry and positive-proof contracts.

use tallyline_shared_kernel::api::model::{
    CommodityId, ReportId, RowId, SubmissionMode, TerminalId, Timestamp,
};
use tallyline_submission::api::model::{
    JobId, PayloadHash, ReceiptDecision, ScheduleDecision, SubmissionCommand, SubmissionJob,
    SubmissionRequest, SubmissionResolutionAction, SubmissionRow, VerifiedUexReport,
};
use tallyline_submission::api::{SubmissionEvidence, SubmissionSchedule};

pub(super) fn queued(id: u64, report: u64) -> Option<SubmissionJob> {
    let mut header = super::header();
    header.report = ReportId(report);
    header.mode = SubmissionMode::Production;
    let first = super::row()?;
    let second = SubmissionRow::new(
        RowId(2),
        CommodityId(8),
        first.price(),
        first.stock(),
        first.status(),
    )
    .ok()?;
    SubmissionJob::queued(
        JobId(id),
        SubmissionRequest::new(header, vec![first, second]).ok()?,
    )
    .ok()
}

pub(super) fn split(id: u64, parent: u64, child: u64) -> Option<SubmissionJob> {
    Some(
        queued(id, parent)?
            .apply(SubmissionCommand::CancelForSplit {
                report: ReportId(child),
                at: Timestamp::from_seconds(140),
            })
            .ok()?
            .into_state(),
    )
}

fn origins(id: u64, report: u64, duplicate: u64) -> Option<(SubmissionJob, SubmissionJob)> {
    let duplicated = queued(id, report)?
        .begin_prepared(
            Timestamp::from_seconds(110),
            PayloadHash::new([8; 32]),
            Timestamp::from_seconds(95),
        )
        .ok()?
        .recover(Timestamp::from_seconds(120))
        .ok()?
        .resolve_delivery(SubmissionResolutionAction::Decision {
            decision: ReceiptDecision::NotReceived,
            at: Timestamp::from_seconds(130),
        })
        .ok()?
        .resolve_delivery(SubmissionResolutionAction::Duplicated {
            report: ReportId(duplicate),
            at: Timestamp::from_seconds(140),
        })
        .ok()?;
    let proof = VerifiedUexReport::from_lookup_at(
        718,
        TerminalId(89),
        CommodityId(7),
        Timestamp::from_seconds(95),
        duplicated.request(),
        Timestamp::from_seconds(95),
    )
    .ok()?;
    let proved = duplicated
        .resolve_delivery(SubmissionResolutionAction::Verified {
            receipts: vec![proof],
            at: Timestamp::from_seconds(150),
        })
        .ok()?;
    Some((duplicated, proved))
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_4_r_sub_9__positive_origin_blocks_split_chain_in_either_history_order() {
    let (unproved, proved) = origins(88, 9, 99).unwrap();
    let parent = split(99, 99, 100).unwrap();
    let child = split(100, 100, 101).unwrap();
    let candidate = queued(101, 101).unwrap();
    assert_eq!(
        SubmissionSchedule::assess(
            &candidate,
            &[unproved, parent.clone(), child.clone()],
            Timestamp::from_seconds(1000),
        ),
        ScheduleDecision::Ready
    );
    for history in [
        [proved.clone(), parent.clone(), child.clone()],
        [proved, child, parent],
    ] {
        assert_eq!(
            SubmissionEvidence::resolved_duplicate_origin(ReportId(101), &history),
            Some(JobId(88))
        );
        assert_eq!(
            SubmissionSchedule::assess(&candidate, &history, Timestamp::from_seconds(1000)),
            ScheduleDecision::Unavailable
        );
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_4_r_sub_9__duplicate_edges_and_cross_job_cycles_retain_only_connected_origins() {
    let (unproved, proved) = origins(88, 9, 99).unwrap();
    let parent = split(99, 99, 100).unwrap();
    let edges = [
        parent.clone(),
        split(100, 100, 99).unwrap(),
        split(103, 100, 101).unwrap(),
        parent,
        split(104, 98, 100).unwrap(),
    ];
    let mut history = vec![unproved];
    history.extend(edges);
    let candidate = queued(101, 101).unwrap();
    assert_eq!(
        SubmissionSchedule::assess(&candidate, &history, Timestamp::from_seconds(1000)),
        ScheduleDecision::Ready
    );
    history[0] = proved;
    assert_eq!(
        SubmissionEvidence::resolved_duplicate_origin(ReportId(101), &history),
        Some(JobId(88))
    );
    assert_eq!(
        SubmissionSchedule::assess(&candidate, &history, Timestamp::from_seconds(1000)),
        ScheduleDecision::Unavailable
    );
    assert_eq!(
        SubmissionEvidence::resolved_duplicate_origin(ReportId(102), &history),
        None
    );
    assert_eq!(
        SubmissionSchedule::assess(
            &queued(102, 102).unwrap(),
            &history,
            Timestamp::from_seconds(1000)
        ),
        ScheduleDecision::Ready
    );
    assert!(
        queued(105, 105)
            .unwrap()
            .apply(SubmissionCommand::CancelForSplit {
                report: ReportId(105),
                at: Timestamp::from_seconds(140),
            })
            .is_err()
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_4_r_sub_9__shared_split_parent_blocks_both_children_and_keeps_history_first_origin() {
    let (_, first) = origins(88, 9, 99).unwrap();
    let (_, second) = origins(87, 10, 100).unwrap();
    let edges = [
        split(99, 99, 100).unwrap(),
        split(100, 100, 101).unwrap(),
        split(103, 100, 102).unwrap(),
    ];
    for (origins, expected) in [
        ([first.clone(), second.clone()], JobId(88)),
        ([second, first], JobId(87)),
    ] {
        let mut history = Vec::from(origins);
        history.extend_from_slice(&edges);
        for report in [101, 102] {
            assert_eq!(
                SubmissionEvidence::resolved_duplicate_origin(ReportId(report), &history),
                Some(expected)
            );
            assert_eq!(
                SubmissionSchedule::assess(
                    &queued(report, report).unwrap(),
                    &history,
                    Timestamp::from_seconds(1000),
                ),
                ScheduleDecision::Unavailable
            );
        }
    }
}
