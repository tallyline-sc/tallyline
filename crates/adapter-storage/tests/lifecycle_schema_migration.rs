// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Genuine accepted schema-sixteen data migrates only after backup and preserves its proofs.

#[path = "support/schema_sixteen_fixture.rs"]
mod fixture;
#[path = "support/schema_sixteen_reader.rs"]
mod reader;

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_4_r_nf_11__genuine_sixteen_requires_backup_and_old_reader_refuses_seventeen() {
    use tallyline_adapter_storage::api::{SqliteReferenceCache, StorageError};
    use tallyline_reference_data::api::AcceptedPriorHistoryStore;
    use tallyline_shared_kernel::api::model::Timestamp;
    use tallyline_submission::api::SubmissionJobRepository;
    use tallyline_submission::api::model::{
        JobId, LifecycleRefresh, RowEditability, UexLifecycleObservation, UexReportStatus,
    };
    use tallyline_workflows::api::{ReceiptEnrichmentStore, SubmissionLifecycleStore};
    let root =
        std::env::temp_dir().join(format!("tallyline-lifecycle-schema-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("application.db");
    fixture::restore(&path).unwrap();
    reader::open(&path).unwrap();
    let before = std::fs::read(&path).unwrap();
    assert!(matches!(
        SqliteReferenceCache::open(&path),
        Err(StorageError::MigrationBackupRequired)
    ));
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(
        SqliteReferenceCache::open_with_migration_backup(&path, |_, _| Err(
            StorageError::InvalidBackup
        ))
        .is_err()
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let backup = root.join("backup.db");
    let mut store = SqliteReferenceCache::open_with_migration_backup(&path, |cache, version| {
        assert_eq!(version, 16);
        cache.backup_into(&backup, Timestamp::from_seconds(1500), Some(6))?;
        Ok(())
    })
    .unwrap();
    let original = SubmissionJobRepository::load(&mut store, JobId(1))
        .unwrap()
        .unwrap();
    let mapping = store.receipt_mapping(original.id()).unwrap().unwrap();
    assert_eq!(
        original.attempts()[0].date_added(),
        Timestamp::from_seconds(900)
    );
    assert_eq!(mapping.receipts()[0].id(), 123);
    assert_eq!(
        mapping.receipts()[0].date_added(),
        Timestamp::from_seconds(900)
    );
    let history = store.accepted_history().unwrap();
    assert_eq!(history.records().len(), 1);
    assert_eq!(store.load_lifecycle(original.id()).unwrap(), None);
    let claim = store
        .claim_lifecycle_read(
            &original,
            LifecycleRefresh::Manual,
            Timestamp::from_seconds(1500),
        )
        .unwrap()
        .unwrap();
    let observation = UexLifecycleObservation::new(
        mapping.receipts()[0].clone(),
        UexReportStatus::Pending,
        RowEditability::Editable,
        Timestamp::from_seconds(1501),
    );
    let committed = store.commit_lifecycle_read(&claim, &[observation]).unwrap();
    assert_eq!(store.accepted_history().unwrap(), history);
    assert_eq!(
        SubmissionJobRepository::load(&mut store, original.id()).unwrap(),
        Some(original.clone())
    );
    drop(store);
    let migrated = std::fs::read(&path).unwrap();
    assert!(matches!(
        reader::open(&path),
        Err(StorageError::UnsupportedSchema {
            found: 17,
            supported: 16
        })
    ));
    assert_eq!(std::fs::read(&path).unwrap(), migrated);
    reader::open(&backup).unwrap();
    let backup_database = rusqlite::Connection::open(&backup).unwrap();
    assert_eq!(
        backup_database
            .pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
            .unwrap(),
        16
    );
    drop(backup_database);
    let mut reopened = SqliteReferenceCache::open(&path).unwrap();
    assert_eq!(
        reopened.load_lifecycle(original.id()).unwrap(),
        Some(committed)
    );
    assert_eq!(
        reopened.receipt_mapping(original.id()).unwrap(),
        Some(mapping)
    );
    assert_eq!(reopened.accepted_history().unwrap(), history);
    drop(reopened);
    std::fs::remove_dir_all(root).unwrap();
}
