// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Status vocabulary code sets without labels or arbitrary scalar values.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{PublicContractEndpoint, PublicContractError};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StatusCodeSets {
    buy: BTreeSet<u8>,
    sell: BTreeSet<u8>,
}

pub(super) fn extract(
    endpoint: PublicContractEndpoint,
    value: &Value,
) -> Result<Option<StatusCodeSets>, PublicContractError> {
    match endpoint {
        PublicContractEndpoint::CommoditiesStatus => {
            let data = value
                .get("data")
                .ok_or(PublicContractError::StatusVocabulary)?;
            Ok(Some(StatusCodeSets {
                buy: side(data, "buy")?,
                sell: side(data, "sell")?,
            }))
        }
        PublicContractEndpoint::GameVersions
        | PublicContractEndpoint::DataParameters
        | PublicContractEndpoint::Commodities
        | PublicContractEndpoint::Terminals => Ok(None),
    }
}

fn side(data: &Value, name: &str) -> Result<BTreeSet<u8>, PublicContractError> {
    let rows = data
        .get(name)
        .and_then(Value::as_array)
        .filter(|rows| !rows.is_empty())
        .ok_or(PublicContractError::StatusVocabulary)?;
    let mut codes = BTreeSet::new();
    for row in rows {
        let code = row
            .get("code")
            .and_then(|value| match value {
                Value::String(text) => text.parse::<u8>().ok(),
                Value::Number(number) => number.as_u64().and_then(|code| u8::try_from(code).ok()),
                Value::Null | Value::Bool(_) | Value::Object(_) | Value::Array(_) => None,
            })
            .filter(|code| *code != 0)
            .ok_or(PublicContractError::StatusVocabulary)?;
        if !codes.insert(code) {
            return Err(PublicContractError::StatusVocabulary);
        }
    }
    Ok(codes)
}

pub(super) fn changes(
    observed: Option<&StatusCodeSets>,
    expected: Option<&StatusCodeSets>,
    endpoint: &str,
) -> Vec<String> {
    match (observed, expected) {
        (None, None) => Vec::new(),
        (Some(current), Some(previous)) => [
            ("buy", &current.buy, &previous.buy),
            ("sell", &current.sell, &previous.sell),
        ]
        .into_iter()
        .filter(|(_, current, previous)| current != previous)
        .map(|(side, current, previous)| {
            format!(
                "status codes changed: {endpoint}.data.{side}[].code ({previous:?} -> {current:?})"
            )
        })
        .collect(),
        (None, Some(_)) | (Some(_), None) => {
            vec![format!("status code availability changed: {endpoint}")]
        }
    }
}

pub(super) fn valid_metadata(
    endpoint: PublicContractEndpoint,
    codes: Option<&StatusCodeSets>,
) -> bool {
    match endpoint {
        PublicContractEndpoint::CommoditiesStatus => codes.is_some_and(|sets| {
            !sets.buy.is_empty()
                && !sets.sell.is_empty()
                && sets.buy.iter().chain(&sets.sell).all(|code| *code != 0)
        }),
        PublicContractEndpoint::GameVersions
        | PublicContractEndpoint::DataParameters
        | PublicContractEndpoint::Commodities
        | PublicContractEndpoint::Terminals => codes.is_none(),
    }
}
