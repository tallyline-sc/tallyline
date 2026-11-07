// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Denominator-preserving single-reader and unanimous-agreement measurements.

use super::ReplayError;
use super::dataset::{Dataset, Row};
use super::observation::Observation;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Correct,
    ParsedWrong,
    Invalid,
    Missing,
    Unscorable,
}

fn outcome(observation: &Observation, expected: Option<&str>) -> Outcome {
    match observation {
        Observation::Missing => Outcome::Missing,
        Observation::Unscorable => Outcome::Unscorable,
        Observation::Value(value) => match expected {
            Some(expected) if value == expected => Outcome::Correct,
            Some(_) => Outcome::ParsedWrong,
            None => Outcome::Unscorable,
        },
        Observation::Invalid(_) => match expected {
            Some(_) => Outcome::Invalid,
            None => Outcome::Unscorable,
        },
    }
}

pub(super) fn report(data: &Dataset) -> Result<Value, ReplayError> {
    let rows = data.rows.iter().collect::<Vec<_>>();
    let mut fields = BTreeMap::<String, Vec<&Row>>::new();
    let mut captures = BTreeMap::<String, Vec<&Row>>::new();
    for row in &rows {
        fields
            .entry(row.field.name().to_owned())
            .or_default()
            .push(row);
        captures.entry(row.capture.clone()).or_default().push(row);
    }
    let mut singles = Vec::new();
    let mut pairs = Vec::new();
    let mut triples = Vec::new();
    for (first, reader) in data.readers.iter().enumerate() {
        singles.push(json!({"reader":reader,"total":single(&rows,first)?,"by_field":grouped_single(&fields,first)?,"by_capture":grouped_single(&captures,first)?}));
        for (second, other) in data.readers.iter().enumerate().skip(first + 1) {
            pairs.push(combination_record(
                &rows,
                &fields,
                &captures,
                &[first, second],
                &[reader, other],
            )?);
            for (third, last) in data.readers.iter().enumerate().skip(second + 1) {
                triples.push(combination_record(
                    &rows,
                    &fields,
                    &captures,
                    &[first, second, third],
                    &[reader, other, last],
                )?);
            }
        }
    }
    Ok(
        json!({"schema_version":1,"evidence":"post-hoc development reassessment; no held-out accuracy or product acceptance","inference":"not_run","packet_sha256":data.sha256,"references":data.references,"fields":rows.len(),"cards":rows.iter().map(|row| &row.card).collect::<BTreeSet<_>>().len(),"captures":captures.len(),"singles":singles,"pairs":pairs,"triples":triples}),
    )
}

fn single(rows: &[&Row], index: usize) -> Result<Value, ReplayError> {
    let mut correct = 0;
    let mut parsed_wrong = 0;
    let mut invalid = 0;
    let mut missing = 0;
    let mut unscorable = 0;
    for row in rows {
        match outcome(
            row.observations
                .get(index)
                .ok_or(ReplayError::InvalidReaderSelection)?,
            row.expected.as_deref(),
        ) {
            Outcome::Correct => correct += 1,
            Outcome::ParsedWrong => parsed_wrong += 1,
            Outcome::Invalid => invalid += 1,
            Outcome::Missing => missing += 1,
            Outcome::Unscorable => unscorable += 1,
        }
    }
    Ok(
        json!({"denominator":rows.len(),"correct":correct,"wrong":parsed_wrong+invalid,"parsed_wrong":parsed_wrong,"invalid":invalid,"missing":missing,"unscorable":unscorable}),
    )
}

fn grouped_single(
    groups: &BTreeMap<String, Vec<&Row>>,
    index: usize,
) -> Result<Value, ReplayError> {
    let mut result = Map::new();
    for (name, rows) in groups {
        result.insert(name.clone(), single(rows, index)?);
    }
    Ok(Value::Object(result))
}

fn combination_record(
    rows: &[&Row],
    fields: &BTreeMap<String, Vec<&Row>>,
    captures: &BTreeMap<String, Vec<&Row>>,
    indices: &[usize],
    readers: &[&String],
) -> Result<Value, ReplayError> {
    let mut by_field = Map::new();
    let mut by_capture = Map::new();
    for (name, rows) in fields {
        by_field.insert(name.clone(), combination(rows, indices)?);
    }
    for (name, rows) in captures {
        by_capture.insert(name.clone(), combination(rows, indices)?);
    }
    Ok(
        json!({"readers":readers,"total":combination(rows,indices)?,"by_field":by_field,"by_capture":by_capture}),
    )
}

fn combination(rows: &[&Row], indices: &[usize]) -> Result<Value, ReplayError> {
    let mut all_correct = 0;
    let mut only_reader_correct = vec![0; indices.len()];
    let mut none_correct = 0;
    let mut some_correct = 0;
    let mut same_wrong_value = 0;
    let mut same_invalid_literal = 0;
    let mut different_values = 0;
    let mut any_missing = 0;
    let mut any_invalid = 0;
    let mut any_unscorable = 0;
    let mut baseline_wrong_detected = 0;
    let mut baseline_correct_flagged = 0;
    for row in rows {
        let observations = indices
            .iter()
            .map(|index| {
                row.observations
                    .get(*index)
                    .ok_or(ReplayError::InvalidReaderSelection)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let outcomes = observations
            .iter()
            .map(|observation| outcome(observation, row.expected.as_deref()))
            .collect::<Vec<_>>();
        let correct = outcomes
            .iter()
            .filter(|value| **value == Outcome::Correct)
            .count();
        if correct == indices.len() {
            all_correct += 1;
        } else if correct == 0 {
            none_correct += 1;
        } else {
            some_correct += 1;
        }
        if correct == 1 {
            for (index, result) in outcomes.iter().enumerate() {
                if *result == Outcome::Correct {
                    *only_reader_correct
                        .get_mut(index)
                        .ok_or(ReplayError::InvalidReaderSelection)? += 1;
                }
            }
        }
        let first = observations
            .first()
            .ok_or(ReplayError::InvalidReaderSelection)?;
        let same = first.value().is_some_and(|value| {
            observations
                .iter()
                .all(|observation| observation.value() == Some(value))
        });
        let accepted = same && row.expected.is_some();
        if accepted && correct == 0 {
            same_wrong_value += 1;
        }
        if observations
            .iter()
            .all(|observation| observation.value().is_some())
            && !same
        {
            different_values += 1;
        }
        same_invalid_literal += usize::from(match first {
            Observation::Invalid(literal) => observations.iter().all(
                |observation| matches!(observation,Observation::Invalid(other) if other==literal),
            ),
            Observation::Value(_) | Observation::Missing | Observation::Unscorable => false,
        });
        any_missing += usize::from(outcomes.contains(&Outcome::Missing));
        any_invalid += usize::from(outcomes.contains(&Outcome::Invalid));
        any_unscorable += usize::from(outcomes.contains(&Outcome::Unscorable));
        let baseline = outcomes
            .first()
            .ok_or(ReplayError::InvalidReaderSelection)?;
        if !accepted && matches!(baseline, Outcome::ParsedWrong | Outcome::Invalid) {
            baseline_wrong_detected += 1;
        }
        if !accepted && *baseline == Outcome::Correct {
            baseline_correct_flagged += 1;
        }
    }
    Ok(
        json!({"denominator":rows.len(),"all_correct":all_correct,"only_reader_correct":only_reader_correct,"some_correct":some_correct,"none_correct":none_correct,"same_wrong_value":same_wrong_value,"same_invalid_literal":same_invalid_literal,"different_values":different_values,"any_missing":any_missing,"any_invalid":any_invalid,"any_unscorable":any_unscorable,"accepted_correct":all_correct,"accepted_wrong":same_wrong_value,"abstained":rows.len()-all_correct-same_wrong_value,"baseline_wrong_detected":baseline_wrong_detected,"baseline_correct_flagged":baseline_correct_flagged}),
    )
}
