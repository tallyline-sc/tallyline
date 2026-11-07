// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Independent legacy numeric formats migrate through the public SQLite backup boundary.

use tallyline_adapter_storage::api::SqliteReferenceCache;
use tallyline_game::api::model::GameEnvironment;
use tallyline_reporting::api::ReportRepository;
use tallyline_reporting::api::model::ReportKey;
use tallyline_shared_kernel::api::model::{CommodityId, TerminalId, Timestamp, TradeSide};

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_11_r_ui_12_q_manual__schema_six_requires_backup_and_preserves_legacy_price_and_status_state()
 {
    use tallyline_adapter_storage::api::StorageError;
    let path = super::test_path("tallyline-price-migrate").unwrap();
    let backup = path.with_extension("backup.db");
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection.execute_batch("CREATE TABLE reports(id INTEGER PRIMARY KEY AUTOINCREMENT,version INTEGER NOT NULL,payload TEXT NOT NULL); CREATE TABLE report_recents(report_id INTEGER PRIMARY KEY,changed_order INTEGER NOT NULL); CREATE TABLE screenshot_requirements(account_digest BLOB PRIMARY KEY NOT NULL CHECK(length(account_digest)=32),observed_at TEXT NOT NULL); PRAGMA user_version=6;").unwrap();
    let payload = r#"{"terminal":11,"side":"buy","environment":"live","observed_at":"900123","game_version":"4.10.2","rows":[{"id":1,"commodity":42,"price":"123.456","stock":"17","status":3,"status_confirmation":[3,"major","live"],"status_deviation":"minor","status_unreviewed":false,"status_confirmation_revoked":false}],"attachment":null}"#;
    let mut document: serde_json::Value = serde_json::from_str(payload).unwrap();
    document["attachment"] = serde_json::json!({"image": vec![1;32], "rectangle": [10,20,30,40], "observed_at": "900123", "time_source": "user", "preview_confirmed": true, "working_copy": vec![2;32]});
    let payload = document.to_string();
    use sha2::{Digest, Sha256};
    let account = Sha256::digest(b"migration-account");
    connection
        .execute(
            "INSERT INTO screenshot_requirements VALUES(?1,'777000')",
            [account.as_slice()],
        )
        .unwrap();
    connection
        .execute("INSERT INTO reports VALUES(1,14,?1)", [&payload])
        .unwrap();
    connection
        .execute("INSERT INTO report_recents VALUES(1,9)", [])
        .unwrap();
    drop(connection);
    assert!(matches!(
        SqliteReferenceCache::open(&path),
        Err(StorageError::MigrationBackupRequired)
    ));
    let mut store = SqliteReferenceCache::open_with_migration_backup(&path, |cache, version| {
        assert_eq!(version, 6);
        cache.backup_into(&backup, Timestamp::from_seconds(1000), None)?;
        Ok(())
    })
    .unwrap();
    let report = store.drafts().unwrap().remove(0);
    use tallyline_submission::api::ScreenshotPolicyStore;
    assert_eq!(
        store.required_since("migration-account").unwrap(),
        Some(Timestamp::from_millis(777000))
    );
    assert_eq!(store.required_since("another-account").unwrap(), None);
    let attachment = report.attachment().unwrap();
    assert!(attachment.preview_confirmed());
    assert_eq!(
        attachment.working_copy(),
        Some(tallyline_reporting::api::model::ImageHash::new([2; 32]))
    );
    assert_eq!(
        attachment.binding(),
        tallyline_reporting::api::model::AttachmentBinding::new(
            tallyline_reporting::api::model::ImageHash::new([1; 32]),
            tallyline_reporting::api::model::AttachmentRegion::new(10, 20, 30, 40).unwrap()
        )
    );
    assert_eq!(report.version(), 14);
    assert_eq!(report.observed_at(), Timestamp::from_millis(900123));
    assert_eq!(report.game_version(), Some("4.10.2"));
    assert_eq!(report.rows()[0].price().unwrap().to_string(), "123.456");
    assert_eq!(report.rows()[0].stock().unwrap().to_string(), "17");
    assert_eq!(
        report.rows()[0]
            .status_confirmation()
            .unwrap()
            .value()
            .get(),
        3
    );
    assert!(report.rows()[0].price_confirmation().is_none());
    assert!(report.rows()[0].price_deviation().is_none());
    assert!(!report.rows()[0].price_unreviewed());
    assert!(!report.rows()[0].price_confirmation_revoked());
    let before = rusqlite::Connection::open(&backup).unwrap();
    assert_eq!(
        before
            .query_row("PRAGMA user_version", [], |row| row.get::<_, u32>(0))
            .unwrap(),
        6
    );
    assert_eq!(
        before
            .query_row("SELECT payload FROM reports", [], |row| row
                .get::<_, String>(0))
            .unwrap(),
        payload
    );
    let current = rusqlite::Connection::open(&path).unwrap();
    assert_eq!(
        current
            .query_row("PRAGMA user_version", [], |row| row.get::<_, u32>(0))
            .unwrap(),
        17
    );
    assert_eq!(
        current
            .query_row("SELECT changed_order FROM report_recents", [], |row| row
                .get::<_, u32>(0))
            .unwrap(),
        9
    );
    drop(current);
    drop(before);
    drop(store);
    std::fs::remove_file(path).unwrap();
    std::fs::remove_file(backup).unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_11_r_ui_12_q_manual__schema_seven_stock_migration_requires_backup_and_preserves_values() {
    use tallyline_reporting::api::model::Report;
    use tallyline_shared_kernel::api::model::ReportId;
    let path = super::test_path("tallyline-stock-migration").unwrap();
    let backup = path.with_extension("before.db");
    let key = ReportKey::new(TerminalId(11), TradeSide::Buy, GameEnvironment::Live).unwrap();
    let mut report = Report::manual(
        ReportId(1),
        key,
        Timestamp::from_seconds(1000),
        Some("4.10.2"),
    )
    .unwrap();
    let row = report.add_commodity(CommodityId(42)).unwrap();
    report
        .set_stock(row, Some("1744".parse().unwrap()))
        .unwrap();
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute_batch(include_str!("../fixtures/schema-seven.sql"))
        .unwrap();
    drop(connection);
    assert!(matches!(
        SqliteReferenceCache::open(&path),
        Err(tallyline_adapter_storage::api::StorageError::MigrationBackupRequired)
    ));
    let mut reopened = SqliteReferenceCache::open_with_migration_backup(&path, |cache, from| {
        assert_eq!(from, 7);
        assert_eq!(
            cache.backup_into(&backup, Timestamp::from_seconds(1000), Some(4))?,
            7
        );
        Ok(())
    })
    .unwrap();
    assert_eq!(reopened.load_draft(report.id()).unwrap(), Some(report));
    drop(reopened);
    std::fs::remove_file(path).unwrap();
    std::fs::remove_file(backup).unwrap();
}
