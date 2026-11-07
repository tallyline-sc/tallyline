// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Genuine schema-fourteen migration, backup admission and old-reader refusal.

use tallyline_adapter_storage::api::{SqliteReferenceCache, StorageError};
use tallyline_shared_kernel::api::model::Timestamp;
use tallyline_submission::api::SubmissionJobRepository;
use tallyline_submission::api::model::JobId;
#[path = "support/schema_fourteen_reader.rs"]
mod reader;

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_4_r_nf_11__previous_fourteen_requires_backup_and_preserves_immutable_history() {
    let root =
        std::env::temp_dir().join(format!("tallyline-split-migration-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("previous.db");
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/schema-fourteen.json")).unwrap();
    let db = rusqlite::Connection::open(&path).unwrap();
    for statement in fixture["schema"].as_array().unwrap() {
        db.execute_batch(statement.as_str().unwrap()).unwrap();
    }
    db.execute_batch("BEGIN; PRAGMA defer_foreign_keys=ON;")
        .unwrap();
    for table in fixture["tables"].as_array().unwrap() {
        for row in table["rows"].as_array().unwrap() {
            let values = row
                .as_array()
                .unwrap()
                .iter()
                .map(|value| match value {
                    serde_json::Value::Null => rusqlite::types::Value::Null,
                    serde_json::Value::String(value) => rusqlite::types::Value::Text(value.clone()),
                    serde_json::Value::Number(value) => {
                        rusqlite::types::Value::Integer(value.as_i64().unwrap())
                    }
                    serde_json::Value::Bool(_)
                    | serde_json::Value::Array(_)
                    | serde_json::Value::Object(_) => panic!("unsupported source fixture value"),
                })
                .collect::<Vec<_>>();
            let placeholders = vec!["?"; values.len()].join(",");
            db.execute(
                &format!(
                    "INSERT INTO {} VALUES({placeholders})",
                    table["name"].as_str().unwrap()
                ),
                rusqlite::params_from_iter(values),
            )
            .unwrap();
        }
    }
    db.execute_batch("COMMIT;").unwrap();
    db.pragma_update(None, "user_version", 14).unwrap();
    let original_payload: String = db
        .query_row(
            "SELECT payload FROM submission_jobs WHERE id=1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    drop(db);
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
        assert_eq!(version, 14);
        cache.backup_into(&backup, Timestamp::from_seconds(2000), Some(6))?;
        Ok(())
    })
    .unwrap();
    let job = SubmissionJobRepository::load(&mut store, JobId(1))
        .unwrap()
        .unwrap();
    assert_eq!(job.attempts().len(), 1);
    assert_eq!(job.request().header().faction_affinity.unwrap().value(), 0);
    assert!(matches!(
        reader::open(&path),
        Err(StorageError::UnsupportedSchema {
            found: 17,
            supported: 14
        })
    ));
    drop(store);
    let db = rusqlite::Connection::open(&path).unwrap();
    assert_eq!(
        db.query_row(
            "SELECT payload FROM submission_jobs WHERE id=1",
            [],
            |row| row.get::<_, String>(0)
        )
        .unwrap(),
        original_payload
    );
    assert_eq!(
        db.pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
            .unwrap(),
        17
    );
    drop(db);
    let db = rusqlite::Connection::open(&backup).unwrap();
    assert_eq!(
        db.pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
            .unwrap(),
        14
    );
    drop(db);
    std::fs::remove_dir_all(root).unwrap();
}
