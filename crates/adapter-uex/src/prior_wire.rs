// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Complete LIVE price priors grouped by their requested terminal identities.

use crate::api::ReadError;
use crate::wire::{Numeric, container_sizes, malformed, parse};
use serde::Deserialize;
use serde_json::Value;
use tallyline_reference_data::api::model::{
    PricePrior, PriorProvenance, PriorValues, TerminalPriors,
};
use tallyline_shared_kernel::api::model::{CommodityId, TerminalId, Timestamp, TradeSide};

#[derive(Deserialize)]
struct PriorWire {
    id_terminal: Numeric,
    id_commodity: Numeric,
    price_buy: Numeric,
    price_sell: Numeric,
    scu_buy: Numeric,
    scu_sell_stock: Numeric,
    status_buy: Numeric,
    status_sell: Numeric,
    container_sizes: Option<String>,
    game_version: String,
    date_modified: Numeric,
}

pub(crate) fn priors(
    data: Value,
    terminals: &[TerminalId],
) -> Result<Vec<TerminalPriors>, ReadError> {
    let rows: Vec<PriorWire> = parse(data, "commodities_prices")?;
    let mut groups = terminals
        .iter()
        .map(|id| (*id, Vec::new()))
        .collect::<std::collections::BTreeMap<_, _>>();
    for wire in rows {
        let terminal = TerminalId(identity(&wire.id_terminal, "id_terminal")?);
        let group = groups
            .get_mut(&terminal)
            .ok_or_else(|| malformed("prior response includes an unrequested terminal"))?;
        group.push(prior(wire)?);
    }
    terminals
        .iter()
        .map(|terminal| {
            let rows = groups
                .remove(terminal)
                .ok_or_else(|| malformed("prior request repeats a terminal"))?;
            TerminalPriors::new(*terminal, rows)
                .map_err(|_| malformed("prior response contains an invalid terminal assortment"))
        })
        .collect()
}
fn prior(wire: PriorWire) -> Result<PricePrior, ReadError> {
    let buy = wire.price_buy.price("price_buy")?;
    let sell = wire.price_sell.price("price_sell")?;
    let (side, price, stock, status) = match (!buy.is_zero(), !sell.is_zero()) {
        (true, false) => (
            TradeSide::Buy,
            buy,
            wire.scu_buy.quantity("scu_buy")?,
            status(&wire.status_buy, "status_buy")?,
        ),
        (false, true) => (
            TradeSide::Sell,
            sell,
            wire.scu_sell_stock.quantity("scu_sell_stock")?,
            status(&wire.status_sell, "status_sell")?,
        ),
        (true, true) | (false, false) => {
            return Err(malformed(
                "prior row does not have exactly one positive traded price",
            ));
        }
    };
    PricePrior::new(
        CommodityId(identity(&wire.id_commodity, "id_commodity")?),
        side,
        PriorValues {
            price,
            stock,
            status,
            container_sizes: container_sizes(wire.container_sizes.as_deref().unwrap_or_default())?,
        },
        PriorProvenance {
            game_version: wire.game_version,
            modified_at: Timestamp::from_seconds(wire.date_modified.whole("date_modified")?),
        },
    )
    .map_err(|_| malformed("prior row identity, price or provenance is invalid"))
}
fn status(value: &Numeric, field: &str) -> Result<Option<std::num::NonZeroU8>, ReadError> {
    let code = u8::try_from(value.whole(field)?)
        .map_err(|_| malformed("prior status exceeds the supported range"))?;
    Ok(std::num::NonZeroU8::new(code))
}
fn identity(value: &Numeric, field: &str) -> Result<u32, ReadError> {
    u32::try_from(value.whole(field)?)
        .ok()
        .filter(|id| *id > 0)
        .ok_or_else(|| malformed("prior identity is outside the supported range"))
}
