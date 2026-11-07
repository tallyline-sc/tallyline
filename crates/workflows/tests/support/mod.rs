// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Manual application behaviour at its public persistence and reference boundaries.

use std::error::Error;
use std::path::Path;
use tallyline_adapter_storage::api::SqliteReferenceCache;
use tallyline_reference_data::api::model::{
    CachedReference, Commodity, CommodityAverages, EndpointData, GameVersions, PricePrior,
    PriorProvenance, PriorValues, ReferenceSnapshot, StatusLevel, StatusLevels, Terminal,
    TerminalPriors, TerminalText, UexEnvironment,
};
use tallyline_reference_data::api::{ReferenceApplication, ReferenceDataCache};
use tallyline_reporting::api::model::SubmissionGate;
use tallyline_shared_kernel::api::Clock;
use tallyline_shared_kernel::api::model::SubmissionMode;
use tallyline_shared_kernel::api::model::{CommodityId, TerminalId, Timestamp, TradeSide};

pub struct Fixed;
impl Clock for Fixed {
    fn now(&self) -> Timestamp {
        Timestamp::from_seconds(1000)
    }
}
struct Cache(Vec<CachedReference>);
impl ReferenceDataCache for Cache {
    type Error = std::convert::Infallible;
    fn load(&mut self) -> Result<Vec<CachedReference>, Self::Error> {
        Ok(self.0.clone())
    }
    fn replace(&mut self, _: &CachedReference) -> Result<(), Self::Error> {
        Ok(())
    }
}
pub fn records() -> Result<Vec<CachedReference>, Box<dyn std::error::Error>> {
    let terminal = Terminal::new(
        TerminalId(11),
        TerminalText {
            name: "Depot".to_owned(),
            nickname: "DP".to_owned(),
            display_name: "Shop".to_owned(),
            location: "Pyro".to_owned(),
        },
        true,
    );
    let prior = PricePrior::new(
        CommodityId(42),
        TradeSide::Buy,
        PriorValues {
            price: "123.45".parse()?,
            stock: "0".parse()?,
            status: std::num::NonZeroU8::new(3),
            container_sizes: vec![],
        },
        PriorProvenance {
            game_version: "4.10.2".to_owned(),
            modified_at: Timestamp::from_seconds(900),
        },
    )?;
    let level = StatusLevel {
        code: std::num::NonZeroU8::new(3).ok_or("invalid fixture status")?,
        name: "Medium".to_owned(),
        short_name: "Med".to_owned(),
        abbreviation: "M".to_owned(),
    };
    Ok(vec![
        EndpointData::Terminals(vec![terminal]),
        EndpointData::Commodities(vec![Commodity::new(
            CommodityId(42),
            "Gold",
            "GOLD",
            true,
            CommodityAverages::new(None, None),
        )]),
        EndpointData::TerminalPriors(TerminalPriors::new(TerminalId(11), vec![prior])?),
        EndpointData::GameVersions(GameVersions {
            live: "4.10.2".to_owned(),
            ptu: None,
        }),
        EndpointData::StatusLevels(StatusLevels {
            buy: vec![level],
            sell: vec![],
        }),
    ]
    .into_iter()
    .map(|data| CachedReference::new(Timestamp::from_seconds(950), data))
    .collect())
}
pub fn snapshot(
    environment: UexEnvironment,
) -> Result<ReferenceSnapshot, Box<dyn std::error::Error>> {
    snapshot_records(records()?, environment)
}

pub fn stored_gate(
    path: &std::path::Path,
) -> Result<
    (
        tallyline_adapter_storage::api::SqliteReferenceCache,
        tallyline_reporting::api::model::SubmissionGate,
    ),
    Box<dyn std::error::Error>,
> {
    stored_gate_with_mode(path, SubmissionMode::DryRun)
}

pub fn stored_gate_with_mode(
    path: &Path,
    mode: SubmissionMode,
) -> Result<(SqliteReferenceCache, SubmissionGate), Box<dyn Error>> {
    use tallyline_game::api::model::{EnvironmentMapping, GameEnvironment};
    use tallyline_reporting::api::model::{
        ReleaseAccountStanding, ReleaseContext, ReleaseScreenshotPolicy, ReleaseWorkingCopy,
        ReportKey, StockSettings,
    };
    use tallyline_reporting::api::{ManualReviewer, ReportRepository};
    use tallyline_shared_kernel::api::model::ObservationAgeSettings;
    let mut store = SqliteReferenceCache::open(path)?;
    for record in records()? {
        store.replace(&record)?;
    }
    let mut report = store.create_manual(
        ReportKey::new(TerminalId(11), TradeSide::Buy, GameEnvironment::Live)?,
        Timestamp::from_seconds(900),
        Some("4.10.2"),
    )?;
    let row = report.add_commodity(CommodityId(42))?;
    report.set_price(row, Some("123.45".parse()?))?;
    report.set_stock(row, Some("0".parse()?))?;
    report.set_status(row, std::num::NonZeroU8::new(3))?;
    store.save_draft(&report, 0)?;
    let reviewer = ManualReviewer::new(
        EnvironmentMapping::default(),
        StockSettings::default(),
        None,
    );
    let gate = reviewer.submission_gate(
        report,
        &snapshot(UexEnvironment::Live)?,
        ReleaseContext::new(
            ReleaseAccountStanding::Accepted("dummy-a".to_owned()),
            mode,
            ReleaseScreenshotPolicy::Unknown,
            ReleaseWorkingCopy::Absent,
        ),
        ObservationAgeSettings::default(),
        Timestamp::from_seconds(1000),
    );
    let history = tallyline_workflows::api::SubmissionQueueStore::queue_jobs(&mut store)
        .map_err(|_| std::io::Error::other("fixture history unavailable"))?;
    let gate = tallyline_workflows::api::assess_release_observation_history(&gate, Some(&history));
    Ok((store, gate))
}

pub fn snapshot_records(
    records: Vec<CachedReference>,
    environment: UexEnvironment,
) -> Result<ReferenceSnapshot, Box<dyn Error>> {
    Ok(ReferenceApplication::load(Cache(records), Fixed)?.snapshot(environment))
}
