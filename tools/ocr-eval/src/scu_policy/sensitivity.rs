// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Finite digit perturbations, distinct from observed recognition errors.

use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

type Families = BTreeMap<&'static str, BTreeSet<u128>>;

pub(super) fn candidates(quantities: &BTreeMap<u64, usize>) -> Result<Vec<Value>, String> {
    let cases = quantities
        .iter()
        .map(|(quantity, weight)| Ok((u128::from(*quantity), *weight, variants(*quantity)?)))
        .collect::<Result<Vec<_>, String>>()?;
    let mut candidates = Vec::new();
    for percent in [10_u32, 19, 20, 25, 50] {
        for minimum in [0_u64, 8, 15, 16, 25, 32] {
            let mut row = BTreeMap::<&str, (usize, usize)>::new();
            let mut distinct = BTreeMap::<&str, (usize, usize)>::new();
            for (prior, weight, families) in &cases {
                for (name, values) in families {
                    let mut major = 0;
                    for value in values {
                        let difference = prior.abs_diff(*value);
                        if difference > u128::from(minimum)
                            && difference
                                .checked_mul(100)
                                .ok_or("research percentage exceeds exact arithmetic")?
                                > prior
                                    .checked_mul(u128::from(percent))
                                    .ok_or("research allowance exceeds exact arithmetic")?
                        {
                            major += 1;
                        }
                    }
                    let weighted = row.entry(name).or_default();
                    weighted.0 += values.len() * weight;
                    weighted.1 += major * weight;
                    let unweighted = distinct.entry(name).or_default();
                    unweighted.0 += values.len();
                    unweighted.1 += major;
                }
            }
            candidates.push(json!({"percent": percent, "minimum_scu": minimum, "row_weighted": counts(&row), "distinct_quantity": counts(&distinct)}));
        }
    }
    Ok(candidates)
}
fn counts(values: &BTreeMap<&str, (usize, usize)>) -> Value {
    Value::Object(
        values
            .iter()
            .map(|(name, (cases, major))| {
                ((*name).to_owned(), json!({"cases": cases, "major": major}))
            })
            .collect(),
    )
}
fn variants(quantity: u64) -> Result<Families, String> {
    let digits = quantity.to_string().chars().collect::<Vec<_>>();
    let mut deletion = BTreeSet::new();
    let mut insertion = BTreeSet::new();
    let mut transposition = BTreeSet::new();
    for position in 0..digits.len() {
        let value = digits
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != position)
            .map(|(_, digit)| *digit)
            .collect::<String>();
        if !value.is_empty() {
            deletion.insert(number(&value)?);
        }
    }
    for position in 0..=digits.len() {
        for digit in '0'..='9' {
            let mut value = digits.clone();
            value.insert(position, digit);
            insertion.insert(number(&value.into_iter().collect::<String>())?);
        }
    }
    for (position, pair) in digits.windows(2).enumerate() {
        let [left, right] = pair else {
            return Err("research digit pair is incomplete".to_owned());
        };
        if left != right {
            let value = digits
                .iter()
                .enumerate()
                .map(|(index, digit)| {
                    if index == position {
                        *right
                    } else if index == position + 1 {
                        *left
                    } else {
                        *digit
                    }
                })
                .collect::<String>();
            transposition.insert(number(&value)?);
        }
    }
    let mut families = BTreeMap::from([
        ("delete_one_digit", deletion),
        ("insert_one_digit", insertion),
        ("transpose_adjacent", transposition),
    ]);
    for values in families.values_mut() {
        values.remove(&u128::from(quantity));
    }
    Ok(families)
}
fn number(value: &str) -> Result<u128, String> {
    value
        .parse()
        .map_err(|_| "research digit edit exceeds exact numeric range".to_owned())
}
