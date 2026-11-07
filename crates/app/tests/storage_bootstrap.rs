// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Coordinated bootstrap contracts against real config and SQLite files.

use std::path::PathBuf;
use tallyline_adapter_files::api::StorageLayout;
use tallyline_adapter_storage::api::SqliteReferenceCache;
use tallyline_shared_kernel::api::Clock;
use tallyline_shared_kernel::api::model::Timestamp;

struct TestDirectory(PathBuf);
impl TestDirectory {
    fn new() -> std::io::Result<Self> {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "tallyline-bootstrap-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&path)?;
        Ok(Self(path))
    }
}
impl Drop for TestDirectory {
    fn drop(&mut self) {
        assert!(std::fs::remove_dir_all(&self.0).is_ok());
    }
}
struct FixedClock;
impl Clock for FixedClock {
    fn now(&self) -> Timestamp {
        Timestamp::from_millis(1234)
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_11__existing_database_and_exact_config_are_backed_up_together_before_migration() {
    let directory = TestDirectory::new().unwrap();
    let layout = StorageLayout::portable(&directory.0);
    assert!(layout.self_test().is_empty());
    drop(SqliteReferenceCache::open(layout.data().join("tallyline.db")).unwrap());
    let previous=br#"{ "schema_version":3,"app":{"theme":"dark","onboarding_complete":true,"update_checks":true}}"#;
    std::fs::write(layout.config().join("settings.json"), previous).unwrap();
    let prepared = tallyline_app::api::initialise_storage(&layout, &FixedClock).unwrap();
    assert!(
        prepared
            .settings
            .load()
            .unwrap()
            .settings
            .onboarding_complete()
    );
    drop(prepared);
    let backup = layout.data().join("backup/1234-v3");
    assert_eq!(
        std::fs::read(backup.join("settings.json")).unwrap(),
        previous
    );
    let cache = SqliteReferenceCache::open(backup.join("tallyline.db")).unwrap();
    let marker = cache.backup_metadata().unwrap().unwrap();
    assert_eq!(marker.taken_at, Timestamp::from_millis(1234));
    assert_eq!(marker.configuration_schema, Some(3));
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(backup.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["database_schema"], 17);
    assert_eq!(manifest["config_schema"], 3);
    assert_eq!(manifest["database_file"], "tallyline.db");
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_11__first_database_is_created_only_after_exact_config_only_backup() {
    let directory = TestDirectory::new().unwrap();
    let layout = StorageLayout::portable(&directory.0);
    assert!(layout.self_test().is_empty());
    let previous = br#"{"schema_version":2,"app":{"theme":"dark","onboarding_complete":false}}"#;
    std::fs::write(layout.config().join("settings.json"), previous).unwrap();
    drop(tallyline_app::api::initialise_storage(&layout, &FixedClock).unwrap());
    let backup = layout.data().join("backup/1234-v2");
    assert_eq!(
        std::fs::read(backup.join("settings.json")).unwrap(),
        previous
    );
    assert!(!backup.join("tallyline.db").exists());
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(backup.join("manifest.json")).unwrap()).unwrap();
    assert!(manifest["database_schema"].is_null());
    assert!(manifest["database_file"].is_null());
    assert!(layout.data().join("tallyline.db").exists());
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_11__backup_failure_preserves_both_existing_files_and_refuses_startup() {
    let directory = TestDirectory::new().unwrap();
    let layout = StorageLayout::portable(&directory.0);
    assert!(layout.self_test().is_empty());
    let database = layout.data().join("tallyline.db");
    drop(SqliteReferenceCache::open(&database).unwrap());
    let original_database = std::fs::read(&database).unwrap();
    let previous = br#"{"schema_version":3,"app":{"theme":"dark","onboarding_complete":true}}"#;
    let settings = layout.config().join("settings.json");
    std::fs::write(&settings, previous).unwrap();
    std::fs::write(layout.data().join("backup"), b"foreign file").unwrap();
    assert!(tallyline_app::api::initialise_storage(&layout, &FixedClock).is_err());
    assert_eq!(std::fs::read(&settings).unwrap(), previous);
    assert_eq!(std::fs::read(&database).unwrap(), original_database);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_11__invalid_config_refuses_database_creation_and_current_bootstrap_does_not_backup() {
    let directory = TestDirectory::new().unwrap();
    let layout = StorageLayout::portable(&directory.0);
    assert!(layout.self_test().is_empty());
    let settings = layout.config().join("settings.json");
    std::fs::write(&settings, br#"{"schema_version":99}"#).unwrap();
    assert!(tallyline_app::api::initialise_storage(&layout, &FixedClock).is_err());
    assert!(!layout.data().join("tallyline.db").exists());
    std::fs::write(&settings, br#"{"schema_version":6}"#).unwrap();
    drop(tallyline_app::api::initialise_storage(&layout, &FixedClock).unwrap());
    drop(tallyline_app::api::initialise_storage(&layout, &FixedClock).unwrap());
    assert!(!layout.data().join("backup").exists());
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_11__joint_schema_upgrades_reuse_only_the_exact_original_config_backup() {
    let directory = TestDirectory::new().unwrap();
    let layout = StorageLayout::portable(&directory.0);
    assert!(layout.self_test().is_empty());
    let database = layout.data().join("tallyline.db");
    {
        let database = rusqlite::Connection::open(&database).unwrap();
        database.execute_batch("CREATE TABLE ref_cache(endpoint TEXT PRIMARY KEY,fetched_at TEXT NOT NULL,payload TEXT NOT NULL); CREATE TABLE ref_commodities(id INTEGER PRIMARY KEY,name TEXT NOT NULL,code TEXT NOT NULL,available_live INTEGER NOT NULL); PRAGMA user_version=1;").unwrap();
    }
    let previous=br#"{ "schema_version":3,"app":{"theme":"light","onboarding_complete":true,"update_checks":true}}"#;
    std::fs::write(layout.config().join("settings.json"), previous).unwrap();
    let mut prepared = tallyline_app::api::initialise_storage(&layout, &FixedClock).unwrap();
    let backup = layout.data().join("backup/1234-v3");
    assert_eq!(
        std::fs::read(backup.join("settings.json")).unwrap(),
        previous
    );
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(backup.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["database_schema"], 1);
    assert_eq!(manifest["config_schema"], 3);
    assert_eq!(
        std::fs::read_dir(layout.data().join("backup"))
            .unwrap()
            .count(),
        1
    );
    assert_eq!(
        prepared.settings.backup_source().unwrap().unwrap().schema,
        6
    );
    assert_eq!(
        prepared
            .cache
            .backup_into(
                &directory.0.join("after.db"),
                Timestamp::from_millis(2000),
                Some(6)
            )
            .unwrap(),
        17
    );
}

#[path = "../../adapter-storage/tests/support/schema_fifteen_fixture.rs"]
mod schema_fifteen_fixture;

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_11_r_nf_11__schema_fifteen_semantic_upgrade_backs_up_exact_database_and_current_config_together()
 {
    let directory = TestDirectory::new().unwrap();
    let layout = StorageLayout::portable(&directory.0);
    assert!(layout.self_test().is_empty());
    let database = layout.data().join("tallyline.db");
    schema_fifteen_fixture::restore(&database).unwrap();
    let settings = layout.config().join("settings.json");
    let config = br#"{ "schema_version":6,"app":{"theme":"dark","onboarding_complete":true}}"#;
    std::fs::write(&settings, config).unwrap();
    let original = std::fs::read(&database).unwrap();
    let backup_root = layout.data().join("backup");
    std::fs::write(&backup_root, b"injected obstruction").unwrap();
    assert!(tallyline_app::api::initialise_storage(&layout, &FixedClock).is_err());
    assert_eq!(std::fs::read(&database).unwrap(), original);
    assert_eq!(std::fs::read(&settings).unwrap(), config);
    std::fs::remove_file(&backup_root).unwrap();
    let prepared = tallyline_app::api::initialise_storage(&layout, &FixedClock).unwrap();
    drop(prepared);
    let backup = backup_root.join("1234-v6");
    assert_eq!(std::fs::read(backup.join("settings.json")).unwrap(), config);
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(backup.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["database_schema"], 15);
    assert_eq!(manifest["config_schema"], 6);
    let db = rusqlite::Connection::open(backup.join("tallyline.db")).unwrap();
    assert_eq!(
        db.pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
            .unwrap(),
        15
    );
    let history: String = db
        .query_row(
            "SELECT payload FROM submission_jobs WHERE id=1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    drop(db);
    let current = rusqlite::Connection::open(&database).unwrap();
    assert_eq!(
        current
            .pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
            .unwrap(),
        17
    );
    assert_eq!(
        current
            .query_row(
                "SELECT payload FROM submission_jobs WHERE id=1",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
        history
    );
    drop(current);
    drop(tallyline_app::api::initialise_storage(&layout, &FixedClock).unwrap());
    assert_eq!(std::fs::read_dir(&backup_root).unwrap().count(), 1);
}
