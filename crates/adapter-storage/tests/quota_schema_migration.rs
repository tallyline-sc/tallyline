// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Authentic previous-schema quota migration and rollback through real SQLite.

use tallyline_adapter_storage::api::{SqliteReferenceCache, StorageError};
use tallyline_shared_kernel::api::model::Timestamp;
use tallyline_submission::api::SubmissionJobRepository;
use tallyline_submission::api::model::JobId;
#[path = "support/schema_twelve_fixture.rs"]
mod fixture;
#[path = "support/schema_twelve_reader.rs"]
mod reader;

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_sub_11__genuine_twelve_migration_preserves_history_and_rolls_back_failure() {
    let root =
        std::env::temp_dir().join(format!("tallyline-quota-migration-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("previous.db");
    let expected = fixture::create(&path).unwrap();
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
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch("CREATE TABLE submission_token_bindings(blocker INTEGER)")
        .unwrap();
    drop(db);
    assert!(
        SqliteReferenceCache::open_with_migration_backup(&path, |cache, version| {
            assert_eq!(version, 12);
            cache.backup_into(
                &root.join("failed-backup.db"),
                Timestamp::from_seconds(1600),
                Some(6),
            )?;
            Ok(())
        })
        .is_err()
    );
    let db = rusqlite::Connection::open(&path).unwrap();
    assert_eq!(
        db.pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
            .unwrap(),
        12
    );
    assert_eq!(
        db.query_row(
            "SELECT payload FROM submission_jobs WHERE id=1",
            [],
            |row| row.get::<_, String>(0)
        )
        .unwrap(),
        expected.job
    );
    assert!(
        db.prepare("SELECT token_scope FROM submission_service_fences")
            .is_err()
    );
    db.execute_batch("DROP TABLE submission_token_bindings")
        .unwrap();
    drop(db);
    let backup = root.join("coordinated-backup.db");
    let mut store = SqliteReferenceCache::open_with_migration_backup(&path, |cache, version| {
        assert_eq!(version, 12);
        cache.backup_into(&backup, Timestamp::from_seconds(1601), Some(6))?;
        Ok(())
    })
    .unwrap();
    let job = SubmissionJobRepository::load(&mut store, JobId(1))
        .unwrap()
        .unwrap();
    assert_eq!(job.resolution_actions().len(), 1);
    assert_eq!(job.attempts().len(), 1);
    assert_eq!(
        reader::open(&path).err().map(|error| error.to_string()),
        Some("database schema 17 is newer than supported schema 12".to_owned())
    );
    let db = rusqlite::Connection::open(&path).unwrap();
    assert_eq!(
        db.query_row(
            "SELECT payload FROM submission_jobs WHERE id=1",
            [],
            |row| row.get::<_, String>(0)
        )
        .unwrap(),
        expected.job
    );
    assert_eq!(
        db.query_row("SELECT payload FROM reports WHERE id=1", [], |row| row
            .get::<_, String>(
            0
        ))
        .unwrap(),
        expected.report
    );
    drop(db);
    drop(store);
    let backup_db = rusqlite::Connection::open(&backup).unwrap();
    assert_eq!(
        backup_db
            .pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
            .unwrap(),
        12
    );
    assert_eq!(
        backup_db
            .query_row("SELECT config_schema FROM backup_meta", [], |row| row
                .get::<_, u32>(0))
            .unwrap(),
        6
    );
    drop(backup_db);
    std::fs::remove_dir_all(root).unwrap();
}
