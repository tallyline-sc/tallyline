// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Validated mutation commands and exhaustive outcome checking.

use std::collections::BTreeSet;
use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

use serde_json::Value;

/// A zero-based shard of the complete mutation selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Shard {
    index: usize,
    count: usize,
}

impl FromStr for Shard {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (index, count) = value.split_once('/').ok_or("shard must be k/n")?;
        let parse = |part: &str| {
            if part.is_empty()
                || !part.bytes().all(|byte| byte.is_ascii_digit())
                || (part.len() > 1 && part.starts_with('0'))
            {
                return Err("shard components must be canonical unsigned integers".to_owned());
            }
            part.parse::<usize>().map_err(|error| error.to_string())
        };
        let index = parse(index)?;
        let count = parse(count)?;
        if index >= count {
            return Err("shard index must be below its non-zero count".to_owned());
        }
        Ok(Self { index, count })
    }
}

impl fmt::Display for Shard {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}/{}", self.index, self.count)
    }
}

impl Shard {
    /// Returns the shard's distinct report directory component.
    #[must_use]
    pub fn report_directory(self) -> String {
        format!("shard-{}-of-{}", self.index, self.count)
    }
}

/// Mutation command options shared by listing and execution.
#[derive(Debug, Eq, PartialEq)]
pub struct Options {
    /// Optional unified diff restricting mutated source lines.
    pub diff: Option<PathBuf>,
    /// Optional partition of all selected mutants.
    pub shard: Option<Shard>,
}

impl Options {
    /// Parses non-repeated options in either order.
    ///
    /// # Errors
    /// Rejects missing values, unknown options and invalid shards.
    pub fn parse(arguments: &[&str]) -> Result<Self, String> {
        let mut options = Self {
            diff: None,
            shard: None,
        };
        let mut remaining = arguments;
        while let [flag, value, rest @ ..] = remaining {
            match *flag {
                "--in-diff" if options.diff.is_none() && !value.starts_with("--") => {
                    options.diff = Some(PathBuf::from(value))
                }
                "--shard" if options.shard.is_none() => options.shard = Some(value.parse()?),
                _ => return Err(format!("invalid or repeated mutation option: {flag}")),
            }
            remaining = rest;
        }
        if !remaining.is_empty() {
            return Err("mutation option is missing its value".to_owned());
        }
        Ok(options)
    }
}

/// Validates every listed mutant completed exactly once before scoring.
///
/// # Errors
/// Rejects missing, duplicate or foreign outcomes and any failed score gate.
pub fn selected_score(report: &str, selection: &str) -> Result<f64, String> {
    selected_counts(report, selection)?.score()
}

/// Validates exact completion without assigning a score to an unviable-only shard.
///
/// # Errors
/// Rejects missing, duplicate, foreign or failed outcomes.
pub fn selected_counts(
    report: &str,
    selection: &str,
) -> Result<crate::quality::MutationCounts, String> {
    let listed: Value = serde_json::from_str(selection).map_err(|error| error.to_string())?;
    let listed = listed
        .as_array()
        .ok_or("mutation selection is not an array")?;
    let names = |mutants: &[Value]| -> Result<BTreeSet<String>, String> {
        let mut names = BTreeSet::new();
        for mutant in mutants {
            let name = mutant
                .get("name")
                .and_then(Value::as_str)
                .filter(|name| !name.is_empty())
                .ok_or("mutant has no name")?;
            if !names.insert(name.to_owned()) {
                return Err("duplicate mutant identity".to_owned());
            }
        }
        Ok(names)
    };
    let expected = names(listed)?;
    let document: Value = serde_json::from_str(report).map_err(|error| error.to_string())?;
    let outcomes = document
        .get("outcomes")
        .and_then(Value::as_array)
        .ok_or("mutation report has no outcomes")?;
    let mutants: Vec<Value> = outcomes
        .iter()
        .filter_map(|outcome| outcome.pointer("/scenario/Mutant").cloned())
        .collect();
    if names(&mutants)? != expected {
        return Err("completed mutants disagree with the exact selection".to_owned());
    }
    crate::quality::completed_mutation(report)
}

/// Number of required CI shards.
pub const CI_SHARD_COUNT: usize = 32;

/// Identity shared by every artifact of one workflow attempt.
pub struct RunIdentity<'a> {
    /// Exact checked-out source commit.
    pub head: &'a str,
    /// Hosted workflow run identifier.
    pub run: &'a str,
    /// Hosted workflow attempt identifier.
    pub attempt: &'a str,
}

/// Validates all required shard artifacts and scores the complete selection.
///
/// # Errors
/// Rejects missing or foreign evidence, partition gaps, failed reports and invalid whole-selection scores.
pub fn aggregate(
    directory: &std::path::Path,
    identity: &RunIdentity<'_>,
) -> Result<Option<f64>, String> {
    let read = |path: &std::path::Path| {
        std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))
    };
    let mut full: Option<Vec<Value>> = None;
    let mut totals = crate::quality::MutationCounts::default();
    for index in 0..CI_SHARD_COUNT {
        let shard = directory.join(format!(
            "core-mutation-{}-{}-shard-{index}-of-{CI_SHARD_COUNT}",
            identity.run, identity.attempt
        ));
        let expected_identity = format!(
            "{}\n{}\n{}\n{index}/{CI_SHARD_COUNT}\n",
            identity.head, identity.run, identity.attempt
        );
        if read(&shard.join("identity.txt"))? != expected_identity {
            return Err(
                "mutation artifact belongs to a different head, run, attempt or shard".to_owned(),
            );
        }
        let all: Vec<Value> =
            serde_json::from_str(&read(&shard.join("all-selected-mutants.json"))?)
                .map_err(|error| error.to_string())?;
        match &full {
            Some(expected) if expected != &all => {
                return Err("shards disagree on the complete selection".to_owned());
            }
            Some(_) => {}
            None => full = Some(all.clone()),
        }
        let selection = read(&shard.join("selected-mutants.json"))?;
        let selected: Vec<Value> =
            serde_json::from_str(&selection).map_err(|error| error.to_string())?;
        let expected: Vec<Value> = all
            .into_iter()
            .enumerate()
            .filter_map(|(position, mutant)| (position % CI_SHARD_COUNT == index).then_some(mutant))
            .collect();
        if selected != expected {
            return Err(
                "shard selection disagrees with the complete round-robin partition".to_owned(),
            );
        }
        let report = shard.join("mutants.out/outcomes.json");
        if selected.is_empty() {
            if report.exists() {
                return Err("empty shard contains an unexpected report".to_owned());
            }
            continue;
        }
        let report_text = read(&report)?;
        let counts = selected_counts(&report_text, &selection)?;
        crate::mutation_evidence::verify_cause_logs(&shard.join("mutants.out"), &report_text)?;
        totals.caught = totals
            .caught
            .checked_add(counts.caught)
            .ok_or("caught counts overflow")?;
        totals.missed = totals
            .missed
            .checked_add(counts.missed)
            .ok_or("missed counts overflow")?;
        totals.unviable = totals
            .unviable
            .checked_add(counts.unviable)
            .ok_or("unviable counts overflow")?;
    }
    let full = full.ok_or("missing complete mutation selection")?;
    let mut names = BTreeSet::new();
    for mutant in &full {
        let name = mutant
            .get("name")
            .and_then(Value::as_str)
            .ok_or("mutant has no name")?;
        if !names.insert(name) {
            return Err("complete selection contains a duplicate mutant".to_owned());
        }
    }
    if full.is_empty() {
        return Ok(None);
    }
    totals.score().map(Some)
}
