// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! The anti-corruption layer: UEX wire objects, parsed defensively and translated into the model.

use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Number, Value};

use crate::api::ReadError;
use crate::api::model::{
    CommodityId, ContainerSize, GameVersions, PriceRow, ReportAccess, StatusLevel, StatusLevels,
    TerminalId, TradedSide, UexAccount,
};

const HTML_ENTITIES: [(&str, &str); 6] = [
    ("&lt;", "<"),
    ("&gt;", ">"),
    ("&quot;", "\""),
    ("&apos;", "'"),
    ("&#39;", "'"),
    ("&amp;", "&"),
];
const CONTAINER_SIZE_SEPARATOR: char = ',';

/// A number that UEX may send as a JSON number or as a numeric string.
#[derive(Deserialize)]
#[serde(untagged)]
pub(crate) enum Numeric {
    Number(Number),
    Text(String),
}

impl Numeric {
    pub(crate) fn whole(&self, field: &str) -> Result<u64, ReadError> {
        let parsed = match self {
            Self::Number(number) => number.as_u64(),
            Self::Text(text) => text.trim().parse().ok(),
        };
        parsed.ok_or_else(|| malformed(&format!("{field} is not a whole number")))
    }

    pub(crate) fn price(
        &self,
        field: &str,
    ) -> Result<tallyline_shared_kernel::api::model::PricePerScu, ReadError> {
        let parsed = match self {
            Self::Number(number) => number.to_string().parse(),
            Self::Text(text) => text.trim().parse(),
        };
        parsed.map_err(|_| {
            malformed(&format!(
                "{field} is not a supported non-negative exact price"
            ))
        })
    }

    pub(crate) fn quantity(
        &self,
        field: &str,
    ) -> Result<tallyline_shared_kernel::api::model::ScuQuantity, ReadError> {
        let parsed = match self {
            Self::Number(number) => number.to_string().parse(),
            Self::Text(text) => text.trim().parse(),
        };
        parsed.map_err(|_| {
            malformed(&format!(
                "{field} is not a supported non-negative exact SCU quantity"
            ))
        })
    }

    pub(crate) fn flag(&self, field: &str) -> Result<bool, ReadError> {
        match self.whole(field)? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(malformed(&format!("{field} is not a 0/1 flag"))),
        }
    }
}

#[derive(Deserialize)]
struct GameVersionsWire {
    live: String,
    ptu: Option<String>,
}

#[derive(Deserialize)]
struct StatusLevelsWire {
    buy: Vec<StatusLevelWire>,
    sell: Vec<StatusLevelWire>,
}

#[derive(Deserialize)]
struct StatusLevelWire {
    code: Numeric,
    name: String,
    name_short: String,
    name_abbr: String,
}

#[derive(Deserialize)]
struct PriceRowWire {
    id_terminal: Numeric,
    id_commodity: Numeric,
    price_buy: Numeric,
    price_sell: Numeric,
    status_buy: Numeric,
    status_sell: Numeric,
    container_sizes: Option<String>,
}

#[derive(Deserialize)]
struct AccountWire {
    username: String,
    is_datarunner: Numeric,
    is_datarunner_banned: Numeric,
    is_staff: Numeric,
}

pub(crate) fn game_versions(data: Value) -> Result<GameVersions, ReadError> {
    let wire: GameVersionsWire = parse(data, "game_versions")?;
    Ok(GameVersions {
        live: wire.live,
        ptu: wire.ptu.filter(|ptu| !ptu.is_empty()),
    })
}

pub(crate) fn status_levels(data: Value) -> Result<StatusLevels, ReadError> {
    let wire: StatusLevelsWire = parse(data, "commodities_status")?;
    Ok(StatusLevels {
        buy: side_levels(wire.buy)?,
        sell: side_levels(wire.sell)?,
    })
}

pub(crate) fn price_rows(data: Value) -> Result<Vec<PriceRow>, ReadError> {
    let wire: Vec<PriceRowWire> = parse(data, "commodities_prices")?;
    wire.into_iter().map(price_row).collect()
}

pub(crate) fn account(data: Value) -> Result<UexAccount, ReadError> {
    let wire: AccountWire = parse(data, "user")?;
    Ok(UexAccount {
        username: wire.username,
        is_datarunner: wire.is_datarunner.flag("is_datarunner")?,
        is_datarunner_banned: wire.is_datarunner_banned.flag("is_datarunner_banned")?,
        is_staff: wire.is_staff.flag("is_staff")?,
    })
}

pub(crate) fn report_access(data: &Value) -> Result<ReportAccess, ReadError> {
    match data {
        Value::Array(rows) => Ok(ReportAccess { rows: rows.len() }),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Object(_) => {
            Err(malformed("data_info data is not an array"))
        }
    }
}

fn side_levels(wire: Vec<StatusLevelWire>) -> Result<Vec<StatusLevel>, ReadError> {
    wire.into_iter().map(status_level).collect()
}

fn status_level(wire: StatusLevelWire) -> Result<StatusLevel, ReadError> {
    Ok(StatusLevel {
        code: level(&wire.code, "code")?,
        name: unescaped(&wire.name),
        name_short: unescaped(&wire.name_short),
        name_abbr: unescaped(&wire.name_abbr),
    })
}

fn price_row(wire: PriceRowWire) -> Result<PriceRow, ReadError> {
    let buy_price = wire.price_buy.price("price_buy")?;
    let sell_price = wire.price_sell.price("price_sell")?;
    let buys = !buy_price.is_zero();
    let sells = !sell_price.is_zero();
    let side = match (buys, sells) {
        (true, false) => TradedSide::Buy,
        (false, true) => TradedSide::Sell,
        (true, true) => TradedSide::Both,
        (false, false) => TradedSide::Neither,
    };
    let status = match side {
        TradedSide::Buy => optional_level(&wire.status_buy, "status_buy")?,
        TradedSide::Sell => optional_level(&wire.status_sell, "status_sell")?,
        TradedSide::Both | TradedSide::Neither => None,
    };
    Ok(PriceRow {
        terminal: TerminalId(wire.id_terminal.whole("id_terminal")?),
        commodity: CommodityId(wire.id_commodity.whole("id_commodity")?),
        side,
        price: match side {
            TradedSide::Buy => Some(buy_price),
            TradedSide::Sell => Some(sell_price),
            TradedSide::Both | TradedSide::Neither => None,
        },
        status,
        container_sizes: container_sizes(wire.container_sizes.as_deref().unwrap_or_default())?,
    })
}

fn level(value: &Numeric, field: &str) -> Result<u8, ReadError> {
    u8::try_from(value.whole(field)?)
        .ok()
        .filter(|level| *level > 0)
        .ok_or_else(|| malformed(&format!("{field} is not a status level")))
}

fn optional_level(value: &Numeric, field: &str) -> Result<Option<u8>, ReadError> {
    if value.whole(field)? == 0 {
        return Ok(None);
    }
    level(value, field).map(Some)
}

pub(crate) fn container_sizes(csv: &str) -> Result<Vec<ContainerSize>, ReadError> {
    if csv.trim().is_empty() {
        return Ok(Vec::new());
    }
    csv.split(CONTAINER_SIZE_SEPARATOR)
        .map(|entry| {
            entry
                .trim()
                .parse()
                .ok()
                .and_then(ContainerSize::from_scu)
                .ok_or_else(|| malformed("container_sizes holds a size outside the documented set"))
        })
        .collect()
}

pub(crate) fn unescaped(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('&') {
        let (before, from_ampersand) = rest.split_at(start);
        result.push_str(before);
        let entity = HTML_ENTITIES
            .iter()
            .find(|(escaped, _)| from_ampersand.starts_with(escaped));
        let (pushed, skipped) = match entity {
            Some((escaped, plain)) => (*plain, escaped.len()),
            None => ("&", 1),
        };
        result.push_str(pushed);
        rest = from_ampersand.get(skipped..).unwrap_or_default();
    }
    result.push_str(rest);
    result
}

pub(crate) fn parse<T: DeserializeOwned>(data: Value, resource: &str) -> Result<T, ReadError> {
    serde_json::from_value(data).map_err(|_| {
        malformed(&format!(
            "{resource} data does not match the documented shape"
        ))
    })
}

pub(crate) fn malformed(detail: &str) -> ReadError {
    ReadError::Malformed {
        detail: detail.to_owned(),
    }
}
