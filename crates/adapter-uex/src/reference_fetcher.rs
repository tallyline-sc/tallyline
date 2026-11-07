// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! The asynchronous typed reference-data port implemented by the HTTP leaf.

use crate::api::{ReadError, UexReader};
use tallyline_reference_data::api::ReferenceDataFetcher;
use tallyline_reference_data::api::model::{
    Endpoint, EndpointData, GameVersions, StatusLevel, StatusLevels,
};

const MAX_TERMINAL_BATCH: usize = 10;

impl ReferenceDataFetcher for UexReader {
    type Error = ReadError;

    async fn fetch(&self, endpoint: Endpoint) -> Result<EndpointData, ReadError> {
        match endpoint {
            Endpoint::TerminalPriors(terminal) => {
                let mut priors = self.fetch_terminal_priors(&[terminal]).await?;
                priors
                    .pop()
                    .map(EndpointData::TerminalPriors)
                    .ok_or_else(|| {
                        crate::wire::malformed("prior batch omitted the requested terminal")
                    })
            }
            Endpoint::Parameters => {
                let data = self.transport.get("data_parameters/", &[], &[]).await?;
                self.transport
                    .process(move || {
                        crate::parameter_wire::parameters(data).map(EndpointData::Parameters)
                    })
                    .await
            }
            Endpoint::Commodities => {
                let data = self.transport.get("commodities/", &[], &[]).await?;
                self.transport
                    .process(move || {
                        crate::reference_wire::commodities(data).map(EndpointData::Commodities)
                    })
                    .await
            }
            Endpoint::Terminals => self.terminals().await.map(EndpointData::Terminals),
            Endpoint::StatusLevels => {
                let levels = self.commodity_status_levels().await?;
                self.transport
                    .process(move || {
                        Ok(EndpointData::StatusLevels(StatusLevels {
                            buy: mapped_status_levels(levels.buy)?,
                            sell: mapped_status_levels(levels.sell)?,
                        }))
                    })
                    .await
            }
            Endpoint::GameVersions => {
                let versions = self.game_versions().await?;
                Ok(EndpointData::GameVersions(GameVersions {
                    live: versions.live,
                    ptu: versions.ptu,
                }))
            }
        }
    }
    async fn fetch_terminal_priors(
        &self,
        terminals: &[tallyline_shared_kernel::api::model::TerminalId],
    ) -> Result<Vec<tallyline_reference_data::api::model::TerminalPriors>, ReadError> {
        let distinct = terminals
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        if !(1..=MAX_TERMINAL_BATCH).contains(&terminals.len())
            || terminals.iter().any(|id| id.0 == 0)
            || distinct.len() != terminals.len()
        {
            return Err(crate::wire::malformed(
                "prior batch must contain one to ten distinct positive terminal identities",
            ));
        }
        let ids = terminals
            .iter()
            .map(|id| id.0.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let data = self
            .transport
            .get("commodities_prices/", &[("id_terminal", &ids)], &[])
            .await?;
        let terminals = terminals.to_vec();
        self.transport
            .process(move || crate::prior_wire::priors(data, &terminals))
            .await
    }
}

fn status_level(level: crate::api::model::StatusLevel) -> Result<StatusLevel, ReadError> {
    Ok(StatusLevel {
        code: std::num::NonZeroU8::new(level.code).ok_or_else(|| ReadError::Malformed {
            detail: "status level code is zero".to_owned(),
        })?,
        name: level.name,
        short_name: level.name_short,
        abbreviation: level.name_abbr,
    })
}

fn mapped_status_levels(
    levels: Vec<crate::api::model::StatusLevel>,
) -> Result<Vec<StatusLevel>, ReadError> {
    let mut codes = std::collections::BTreeSet::new();
    levels
        .into_iter()
        .map(|level| {
            if !codes.insert(level.code) {
                return Err(ReadError::Malformed {
                    detail: "status vocabulary repeats a code within one side".to_owned(),
                });
            }
            status_level(level)
        })
        .collect()
}
