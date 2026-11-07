// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Receipt enrichment cannot replace acceptance or invent an unrelated remote identity.

use tallyline_shared_kernel::api::model::{
    CommodityId, ReportId, RowId, SubmissionMode, TerminalId, Timestamp, TradeSide, UexEnvironment,
    UexReportId,
};
use tallyline_submission::api::model::{
    JobId, PayloadHash, SubmissionHeader, SubmissionJob, SubmissionRequest, SubmissionResult,
    SubmissionRow, VerifiedReceiptMapping, VerifiedUexReport,
};

fn accepted() -> Option<SubmissionJob> {
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
    SubmissionJob::queued(JobId(7), request)
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
        .ok()
}
fn receipt(id: u64, job: &SubmissionJob) -> Option<VerifiedUexReport> {
    VerifiedUexReport::from_lookup_at(
        id,
        TerminalId(89),
        CommodityId(7),
        Timestamp::from_seconds(95),
        job.request(),
        Timestamp::from_seconds(95),
    )
    .ok()
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9__verified_mapping_binds_the_actual_accepted_ids_and_preserves_successful_attempts() {
    let job = accepted().unwrap();
    assert!(VerifiedReceiptMapping::new(&job, vec![receipt(43, &job).unwrap()]).is_err());
    assert!(VerifiedReceiptMapping::new(&job, vec![]).is_err());
    let mapping = VerifiedReceiptMapping::new(&job, vec![receipt(42, &job).unwrap()]).unwrap();
    assert_eq!(mapping.job(), JobId(7));
    assert_eq!(mapping.revision(), 3);
    assert_eq!(mapping.receipts()[0].id(), 42);
    assert!(mapping.matches_job(&job));
    assert_eq!(job.recover(Timestamp::from_seconds(120)).unwrap(), job);
    assert_eq!(job.attempts().len(), 1);
    let other = SubmissionJob::queued(JobId(8), job.request().clone()).unwrap();
    assert!(!mapping.matches_job(&other));
    assert!(VerifiedReceiptMapping::new(&other, vec![receipt(42, &job).unwrap()]).is_err());
}
