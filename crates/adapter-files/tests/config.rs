// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! ConfigFile contract tests against actual files.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use tallyline_adapter_files::api::ConfigFile;
use tallyline_workflows::api::AppSettingsStore;
use tallyline_workflows::api::model::{AppSettings, Theme};

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> std::io::Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let directory = std::env::temp_dir().join(format!(
            "tallyline-config-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&directory)?;
        Ok(Self(directory))
    }
}

impl Drop for TestDirectory {
    #[expect(
        clippy::unwrap_used,
        reason = "Cleanup failure must fail the real-filesystem contract test."
    )]
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

fn backed_store(path: PathBuf) -> ConfigFile {
    let backup = path.with_extension("backup.json");
    ConfigFile::new(path).with_migration_backup(move |_, bytes| std::fs::write(&backup, bytes))
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_5__settings_survive_reopening_and_atomic_replacement() {
    let directory = TestDirectory::new().unwrap();
    let path = directory.0.join("settings.json");
    let mut store = ConfigFile::new(path.clone());
    let initial = store.load().unwrap();
    assert_eq!(initial.settings, AppSettings::default());
    assert!(initial.errors.is_empty());
    store
        .save(&AppSettings::from_persisted(Theme::Light, false))
        .unwrap();
    store
        .save(&AppSettings::from_persisted(Theme::HighContrast, true))
        .unwrap();
    let reopened = ConfigFile::new(path).load().unwrap();
    assert_eq!(
        reopened.settings,
        AppSettings::from_persisted(Theme::HighContrast, true)
    );
    assert!(reopened.errors.is_empty());
    assert_eq!(std::fs::read_dir(&directory.0).unwrap().count(), 1);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_5__invalid_settings_report_rejected_value_and_default() {
    let directory = TestDirectory::new().unwrap();
    let path = directory.0.join("settings.json");
    std::fs::write(
        &path,
        br#"{"schema_version":1,"app":{"theme":42,"onboarding_complete":false}}"#,
    )
    .unwrap();
    let loaded = ConfigFile::new(path).load().unwrap();
    assert_eq!(loaded.settings.theme(), Theme::System);
    assert_eq!(loaded.errors.len(), 1);
    let issue = loaded.errors.first().unwrap();
    assert_eq!(issue.key, "settings.theme");
    assert_eq!(issue.rejected, "42");
    assert_eq!(issue.default, "system");
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_2_q_portable_r_nf_5__portable_storage_stays_under_data_and_failed_self_tests_are_reported()
{
    let directory = TestDirectory::new().unwrap();
    let layout = tallyline_adapter_files::api::StorageLayout::portable(&directory.0);
    assert_eq!(layout.config(), directory.0.join("data/config"));
    assert_eq!(layout.data(), directory.0.join("data/data"));
    assert_eq!(layout.logs(), directory.0.join("data/logs"));
    assert_eq!(layout.cache(), directory.0.join("data/cache"));
    assert!(layout.self_test().is_empty());
    assert!(layout.config().is_dir());
    std::fs::remove_dir(layout.logs()).unwrap();
    std::fs::write(layout.logs(), b"blocked").unwrap();
    let failures = layout.self_test();
    assert_eq!(failures.len(), 1);
    assert_eq!(failures.first().unwrap().path, layout.logs());
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_5__installed_layout_uses_only_profile_and_xdg_roots() {
    let directory = TestDirectory::new().unwrap();
    let roaming = directory.0.join("roaming");
    let local = directory.0.join("local");
    let windows = tallyline_adapter_files::api::StorageLayout::installed_windows(&roaming, &local);
    assert_eq!(windows.config(), roaming.join("Tallyline"));
    assert_eq!(windows.data(), local.join("Tallyline/data"));
    assert_eq!(windows.logs(), local.join("Tallyline/logs"));
    assert_eq!(windows.cache(), local.join("Tallyline/cache"));
    let linux = tallyline_adapter_files::api::StorageLayout::installed_linux(
        &roaming,
        &local,
        &directory.0.join("state"),
        &directory.0.join("cache"),
    );
    assert_eq!(linux.config(), roaming.join("tallyline"));
    assert_eq!(linux.data(), local.join("tallyline"));
    assert_eq!(linux.logs(), directory.0.join("state/tallyline/logs"));
    assert_eq!(linux.cache(), directory.0.join("cache/tallyline"));
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_5__saving_app_preferences_preserves_other_context_sections() {
    let directory = TestDirectory::new().unwrap();
    let path = directory.0.join("settings.json");
    std::fs::write(&path, br#"{"schema_version":1,"capture":{"folders":[]},"app":{"theme":"dark","onboarding_complete":false}}"#).unwrap();
    backed_store(path.clone())
        .save(&AppSettings::from_persisted(Theme::Light, false))
        .unwrap();
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(
        saved.get("capture"),
        Some(&serde_json::json!({"folders": []}))
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_5__future_schema_is_never_overwritten() {
    let directory = TestDirectory::new().unwrap();
    let path = directory.0.join("settings.json");
    let original = br#"{"schema_version":99,"app":{"theme":"dark","onboarding_complete":false}}"#;
    std::fs::write(&path, original).unwrap();
    let mut store = ConfigFile::new(path.clone());
    assert!(store.load().is_err());
    assert!(store.save(&AppSettings::default()).is_err());
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_5__installed_data_and_logs_match_the_architecture_storage_rows() {
    use tallyline_adapter_files::api::StorageLayout;
    let windows = StorageLayout::installed_windows(
        std::path::Path::new("roaming"),
        std::path::Path::new("local"),
    );
    let linux = StorageLayout::installed_linux(
        std::path::Path::new("config"),
        std::path::Path::new("data"),
        std::path::Path::new("state"),
        std::path::Path::new("cache"),
    );
    assert_eq!(
        (windows.data(), linux.logs()),
        (
            std::path::Path::new("local/Tallyline/data"),
            std::path::Path::new("state/tallyline/logs")
        )
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_5__invalid_onboarding_reports_only_the_rejected_setting_and_default() {
    let directory = TestDirectory::new().unwrap();
    let path = directory.0.join("settings.json");
    for rejected in [
        serde_json::json!("true"),
        serde_json::json!(42),
        serde_json::Value::Null,
    ] {
        let document = serde_json::json!({"schema_version":1,"app":{"theme":"dark","onboarding_complete":rejected},"account":{"token":"private-marker"}});
        std::fs::write(&path, serde_json::to_vec(&document).unwrap()).unwrap();
        let loaded = ConfigFile::new(path.clone()).load().unwrap();
        assert!(!loaded.settings.onboarding_complete());
        assert_eq!(loaded.errors.len(), 1);
        let issue = loaded.errors.first().unwrap();
        assert_eq!(issue.key, "settings.onboarding_complete");
        assert_eq!(issue.rejected, rejected.to_string());
        assert_eq!(issue.default, "false");
        assert!(!issue.rejected.contains("private-marker"));
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ocr_20_r_nf_5__version_one_preferences_migrate_with_explicit_opt_in_and_preserve_app_settings()
{
    use tallyline_game::api::GameSettingsStore;
    use tallyline_game::api::model::{GameSettings, WitnessMode};
    let directory = TestDirectory::new().unwrap();
    let path = directory.0.join("settings.json");
    std::fs::write(
        &path,
        br#"{"schema_version":1,"app":{"theme":"light","onboarding_complete":true}}"#,
    )
    .unwrap();
    let mut store = backed_store(path.clone());
    assert_eq!(
        store.load_game_preferences().unwrap().settings,
        GameSettings::default()
    );
    store.change_game_channel("synthetic-channel").unwrap();
    store
        .save_game_settings(&GameSettings {
            witness: WitnessMode::On,
            ..GameSettings::default()
        })
        .unwrap();
    let loaded = ConfigFile::new(path.clone())
        .load_game_preferences()
        .unwrap();
    assert_eq!(loaded.settings.witness, WitnessMode::On);
    assert_eq!(
        loaded.channel_directory.as_deref(),
        Some("synthetic-channel")
    );
    assert_eq!(
        store.load().unwrap().settings,
        AppSettings::from_persisted(Theme::Light, true)
    );
    let document: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(document.get("schema_version"), Some(&serde_json::json!(6)));
    store.change_game_channel("another-channel").unwrap();
    assert_eq!(
        store.load_game_preferences().unwrap().settings.witness,
        WitnessMode::Off
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ocr_20_r_nf_5__invalid_game_settings_default_off_without_echoing_private_rejected_values() {
    let directory = TestDirectory::new().unwrap();
    let path = directory.0.join("settings.json");
    std::fs::write(&path,br#"{"schema_version":2,"app":{"theme":"dark","onboarding_complete":false},"game":{"log_witness":"SYNTHETIC_PRIVATE_REJECTED","channel_directory":42}}"#).unwrap();
    let loaded = ConfigFile::new(path).load_game_preferences().unwrap();
    assert_eq!(
        loaded.settings.witness,
        tallyline_game::api::model::WitnessMode::Off
    );
    assert_eq!(loaded.channel_directory, None);
    assert_eq!(loaded.errors.len(), 2);
    assert!(!format!("{:?}", loaded.errors).contains("SYNTHETIC_"));
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_7_r_nf_5__previous_schemas_migrate_with_checks_off_and_invalid_values_are_visible() {
    use tallyline_workflows::api::model::UpdateChecks;
    let directory = TestDirectory::new().unwrap();
    let path = directory.0.join("settings.json");
    for schema in [1, 2] {
        std::fs::write(&path, format!(r#"{{"schema_version":{schema},"app":{{"theme":"dark","onboarding_complete":true}},"game":{{"log_witness":true}}}}"#)).unwrap();
        let mut store = backed_store(path.clone());
        let loaded = store.load().unwrap();
        assert_eq!(loaded.settings.update_checks(), UpdateChecks::Disabled);
        assert_eq!(loaded.settings.last_update_attempt(), None);
        assert!(loaded.errors.is_empty());
        store.save(&loaded.settings).unwrap();
        let reopened = store.load().unwrap();
        assert_eq!(reopened.settings, loaded.settings);
        assert!(
            store.load_game_preferences().unwrap().settings.witness
                == tallyline_game::api::model::WitnessMode::On
        );
    }
    for value in [r#""private-marker""#, "-1", "{}"] {
        std::fs::write(&path, format!(r#"{{"schema_version":3,"app":{{"theme":"dark","onboarding_complete":true,"update_checks":true,"last_update_attempt_ms":{value}}}}}"#)).unwrap();
        let loaded = ConfigFile::new(path.clone()).load().unwrap();
        assert_eq!(loaded.settings.update_checks(), UpdateChecks::Disabled);
        assert!(
            loaded
                .errors
                .iter()
                .any(|issue| issue.key == "settings.last_update_attempt")
        );
        assert!(!format!("{:?}", loaded.errors).contains("private-marker"));
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_11_r_nf_7__migration_requires_backup_and_preserves_exact_old_bytes_on_failure() {
    let directory = TestDirectory::new().unwrap();
    let path = directory.0.join("settings.json");
    let old = br#"{ "schema_version": 2, "app": {"theme":"dark","onboarding_complete":false},"other":{"value":42}}"#;
    std::fs::write(&path, old).unwrap();
    assert!(ConfigFile::new(path.clone()).migrate().is_err());
    assert_eq!(std::fs::read(&path).unwrap(), old);
    let original_path = path.clone();
    let mut failed = ConfigFile::new(path.clone()).with_migration_backup(move |schema, bytes| {
        assert_eq!(schema, 2);
        assert_eq!(bytes, old);
        assert_eq!(std::fs::read(&original_path)?, old);
        Err(std::io::Error::other("backup unavailable"))
    });
    assert!(failed.migrate().is_err());
    assert_eq!(std::fs::read(&path).unwrap(), old);
    let backup = directory.0.join("previous.json");
    let saved = backup.clone();
    let mut store = ConfigFile::new(path.clone()).with_migration_backup(move |schema, bytes| {
        assert_eq!(schema, 2);
        std::fs::write(&saved, bytes)
    });
    store.migrate().unwrap();
    assert_eq!(std::fs::read(backup).unwrap(), old);
    let current: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(current.get("schema_version"), Some(&serde_json::json!(6)));
    assert_eq!(current.get("other"), Some(&serde_json::json!({"value":42})));
}

struct BackupClock(u64);
impl tallyline_shared_kernel::api::Clock for BackupClock {
    fn now(&self) -> tallyline_shared_kernel::api::model::Timestamp {
        tallyline_shared_kernel::api::model::Timestamp::from_millis(self.0)
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_11__bootstrap_backups_require_database_absence_and_keep_three_recognised_sets() {
    use tallyline_adapter_files::api::{BackupSetStore, StorageLayout};
    let directory = TestDirectory::new().unwrap();
    let layout = StorageLayout::portable(&directory.0);
    assert!(layout.self_test().is_empty());
    let old = b"exact previous config bytes";
    let backups = BackupSetStore::new(layout.data().join("backup"));
    for millis in 1..=4 {
        let saved = backups
            .backup_without_database(&layout, 2, old, &BackupClock(millis))
            .unwrap();
        assert_eq!(std::fs::read(saved.join("settings.json")).unwrap(), old);
        let manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(saved.join("manifest.json")).unwrap()).unwrap();
        assert_eq!(
            manifest.get("taken_at_ms"),
            Some(&serde_json::json!(millis))
        );
        assert_eq!(
            manifest.get("database_schema"),
            Some(&serde_json::Value::Null)
        );
    }
    assert_eq!(
        std::fs::read_dir(layout.data().join("backup"))
            .unwrap()
            .count(),
        3
    );
    std::fs::write(layout.data().join("tallyline.db"), b"existing database").unwrap();
    assert!(
        backups
            .backup_without_database(&layout, 2, old, &BackupClock(5))
            .is_err()
    );
    assert_eq!(
        std::fs::read_dir(layout.data().join("backup"))
            .unwrap()
            .count(),
        3
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_7_r_ui_16__invalid_cached_tags_are_discarded_without_replaying_response_links() {
    use tallyline_workflows::api::model::{ReleaseVersion, UpdateStatus};
    let directory = TestDirectory::new().unwrap();
    let path = directory.0.join("settings.json");
    for tag in [
        "https://untrusted.invalid/",
        "v2.0.0-rc.1",
        "v2.0.0/../../private",
        "private-marker",
    ] {
        let document = serde_json::json!({"schema_version":3,"app":{"theme":"dark","onboarding_complete":true,"update_checks":true,"available_release":tag,"dismissed_release":tag}});
        std::fs::write(&path, serde_json::to_vec(&document).unwrap()).unwrap();
        let loaded = ConfigFile::new(path.clone()).load().unwrap();
        assert_eq!(
            loaded
                .settings
                .cached_update(ReleaseVersion::parse("v1.0.0").unwrap()),
            UpdateStatus::NotChecked
        );
        assert!(
            loaded
                .errors
                .iter()
                .all(|issue| issue.key == "settings.update_notice")
        );
        assert!(!format!("{:?}", loaded.errors).contains("private-marker"));
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_11__backup_sets_preserve_foreign_files_and_publish_neither_failed_nor_overflowed_sets() {
    use tallyline_adapter_files::api::BackupSetStore;
    let directory = TestDirectory::new().unwrap();
    let root = directory.0.join("backup");
    let backups = BackupSetStore::new(root.clone());
    assert!(
        backups
            .write(None, &BackupClock(1), |_, _| Err(std::io::Error::other(
                "SQLite backup failed"
            )))
            .is_err()
    );
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
    let first = backups
        .write(None, &BackupClock(2), |path, _| {
            std::fs::write(path, b"synthetic callback bytes")?;
            Ok(Some(1))
        })
        .unwrap();
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(first.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest.get("config_file"), Some(&serde_json::Value::Null));
    assert_eq!(
        manifest.get("config_schema"),
        Some(&serde_json::Value::Null)
    );
    std::fs::create_dir(root.join("unrecognised")).unwrap();
    std::fs::write(root.join("unrecognised/private"), b"untouched").unwrap();
    for millis in 3..=6 {
        backups
            .write(Some((2, b"config")), &BackupClock(millis), |_, _| Ok(None))
            .unwrap();
    }
    assert_eq!(
        std::fs::read(root.join("unrecognised/private")).unwrap(),
        b"untouched"
    );
    assert!(!first.exists());
    struct OverflowClock;
    impl tallyline_shared_kernel::api::Clock for OverflowClock {
        fn now(&self) -> tallyline_shared_kernel::api::model::Timestamp {
            tallyline_shared_kernel::api::model::Timestamp::from_seconds(u64::MAX)
        }
    }
    assert!(
        backups
            .write(None, &OverflowClock, |_, _| Ok(None))
            .is_err()
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_7__legacy_unknown_release_fields_never_enable_network_checks_during_migration() {
    use tallyline_workflows::api::model::UpdateChecks;
    let directory = TestDirectory::new().unwrap();
    let path = directory.0.join("settings.json");
    std::fs::write(&path, br#"{"schema_version":2,"app":{"theme":"dark","onboarding_complete":true,"update_checks":true,"last_update_attempt_ms":5,"available_release":"v2.0.0"}}"#).unwrap();
    let mut store = backed_store(path);
    assert_eq!(
        store.load().unwrap().settings.update_checks(),
        UpdateChecks::Disabled
    );
    store.migrate().unwrap();
    let loaded = store.load().unwrap();
    assert_eq!(loaded.settings.update_checks(), UpdateChecks::Disabled);
    assert_eq!(loaded.settings.available_release(), None);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_7__unrepresentable_attempt_time_refuses_save_without_panicking_or_replacing_preferences() {
    use tallyline_shared_kernel::api::model::Timestamp;
    use tallyline_workflows::api::model::UpdateChecks;
    let directory = TestDirectory::new().unwrap();
    let path = directory.0.join("settings.json");
    let mut store = ConfigFile::new(path.clone());
    store.save(&AppSettings::default()).unwrap();
    let previous = std::fs::read(&path).unwrap();
    let settings = AppSettings::default().with_update_state(
        UpdateChecks::Enabled,
        Some(Timestamp::from_seconds(u64::MAX)),
    );
    assert!(store.save(&settings).is_err());
    assert_eq!(std::fs::read(path).unwrap(), previous);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_11__retention_keeps_latest_completed_creations_after_clock_rollback_and_restart() {
    use tallyline_adapter_files::api::BackupSetStore;
    let directory = TestDirectory::new().unwrap();
    let root = directory.0.join("backup");
    let store = BackupSetStore::new(root.clone());
    let first = store
        .write(Some((2, b"first")), &BackupClock(1000), |_, _| Ok(None))
        .unwrap();
    let second = store
        .write(Some((2, b"second")), &BackupClock(2000), |_, _| Ok(None))
        .unwrap();
    let third = store
        .write(Some((2, b"third")), &BackupClock(3000), |_, _| Ok(None))
        .unwrap();
    let rollback = store
        .write(
            Some((2, b"rollback snapshot")),
            &BackupClock(500),
            |path, _| {
                std::fs::write(path, b"coupled database snapshot")?;
                Ok(Some(1))
            },
        )
        .unwrap();
    assert_eq!(
        std::fs::read(rollback.join("settings.json")).unwrap(),
        b"rollback snapshot"
    );
    assert_eq!(
        std::fs::read(rollback.join("tallyline.db")).unwrap(),
        b"coupled database snapshot"
    );
    assert!(!first.exists());
    assert!(second.exists());
    assert!(third.exists());
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 3);
    let reopened = BackupSetStore::new(root.clone());
    let latest = reopened
        .write(Some((2, b"after restart")), &BackupClock(100), |_, _| {
            Ok(None)
        })
        .unwrap();
    assert_eq!(
        std::fs::read(latest.join("settings.json")).unwrap(),
        b"after restart"
    );
    assert!(rollback.exists());
    assert!(third.exists());
    assert!(!second.exists());
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 3);
    assert!(
        reopened
            .write(Some((2, b"collision")), &BackupClock(100), |_, _| Ok(None))
            .is_err()
    );
    assert!(
        reopened
            .write(Some((2, b"failed")), &BackupClock(50), |_, _| Err(
                std::io::Error::other("backup failed")
            ))
            .is_err()
    );
    assert_eq!(
        std::fs::read(latest.join("settings.json")).unwrap(),
        b"after restart"
    );
    assert_eq!(
        std::fs::read(rollback.join("settings.json")).unwrap(),
        b"rollback snapshot"
    );
    assert_eq!(
        std::fs::read(third.join("settings.json")).unwrap(),
        b"third"
    );
    assert_eq!(std::fs::read_dir(root).unwrap().count(), 3);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_11__retention_reads_unsequenced_manifests_and_refuses_exhausted_creation_order() {
    use tallyline_adapter_files::api::BackupSetStore;
    let directory = TestDirectory::new().unwrap();
    let root = directory.0.join("backup");
    std::fs::create_dir(&root).unwrap();
    for (time, bytes) in [
        (1000, b"first".as_slice()),
        (2000, b"second".as_slice()),
        (3000, b"third".as_slice()),
    ] {
        let folder = root.join(format!("{time}-v2"));
        std::fs::create_dir(&folder).unwrap();
        std::fs::write(folder.join("settings.json"), bytes).unwrap();
        std::fs::write(folder.join("manifest.json"), format!(r#"{{"schema_version":1,"taken_at_ms":{time},"config_schema":2,"database_schema":null,"config_file":"settings.json","database_file":null}}"#)).unwrap();
    }
    let saved = BackupSetStore::new(root.clone())
        .write(Some((2, b"new creation")), &BackupClock(500), |_, _| {
            Ok(None)
        })
        .unwrap();
    assert_eq!(
        std::fs::read(saved.join("settings.json")).unwrap(),
        b"new creation"
    );
    assert!(!root.join("1000-v2").exists());
    assert!(root.join("2000-v2").exists());
    assert!(root.join("3000-v2").exists());
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 3);
    let exhausted = directory.0.join("exhausted");
    let previous = exhausted.join("1000-v2");
    std::fs::create_dir_all(&previous).unwrap();
    std::fs::write(previous.join("settings.json"), b"unchanged previous config").unwrap();
    std::fs::write(previous.join("manifest.json"), br#"{"schema_version":1,"sequence":18446744073709551615,"taken_at_ms":1000,"config_schema":2,"database_schema":null,"config_file":"settings.json","database_file":null}"#).unwrap();
    assert!(
        BackupSetStore::new(exhausted.clone())
            .write(Some((2, b"new")), &BackupClock(500), |_, _| Ok(None))
            .is_err()
    );
    assert_eq!(
        std::fs::read(previous.join("settings.json")).unwrap(),
        b"unchanged previous config"
    );
    assert_eq!(std::fs::read_dir(exhausted).unwrap().count(), 1);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_cap_3a__schema_four_game_mapping_persists_independently_of_witness_preferences() {
    use tallyline_game::api::GameSettingsStore;
    use tallyline_game::api::model::{GameEnvironment, GameSettings, WitnessMode};
    use tallyline_shared_kernel::api::model::UexEnvironment;
    let directory = TestDirectory::new().unwrap();
    let path = directory.0.join("settings.json");
    let initial = GameSettings::default();
    let changed = GameSettings {
        witness: WitnessMode::On,
        environment_mapping: initial
            .environment_mapping
            .with_environment(GameEnvironment::Hotfix, UexEnvironment::Ptu),
    };
    let mut store = ConfigFile::new(path.clone());
    store.save_game_settings(&changed).unwrap();
    let reopened = ConfigFile::new(path.clone());
    assert_eq!(reopened.load_game_preferences().unwrap().settings, changed);
    let document: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(document["schema_version"], 6);
    store.change_game_channel("another-channel").unwrap();
    let loaded = store.load_game_preferences().unwrap();
    assert_eq!(loaded.settings.witness, WitnessMode::Off);
    assert_eq!(
        loaded.settings.environment_mapping,
        changed.environment_mapping
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_11_r_cap_3a__schema_three_migration_backs_up_and_preserves_release_and_witness_preferences()
{
    use tallyline_game::api::model::WitnessMode;
    use tallyline_workflows::api::model::UpdateChecks;
    let directory = TestDirectory::new().unwrap();
    let path = directory.0.join("settings.json");
    let old = br#"{ "schema_version":3,"app":{"theme":"dark","onboarding_complete":true,"update_checks":true,"last_update_attempt_ms":1234,"available_release":"v2.0.0","dismissed_release":"v1.0.0"},"game":{"log_witness":true,"channel_directory":"synthetic-channel"}}"#;
    std::fs::write(&path, old).unwrap();
    assert!(ConfigFile::new(path.clone()).migrate().is_err());
    assert_eq!(std::fs::read(&path).unwrap(), old);
    let mut store = backed_store(path.clone());
    store.migrate().unwrap();
    assert_eq!(
        std::fs::read(path.with_extension("backup.json")).unwrap(),
        old
    );
    let loaded = store.load().unwrap();
    assert_eq!(loaded.settings.update_checks(), UpdateChecks::Enabled);
    assert_eq!(
        loaded.settings.last_update_attempt().unwrap().millis(),
        1234
    );
    assert_eq!(
        loaded.settings.available_release().unwrap().to_string(),
        "v2.0.0"
    );
    assert_eq!(
        loaded.settings.dismissed_release().unwrap().to_string(),
        "v1.0.0"
    );
    let game = store.load_game_preferences().unwrap();
    assert_eq!(game.settings.witness, WitnessMode::On);
    assert_eq!(game.channel_directory.as_deref(), Some("synthetic-channel"));
    store.migrate().unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_cap_3a__invalid_mapping_reports_safe_defaults_without_disclosing_raw_values() {
    use tallyline_game::api::model::EnvironmentMapping;
    let directory = TestDirectory::new().unwrap();
    let path = directory.0.join("settings.json");
    for value in [
        serde_json::json!("private-marker"),
        serde_json::json!({"hotfix":"private-marker"}),
    ] {
        std::fs::write(
            &path,
            serde_json::to_vec(
                &serde_json::json!({"schema_version":4,"game":{"environment_mapping":value}}),
            )
            .unwrap(),
        )
        .unwrap();
        let loaded = ConfigFile::new(path.clone())
            .load_game_preferences()
            .unwrap();
        assert_eq!(
            loaded.settings.environment_mapping,
            EnvironmentMapping::default()
        );
        assert_eq!(loaded.errors.len(), 1);
        assert_eq!(loaded.errors[0].key, "settings.environment_mapping");
        assert!(!format!("{:?}", loaded.errors).contains("private-marker"));
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_11__backup_source_preserves_exact_known_bytes_and_refuses_unknown_schemas() {
    let directory = TestDirectory::new().unwrap();
    let path = directory.0.join("settings.json");
    let store = ConfigFile::new(path.clone());
    assert!(store.backup_source().unwrap().is_none());
    let previous = br#"{ "schema_version":3,"app":{"theme":"light"}}"#;
    std::fs::write(&path, previous).unwrap();
    let source = store.backup_source().unwrap().unwrap();
    assert_eq!(source.schema, 3);
    assert_eq!(source.bytes, previous);
    for invalid in [br#"{"schema_version":7}"#.as_slice(), br#"not JSON"#] {
        std::fs::write(&path, invalid).unwrap();
        assert!(store.backup_source().is_err());
        assert_eq!(std::fs::read(&path).unwrap(), invalid);
    }
}

#[cfg(unix)]
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_5__writable_shared_directories_warn_while_real_probe_failures_remain_fatal() {
    use std::os::unix::fs::PermissionsExt;
    use tallyline_adapter_files::api::model::DirectoryIssueKind;
    let directory = TestDirectory::new().unwrap();
    let layout = tallyline_adapter_files::api::StorageLayout::portable(&directory.0);
    assert!(layout.self_test().is_empty());
    std::fs::set_permissions(layout.config(), std::fs::Permissions::from_mode(0o755)).unwrap();
    let issues = layout.self_test();
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].kind, DirectoryIssueKind::PermissionsWarning);
    assert_eq!(
        std::fs::metadata(layout.config())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o755
    );
    std::fs::write(
        layout.config().join(".tallyline-write-test"),
        b"dummy-existing-probe",
    )
    .unwrap();
    let failures = layout.self_test();
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].kind, DirectoryIssueKind::IoFailure);
    assert_eq!(
        std::fs::read(layout.config().join(".tallyline-write-test")).unwrap(),
        b"dummy-existing-probe"
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_5_r_val_2_q_manual__stock_bands_persist_and_invalid_bands_default_independently() {
    use tallyline_reporting::api::model::StockSettings;
    use tallyline_shared_kernel::api::model::Percent;
    let directory = TestDirectory::new().unwrap();
    let path = directory.0.join("settings.json");
    let mut store = ConfigFile::new(path.clone());
    let settings = AppSettings::default()
        .with_stock_settings(StockSettings::new(Percent::whole(10), "8".parse().unwrap()).unwrap());
    store.save(&settings).unwrap();
    assert_eq!(
        ConfigFile::new(path.clone())
            .load()
            .unwrap()
            .settings
            .stock_settings(),
        settings.stock_settings()
    );
    let mut document: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    document["app"]["stock_percent"] = serde_json::json!(5000);
    document["app"]["stock_minimum"] = serde_json::json!("8");
    std::fs::write(&path, serde_json::to_vec(&document).unwrap()).unwrap();
    let loaded = ConfigFile::new(path.clone()).load().unwrap();
    assert_eq!(
        loaded.settings.stock_settings().tolerance().percent().get(),
        20
    );
    assert_eq!(
        loaded
            .settings
            .stock_settings()
            .tolerance()
            .minimum()
            .to_string(),
        "8"
    );
    assert_eq!(loaded.errors.len(), 1);
    document["app"]["stock_percent"] = serde_json::json!(10);
    document["app"]["stock_minimum"] = serde_json::json!("17");
    std::fs::write(&path, serde_json::to_vec(&document).unwrap()).unwrap();
    let loaded = ConfigFile::new(path).load().unwrap();
    assert_eq!(
        loaded.settings.stock_settings().tolerance().percent().get(),
        10
    );
    assert_eq!(
        loaded
            .settings
            .stock_settings()
            .tolerance()
            .minimum()
            .to_string(),
        "16"
    );
    assert_eq!(loaded.errors.len(), 1);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_11_r_nf_5__schema_four_stock_defaults_follow_exact_config_backup() {
    let directory = TestDirectory::new().unwrap();
    let path = directory.0.join("settings.json");
    let previous = br#"{ "schema_version":4,"app":{"theme":"light","onboarding_complete":true,"update_checks":false},"game":{"environment_mapping":{"hotfix":"ptu"}}}"#;
    std::fs::write(&path, previous).unwrap();
    assert!(ConfigFile::new(path.clone()).migrate().is_err());
    assert_eq!(std::fs::read(&path).unwrap(), previous);
    backed_store(path.clone()).migrate().unwrap();
    assert_eq!(
        std::fs::read(path.with_extension("backup.json")).unwrap(),
        previous
    );
    let loaded = ConfigFile::new(path.clone()).load().unwrap();
    assert!(loaded.errors.is_empty());
    assert_eq!(
        loaded.settings.stock_settings(),
        tallyline_reporting::api::model::StockSettings::default()
    );
    let document: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(document["schema_version"], 6);
    assert_eq!(document["game"]["environment_mapping"]["hotfix"], "ptu");
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_5_r_sub_8_r_val_6__submission_mode_and_tightened_observation_age_survive_reopening() {
    use tallyline_shared_kernel::api::model::{ObservationAgeSettings, SubmissionMode};
    let directory = TestDirectory::new().unwrap();
    let path = directory.0.join("settings.json");
    let mut store = ConfigFile::new(path.clone());
    let age = ObservationAgeSettings::new(std::time::Duration::from_millis(30_000)).unwrap();
    let chosen = AppSettings::from_persisted(Theme::Dark, true)
        .with_submission_mode(Some(SubmissionMode::Production))
        .with_observation_age(age);
    store.save(&chosen).unwrap();
    let loaded = ConfigFile::new(path.clone()).load().unwrap();
    assert_eq!(
        loaded.settings.submission_mode_override(),
        Some(SubmissionMode::Production)
    );
    assert_eq!(loaded.settings.observation_age(), age);
    assert_eq!(loaded.settings, chosen);
    assert!(loaded.errors.is_empty());
    let mut document: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    document["app"]["submission_mode"] = serde_json::json!("unsupported");
    document["app"]["observation_age_ms"] = serde_json::json!(3_600_001);
    std::fs::write(&path, serde_json::to_vec(&document).unwrap()).unwrap();
    let loaded = ConfigFile::new(path).load().unwrap();
    assert_eq!(
        loaded.settings.submission_mode_override(),
        Some(SubmissionMode::DryRun)
    );
    assert_eq!(
        loaded.settings.observation_age().maximum(),
        std::time::Duration::from_secs(3600)
    );
    assert_eq!(loaded.errors.len(), 2);
    assert_eq!(loaded.settings.theme(), Theme::Dark);
    assert!(loaded.settings.onboarding_complete());
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_11_r_nf_5_r_sub_8__schema_five_submission_defaults_require_exact_backup_and_ignore_unknown_old_mode_fields()
 {
    use tallyline_shared_kernel::api::model::SubmissionMode;
    use tallyline_workflows::api::model::UpdateChecks;
    let directory = TestDirectory::new().unwrap();
    let path = directory.0.join("settings.json");
    let previous=br#"{ "schema_version":5,"app":{"theme":"light","onboarding_complete":true,"stock_percent":10,"stock_minimum":"8","update_checks":true,"last_update_attempt_ms":123000,"available_release":"v9.8.7","submission_mode":"production","observation_age_ms":1},"game":{"environment_mapping":{"hotfix":"ptu"}}}"#;
    std::fs::write(&path, previous).unwrap();
    let old = ConfigFile::new(path.clone()).load().unwrap();
    assert_eq!(old.settings.submission_mode_override(), None);
    assert!(ConfigFile::new(path.clone()).migrate().is_err());
    assert_eq!(std::fs::read(&path).unwrap(), previous);
    let mut failed = ConfigFile::new(path.clone())
        .with_migration_backup(|_, _| Err(std::io::Error::other("recorded backup failure")));
    assert!(failed.migrate().is_err());
    assert_eq!(std::fs::read(&path).unwrap(), previous);
    backed_store(path.clone()).migrate().unwrap();
    assert_eq!(
        std::fs::read(path.with_extension("backup.json")).unwrap(),
        previous
    );
    let loaded = ConfigFile::new(path.clone()).load().unwrap();
    assert!(loaded.errors.is_empty());
    assert_eq!(loaded.settings.submission_mode_override(), None);
    assert_eq!(
        loaded.settings.submission_mode(SubmissionMode::DryRun),
        SubmissionMode::DryRun
    );
    assert_eq!(
        loaded.settings.observation_age().maximum(),
        std::time::Duration::from_secs(3600)
    );
    assert_eq!(
        loaded.settings.stock_settings().tolerance().percent().get(),
        10
    );
    assert_eq!(
        loaded
            .settings
            .stock_settings()
            .tolerance()
            .minimum()
            .to_string(),
        "8"
    );
    assert_eq!(loaded.settings.theme(), Theme::Light);
    assert!(loaded.settings.onboarding_complete());
    assert_eq!(loaded.settings.update_checks(), UpdateChecks::Enabled);
    assert_eq!(
        loaded.settings.last_update_attempt(),
        Some(tallyline_shared_kernel::api::model::Timestamp::from_millis(
            123000
        ))
    );
    let document: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(document["schema_version"], 6);
    assert_eq!(document["game"]["environment_mapping"]["hotfix"], "ptu");
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_5_r_sub_8__first_game_preference_write_keeps_complete_safe_application_defaults() {
    use tallyline_game::api::GameSettingsStore;
    let directory = TestDirectory::new().unwrap();
    let path = directory.0.join("settings.json");
    let mut store = ConfigFile::new(path.clone());
    store
        .save_game_settings(&tallyline_game::api::model::GameSettings::default())
        .unwrap();
    let loaded = ConfigFile::new(path).load().unwrap();
    assert!(
        loaded.errors.is_empty(),
        "A normal first context save must not create an invalid current-schema application document"
    );
    assert_eq!(loaded.settings, AppSettings::default());
}
