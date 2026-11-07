// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Pending lifecycle batches have explicit account and memory bounds.

use super::lifecycle;
use tallyline_shared_kernel::api::model::{CommodityId, ReportId, RowId, TerminalId, Timestamp};
use tallyline_submission::api::model::{
    JobId, LifecycleReadBatch, LifecycleReadClaim, LifecycleRefresh, PayloadHash, SubmissionJob,
    SubmissionLifecycle, SubmissionRequest, SubmissionResult, SubmissionRow, VerifiedUexReport,
};

pub(super) fn claim(job_id: u64, row_count: u32, account: &str) -> Option<LifecycleReadClaim> {
    claim_with_identity(job_id, row_count, account, job_id.checked_mul(1000)?, 95)
}
fn claim_with_identity(
    job_id: u64,
    row_count: u32,
    account: &str,
    remote_base: u64,
    date_added: u64,
) -> Option<LifecycleReadClaim> {
    let mut header = lifecycle()?.source().request().header().clone();
    header.account = account.to_owned();
    header.report = ReportId(job_id);
    let rows = (1..=row_count)
        .map(|id| {
            SubmissionRow::new(
                RowId(u64::from(id)),
                CommodityId(id),
                "120".parse().ok()?,
                "0".parse().ok()?,
                1,
            )
            .ok()
        })
        .collect::<Option<Vec<_>>>()?;
    let request = SubmissionRequest::new(header, rows).ok()?;
    let receipts = request
        .rows()
        .iter()
        .map(|row| {
            VerifiedUexReport::from_lookup_at(
                remote_base.checked_add(u64::from(row.commodity().0))?,
                TerminalId(89),
                row.commodity(),
                Timestamp::from_seconds(date_added),
                &request,
                Timestamp::from_seconds(date_added),
            )
            .ok()
        })
        .collect::<Option<Vec<_>>>()?;
    let job = SubmissionJob::queued(JobId(job_id), request)
        .ok()?
        .begin_prepared(
            Timestamp::from_seconds(110),
            PayloadHash::new([12; 32]),
            Timestamp::from_seconds(date_added),
        )
        .ok()?
        .complete(
            SubmissionResult::ProductionSucceeded(receipts),
            Timestamp::from_seconds(115),
        )
        .ok()?;
    SubmissionLifecycle::try_from_job(&job)
        .ok()?
        .claim(LifecycleRefresh::Manual, Timestamp::from_seconds(115))
        .ok()?
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_4_r_nf_3__one_lifecycle_batch_is_nonempty_account_bound_and_limited_to_a_hundred_jobs_or_five_hundred_targets()
 {
    assert!(LifecycleReadBatch::try_new(Vec::new()).is_err());
    let early = claim(507, 1, "dummy-a").unwrap();
    let late = early
        .lifecycle()
        .claim(LifecycleRefresh::Manual, Timestamp::from_seconds(175))
        .expect("valid later time")
        .expect("minute elapsed");
    assert!(LifecycleReadBatch::try_new(vec![claim(508, 1, "dummy-a").unwrap(), late]).is_err());
    let hundred = (1..=100)
        .map(|id| claim(id, 1, "dummy-a").unwrap())
        .collect();
    assert_eq!(
        LifecycleReadBatch::try_new(hundred)
            .expect("job boundary fits")
            .claims()
            .len(),
        100
    );
    let excessive = (1..=101)
        .map(|id| claim(id, 1, "dummy-a").unwrap())
        .collect();
    assert!(LifecycleReadBatch::try_new(excessive).is_err());
    assert!(LifecycleReadBatch::try_new(vec![claim(501, 500, "dummy-a").unwrap()]).is_ok());
    assert!(
        LifecycleReadBatch::try_new(vec![
            claim(502, 500, "dummy-a").unwrap(),
            claim(503, 1, "dummy-a").unwrap()
        ])
        .is_err()
    );
    assert!(
        LifecycleReadBatch::try_new(vec![
            claim(504, 1, "dummy-a").unwrap(),
            claim(505, 1, "dummy-b").unwrap()
        ])
        .is_err()
    );
    assert!(
        LifecycleReadBatch::try_new(vec![
            claim_with_identity(506, 1, "dummy-a", 7000, 94).unwrap(),
            claim_with_identity(506, 1, "dummy-a", 7000, 95).unwrap()
        ])
        .is_err()
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_4_r_sub_9__a_remote_id_and_partition_cannot_target_two_jobs_but_a_different_partition_can()
{
    let first = claim_with_identity(600, 1, "dummy-a", 7000, 95).unwrap();
    let second = claim_with_identity(601, 1, "dummy-a", 7000, 95).unwrap();
    assert!(LifecycleReadBatch::try_new(vec![first, second]).is_err());
    let earlier_partition = claim_with_identity(602, 1, "dummy-a", 7000, 94).unwrap();
    let later_partition = claim_with_identity(603, 1, "dummy-a", 7000, 95).unwrap();
    assert!(LifecycleReadBatch::try_new(vec![earlier_partition, later_partition]).is_ok());
}
