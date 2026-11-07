// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact canonical values and explicit retained reading outcomes.

use super::ReplayError;
use serde_json::Value;

const MAX_LITERAL_BYTES: usize = 4096;
const MAX_DECIMAL_BYTES: usize = 128;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Field {
    Price,
    Scu,
    Status,
}

impl Field {
    pub fn name(self) -> &'static str {
        match self {
            Self::Price => "pricePerScu",
            Self::Scu => "scu",
            Self::Status => "status",
        }
    }

    pub(super) fn parse(value: &str) -> Result<Self, ReplayError> {
        match value {
            "pricePerScu" => Ok(Self::Price),
            "scu" => Ok(Self::Scu),
            "status" => Ok(Self::Status),
            _ => Err(ReplayError::UnsupportedField),
        }
    }

    pub(super) fn canonical(self, value: &Value) -> Result<String, ReplayError> {
        match self {
            Self::Price | Self::Scu => match value {
                Value::String(text) => decimal(text),
                Value::Number(number) => number
                    .as_u64()
                    .map(|number| number.to_string())
                    .ok_or(ReplayError::InvalidDecimal),
                Value::Null | Value::Bool(_) | Value::Array(_) | Value::Object(_) => {
                    Err(ReplayError::InvalidDecimal)
                }
            },
            Self::Status => {
                let text = value.as_str().ok_or(ReplayError::InvalidStatus)?;
                if text.is_empty() || text.len() > MAX_LITERAL_BYTES {
                    return Err(ReplayError::InvalidStatus);
                }
                Ok(text
                    .split_ascii_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .to_ascii_uppercase())
            }
        }
    }
}

pub(super) enum Observation {
    Value(String),
    Invalid(String),
    Missing,
    Unscorable,
}

impl Observation {
    pub fn value(&self) -> Option<&str> {
        match self {
            Self::Value(value) => Some(value),
            Self::Invalid(_) | Self::Missing | Self::Unscorable => None,
        }
    }

    pub(super) fn read(
        value: &Value,
        field: Field,
        reference: Option<&str>,
    ) -> Result<Self, ReplayError> {
        let literal = optional_literal(value, "literal")?;
        let normalised = optional_literal(value, "normalised_literal")?;
        match text(value, "outcome")? {
            "value" => {
                if literal.is_none_or(str::is_empty) || normalised.is_none_or(str::is_empty) {
                    return Err(ReplayError::InvalidObservation);
                }
                Ok(Self::Value(field.canonical(
                    value.get("value").ok_or(ReplayError::InvalidObservation)?,
                )?))
            }
            "invalid" => {
                if literal.is_none_or(str::is_empty) {
                    return Err(ReplayError::InvalidObservation);
                }
                Ok(Self::Invalid(
                    normalised
                        .filter(|value| !value.is_empty())
                        .ok_or(ReplayError::InvalidObservation)?
                        .to_owned(),
                ))
            }
            "missing" => {
                if literal.is_some_and(|value| !value.is_empty())
                    || normalised.is_some_and(|value| !value.is_empty())
                {
                    return Err(ReplayError::InvalidObservation);
                }
                Ok(Self::Missing)
            }
            "unscorable" => match text(value, "reason")? {
                "alignment" => Ok(Self::Unscorable),
                "reference_unknown" if reference.is_none() => Ok(Self::Unscorable),
                _ => Err(ReplayError::InvalidUnscorable),
            },
            _ => Err(ReplayError::UnsupportedOutcome),
        }
    }
}

fn decimal(text: &str) -> Result<String, ReplayError> {
    if text.is_empty() || text.len() > MAX_DECIMAL_BYTES {
        return Err(ReplayError::InvalidDecimal);
    }
    let (whole, fraction) = text
        .split_once('.')
        .map_or((text, None), |(whole, fraction)| (whole, Some(fraction)));
    if whole.is_empty()
        || !whole.bytes().all(|value| value.is_ascii_digit())
        || fraction.is_some_and(|value| {
            value.is_empty() || !value.bytes().all(|value| value.is_ascii_digit())
        })
    {
        return Err(ReplayError::InvalidDecimal);
    }
    let whole = whole.trim_start_matches('0');
    let whole = if whole.is_empty() { "0" } else { whole };
    let fraction = fraction.unwrap_or("").trim_end_matches('0');
    Ok(if fraction.is_empty() {
        whole.to_owned()
    } else {
        format!("{whole}.{fraction}")
    })
}

fn optional_literal<'a>(value: &'a Value, name: &str) -> Result<Option<&'a str>, ReplayError> {
    match value.get(name) {
        Some(Value::Null) => Ok(None),
        Some(Value::String(text)) if text.len() <= MAX_LITERAL_BYTES => Ok(Some(text)),
        _ => Err(ReplayError::InvalidLiteral),
    }
}

fn text<'a>(value: &'a Value, name: &'static str) -> Result<&'a str, ReplayError> {
    value
        .get(name)
        .and_then(Value::as_str)
        .ok_or(ReplayError::TextRequired(name))
}
