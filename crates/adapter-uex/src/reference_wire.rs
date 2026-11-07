// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Public reference vocabulary translated into context-owned records.

use crate::api::ReadError;
use crate::wire::{Numeric, malformed, parse, unescaped};
use serde::Deserialize;
use serde_json::Value;
use tallyline_reference_data::api::model::{Terminal, TerminalText};
use tallyline_shared_kernel::api::model::TerminalId;

#[derive(Deserialize)]
struct TerminalWire {
    id: Numeric,
    name: String,
    nickname: String,
    displayname: String,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    max_container_size: Option<Numeric>,
    is_player_owned: Numeric,
    is_available_live: Numeric,
    #[serde(default)]
    star_system_name: Option<String>,
    #[serde(default)]
    planet_name: Option<String>,
    #[serde(default)]
    orbit_name: Option<String>,
    #[serde(default)]
    moon_name: Option<String>,
    #[serde(default)]
    space_station_name: Option<String>,
    #[serde(default)]
    outpost_name: Option<String>,
    #[serde(default)]
    city_name: Option<String>,
}

pub(crate) fn terminals(data: Value) -> Result<Vec<Terminal>, ReadError> {
    let rows: Vec<TerminalWire> = parse(data, "terminals")?;
    let mut terminals = Vec::new();
    let mut identities = std::collections::BTreeSet::new();
    for row in rows {
        if row.kind != "commodity" || row.is_player_owned.flag("is_player_owned")? {
            continue;
        }
        let id = u32::try_from(row.id.whole("id")?)
            .ok()
            .filter(|id| *id > 0)
            .ok_or_else(|| malformed("terminal id is outside the supported range"))?;
        if !identities.insert(id) {
            return Err(malformed("terminal response repeats an identity"));
        }
        let mut locations = Vec::new();
        for name in [
            row.star_system_name,
            row.planet_name,
            row.orbit_name,
            row.moon_name,
            row.space_station_name,
            row.outpost_name,
            row.city_name,
        ]
        .into_iter()
        .flatten()
        {
            let decoded = unescaped(&name);
            if !decoded.is_empty() && !locations.contains(&decoded) {
                locations.push(decoded);
            }
        }
        let maximum = row
            .max_container_size
            .map(|value| {
                let maximum = u32::try_from(value.whole("max_container_size")?)
                    .map_err(|_| malformed("max_container_size exceeds the supported range"))?;
                Ok(std::num::NonZeroU32::new(maximum))
            })
            .transpose()?
            .flatten();
        terminals.push(
            Terminal::new(
                TerminalId(id),
                TerminalText {
                    name: unescaped(&row.name),
                    nickname: unescaped(&row.nickname),
                    display_name: unescaped(&row.displayname),
                    location: locations.join(" / "),
                },
                row.is_available_live.flag("is_available_live")?,
            )
            .with_max_container_size(maximum),
        );
    }
    Ok(terminals)
}

#[derive(Deserialize)]
struct CommodityWire {
    id: Numeric,
    name: String,
    code: Option<String>,
    is_available_live: Numeric,
    price_buy: Numeric,
    price_sell: Numeric,
}

pub(crate) fn commodities(
    data: Value,
) -> Result<Vec<tallyline_reference_data::api::model::Commodity>, ReadError> {
    use tallyline_reference_data::api::model::{Commodity, CommodityAverages};
    use tallyline_shared_kernel::api::model::CommodityId;
    let rows: Vec<CommodityWire> = parse(data, "commodities")?;
    let mut commodities = Vec::new();
    let mut identities = std::collections::BTreeSet::new();
    for row in rows {
        let id = u32::try_from(row.id.whole("id")?)
            .ok()
            .filter(|id| *id > 0)
            .ok_or_else(|| malformed("commodity id is outside the supported range"))?;
        if !identities.insert(id) {
            return Err(malformed("commodity response repeats an identity"));
        }
        commodities.push(Commodity::new(
            CommodityId(id),
            &unescaped(&row.name),
            &unescaped(row.code.as_deref().unwrap_or_default()),
            row.is_available_live.flag("is_available_live")?,
            CommodityAverages::new(
                Some(row.price_buy.price("price_buy")?),
                Some(row.price_sell.price("price_sell")?),
            ),
        ));
    }
    Ok(commodities)
}
