// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Atomic reference cache projection and mapped JSON persistence.

use crate::api::StorageError;
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tallyline_reference_data::api::ReferenceDataCache;
use tallyline_reference_data::api::model::{
    CachedReference, Commodity, CommodityAverages, EndpointData, GameVersions, Terminal,
    TerminalText, UexEnvironment,
};
use tallyline_shared_kernel::api::model::{
    CommodityId, PricePerScu, TerminalId, Timestamp, TradeSide,
};

const SCHEMA_VERSION: u32 = 17;
const SQLITE_BUSY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

const MIGRATION:&str="CREATE TABLE ref_cache(endpoint TEXT PRIMARY KEY, fetched_at TEXT NOT NULL, payload TEXT NOT NULL);
CREATE TABLE ref_commodities(id INTEGER PRIMARY KEY, name TEXT NOT NULL, code TEXT NOT NULL, available_live INTEGER NOT NULL);
CREATE INDEX ref_commodities_name ON ref_commodities(name);
CREATE INDEX ref_commodities_code ON ref_commodities(code);
PRAGMA user_version=1;";

/// Owns the reference cache connection on its dedicated storage worker.
pub struct SqliteReferenceCache {
    pub(crate) connection: Connection,
}

impl SqliteReferenceCache {
    /// Opens the application database and applies forward migrations.
    ///
    /// # Errors
    /// Returns an inaccessible database, unknown schema or failed migration.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        Self::open_with_migration_backup(path, |_, _| Err(StorageError::MigrationBackupRequired))
    }

    /// Opens a cache, backing up existing old schemas before the forward transaction.
    ///
    /// The application coordinator must commit a verified backup set before returning success.
    ///
    /// # Errors
    /// Returns an unsupported schema, failed backup or rolled-back migration.
    pub fn open_with_migration_backup(
        path: impl AsRef<Path>,
        backup: impl FnOnce(&mut Self, u32) -> Result<(), StorageError>,
    ) -> Result<Self, StorageError> {
        let path = path.as_ref();
        if path.try_exists().map_err(StorageError::Filesystem)? {
            let readonly = Connection::open_with_flags(
                path,
                OpenFlags::SQLITE_OPEN_READ_ONLY
                    | OpenFlags::SQLITE_OPEN_NO_MUTEX
                    | OpenFlags::SQLITE_OPEN_NOFOLLOW,
            )?;
            let found: u32 = readonly.pragma_query_one(None, "user_version")?;
            if found > SCHEMA_VERSION {
                return Err(StorageError::UnsupportedSchema {
                    found,
                    supported: SCHEMA_VERSION,
                });
            }
        }
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )?;
        connection.busy_timeout(SQLITE_BUSY_TIMEOUT)?;
        let mut cache = Self { connection };
        let version: u32 = cache.connection.pragma_query_one(None, "user_version")?;
        if version > SCHEMA_VERSION {
            return Err(StorageError::UnsupportedSchema {
                found: version,
                supported: SCHEMA_VERSION,
            });
        }
        if version < SCHEMA_VERSION {
            if version != 0 {
                backup(&mut cache, version)?;
            }
            let transaction = cache.connection.transaction()?;
            for next in version + 1..=SCHEMA_VERSION {
                match next {
                    1 => transaction.execute_batch(MIGRATION)?,
                    2 => crate::migration::schema_two(&transaction)?,
                    3 => crate::migration::schema_three(&transaction)?,
                    4 => crate::migration::schema_four(&transaction)?,
                    5 => crate::migration::schema_five(&transaction)?,
                    6 => crate::migration::schema_six(&transaction)?,
                    7 => crate::migration::schema_seven(&transaction)?,
                    8 => crate::migration::schema_eight(&transaction)?,
                    9 => crate::migration::schema_nine(&transaction)?,
                    10 => crate::migration::schema_ten(&transaction)?,
                    11 => crate::migration::schema_eleven(&transaction)?,
                    12 => crate::migration::schema_twelve(&transaction)?,
                    13 => crate::migration::schema_thirteen(&transaction)?,
                    14 => crate::migration::schema_fourteen(&transaction)?,
                    15 => crate::migration::schema_fifteen(&transaction)?,
                    16 => crate::migration::schema_sixteen(&transaction)?,
                    17 => crate::migration::schema_seventeen(&transaction)?,
                    _ => return Err(StorageError::InvalidCache),
                }
            }
            transaction.commit()?;
        }
        Ok(cache)
    }
}

#[derive(Serialize, Deserialize)]
struct CommodityDocument {
    id: u32,
    name: String,
    code: String,
    available_live: bool,
    #[serde(default)]
    average_buy: Option<String>,
    #[serde(default)]
    average_sell: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct TerminalDocument {
    id: u32,
    name: String,
    nickname: String,
    display_name: String,
    location: String,
    available_live: bool,
    #[serde(default)]
    max_container_size: Option<std::num::NonZeroU32>,
}

#[derive(Serialize, Deserialize)]
struct GameVersionsDocument {
    live: String,
    ptu: Option<String>,
}
pub(crate) fn timestamp(fetched_at: &str) -> Result<Timestamp, StorageError> {
    let millis = fetched_at
        .parse::<u128>()
        .map_err(|_| StorageError::InvalidCache)?;
    if let Ok(millis) = u64::try_from(millis) {
        Ok(Timestamp::from_millis(millis))
    } else if millis.is_multiple_of(1_000) {
        Ok(Timestamp::from_seconds(
            u64::try_from(millis / 1_000).map_err(|_| StorageError::InvalidCache)?,
        ))
    } else {
        Err(StorageError::InvalidCache)
    }
}

pub(crate) fn decode(endpoint: &str, payload: &str) -> Result<EndpointData, StorageError> {
    if endpoint.starts_with("terminal_priors:") {
        let priors = crate::prior_document::decode(payload)?;
        if endpoint != format!("terminal_priors:{}", priors.terminal().0) {
            return Err(StorageError::InvalidCache);
        }
        return Ok(EndpointData::TerminalPriors(priors));
    }
    match endpoint {
        "commodities" => {
            let documents: Vec<CommodityDocument> =
                serde_json::from_str(payload).map_err(|_| StorageError::InvalidCache)?;
            Ok(EndpointData::Commodities(
                documents
                    .into_iter()
                    .map(|document| {
                        Ok(Commodity::new(
                            CommodityId(document.id),
                            &document.name,
                            &document.code,
                            document.available_live,
                            CommodityAverages::new(
                                parse_average(document.average_buy)?,
                                parse_average(document.average_sell)?,
                            ),
                        ))
                    })
                    .collect::<Result<Vec<_>, StorageError>>()?,
            ))
        }
        "terminals" => {
            let documents: Vec<TerminalDocument> =
                serde_json::from_str(payload).map_err(|_| StorageError::InvalidCache)?;
            Ok(EndpointData::Terminals(
                documents
                    .into_iter()
                    .map(|document| {
                        Terminal::new(
                            TerminalId(document.id),
                            TerminalText {
                                name: document.name,
                                nickname: document.nickname,
                                display_name: document.display_name,
                                location: document.location,
                            },
                            document.available_live,
                        )
                        .with_max_container_size(document.max_container_size)
                    })
                    .collect(),
            ))
        }
        "game_versions" => {
            let document: GameVersionsDocument =
                serde_json::from_str(payload).map_err(|_| StorageError::InvalidCache)?;
            Ok(EndpointData::GameVersions(GameVersions {
                live: document.live,
                ptu: document.ptu,
            }))
        }
        "parameters" => crate::parameter_document::decode(payload).map(EndpointData::Parameters),
        "status_levels" => crate::status_document::decode(payload).map(EndpointData::StatusLevels),
        _ => Err(StorageError::InvalidCache),
    }
}

pub(crate) fn encode(data: &EndpointData) -> Result<(String, String), StorageError> {
    let (endpoint, payload) = match data {
        EndpointData::TerminalPriors(priors) => (
            format!("terminal_priors:{}", priors.terminal().0),
            crate::prior_document::encode(priors),
        ),
        EndpointData::Parameters(parameters) => (
            "parameters".to_owned(),
            crate::parameter_document::encode(parameters),
        ),
        EndpointData::Commodities(commodities) => {
            let documents = commodities
                .iter()
                .map(|commodity| CommodityDocument {
                    id: commodity.id().0,
                    name: commodity.name().to_owned(),
                    code: commodity.code().to_owned(),
                    available_live: commodity.available(UexEnvironment::Live),
                    average_buy: commodity
                        .average_price(UexEnvironment::Live, TradeSide::Buy)
                        .map(|price| price.to_string()),
                    average_sell: commodity
                        .average_price(UexEnvironment::Live, TradeSide::Sell)
                        .map(|price| price.to_string()),
                })
                .collect::<Vec<_>>();
            ("commodities".to_owned(), serde_json::to_string(&documents))
        }
        EndpointData::Terminals(terminals) => {
            let documents = terminals
                .iter()
                .map(|terminal| TerminalDocument {
                    id: terminal.id().0,
                    name: terminal.text().name.clone(),
                    nickname: terminal.text().nickname.clone(),
                    display_name: terminal.text().display_name.clone(),
                    location: terminal.text().location.clone(),
                    available_live: terminal.available(UexEnvironment::Live),
                    max_container_size: terminal.max_container_size(),
                })
                .collect::<Vec<_>>();
            ("terminals".to_owned(), serde_json::to_string(&documents))
        }
        EndpointData::GameVersions(versions) => (
            "game_versions".to_owned(),
            serde_json::to_string(&GameVersionsDocument {
                live: versions.live.clone(),
                ptu: versions.ptu.clone(),
            }),
        ),
        EndpointData::StatusLevels(levels) => {
            if !crate::status_document::valid(levels) {
                return Err(StorageError::InvalidCache);
            }
            (
                "status_levels".to_owned(),
                crate::status_document::encode(levels),
            )
        }
    };
    Ok((endpoint, payload.map_err(|_| StorageError::InvalidCache)?))
}

impl ReferenceDataCache for SqliteReferenceCache {
    type Error = StorageError;

    fn load(&mut self) -> Result<Vec<CachedReference>, Self::Error> {
        load_references(&self.connection)
    }

    fn replace(&mut self, entry: &CachedReference) -> Result<(), Self::Error> {
        let (endpoint, payload) = encode(entry.data())?;
        let transaction = self.connection.transaction()?;
        transaction.execute("INSERT INTO ref_cache(endpoint,fetched_at,payload) VALUES(?1,?2,?3) ON CONFLICT(endpoint) DO UPDATE SET fetched_at=excluded.fetched_at,payload=excluded.payload",(endpoint,entry.fetched_at().millis().to_string(),payload))?;
        match entry.data() {
            EndpointData::Commodities(commodities) => {
                transaction.execute("DELETE FROM ref_commodities", [])?;
                for commodity in commodities {
                    transaction.execute("INSERT INTO ref_commodities(id,name,code,available_live) VALUES(?1,?2,?3,?4)",
                        (commodity.id().0,commodity.name(),commodity.code(),commodity.available(UexEnvironment::Live)))?;
                }
            }
            EndpointData::Terminals(terminals) => {
                crate::migration::index_terminals(&transaction, terminals)?
            }
            EndpointData::GameVersions(_)
            | EndpointData::StatusLevels(_)
            | EndpointData::Parameters(_)
            | EndpointData::TerminalPriors(_) => {}
        }
        crate::accepted_priors::refreshed(&transaction, entry.data())?;
        transaction.commit()?;
        Ok(())
    }
}

fn parse_average(value: Option<String>) -> Result<Option<PricePerScu>, StorageError> {
    value
        .map(|value| value.parse().map_err(|_| StorageError::InvalidCache))
        .transpose()
}

pub(crate) fn load_references(
    connection: &Connection,
) -> Result<Vec<CachedReference>, StorageError> {
    let mut entries = Vec::new();
    for endpoint in [
        "commodities",
        "game_versions",
        "status_levels",
        "terminals",
        "parameters",
    ] {
        let cached = connection
            .query_row(
                "SELECT fetched_at,payload FROM ref_cache WHERE endpoint=?1",
                [endpoint],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        if let Some((fetched_at, payload)) = cached {
            entries.push(CachedReference::new(
                timestamp(&fetched_at)?,
                decode(endpoint, &payload)?,
            ));
        }
    }
    let mut statement=connection.prepare("SELECT endpoint,fetched_at,payload FROM ref_cache WHERE endpoint GLOB 'terminal_priors:*' ORDER BY endpoint")?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    for record in rows {
        let (endpoint, fetched_at, payload) = record?;
        entries.push(CachedReference::new(
            timestamp(&fetched_at)?,
            decode(&endpoint, &payload)?,
        ));
    }
    Ok(entries)
}
