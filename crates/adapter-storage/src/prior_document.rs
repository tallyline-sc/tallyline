// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Private lossless persisted projection of one terminal assortment.

use crate::api::StorageError;
use crate::reference_cache::timestamp;
use serde::{Deserialize, Serialize};
use tallyline_reference_data::api::model::{
    PricePrior, PriorProvenance, PriorValues, TerminalPriors,
};
use tallyline_shared_kernel::api::model::{CommodityId, ContainerSize, TerminalId, TradeSide};

#[derive(Serialize, Deserialize)]
struct PriorsDocument {
    terminal: u32,
    rows: Vec<RowDocument>,
}
#[derive(Serialize, Deserialize)]
enum SideDocument {
    Buy,
    Sell,
}
#[derive(Serialize, Deserialize)]
struct RowDocument {
    commodity: u32,
    side: SideDocument,
    price: String,
    stock: String,
    status: Option<std::num::NonZeroU8>,
    container_sizes: Vec<u32>,
    game_version: String,
    modified_at: String,
}

pub(crate) fn encode(priors: &TerminalPriors) -> Result<String, serde_json::Error> {
    serde_json::to_string(&PriorsDocument {
        terminal: priors.terminal().0,
        rows: priors
            .rows()
            .iter()
            .map(|row| RowDocument {
                commodity: row.commodity().0,
                side: match row.side() {
                    TradeSide::Buy => SideDocument::Buy,
                    TradeSide::Sell => SideDocument::Sell,
                },
                price: row.values().price.to_string(),
                stock: row.values().stock.to_string(),
                status: row.values().status,
                container_sizes: row
                    .values()
                    .container_sizes
                    .iter()
                    .map(|size| size.scu())
                    .collect(),
                game_version: row.provenance().game_version.clone(),
                modified_at: row.provenance().modified_at.millis().to_string(),
            })
            .collect(),
    })
}
pub(crate) fn decode(payload: &str) -> Result<TerminalPriors, StorageError> {
    let document: PriorsDocument =
        serde_json::from_str(payload).map_err(|_| StorageError::InvalidCache)?;
    let rows = document
        .rows
        .into_iter()
        .map(row)
        .collect::<Result<Vec<_>, _>>()?;
    TerminalPriors::new(TerminalId(document.terminal), rows).map_err(|_| StorageError::InvalidCache)
}
fn row(document: RowDocument) -> Result<PricePrior, StorageError> {
    let side = match document.side {
        SideDocument::Buy => TradeSide::Buy,
        SideDocument::Sell => TradeSide::Sell,
    };
    let values = PriorValues {
        price: document
            .price
            .parse()
            .map_err(|_| StorageError::InvalidCache)?,
        stock: document
            .stock
            .parse()
            .map_err(|_| StorageError::InvalidCache)?,
        status: document.status,
        container_sizes: document
            .container_sizes
            .into_iter()
            .map(|size| ContainerSize::from_scu(size).ok_or(StorageError::InvalidCache))
            .collect::<Result<Vec<_>, _>>()?,
    };
    let provenance = PriorProvenance {
        game_version: document.game_version,
        modified_at: timestamp(&document.modified_at)?,
    };
    PricePrior::new(CommodityId(document.commodity), side, values, provenance)
        .map_err(|_| StorageError::InvalidCache)
}
