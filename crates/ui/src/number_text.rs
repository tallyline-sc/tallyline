// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact English numeric presentation without binary arithmetic.

use tallyline_shared_kernel::api::model::{Delta, PricePerScu, ScuQuantity};

pub(crate) fn price(value: PricePerScu) -> String {
    grouped(&value.to_string())
}

pub(crate) fn quantity(value: ScuQuantity) -> String {
    grouped(&value.to_string())
}

pub(crate) fn difference(value: Delta) -> String {
    let text = grouped(&value.to_string());
    if value.is_zero() || text.starts_with('\u{2212}') {
        text
    } else {
        format!("+{text}")
    }
}

pub(crate) fn percentage(value: &str) -> String {
    format!("{}%", grouped(value))
}

fn grouped(value: &str) -> String {
    let (sign, digits) = if let Some(digits) = value.strip_prefix('-') {
        ("\u{2212}", digits)
    } else if let Some(digits) = value.strip_prefix('+') {
        ("+", digits)
    } else {
        ("", value)
    };
    let (integer, fraction) = digits
        .split_once('.')
        .map_or((digits, None), |(integer, fraction)| {
            (integer, Some(fraction))
        });
    let mut text = String::from(sign);
    for (index, digit) in integer.chars().enumerate() {
        if index != 0 && (integer.len() - index).is_multiple_of(3) {
            text.push(',');
        }
        text.push(digit);
    }
    if let Some(fraction) = fraction {
        text.push('.');
        text.push_str(fraction);
    }
    text
}
