// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Private Linux fallback contracts on owned temporary directories.

#![cfg(target_os = "linux")]

use std::os::unix::fs::{MetadataExt, PermissionsExt};
use tallyline_adapter_platform::api::LinuxFallbackStore;
use tallyline_shared_kernel::api::model::Timestamp;
use tallyline_submission::api::SecretStore;
use tallyline_submission::api::model::{CredentialValues, SavedCredentials, ValidatedAccount};

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_nf_4__warned_fallback_replaces_a_private_bounded_record_and_removes_only_its_record() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut store = LinuxFallbackStore::open(directory.path()).unwrap();
    assert!(store.load().await.unwrap().is_none());
    for key in ["dummy-first", "dummy-replacement"] {
        let saved = SavedCredentials::from_storage(
            CredentialValues::new(key.to_owned(), None).unwrap(),
            ValidatedAccount::new("dummy-runner".to_owned()),
            Timestamp::from_seconds(123),
            None,
        )
        .unwrap();
        store.save(&saved).await.unwrap();
        assert!(store.load().await.unwrap().unwrap() == saved);
        let metadata = std::fs::symlink_metadata(directory.path().join("uex-credentials")).unwrap();
        assert_eq!(metadata.mode() & 0o777, 0o600);
        assert_eq!(metadata.uid(), rustix::process::geteuid().as_raw());
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }
    store.remove().await.unwrap();
    assert!(store.load().await.unwrap().is_none());
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_nf_4__changed_directory_permissions_and_unsafe_record_shapes_are_refused() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut store = LinuxFallbackStore::open(directory.path()).unwrap();
    let record = directory.path().join("uex-credentials");
    let saved = SavedCredentials::from_storage(
        CredentialValues::new("dummy-key".to_owned(), None).unwrap(),
        ValidatedAccount::new("dummy-runner".to_owned()),
        Timestamp::from_seconds(123),
        None,
    )
    .unwrap();
    store.save(&saved).await.unwrap();
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o705)).unwrap();
    assert!(store.load().await.is_err());
    assert!(store.save(&saved).await.is_err());
    assert!(store.remove().await.is_err());
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::set_permissions(&record, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(store.load().await.is_err());
    assert!(store.save(&saved).await.is_err());
    assert!(store.remove().await.is_err());
    assert_eq!(std::fs::metadata(&record).unwrap().mode() & 0o777, 0o644);
    std::fs::remove_file(&record).unwrap();
    let outside = directory.path().join("unrelated");
    std::fs::write(&outside, b"dummy-unrelated").unwrap();
    std::os::unix::fs::symlink(&outside, &record).unwrap();
    assert!(store.load().await.is_err());
    assert!(store.save(&saved).await.is_err());
    assert!(store.remove().await.is_err());
    assert_eq!(std::fs::read(&outside).unwrap(), b"dummy-unrelated");
    let linked = directory.path().join("directory-link");
    std::os::unix::fs::symlink(directory.path(), &linked).unwrap();
    assert!(LinuxFallbackStore::open(&linked).is_err());
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_nf_4__corrupt_oversized_and_hardlinked_records_are_refused_without_modification() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut store = LinuxFallbackStore::open(directory.path()).unwrap();
    let record = directory.path().join("uex-credentials");
    for bytes in [vec![b'x'], vec![b'x'; 2561]] {
        std::fs::write(&record, &bytes).unwrap();
        std::fs::set_permissions(&record, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert!(store.load().await.is_err());
        assert_eq!(std::fs::read(&record).unwrap(), bytes);
    }
    std::fs::hard_link(&record, directory.path().join("hard-link")).unwrap();
    assert!(store.load().await.is_err());
    assert!(store.remove().await.is_err());
    assert!(LinuxFallbackStore::open(std::path::Path::new("relative")).is_err());
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o750)).unwrap();
    assert!(LinuxFallbackStore::open(directory.path()).is_err());
}

#[tokio::test]
#[ignore = "Requires root in the isolated dummy-only Linux container."]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_nf_4__foreign_owned_record_and_directory_are_refused() {
    assert_eq!(rustix::process::geteuid().as_raw(), 0);
    let directory = tempfile::tempdir().unwrap();
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut store = LinuxFallbackStore::open(directory.path()).unwrap();
    let record = directory.path().join("uex-credentials");
    std::fs::write(&record, b"dummy-foreign-record").unwrap();
    std::fs::set_permissions(&record, std::fs::Permissions::from_mode(0o600)).unwrap();
    rustix::fs::chown(&record, Some(rustix::process::Uid::from_raw(1000)), None).unwrap();
    assert!(store.load().await.is_err());
    assert!(store.remove().await.is_err());
    rustix::fs::chown(
        directory.path(),
        Some(rustix::process::Uid::from_raw(1000)),
        None,
    )
    .unwrap();
    assert!(LinuxFallbackStore::open(directory.path()).is_err());
}
