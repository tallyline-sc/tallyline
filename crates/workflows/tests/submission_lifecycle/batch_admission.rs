// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared account admission and exact cross-job identities are durable and atomic.

use super::accepted_fixture::error;
use super::support;
use tallyline_adapter_storage::api::SqliteReferenceCache;
use tallyline_submission::api::model::SubmissionJob;

fn accepted_pair(
    path: &std::path::Path,
    second_remote: u64,
) -> std::io::Result<(SqliteReferenceCache, Vec<SubmissionJob>)> {
    use tallyline_shared_kernel::api::model::{SubmissionMode, Timestamp};
    use tallyline_submission::api::model::PayloadHash;
    use tallyline_workflows::api::ManualReleaseStore;
    let (mut store, first_gate) =
        support::stored_gate_with_mode(path, SubmissionMode::Production).map_err(error)?;
    let first = store.release_manual(&first_gate).map_err(error)?;
    let (mut second_store, second_gate) =
        support::stored_gate_with_mode(path, SubmissionMode::Production).map_err(error)?;
    let second = second_store.release_manual(&second_gate).map_err(error)?;
    drop(second_store);
    let mut jobs = Vec::new();
    for (release, id, sent_at) in [(first, 123, 1000), (second, second_remote, 1301)] {
        let (accepted, _) = super::accepted_fixture::accepted_and_enriched(
            &mut store,
            release.job(),
            id,
            super::accepted_fixture::AcceptedAttempt {
                started_at: Timestamp::from_seconds(sent_at),
                completed_at: Timestamp::from_seconds(sent_at + 1),
                date_added: Timestamp::from_seconds(900),
                payload_hash: PayloadHash::new([11; 32]),
            },
        )?;
        jobs.push(accepted.state().clone());
    }
    Ok((store, jobs))
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_4_r_nf_11__manual_admission_is_shared_by_two_jobs_and_one_batch_claims_both_atomically() {
    use tallyline_shared_kernel::api::model::Timestamp;
    use tallyline_submission::api::model::LifecycleRefresh;
    use tallyline_workflows::api::SubmissionLifecycleStore;
    let directory = tempfile::tempdir_in(std::env::temp_dir()).unwrap();
    let path = directory.path().join("account-batch.db");
    let (mut store, jobs) = accepted_pair(&path, 124).unwrap();
    let first_claim = store
        .claim_lifecycle_batch(
            &[&jobs[0]],
            LifecycleRefresh::Manual,
            Timestamp::from_seconds(1500),
        )
        .unwrap();
    assert_eq!(first_claim.unwrap().claims().len(), 1);
    assert_eq!(store.load_lifecycle(jobs[1].id()).unwrap(), None);
    drop(store);
    let mut reopened = SqliteReferenceCache::open(&path).unwrap();
    assert!(
        reopened
            .claim_lifecycle_batch(
                &[&jobs[1]],
                LifecycleRefresh::Manual,
                Timestamp::from_seconds(1559)
            )
            .unwrap()
            .is_none()
    );
    assert_eq!(reopened.load_lifecycle(jobs[1].id()).unwrap(), None);
    let both = reopened
        .claim_lifecycle_batch(
            &[&jobs[0], &jobs[1]],
            LifecycleRefresh::Manual,
            Timestamp::from_seconds(1560),
        )
        .unwrap();
    let both = both.unwrap();
    assert_eq!(both.claims().len(), 2);
    assert_ne!(
        both.claims()[0].lifecycle().job(),
        both.claims()[1].lifecycle().job()
    );
    for job in &jobs {
        assert_eq!(
            reopened
                .load_lifecycle(job.id())
                .unwrap()
                .unwrap()
                .admitted_at(),
            Some(Timestamp::from_seconds(1560))
        );
    }
    assert_eq!(
        reopened
            .claim_lifecycle_batch(
                &[&jobs[0]],
                LifecycleRefresh::Automatic,
                Timestamp::from_seconds(1860)
            )
            .unwrap()
            .unwrap()
            .claims()
            .len(),
        1
    );
    assert_eq!(
        reopened
            .load_lifecycle(jobs[0].id())
            .unwrap()
            .unwrap()
            .manual_admitted_at(),
        Some(Timestamp::from_seconds(1560))
    );
    assert_eq!(
        reopened
            .claim_lifecycle_batch(
                &[&jobs[1]],
                LifecycleRefresh::Manual,
                Timestamp::from_seconds(1860)
            )
            .unwrap()
            .unwrap()
            .claims()
            .len(),
        1
    );
    assert!(
        reopened
            .claim_lifecycle_batch(
                &[&jobs[0]],
                LifecycleRefresh::Manual,
                Timestamp::from_seconds(1919)
            )
            .unwrap()
            .is_none()
    );
    assert_eq!(
        reopened
            .claim_lifecycle_batch(
                &[&jobs[0]],
                LifecycleRefresh::Manual,
                Timestamp::from_seconds(1920)
            )
            .unwrap()
            .unwrap()
            .claims()
            .len(),
        1
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_4_r_sub_9_r_nf_11__repeated_remote_identity_across_jobs_refuses_the_whole_read_without_admission()
 {
    use tallyline_shared_kernel::api::model::Timestamp;
    use tallyline_submission::api::model::{LifecycleRefresh, SubmissionRepositoryError};
    use tallyline_workflows::api::SubmissionLifecycleStore;
    let directory = tempfile::tempdir_in(std::env::temp_dir()).unwrap();
    let (mut store, jobs) =
        accepted_pair(&directory.path().join("ambiguous-batch.db"), 123).unwrap();
    assert_eq!(
        store.claim_lifecycle_batch(
            &[&jobs[0], &jobs[1]],
            LifecycleRefresh::Manual,
            Timestamp::from_seconds(1500)
        ),
        Err(SubmissionRepositoryError::Invalid)
    );
    for job in &jobs {
        assert_eq!(store.load_lifecycle(job.id()).unwrap(), None);
    }
    assert!(
        store
            .claim_lifecycle_read(
                &jobs[0],
                LifecycleRefresh::Manual,
                Timestamp::from_seconds(1500)
            )
            .unwrap()
            .is_some()
    );
}
