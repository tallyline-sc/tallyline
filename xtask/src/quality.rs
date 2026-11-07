// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Core package selection for coverage and mutation gates.

use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// The non-decreasing score floor established by the full core measurement.
pub const MUTATION_SCORE_FLOOR: f64 = 100.0;

/// Selects the documented core crates from locked Cargo metadata.
///
/// # Errors
/// Returns an error for malformed metadata or ambiguous package identities.
pub fn core_packages(metadata: &str) -> Result<Vec<String>, String> {
    let document: Value = serde_json::from_str(metadata).map_err(|error| error.to_string())?;
    let packages = document
        .get("packages")
        .unwrap_or(&Value::Null)
        .as_array()
        .ok_or("metadata has no packages array")?;
    let members = document
        .get("workspace_members")
        .unwrap_or(&Value::Null)
        .as_array()
        .ok_or("metadata has no workspace_members array")?;
    let mut identities = BTreeMap::new();
    for package in packages {
        let id = package
            .get("id")
            .unwrap_or(&Value::Null)
            .as_str()
            .ok_or("package has no string id")?;
        let name = package
            .get("name")
            .unwrap_or(&Value::Null)
            .as_str()
            .ok_or("package has no string name")?;
        if identities.insert(id, name).is_some() {
            return Err(format!("duplicate package id {id}"));
        }
    }
    let mut selected = BTreeSet::new();
    let mut seen = BTreeSet::new();
    for member in members {
        let id = member.as_str().ok_or("workspace member is not a string")?;
        if !seen.insert(id) {
            return Err(format!("duplicate workspace member {id}"));
        }
        let name = identities
            .get(id)
            .ok_or_else(|| format!("workspace member {id} has no package"))?;
        if crate::architecture::is_core(name) && !selected.insert((*name).to_owned()) {
            return Err(format!("duplicate core package name {name}"));
        }
    }
    Ok(selected.into_iter().collect())
}

/// Completed mutation counts after validating baseline, totals and outcomes.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MutationCounts {
    /// Mutants rejected by workspace tests.
    pub caught: u64,
    /// Mutants surviving workspace tests.
    pub missed: u64,
    /// Mutants that could not build.
    pub unviable: u64,
}

impl MutationCounts {
    /// Enforces the frozen whole-selection score and requires viable mutants.
    ///
    /// # Errors
    /// Rejects overflow, all-unviable measurements and below-floor scores.
    pub fn score(self) -> Result<f64, String> {
        let viable = self
            .caught
            .checked_add(self.missed)
            .ok_or("mutation counts overflow")?;
        if viable == 0 {
            return Err("mutation run has no viable mutants; no score measured".to_owned());
        }
        let score = self.caught as f64 / viable as f64 * 100.0;
        if score < MUTATION_SCORE_FLOOR {
            return Err(format!(
                "mutation score {score:.2}% is below {MUTATION_SCORE_FLOOR:.0}%"
            ));
        }
        Ok(score)
    }
}

/// Validates a completed pinned-tool mutation report and enforces the frozen score floor.
///
/// # Errors
/// Returns an error for malformed, incomplete, failed or below-floor results.
pub fn mutation_score(report: &str) -> Result<f64, String> {
    completed_mutation(report)?.score()
}

/// Validates completed counts without treating unviable mutants as a score.
///
/// # Errors
/// Rejects malformed, incomplete, timed-out or failed reports.
pub fn completed_mutation(report: &str) -> Result<MutationCounts, String> {
    let document: Value = serde_json::from_str(report).map_err(|error| error.to_string())?;
    if document
        .get("cargo_mutants_version")
        .unwrap_or(&Value::Null)
        != "27.1.0"
        || !document.get("end_time").unwrap_or(&Value::Null).is_string()
    {
        return Err("mutation report is incomplete or from an unpinned tool".to_owned());
    }
    let count = |key: &str| {
        document
            .get(key)
            .unwrap_or(&Value::Null)
            .as_u64()
            .ok_or_else(|| format!("missing integer {key}"))
    };
    let caught = count("caught")?;
    let missed = count("missed")?;
    let unviable = count("unviable")?;
    if count("timeout")? != 0 || count("success")? != 0 {
        return Err("mutation run contains timeouts or unexpected successes".to_owned());
    }
    let total = caught
        .checked_add(missed)
        .and_then(|n| n.checked_add(unviable))
        .ok_or("mutation counts overflow")?;
    if count("total_mutants")? != total {
        return Err("mutation counts disagree".to_owned());
    }
    let outcomes = document
        .get("outcomes")
        .unwrap_or(&Value::Null)
        .as_array()
        .ok_or("mutation report has no outcomes")?;
    let mut actual = BTreeMap::<&str, u64>::new();
    let mut baseline = 0;
    for outcome in outcomes {
        let summary = outcome
            .get("summary")
            .unwrap_or(&Value::Null)
            .as_str()
            .ok_or("outcome has no summary")?;
        if outcome.get("scenario").unwrap_or(&Value::Null) == "Baseline" {
            if summary != "Success" {
                return Err("mutation baseline failed".to_owned());
            }
            baseline += 1;
        } else if outcome
            .pointer("/scenario/Mutant")
            .is_some_and(Value::is_object)
            && matches!(summary, "CaughtMutant" | "MissedMutant" | "Unviable")
        {
            *actual.entry(summary).or_default() += 1;
        } else {
            return Err("mutation outcome is invalid".to_owned());
        }
    }
    if baseline != 1
        || actual.get("CaughtMutant").copied().unwrap_or(0) != caught
        || actual.get("MissedMutant").copied().unwrap_or(0) != missed
        || actual.get("Unviable").copied().unwrap_or(0) != unviable
    {
        return Err("mutation outcomes disagree with totals".to_owned());
    }
    Ok(MutationCounts {
        caught,
        missed,
        unviable,
    })
}
