// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Success-envelope checks without reflected response strings.

use serde_json::Value;

use super::{PublicContractCode, PublicContractEndpoint, PublicContractError};

pub(super) fn validate(value: &Value, http_status: u16) -> Result<(), PublicContractError> {
    let object = value
        .as_object()
        .ok_or(PublicContractError::EnvelopeStatus)?;
    let status = object
        .get("status")
        .and_then(Value::as_str)
        .ok_or(PublicContractError::EnvelopeStatus)?;
    if status != "ok" {
        let code = PublicContractCode::from_identifier(status)
            .ok_or(PublicContractError::EnvelopeStatus)?;
        return Err(PublicContractError::Rejected { http_status, code });
    }
    if !(200..300).contains(&http_status) {
        return Err(PublicContractError::HttpStatus { code: http_status });
    }
    let code = object.get("http_code").and_then(|value| match value {
        Value::String(text) => text.parse::<u16>().ok(),
        Value::Number(number) => number.as_u64().and_then(|value| u16::try_from(value).ok()),
        Value::Null | Value::Bool(_) | Value::Object(_) | Value::Array(_) => None,
    });
    if code != Some(http_status) {
        return Err(PublicContractError::EnvelopeCode);
    }
    if !object.contains_key("data") {
        return Err(PublicContractError::MissingData);
    }
    Ok(())
}

pub(super) fn validate_data(
    value: &Value,
    endpoint: PublicContractEndpoint,
) -> Result<(), PublicContractError> {
    let observed = value.get("data").is_some_and(|data| match endpoint {
        PublicContractEndpoint::Commodities | PublicContractEndpoint::Terminals => {
            data.as_array().is_some_and(|rows| {
                !rows.is_empty()
                    && rows
                        .iter()
                        .all(|row| row.as_object().is_some_and(|object| !object.is_empty()))
            })
        }
        PublicContractEndpoint::GameVersions
        | PublicContractEndpoint::CommoditiesStatus
        | PublicContractEndpoint::DataParameters => {
            data.as_object().is_some_and(|object| !object.is_empty())
        }
    });
    if !observed {
        return Err(PublicContractError::UnobservableData);
    }
    Ok(())
}
