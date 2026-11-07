// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Semantic reader gate for permanent-refusal copy histories.

use super::{fixture, reader};

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_11_r_nf_11__genuine_fifteen_requires_backup_and_old_reader_refuses_seventeen() {
    use tallyline_adapter_storage::api::{SqliteReferenceCache, StorageError};
    use tallyline_reporting::api::ReportHistoryRepository;
    use tallyline_shared_kernel::api::model::Timestamp;
    use tallyline_submission::api::SubmissionJobRepository;
    use tallyline_submission::api::model::{JobId, JobPhase};
    use tallyline_workflows::api::{EventOutbox, ResolutionDraftStore};
    let root =
        std::env::temp_dir().join(format!("tallyline-rejected-schema-{}", std::process::id()));
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
        assert_eq!(version, 15);
        cache.backup_into(&backup, Timestamp::from_seconds(2000), Some(6))?;
        Ok(())
    })
    .unwrap();
    let original = SubmissionJobRepository::load(&mut store, JobId(1))
        .unwrap()
        .unwrap();
    assert_eq!(original.phase(), JobPhase::Rejected);
    assert_eq!(original.attempts().len(), 1);
    assert!(original.resolution_actions().is_empty());
    let report = store
        .retained_report(original.request().header().report)
        .unwrap()
        .unwrap();
    let events = store.pending_events().unwrap();
    let copy = store
        .duplicate_resolved(
            original.id(),
            original.revision(),
            Timestamp::from_seconds(2001),
        )
        .unwrap();
    assert_eq!(copy.origin.request(), original.request());
    assert_eq!(copy.origin.result(), original.result());
    assert_eq!(copy.origin.attempts(), original.attempts());
    assert_eq!(store.retained_report(report.id()).unwrap(), Some(report));
    assert_eq!(store.pending_events().unwrap().len(), events.len() + 1);
    drop(store);
    let migrated = std::fs::read(&path).unwrap();
    assert!(matches!(
        reader::open(&path),
        Err(StorageError::UnsupportedSchema {
            found: 17,
            supported: 15
        })
    ));
    assert_eq!(std::fs::read(&path).unwrap(), migrated);
    let db = rusqlite::Connection::open(&backup).unwrap();
    assert_eq!(
        db.pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
            .unwrap(),
        15
    );
    drop(db);
    let mut reopened = SqliteReferenceCache::open(&path).unwrap();
    assert_eq!(
        SubmissionJobRepository::load(&mut reopened, JobId(1)).unwrap(),
        Some(copy.origin)
    );
    drop(reopened);
    std::fs::remove_dir_all(root).unwrap();
}
