// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Strict parsing of the versioned Markdown transcription format.

use std::collections::BTreeSet;
use std::fmt;
use tallyline_recognition::api::model::{RawCard, RawField, RawTranscription};
use tallyline_shared_kernel::api::model::TradeSide;

const MAX_ANSWER_BYTES: usize = 262_144;
const MAX_FIELD_BYTES: usize = 512;
const MAX_CARDS: usize = 128;
const COLUMNS: [&str; 5] = ["name", "status", "scu", "price_per_scu", "cargo_sizes"];

/// An answer that cannot establish a complete supported transcription.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseError {
    /// The answer, field or card count exceeds its bound.
    LimitExceeded,
    /// A header, column, delimiter or row is malformed or unsupported.
    InvalidFormat,
    /// A commodity name is repeated and cannot identify one row.
    DuplicateCard,
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::LimitExceeded => "transcription exceeds its bounds",
            Self::InvalidFormat => "transcription has an unsupported format",
            Self::DuplicateCard => "transcription repeats a commodity name",
        })
    }
}
impl std::error::Error for ParseError {}

/// Parses complete version-one header lines and a five-column Markdown table.
///
/// # Errors
/// Returns an error for malformed, duplicate or oversized input without partial rows.
pub fn parse_answer(answer: &str) -> Result<RawTranscription, ParseError> {
    if answer.len() > MAX_ANSWER_BYTES {
        return Err(ParseError::LimitExceeded);
    }
    let mut lines = answer
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty());
    let tab = lines
        .next()
        .and_then(|line| line.strip_prefix("TAB: "))
        .ok_or(ParseError::InvalidFormat)?;
    let side = match tab {
        "Buy" => RawField::Read(TradeSide::Buy),
        "Sell" => RawField::Read(TradeSide::Sell),
        "?" => RawField::Unreadable,
        _ => return Err(ParseError::InvalidFormat),
    };
    let location = field(
        lines
            .next()
            .and_then(|line| line.strip_prefix("LOCATION: "))
            .ok_or(ParseError::InvalidFormat)?,
    )?;
    let header = cells(lines.next().ok_or(ParseError::InvalidFormat)?)?;
    if header != COLUMNS {
        return Err(ParseError::InvalidFormat);
    }
    let divider = cells(lines.next().ok_or(ParseError::InvalidFormat)?)?;
    if divider
        .iter()
        .any(|cell| cell.len() < 3 || !cell.bytes().all(|byte| byte == b'-'))
    {
        return Err(ParseError::InvalidFormat);
    }
    let mut cards = Vec::new();
    let mut names = BTreeSet::new();
    for line in lines {
        if cards.len() == MAX_CARDS {
            return Err(ParseError::LimitExceeded);
        }
        let [name, status, quantity, price, cargo_sizes] = cells(line)?;
        let name = field(name)?;
        if let RawField::Read(text) = &name {
            let key = text
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .to_lowercase();
            if !names.insert(key) {
                return Err(ParseError::DuplicateCard);
            }
        }
        cards.push(RawCard {
            name,
            status: field(status)?,
            quantity_text: field(quantity)?,
            price_text: field(price)?,
            cargo_sizes: field(cargo_sizes)?,
        });
    }
    Ok(RawTranscription {
        side,
        location,
        cards,
    })
}

fn cells(line: &str) -> Result<[&str; 5], ParseError> {
    let inner = line
        .strip_prefix('|')
        .and_then(|text| text.strip_suffix('|'))
        .ok_or(ParseError::InvalidFormat)?;
    let mut parts = inner.split('|').map(str::trim);
    let mut columns = [""; 5];
    for column in &mut columns {
        *column = parts.next().ok_or(ParseError::InvalidFormat)?;
    }
    if parts.next().is_some() {
        return Err(ParseError::InvalidFormat);
    }
    Ok(columns)
}

fn field(text: &str) -> Result<RawField<String>, ParseError> {
    if text.len() > MAX_FIELD_BYTES {
        return Err(ParseError::LimitExceeded);
    }
    if text.is_empty() || text.chars().any(char::is_control) {
        return Err(ParseError::InvalidFormat);
    }
    Ok(if text.contains('?') || matches!(text, "…" | "...") {
        RawField::Unreadable
    } else {
        RawField::Read(text.to_owned())
    })
}
