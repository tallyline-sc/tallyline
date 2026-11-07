// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared structural parsing of field-assigned numeric text.

use crate::raw_transcription::RawField;
use tallyline_shared_kernel::api::model::{PricePerScu, ScuQuantity};

/// The active layout profile's price precision policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecimalPrices {
    /// Prices without magnitude suffixes must be whole aUEC.
    WholeOnly,
    /// The layout profile permits fractional aUEC prices.
    Allowed,
}

/// Numeric candidates without scoring, validation or confidence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NumericReading<T> {
    /// Literal and uncertain substituted candidates remain distinguishable.
    Candidates {
        /// Exact digit reading, if the text contains only digits and separators.
        raw: Option<T>,
        /// Reading after replacing the documented letter-digit confusables.
        substituted: Option<T>,
        /// Zero-based character positions in the original field that were replaced.
        confusable_positions: Vec<usize>,
    },
    /// The text cannot establish a supported numeric candidate.
    Unreadable,
    /// A decimal-looking price conflicts with the active whole-price profile.
    Inconsistent,
}

/// Parses unit-free quantity text through the shared structural number parser.
#[must_use]
pub fn parse_quantity_text(text: &RawField<String>) -> NumericReading<ScuQuantity> {
    parse(text, DecimalPrices::WholeOnly, QuantityKind::Scu)
}

/// Parses glyph-free and unit-free price text through the shared number parser.
#[must_use]
pub fn parse_price_text(
    text: &RawField<String>,
    decimals: DecimalPrices,
) -> NumericReading<PricePerScu> {
    parse(text, decimals, QuantityKind::Price)
}

#[derive(Clone, Copy)]
enum QuantityKind {
    Price,
    Scu,
}

fn parse<T: std::str::FromStr>(
    text: &RawField<String>,
    decimals: DecimalPrices,
    kind: QuantityKind,
) -> NumericReading<T> {
    let RawField::Read(text) = text else {
        return NumericReading::Unreadable;
    };
    let Some(NormalisedNumber {
        spelling,
        positions,
        decimal,
        magnitude,
    }) = normalise(text)
    else {
        return NumericReading::Unreadable;
    };
    if decimal {
        match kind {
            QuantityKind::Scu => return NumericReading::Unreadable,
            QuantityKind::Price if decimals == DecimalPrices::WholeOnly && magnitude == 0 => {
                return NumericReading::Inconsistent;
            }
            QuantityKind::Price => (),
        }
    }
    let Ok(value) = spelling.parse::<T>() else {
        return NumericReading::Unreadable;
    };
    if positions.is_empty() {
        NumericReading::Candidates {
            raw: Some(value),
            substituted: None,
            confusable_positions: positions,
        }
    } else {
        NumericReading::Candidates {
            raw: None,
            substituted: Some(value),
            confusable_positions: positions,
        }
    }
}

struct NormalisedNumber {
    spelling: String,
    positions: Vec<usize>,
    decimal: bool,
    magnitude: u8,
}

fn normalise(text: &str) -> Option<NormalisedNumber> {
    const MAX_NUMBER_BYTES: usize = 128;
    if text.is_empty() || text.len() > MAX_NUMBER_BYTES {
        return None;
    }
    let (body, magnitude) = if let Some(body) = text.strip_suffix('K') {
        (body, 3)
    } else if let Some(body) = text.strip_suffix('M') {
        (body, 6)
    } else {
        (text, 0)
    };
    let mut mapped = String::with_capacity(body.len());
    let mut positions = Vec::new();
    for (position, character) in body.chars().enumerate() {
        let digit = match character {
            'B' => '8',
            'O' | 'D' | 'Q' => '0',
            'S' => '5',
            'I' | 'l' | '|' => '1',
            'Z' => '2',
            'G' => '6',
            '0'..='9' | ',' | '.' | ' ' => character,
            _ => return None,
        };
        if digit != character {
            positions.push(position);
        }
        mapped.push(digit);
    }
    let groups: Vec<_> = mapped.split([',', '.', ' ']).collect();
    let first = groups.first()?;
    if first.is_empty() || !first.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let mut normalised = (*first).to_owned();
    let mut decimal = false;
    if first.len() > 3 && (groups.len() > 2 || groups.get(1).is_some_and(|group| group.len() == 3))
    {
        return None;
    }
    for (index, group) in groups.iter().enumerate().skip(1) {
        if group.is_empty() || !group.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        match group.len() {
            3 => normalised.push_str(group),
            1 | 2 if index + 1 == groups.len() => {
                decimal = true;
                normalised.push('.');
                normalised.push_str(group);
            }
            _ => return None,
        }
    }
    if magnitude > 0 {
        normalised.push_str(&format!("e{magnitude}"));
    }
    Some(NormalisedNumber {
        spelling: normalised,
        positions,
        decimal,
        magnitude,
    })
}
