// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Source-bound previous-format admission and backup contracts through real SQLite.

use rusqlite::Connection;
use tallyline_adapter_storage::api::{SqliteReferenceCache, StorageError};
use tallyline_shared_kernel::api::model::Timestamp;
use tallyline_submission::api::SubmissionJobRepository;
use tallyline_submission::api::model::{
    JobId, JobPhase, RowFailureKind, SubmissionResult, SubmissionRowResult,
};

#[path = "support/schema_eleven_reader.rs"]
mod schema_eleven_reader;

struct TestDirectory(std::path::PathBuf);
impl TestDirectory {
    fn new() -> std::io::Result<Self> {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "tallyline-schema-eleven-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&path)?;
        Ok(Self(path))
    }
}
impl Drop for TestDirectory {
    fn drop(&mut self) {
        assert!(
            std::fs::remove_dir_all(&self.0).is_ok(),
            "temporary migration database cleanup failed"
        );
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_11_r_sub_9__genuine_schema_eleven_requires_backup_preserves_payload_and_refuses_new_reader_format()
 {
    let directory = TestDirectory::new().unwrap();
    let path = directory.0.join("previous.db");
    let connection = Connection::open(&path).unwrap();
    connection
        .execute_batch(include_str!("fixtures/schema-nine.sql"))
        .unwrap();
    connection
        .execute_batch(include_str!("fixtures/schema-ten-extension.sql"))
        .unwrap();
    connection
        .execute_batch(include_str!("fixtures/schema-eleven-extension.sql"))
        .unwrap();
    connection
        .execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0;")
        .unwrap();
    let report = include_str!("fixtures/schema-eleven-report.json").trim();
    let job = include_str!("fixtures/schema-eleven-job.json").trim();
    connection
        .execute(
            "INSERT INTO reports(id,version,payload,phase) VALUES(1,7,?1,'partially_accepted')",
            [report],
        )
        .unwrap();
    let mut released: serde_json::Value = serde_json::from_str(report).unwrap();
    released["lifecycle"]["state"] = serde_json::json!("queued");
    connection.execute("INSERT INTO submission_jobs(id,report_id,revision,phase,payload,release_report,release_report_version) VALUES(1,1,3,'partially_accepted',?1,?2,6)",(job,released.to_string())).unwrap();
    schema_eleven_reader::open(&path).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    assert!(matches!(
        SqliteReferenceCache::open(&path),
        Err(StorageError::MigrationBackupRequired)
    ));
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert!(
        SqliteReferenceCache::open_with_migration_backup(&path, |_, _| Err(
            StorageError::InvalidBackup
        ))
        .is_err()
    );
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    let mut blocker = None;
    let failed = SqliteReferenceCache::open_with_migration_backup(&path, |cache, version| {
        assert_eq!(version, 11);
        cache.backup_into(
            &directory.0.join("failed-attempt-backup.db"),
            Timestamp::from_seconds(1500),
            Some(6),
        )?;
        let locked = Connection::open(&path)?;
        locked.execute_batch("BEGIN IMMEDIATE")?;
        blocker = Some(locked);
        Ok(())
    });
    assert!(
        failed.is_err(),
        "a write lock must roll back the forward gate"
    );
    drop(blocker);
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
            .unwrap(),
        11
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT payload FROM submission_jobs WHERE id=1",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
        job
    );
    let backup = directory.0.join("coordinated-backup.db");
    let mut migrated = SqliteReferenceCache::open_with_migration_backup(&path, |cache, version| {
        assert_eq!(version, 11);
        assert_eq!(
            cache.backup_into(&backup, Timestamp::from_seconds(1501), Some(6))?,
            11
        );
        Ok(())
    })
    .unwrap();
    let restored = SubmissionJobRepository::load(&mut migrated, JobId(1))
        .unwrap()
        .unwrap();
    assert_eq!(restored.phase(), JobPhase::PartiallyAccepted);
    assert_eq!(restored.attempts().len(), 1);
    assert!(restored.resolution_actions().is_empty());
    let Some(SubmissionResult::PartialError { code, .. }) = restored.result() else {
        panic!("previous-schema refusal must retain its original code");
    };
    assert_eq!(code.as_str(), "database_error");
    assert_eq!(code.row_failure(), RowFailureKind::Database);
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
            .unwrap(),
        17
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT payload FROM submission_jobs WHERE id=1",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
        job
    );
    assert_eq!(
        connection
            .query_row("SELECT payload FROM reports WHERE id=1", [], |row| row
                .get::<_, String>(
                0
            ))
            .unwrap(),
        report
    );
    assert!(matches!(
        schema_eleven_reader::open(&path),
        Err(StorageError::UnsupportedSchema {
            found: 17,
            supported: 11
        })
    ));
    schema_eleven_reader::open(&backup).unwrap();
    let backup_connection = Connection::open(&backup).unwrap();
    assert_eq!(
        backup_connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
            .unwrap(),
        11
    );
    assert_eq!(
        backup_connection
            .query_row(
                "SELECT payload FROM submission_jobs WHERE id=1",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
        job
    );
    assert_eq!(
        backup_connection
            .query_row("SELECT config_schema FROM backup_meta", [], |row| row
                .get::<_, u32>(0))
            .unwrap(),
        6
    );
    drop(backup_connection);
    drop(connection);
    drop(migrated);
    let mut reopened = SqliteReferenceCache::open(&path).unwrap();
    assert_eq!(
        SubmissionJobRepository::load(&mut reopened, JobId(1)).unwrap(),
        Some(restored)
    );
    let fault = Connection::open(&path).unwrap();
    for (legacy, expected, cooldown) in [
        ("duplicated_report", RowFailureKind::Duplicate, true),
        (
            "invalid_status_sell",
            RowFailureKind::InvalidSellStatus,
            false,
        ),
        ("Future_Code", RowFailureKind::Unrecognised, false),
    ] {
        fault.execute(
            "UPDATE submission_jobs SET payload=json_set(?1,'$.result.partial_error[0]',?2,'$.result.partial_error[1]',0) WHERE id=1",
            (job, legacy),
        ).unwrap();
        let admitted = SubmissionJobRepository::load(&mut reopened, JobId(1))
            .unwrap()
            .unwrap();
        let result = admitted
            .row_result(admitted.request().rows()[0].id())
            .unwrap();
        let SubmissionRowResult::Rejected(code) = result else {
            panic!("legacy named refusal must retain its row disposition");
        };
        assert_eq!(code.as_str(), legacy);
        assert_eq!(code.row_failure(), expected);
        assert_eq!(result.retains_cooldown(), cooldown);
    }
    fault
        .execute("UPDATE submission_jobs SET payload=?1 WHERE id=1", [job])
        .unwrap();
    drop(reopened);
}
