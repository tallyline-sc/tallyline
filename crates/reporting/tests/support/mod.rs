// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Manual application behaviour at its public persistence and reference boundaries.

use tallyline_game::api::model::GameEnvironment;
use tallyline_reference_data::api::model::{
    CachedReference, Commodity, CommodityAverages, EndpointData, GameVersions, PricePrior,
    PriorProvenance, PriorValues, ReferenceSnapshot, StatusLevel, StatusLevels, Terminal,
    TerminalPriors, TerminalText, UexEnvironment,
};
use tallyline_reference_data::api::{ReferenceApplication, ReferenceDataCache};
use tallyline_reporting::api::model::{Report, ReportId, ReportKey};
use tallyline_reporting::api::{ReportRepository, RepositoryError};
use tallyline_shared_kernel::api::Clock;
use tallyline_shared_kernel::api::model::{CommodityId, TerminalId, Timestamp, TradeSide};

pub struct Fixed;
impl Clock for Fixed {
    fn now(&self) -> Timestamp {
        Timestamp::from_seconds(1000)
    }
}
#[derive(Default)]
pub struct Repository(pub Vec<Report>);
impl ReportRepository for Repository {
    fn create_manual(
        &mut self,
        key: ReportKey,
        at: Timestamp,
        version: Option<&str>,
    ) -> Result<Report, RepositoryError> {
        let id = self.0.iter().map(|report| report.id().0).max().unwrap_or(0) + 1;
        let report =
            Report::manual(ReportId(id), key, at, version).map_err(|_| RepositoryError::Invalid)?;
        self.0.push(report.clone());
        Ok(report)
    }
    fn load_draft(&mut self, id: ReportId) -> Result<Option<Report>, RepositoryError> {
        Ok(self.0.iter().find(|report| report.id() == id).cloned())
    }
    fn drafts(&mut self) -> Result<Vec<Report>, RepositoryError> {
        Ok(self.0.clone())
    }
    fn save_draft(&mut self, report: &Report, expected: u64) -> Result<(), RepositoryError> {
        let stored = self
            .0
            .iter_mut()
            .find(|stored| stored.id() == report.id())
            .ok_or(RepositoryError::Invalid)?;
        if report.version() <= expected {
            return Err(RepositoryError::Invalid);
        }
        if stored.version() != expected {
            return Err(RepositoryError::Conflict);
        }
        *stored = report.clone();
        Ok(())
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
pub fn snapshot(
    environment: UexEnvironment,
) -> Result<ReferenceSnapshot, Box<dyn std::error::Error>> {
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
    Ok(cached_snapshot(
        environment,
        vec![
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
        ],
    ))
}
pub fn key() -> Result<ReportKey, tallyline_reporting::api::model::InvalidReport> {
    ReportKey::new(TerminalId(11), TradeSide::Buy, GameEnvironment::Live)
}

pub fn cached_snapshot(
    environment: UexEnvironment,
    entries: Vec<EndpointData>,
) -> ReferenceSnapshot {
    ReferenceApplication::load(
        Cache(
            entries
                .into_iter()
                .map(|data| CachedReference::new(Timestamp::from_seconds(950), data))
                .collect(),
        ),
        Fixed,
    )
    .unwrap()
    .snapshot(environment)
}
