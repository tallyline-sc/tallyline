// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Synthetic schema-nine SQLite compatibility through the public storage ports.

use std::path::Path;
use tallyline_adapter_storage::api::{SqliteReferenceCache, StorageError};
use tallyline_shared_kernel::api::model::Timestamp;
use tallyline_submission::api::SubmissionJobRepository;
use tallyline_submission::api::model::{JobId, JobPhase};
use tallyline_workflows::api::EventOutbox;

#[path = "schema_nine_reader.rs"]
mod schema_nine_reader;

type TestResult = Result<(), Box<dyn std::error::Error>>;

pub(super) fn verify(directory: &Path) -> TestResult {
    let path = directory.join("legacy-nine.db");
    let backup = directory.join("legacy-nine-backup.db");
    let connection = rusqlite::Connection::open(&path)?;
    connection.execute_batch(include_str!("../fixtures/schema-nine.sql"))?;
    let release = serde_json::json!({
        "lifecycle":{"state":"queued","release":{"account":"fixture-account","mode":"production","environment":"live","release_version":5,"job":1},"age":null,"finding":null,"newer_observations":[]},
        "terminal":11,"side":"buy","environment":"live","observed_at":"900000","game_version":"4.10.2","attachment":null,
        "rows":[{"id":1,"commodity":42,"price":"123.45","stock":"0","status":3,"price_decision":{"confirmation":null,"deviation":null,"unreviewed":false,"revoked":false},"stock_decision":{"confirmation":null,"deviation":null,"unreviewed":false,"revoked":false},"status_confirmation":null,"status_deviation":null,"status_unreviewed":false,"status_confirmation_revoked":false}]
    });
    let mut current = release.clone();
    *current
        .get_mut("lifecycle")
        .and_then(|value| value.get_mut("state"))
        .ok_or("missing fixture lifecycle")? = serde_json::json!("outcome_unknown");
    let job = serde_json::json!({
        "header":{"report":1,"report_version":6,"account":"fixture-account","mode":"production","terminal":11,"side":"buy","environment":"live","game_version":"4.10.2","observed_at":"900000"},
        "rows":[{"id":1,"commodity":42,"price":"123.45","stock":"0","status":3}],"newer_approvals":[],"phase":"outcome_unknown",
        "attempts":[{"started_at":"1000000","rows":1,"payload_hash":[7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7],"date_added":"900000","ended_at":"1001000","outcome":"unknown"}],
        "result":"unknown","review_reason":null
    });
    connection.execute(
        "INSERT INTO reports(id,version,payload,phase) VALUES(1,7,?1,'outcome_unknown')",
        [current.to_string()],
    )?;
    connection.execute("INSERT INTO report_recents VALUES(1,1)", [])?;
    connection.execute("INSERT INTO submission_jobs(id,report_id,revision,phase,payload,release_report,release_report_version) VALUES(1,1,3,'outcome_unknown',?1,?2,6)", (job.to_string(),release.to_string()))?;
    connection.execute_batch("INSERT INTO domain_outbox VALUES('reporting',1,5,'released',0); INSERT INTO domain_outbox VALUES('submission',1,3,'outcome_unknown',0);")?;
    drop(connection);
    schema_nine_reader::open(&path)?;
    let bytes = std::fs::read(&path)?;
    assert!(matches!(
        SqliteReferenceCache::open(&path),
        Err(StorageError::MigrationBackupRequired)
    ));
    assert_eq!(std::fs::read(&path)?, bytes);
    assert!(
        SqliteReferenceCache::open_with_migration_backup(&path, |_, _| Err(
            StorageError::MigrationBackupRequired
        ))
        .is_err()
    );
    assert_eq!(std::fs::read(&path)?, bytes);
    let fault = rusqlite::Connection::open(&path)?;
    fault.execute_batch("CREATE VIEW submission_service_fences AS SELECT 1;")?;
    drop(fault);
    assert!(
        SqliteReferenceCache::open_with_migration_backup(&path, |cache, version| {
            assert_eq!(version, 9);
            cache.backup_into(&backup, Timestamp::from_seconds(1500), Some(6))?;
            Ok(())
        })
        .is_err()
    );
    let fault = rusqlite::Connection::open(&path)?;
    assert_eq!(
        fault.pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))?,
        9
    );
    assert_eq!(
        fault.query_row(
            "SELECT payload FROM submission_jobs WHERE id=1",
            [],
            |row| row.get::<_, String>(0)
        )?,
        job.to_string()
    );
    fault.execute_batch("DROP VIEW submission_service_fences;")?;
    drop(fault);
    let mut migrated =
        SqliteReferenceCache::open_with_migration_backup(&path, |cache, version| {
            assert_eq!(version, 9);
            cache.backup_into(
                &directory.join("legacy-nine-final-backup.db"),
                Timestamp::from_seconds(1501),
                Some(6),
            )?;
            Ok(())
        })?;
    let restored = SubmissionJobRepository::load(&mut migrated, JobId(1))
        .map_err(|error| std::io::Error::other(format!("{error:?}")))?
        .ok_or_else(|| std::io::Error::other("missing legacy job"))?;
    assert_eq!(restored.phase(), JobPhase::OutcomeUnknown);
    assert_eq!(restored.attempts().len(), 1);
    assert_eq!(
        restored
            .attempts()
            .first()
            .ok_or("missing legacy attempt")?
            .date_added(),
        Timestamp::from_seconds(900)
    );
    assert_eq!(
        restored
            .attempts()
            .first()
            .ok_or("missing legacy attempt")?
            .payload_hash()
            .bytes(),
        [7; 32]
    );
    assert!(
        !restored
            .attempts()
            .first()
            .ok_or("missing legacy attempt")?
            .receipt_checked()
    );
    assert!(!restored.service_pause_acknowledged());
    assert!(
        restored
            .attempts()
            .first()
            .ok_or("missing legacy attempt")?
            .account_evidence()
            .is_none()
    );
    assert_eq!(
        restored
            .request()
            .rows()
            .first()
            .ok_or("missing legacy row")?
            .price()
            .to_string(),
        "123.45"
    );
    assert_eq!(
        migrated
            .pending_events()
            .map_err(|error| std::io::Error::other(format!("{error:?}")))?
            .len(),
        2
    );
    drop(migrated);
    let mut reopened = SqliteReferenceCache::open(&path)?;
    assert_eq!(
        SubmissionJobRepository::load(&mut reopened, JobId(1))
            .map_err(|error| std::io::Error::other(format!("{error:?}")))?,
        Some(restored)
    );
    assert_eq!(
        reopened
            .pending_events()
            .map_err(|error| std::io::Error::other(format!("{error:?}")))?
            .len(),
        2
    );
    drop(reopened);
    assert!(matches!(
        schema_nine_reader::open(&path),
        Err(StorageError::UnsupportedSchema {
            found: 17,
            supported: 9
        })
    ));
    let after = rusqlite::Connection::open(&path)?;
    assert_eq!(
        after.pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))?,
        17
    );
    assert_eq!(
        after.query_row(
            "SELECT payload FROM submission_jobs WHERE id=1",
            [],
            |row| row.get::<_, String>(0)
        )?,
        job.to_string()
    );
    assert_eq!(
        after.query_row("SELECT payload FROM reports WHERE id=1", [], |row| row
            .get::<_, String>(
            0
        ))?,
        current.to_string()
    );
    Ok(())
}
