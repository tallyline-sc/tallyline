// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Previous-schema affinity admission and unchanged persisted data through real SQLite.

use tallyline_adapter_storage::api::{SqliteReferenceCache, StorageError};
use tallyline_reporting::api::ReportRepository;
use tallyline_reporting::api::model::ReportId;
use tallyline_shared_kernel::api::model::Timestamp;
#[path = "support/schema_thirteen_reader.rs"]
mod reader;

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_13_r_nf_11__schema_thirteen_requires_backup_preserves_original_payload_and_refuses_old_readers()
 {
    let root = std::env::temp_dir().join(format!(
        "tallyline-affinity-migration-{}",
        std::process::id()
    ));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("previous.db");
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/schema-thirteen.json")).unwrap();
    let db = rusqlite::Connection::open(&path).unwrap();
    for statement in fixture["schema"].as_array().unwrap() {
        db.execute_batch(statement.as_str().unwrap()).unwrap();
    }
    let report: (i64, i64, String, String) =
        serde_json::from_value(fixture["report"].clone()).unwrap();
    db.execute(
        "INSERT INTO reports(id,version,payload,phase) VALUES(?1,?2,?3,?4)",
        (&report.0, &report.1, &report.2, &report.3),
    )
    .unwrap();
    db.pragma_update(None, "user_version", 13).unwrap();
    drop(db);
    reader::open(&path).unwrap();
    let original = std::fs::read(&path).unwrap();
    assert!(matches!(
        SqliteReferenceCache::open(&path),
        Err(StorageError::MigrationBackupRequired)
    ));
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert!(
        SqliteReferenceCache::open_with_migration_backup(&path, |_, _| Err(
            StorageError::InvalidBackup
        ))
        .is_err()
    );
    assert_eq!(std::fs::read(&path).unwrap(), original);
    let backup = root.join("backup.db");
    let mut store = SqliteReferenceCache::open_with_migration_backup(&path, |cache, version| {
        assert_eq!(version, 13);
        cache.backup_into(&backup, Timestamp::from_seconds(2000), Some(6))?;
        Ok(())
    })
    .unwrap();
    let loaded = store.load_draft(ReportId(1)).unwrap().unwrap();
    assert_eq!(loaded.faction_affinity(), None);
    assert_eq!(loaded.version(), 1);
    assert!(matches!(
        reader::open(&path),
        Err(StorageError::UnsupportedSchema {
            found: 17,
            supported: 13
        })
    ));
    drop(store);
    let db = rusqlite::Connection::open(&path).unwrap();
    assert_eq!(
        db.query_row("SELECT payload FROM reports WHERE id=1", [], |row| row
            .get::<_, String>(
            0
        ))
        .unwrap(),
        report.2
    );
    assert_eq!(
        db.pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
            .unwrap(),
        17
    );
    drop(db);
    let backed_up = rusqlite::Connection::open(&backup).unwrap();
    assert_eq!(
        backed_up
            .pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
            .unwrap(),
        13
    );
    drop(backed_up);
    let empty = root.join("empty.db");
    drop(SqliteReferenceCache::open(&empty).unwrap());
    let db = rusqlite::Connection::open(&empty).unwrap();
    assert_eq!(
        db.pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
            .unwrap(),
        17
    );
    drop(db);
    std::fs::remove_dir_all(root).unwrap();
}
