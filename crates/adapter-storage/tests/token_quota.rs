// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Authenticated-read quota persistence without a submission job.

use tallyline_adapter_storage::api::SqliteReferenceCache;
use tallyline_shared_kernel::api::model::Timestamp;
use tallyline_submission::api::SubmissionQuotaRepository;
use tallyline_submission::api::model::{TokenQuotaObservation, TokenQuotaReset, TokenScope};
use tallyline_workflows::api::SubmissionQueueStore;

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_sub_11__authenticated_quota_survives_restart_and_expires_without_invented_jobs() {
    let path = std::env::temp_dir().join(format!("tallyline-read-quota-{}.db", std::process::id()));
    let mut store = SqliteReferenceCache::open(&path).unwrap();
    let scope = TokenScope::new([1; 32]).unwrap();
    let mut different = [1; 32];
    different[16..].fill(2);
    let other = TokenScope::new(different).unwrap();
    let lost = TokenScope::new([3; 32]).unwrap();
    let observation = TokenQuotaObservation::new(
        Some(scope),
        Some(TokenQuotaReset::new(
            Timestamp::from_seconds(1010),
            Some(Timestamp::from_seconds(1000)),
        )),
    );
    store.record_authenticated_quota(observation).unwrap();
    store.record_authenticated_quota(observation).unwrap();
    assert_eq!(store.authenticated_quotas().unwrap(), vec![observation]);
    assert!(!store.quota_allows(Some(scope)).unwrap());
    assert!(store.quota_allows(Some(other)).unwrap());
    assert!(!store.quota_allows(Some(lost)).unwrap());
    assert!(store.queue_jobs().unwrap().is_empty());
    drop(store);
    let mut store = SqliteReferenceCache::open(&path).unwrap();
    assert_eq!(store.authenticated_quotas().unwrap(), vec![observation]);
    assert!(
        store
            .expire_quotas(Timestamp::from_millis(1_009_999))
            .unwrap()
            .is_empty()
    );
    assert!(!store.quota_allows(Some(scope)).unwrap());
    assert!(
        store
            .expire_quotas(Timestamp::from_seconds(1010))
            .unwrap()
            .is_empty()
    );
    assert!(store.authenticated_quotas().unwrap().is_empty());
    store.record_authenticated_quota(observation).unwrap();
    assert!(store.authenticated_quotas().unwrap().is_empty());
    assert!(store.queue_jobs().unwrap().is_empty());
    drop(store);
    std::fs::remove_file(path).unwrap();
}
