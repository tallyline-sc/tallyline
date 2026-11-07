// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Reference cache contracts against real temporary SQLite files.

use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use tallyline_adapter_storage::api::SqliteReferenceCache;
use tallyline_reference_data::api::ReferenceDataCache;
use tallyline_reference_data::api::model::{
    CachedReference, Commodity, CommodityAverages, EndpointData,
};
use tallyline_shared_kernel::api::model::{CommodityId, Timestamp};

struct TestDirectory(PathBuf);
impl TestDirectory {
    fn new() -> std::io::Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "tallyline-reference-db-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path)?;
        Ok(Self(path))
    }
}
impl Drop for TestDirectory {
    fn drop(&mut self) {
        assert!(
            std::fs::remove_dir_all(&self.0).is_ok(),
            "temporary database cleanup failed"
        );
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_api_1_r_nf_11__empty_database_migrates_and_cached_data_survives_reopening() {
    let directory = TestDirectory::new().unwrap();
    let path = directory.0.join("tallyline.db");
    let entry = CachedReference::new(
        Timestamp::from_seconds(1_800),
        EndpointData::Commodities(vec![Commodity::new(
            CommodityId(42),
            "Agricium",
            "AGRI",
            true,
            CommodityAverages::new(Some("120".parse().unwrap()), None),
        )]),
    );
    {
        let mut cache = SqliteReferenceCache::open(&path).unwrap();
        assert!(cache.load().unwrap().is_empty());
        cache.replace(&entry).unwrap();
    }
    let mut reopened = SqliteReferenceCache::open(&path).unwrap();
    assert_eq!(reopened.load().unwrap(), vec![entry]);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_api_1__cached_fetch_times_keep_millisecond_precision_and_full_second_range() {
    let directory = TestDirectory::new().unwrap();
    let mut cache = SqliteReferenceCache::open(directory.0.join("tallyline.db")).unwrap();
    for fetched_at in [
        Timestamp::from_millis(1_701_234_567_891),
        Timestamp::from_seconds(u64::MAX),
    ] {
        let entry = CachedReference::new(fetched_at, EndpointData::Commodities(vec![]));
        cache.replace(&entry).unwrap();
        assert_eq!(cache.load().unwrap(), vec![entry]);
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_11__future_database_is_refused_without_modifying_its_bytes() {
    let directory = TestDirectory::new().unwrap();
    let path = directory.0.join("future.db");
    {
        let database = rusqlite::Connection::open(&path).unwrap();
        database.pragma_update(None, "user_version", 18).unwrap();
    }
    let before = std::fs::read(&path).unwrap();
    assert!(matches!(
        SqliteReferenceCache::open(&path),
        Err(
            tallyline_adapter_storage::api::StorageError::UnsupportedSchema {
                found: 18,
                supported: 17
            }
        )
    ));
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_api_1__failed_transaction_preserves_previous_content_and_successful_fetch_time() {
    let directory = TestDirectory::new().unwrap();
    let path = directory.0.join("tallyline.db");
    let old = CachedReference::new(
        Timestamp::from_seconds(1_800),
        EndpointData::Commodities(vec![Commodity::new(
            CommodityId(1),
            "Old",
            "A",
            true,
            CommodityAverages::new(Some("120".parse().unwrap()), None),
        )]),
    );
    {
        let mut cache = SqliteReferenceCache::open(&path).unwrap();
        cache.replace(&old).unwrap();
        let duplicate = CachedReference::new(
            Timestamp::from_seconds(3_600),
            EndpointData::Commodities(vec![
                Commodity::new(
                    CommodityId(2),
                    "First",
                    "B",
                    true,
                    CommodityAverages::default(),
                ),
                Commodity::new(
                    CommodityId(2),
                    "Second",
                    "C",
                    true,
                    CommodityAverages::default(),
                ),
            ]),
        );
        assert!(cache.replace(&duplicate).is_err());
        assert_eq!(cache.load().unwrap(), vec![old.clone()]);
    }
    let mut reopened = SqliteReferenceCache::open(&path).unwrap();
    assert_eq!(reopened.load().unwrap(), vec![old]);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_api_1_r_ui_8__terminals_and_commodities_reopen_with_independent_fetch_times() {
    use tallyline_reference_data::api::model::{Terminal, TerminalText};
    use tallyline_shared_kernel::api::model::TerminalId;
    let directory = TestDirectory::new().unwrap();
    let path = directory.0.join("tallyline.db");
    let commodity = CachedReference::new(
        Timestamp::from_millis(10_001),
        EndpointData::Commodities(vec![]),
    );
    let terminals = CachedReference::new(
        Timestamp::from_millis(90_999),
        EndpointData::Terminals(vec![
            Terminal::new(
                TerminalId(17),
                TerminalText {
                    name: "Trade Centre".to_owned(),
                    nickname: "TDD".to_owned(),
                    display_name: "Trade Centre".to_owned(),
                    location: "Stanton / ArcCorp / Area18".to_owned(),
                },
                false,
            )
            .with_max_container_size(std::num::NonZeroU32::new(32)),
        ]),
    );
    {
        let mut cache = SqliteReferenceCache::open(&path).unwrap();
        cache.replace(&commodity).unwrap();
        cache.replace(&terminals).unwrap();
    }
    let mut cache = SqliteReferenceCache::open(&path).unwrap();
    assert_eq!(cache.load().unwrap(), vec![commodity, terminals]);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_cap_3b__version_cache_roundtrip_preserves_non_semver_live_and_absent_ptu() {
    use tallyline_reference_data::api::model::GameVersions;
    let directory = TestDirectory::new().unwrap();
    let path = directory.0.join("tallyline.db");
    for ptu in [Some("3.24.2a".to_owned()), None] {
        let entry = CachedReference::new(
            Timestamp::from_millis(90_999),
            EndpointData::GameVersions(GameVersions {
                live: "4.9".to_owned(),
                ptu,
            }),
        );
        {
            let mut cache = SqliteReferenceCache::open(&path).unwrap();
            cache.replace(&entry).unwrap();
        }
        let mut reopened = SqliteReferenceCache::open(&path).unwrap();
        assert_eq!(reopened.load().unwrap(), vec![entry]);
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_11__sqlite_backup_includes_uncheckpointed_wal_and_a_restore_marker() {
    use tallyline_adapter_storage::api::model::BackupMetadata;
    let directory = TestDirectory::new().unwrap();
    let path = directory.0.join("tallyline.db");
    let mut cache = SqliteReferenceCache::open(&path).unwrap();
    let writer = rusqlite::Connection::open(&path).unwrap();
    writer.pragma_update(None, "journal_mode", "WAL").unwrap();
    writer.pragma_update(None, "wal_autocheckpoint", 0).unwrap();
    let entry = CachedReference::new(
        Timestamp::from_millis(90_999),
        EndpointData::Commodities(vec![Commodity::new(
            CommodityId(7),
            "WAL record",
            "WAL",
            true,
            CommodityAverages::default(),
        )]),
    );
    cache.replace(&entry).unwrap();
    assert!(
        std::fs::metadata(path.with_extension("db-wal"))
            .unwrap()
            .len()
            > 0
    );
    let destination = directory.0.join("before.db");
    let taken_at = Timestamp::from_millis(123_456_789);
    assert_eq!(
        cache.backup_into(&destination, taken_at, Some(3)).unwrap(),
        17
    );
    let mut backup = SqliteReferenceCache::open(&destination).unwrap();
    assert_eq!(backup.load().unwrap(), vec![entry.clone()]);
    assert_eq!(
        backup.backup_metadata().unwrap(),
        Some(BackupMetadata {
            taken_at,
            from_schema: 17,
            configuration_schema: Some(3)
        })
    );
    assert_eq!(cache.backup_metadata().unwrap(), None);
    assert_eq!(cache.load().unwrap(), vec![entry]);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_11__ambiguous_restore_markers_fail_closed() {
    let directory = TestDirectory::new().unwrap();
    let path = directory.0.join("tallyline.db");
    let cache = SqliteReferenceCache::open(&path).unwrap();
    let writer = rusqlite::Connection::open(&path).unwrap();
    writer.execute_batch("CREATE TABLE backup_meta(taken_at TEXT NOT NULL,from_version INTEGER NOT NULL,config_schema INTEGER); INSERT INTO backup_meta VALUES('123456',1,3),('999999',1,3);").unwrap();
    assert!(matches!(
        cache.backup_metadata(),
        Err(tallyline_adapter_storage::api::StorageError::InvalidBackup)
    ));
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_11__backup_never_replaces_a_file_and_keeps_absent_config_and_exact_time() {
    let directory = TestDirectory::new().unwrap();
    let mut cache = SqliteReferenceCache::open(directory.0.join("tallyline.db")).unwrap();
    let destination = directory.0.join("existing.db");
    std::fs::write(&destination, b"keep existing bytes").unwrap();
    assert!(matches!(
        cache.backup_into(&destination, Timestamp::from_seconds(10), None),
        Err(tallyline_adapter_storage::api::StorageError::Filesystem(_))
    ));
    assert_eq!(std::fs::read(&destination).unwrap(), b"keep existing bytes");
    let fresh = directory.0.join("new.db");
    let taken_at = Timestamp::from_seconds(u64::MAX);
    cache.backup_into(&fresh, taken_at, None).unwrap();
    let backup = SqliteReferenceCache::open(&fresh).unwrap();
    let metadata = backup.backup_metadata().unwrap().unwrap();
    assert_eq!(metadata.taken_at, taken_at);
    assert_eq!(metadata.configuration_schema, None);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_api_1__status_vocabulary_reopens_without_collapsing_buy_and_sell_labels() {
    use std::num::NonZeroU8;
    use tallyline_reference_data::api::model::{StatusLevel, StatusLevels};
    let directory = TestDirectory::new().unwrap();
    let path = directory.0.join("tallyline.db");
    let entry = CachedReference::new(
        Timestamp::from_seconds(90),
        EndpointData::StatusLevels(StatusLevels {
            buy: vec![StatusLevel {
                code: NonZeroU8::new(7).unwrap(),
                name: "Maximum Inventory (Full)".to_owned(),
                short_name: "Maximum".to_owned(),
                abbreviation: "MA".to_owned(),
            }],
            sell: vec![StatusLevel {
                code: NonZeroU8::new(7).unwrap(),
                name: "Maximum Inventory (No Demand)".to_owned(),
                short_name: "Maximum".to_owned(),
                abbreviation: "ND".to_owned(),
            }],
        }),
    );
    {
        let mut cache = SqliteReferenceCache::open(&path).unwrap();
        cache.replace(&entry).unwrap();
    }
    let mut reopened = SqliteReferenceCache::open(&path).unwrap();
    assert_eq!(reopened.load().unwrap(), vec![entry]);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_api_5__invalid_status_replacement_preserves_the_last_successful_snapshot() {
    use std::num::NonZeroU8;
    use tallyline_reference_data::api::model::{StatusLevel, StatusLevels};
    let directory = TestDirectory::new().unwrap();
    let mut cache = SqliteReferenceCache::open(directory.0.join("tallyline.db")).unwrap();
    let level = StatusLevel {
        code: NonZeroU8::new(1).unwrap(),
        name: "Out of Stock".to_owned(),
        short_name: "Empty".to_owned(),
        abbreviation: "OS".to_owned(),
    };
    let old = CachedReference::new(
        Timestamp::from_seconds(100),
        EndpointData::StatusLevels(StatusLevels {
            buy: vec![level.clone()],
            sell: vec![],
        }),
    );
    cache.replace(&old).unwrap();
    let invalid = CachedReference::new(
        Timestamp::from_seconds(200),
        EndpointData::StatusLevels(StatusLevels {
            buy: vec![level.clone(), level],
            sell: vec![],
        }),
    );
    assert!(matches!(
        cache.replace(&invalid),
        Err(tallyline_adapter_storage::api::StorageError::InvalidCache)
    ));
    assert_eq!(cache.load().unwrap(), vec![old]);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_api_6_r_val_2a__exact_side_specific_averages_survive_sqlite_without_binary_float_conversion() {
    use tallyline_shared_kernel::api::model::{TradeSide, UexEnvironment};
    let directory = TestDirectory::new().unwrap();
    let path = directory.0.join("tallyline.db");
    let buy = "9007199254740993.125".parse().unwrap();
    let sell = "0.00000097".parse().unwrap();
    let entry = CachedReference::new(
        Timestamp::from_millis(1234),
        EndpointData::Commodities(vec![Commodity::new(
            CommodityId(1),
            "Exact",
            "EX",
            true,
            CommodityAverages::new(Some(buy), Some(sell)),
        )]),
    );
    {
        let mut cache = SqliteReferenceCache::open(&path).unwrap();
        cache.replace(&entry).unwrap();
    }
    let loaded = SqliteReferenceCache::open(&path).unwrap().load().unwrap();
    assert_eq!(loaded, vec![entry]);
    let EndpointData::Commodities(items) = loaded[0].data() else {
        panic!("wrong endpoint")
    };
    assert_eq!(
        items[0].average_price(UexEnvironment::Live, TradeSide::Buy),
        Some(buy)
    );
    assert_eq!(
        items[0].average_price(UexEnvironment::Live, TradeSide::Sell),
        Some(sell)
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_11_r_api_6__schema_one_requires_backup_and_drops_ambiguous_legacy_average_without_losing_vocabulary()
 {
    use tallyline_shared_kernel::api::model::{TradeSide, UexEnvironment};
    let directory = TestDirectory::new().unwrap();
    let path = directory.0.join("legacy.db");
    {
        let database = rusqlite::Connection::open(&path).unwrap();
        database.execute_batch("CREATE TABLE ref_cache(endpoint TEXT PRIMARY KEY,fetched_at TEXT NOT NULL,payload TEXT NOT NULL); CREATE TABLE ref_commodities(id INTEGER PRIMARY KEY,name TEXT NOT NULL,code TEXT NOT NULL,available_live INTEGER NOT NULL); PRAGMA user_version=1;").unwrap();
        database.execute("INSERT INTO ref_cache VALUES('commodities','1234',?1)",[r#"[{"id":7,"name":"Legacy Ore","code":"OLD","available_live":true,"average_price":120}]"#]).unwrap();
    }
    let before = std::fs::read(&path).unwrap();
    assert!(SqliteReferenceCache::open(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(
        SqliteReferenceCache::open_with_migration_backup(&path, |_, schema| {
            assert_eq!(schema, 1);
            Err(tallyline_adapter_storage::api::StorageError::InvalidBackup)
        })
        .is_err()
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let backup = directory.0.join("before.db");
    let mut migrated = SqliteReferenceCache::open_with_migration_backup(&path, |cache, schema| {
        assert_eq!(schema, 1);
        assert_eq!(
            cache.backup_into(&backup, Timestamp::from_millis(2000), Some(3))?,
            1
        );
        Ok(())
    })
    .unwrap();
    let loaded = migrated.load().unwrap();
    assert_eq!(loaded[0].fetched_at(), Timestamp::from_millis(1234));
    let EndpointData::Commodities(items) = loaded[0].data() else {
        panic!("wrong endpoint")
    };
    assert_eq!(items[0].id(), CommodityId(7));
    assert_eq!(items[0].name(), "Legacy Ore");
    for side in [TradeSide::Buy, TradeSide::Sell] {
        assert_eq!(items[0].average_price(UexEnvironment::Live, side), None);
    }
    assert_eq!(
        migrated
            .backup_into(
                &directory.0.join("after.db"),
                Timestamp::from_millis(3000),
                Some(4)
            )
            .unwrap(),
        17
    );
    drop(migrated);
    assert!(SqliteReferenceCache::open(&path).is_ok());
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_api_1_r_ui_8__parameter_cache_preserves_all_acceptance_switches_constraints_and_capped_text() {
    use tallyline_reference_data::api::model::{
        CommodityConstraints, ReferenceParameters, SubmissionAcceptance,
    };
    let directory = TestDirectory::new().unwrap();
    let path = directory.0.join("tallyline.db");
    for bits in 0..16 {
        let parameters = ReferenceParameters::new(
            SubmissionAcceptance {
                datacenter: (bits & 1 != 0).into(),
                reports: (bits & 2 != 0).into(),
                commodity: (bits & 4 != 0).into(),
                ptu: (bits & 8 != 0).into(),
            },
            CommodityConstraints {
                price_variation_percent: 25,
                prior_ttl_days: std::num::NonZeroU32::new(15).unwrap(),
                scu_variation_percent: 5000,
            },
            Some("<b>&amp;</b>"),
        );
        let entry = CachedReference::new(
            Timestamp::from_millis(1234),
            EndpointData::Parameters(parameters),
        );
        {
            let mut cache = SqliteReferenceCache::open(&path).unwrap();
            cache.replace(&entry).unwrap();
        }
        assert_eq!(
            SqliteReferenceCache::open(&path).unwrap().load().unwrap(),
            vec![entry]
        );
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_api_1_r_api_6__terminal_prior_cache_preserves_exact_values_wide_times_and_successful_empty_assortments()
 {
    use tallyline_reference_data::api::model::{
        PricePrior, PriorProvenance, PriorValues, TerminalPriors,
    };
    use tallyline_shared_kernel::api::model::{ContainerSize, TerminalId, TradeSide};
    let directory = TestDirectory::new().unwrap();
    let path = directory.0.join("tallyline.db");
    let row = PricePrior::new(
        CommodityId(7),
        TradeSide::Sell,
        PriorValues {
            price: "9007199254740993.125".parse().unwrap(),
            stock: "0.0625".parse().unwrap(),
            status: None,
            container_sizes: vec![ContainerSize::Scu1, ContainerSize::Scu32],
        },
        PriorProvenance {
            game_version: "4.9".to_owned(),
            modified_at: Timestamp::from_seconds(u64::MAX),
        },
    )
    .unwrap();
    let first = CachedReference::new(
        Timestamp::from_millis(1234),
        EndpointData::TerminalPriors(TerminalPriors::new(TerminalId(17), vec![row]).unwrap()),
    );
    let empty = CachedReference::new(
        Timestamp::from_millis(5678),
        EndpointData::TerminalPriors(TerminalPriors::new(TerminalId(18), vec![]).unwrap()),
    );
    {
        let mut cache = SqliteReferenceCache::open(&path).unwrap();
        cache.replace(&first).unwrap();
        cache.replace(&empty).unwrap();
    }
    assert_eq!(
        SqliteReferenceCache::open(&path).unwrap().load().unwrap(),
        vec![first, empty]
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_api_5__prior_cache_refuses_a_terminal_key_that_disagrees_with_its_record() {
    use tallyline_reference_data::api::model::TerminalPriors;
    use tallyline_shared_kernel::api::model::TerminalId;
    let directory = TestDirectory::new().unwrap();
    let path = directory.0.join("tallyline.db");
    {
        let mut cache = SqliteReferenceCache::open(&path).unwrap();
        cache
            .replace(&CachedReference::new(
                Timestamp::from_seconds(1),
                EndpointData::TerminalPriors(TerminalPriors::new(TerminalId(17), vec![]).unwrap()),
            ))
            .unwrap();
    }
    {
        let database = rusqlite::Connection::open(&path).unwrap();
        database.execute("UPDATE ref_cache SET endpoint='terminal_priors:18' WHERE endpoint='terminal_priors:17'",[]).unwrap();
    }
    assert!(SqliteReferenceCache::open(&path).unwrap().load().is_err());
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_11_r_sub_11__recovery_format_backs_up_source_bound_schema_nine_before_migration() {
    let legacy = TestDirectory::new().unwrap();
    legacy_recovery_fixture::verify(&legacy.0).unwrap();
}

#[path = "support/legacy_recovery_fixture.rs"]
mod legacy_recovery_fixture;

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_11_r_sub_9__partial_resolution_format_requires_schema_twelve() {
    let directory = TestDirectory::new().unwrap();
    let path = directory.0.join("resolution.db");
    drop(SqliteReferenceCache::open(&path).unwrap());
    let database = Connection::open(&path).unwrap();
    let version: u32 = database
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(
        version, 17,
        "partial-resolution history must have an explicit older-reader format gate"
    );
}

#[path = "support/schema_ten_reader.rs"]
mod schema_ten_reader;

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_11_r_sub_9__schema_ten_requires_exact_backup_and_its_reader_refuses_resolution_format() {
    use tallyline_adapter_storage::api::StorageError;
    let directory = TestDirectory::new().unwrap();
    let path = directory.0.join("schema-ten.db");
    let backup = directory.0.join("schema-ten-backup.db");
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute_batch(include_str!("fixtures/schema-nine.sql"))
        .unwrap();
    connection
        .execute_batch(include_str!("fixtures/schema-ten-extension.sql"))
        .unwrap();
    drop(connection);
    schema_ten_reader::open(&path).unwrap();
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
    let cache = SqliteReferenceCache::open_with_migration_backup(&path, |cache, schema| {
        assert_eq!(schema, 10);
        assert_eq!(
            cache.backup_into(&backup, Timestamp::from_seconds(1500), Some(6))?,
            10
        );
        Ok(())
    })
    .unwrap();
    drop(cache);
    assert!(matches!(
        schema_ten_reader::open(&path),
        Err(StorageError::UnsupportedSchema {
            found: 17,
            supported: 10
        })
    ));
    let migrated = Connection::open(&path).unwrap();
    assert_eq!(
        migrated
            .pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
            .unwrap(),
        17
    );
    let backed_up = Connection::open(&backup).unwrap();
    assert_eq!(
        backed_up
            .pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
            .unwrap(),
        10
    );
    drop(backed_up);
    drop(migrated);
    drop(SqliteReferenceCache::open(&path).unwrap());
}
