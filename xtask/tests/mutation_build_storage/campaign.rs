// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Genuine CLI campaign outcomes and constructed aggregate partition contracts.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use super::Fixture;

pub(super) fn json_file(path: &Path) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

pub(super) fn aggregate_command(fixture: &Fixture, artifacts: &Path) -> Command {
    let mut command = fixture.cargo();
    command.args(["xtask", "mutation-aggregate"]);
    command.arg(artifacts).args(["fixture-head", "42", "1"]);
    command
}

pub(super) fn assert_rejected(output: &Output, label: &str) {
    assert!(
        !output.status.success(),
        "aggregate accepted {label}:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

pub(super) fn reject_changed_evidence(
    fixture: &Fixture,
    artifacts: &Path,
    path: &Path,
    replacement: &[u8],
    label: &str,
) {
    let original = fs::read(path).unwrap();
    fs::write(path, replacement).unwrap();
    let output = aggregate_command(fixture, artifacts).output();
    fs::write(path, original).unwrap();
    assert_rejected(&output.unwrap(), label);
}

pub(super) fn reject_missing_evidence(fixture: &Fixture, artifacts: &Path, path: &Path) {
    let retained = path.with_extension("retained");
    fs::rename(path, &retained).unwrap();
    let output = aggregate_command(fixture, artifacts).output();
    fs::rename(retained, path).unwrap();
    assert_rejected(&output.unwrap(), path.to_str().unwrap());
}

fn write_partition_fixture(
    artifacts: &Path,
    campaign: &Path,
    all: &serde_json::Value,
    report: &serde_json::Value,
    index: usize,
) {
    let directory = artifacts.join(format!("core-mutation-42-1-shard-{index}-of-32"));
    fs::create_dir_all(directory.join("mutants.out/log")).unwrap();
    let selected: Vec<_> = all
        .as_array()
        .unwrap()
        .iter()
        .skip(index)
        .step_by(32)
        .cloned()
        .collect();
    let original = report["outcomes"].as_array().unwrap();
    let baseline = original
        .iter()
        .find(|outcome| outcome["scenario"] == "Baseline")
        .unwrap();
    let mutant = original
        .iter()
        .find(|outcome| outcome["summary"] == "CaughtMutant")
        .unwrap();
    let mut outcomes = vec![baseline.clone()];
    for selected in &selected {
        let mut outcome = mutant.clone();
        outcome["scenario"]["Mutant"] = selected.clone();
        outcome["log_path"] = "log/cause.log".into();
        outcomes.push(outcome);
    }
    fs::copy(
        campaign
            .join("mutants.out")
            .join(mutant["log_path"].as_str().unwrap()),
        directory.join("mutants.out/log/cause.log"),
    )
    .unwrap();
    let mut partition = report.clone();
    partition["total_mutants"] = selected.len().into();
    partition["caught"] = selected.len().into();
    partition["unviable"] = 0.into();
    partition["outcomes"] = outcomes.into();
    fs::write(
        directory.join("identity.txt"),
        format!("fixture-head\n42\n1\n{index}/32\n"),
    )
    .unwrap();
    fs::write(directory.join("all-selected-mutants.json"), all.to_string()).unwrap();
    fs::write(
        directory.join("selected-mutants.json"),
        serde_json::to_string(&selected).unwrap(),
    )
    .unwrap();
    fs::write(
        directory.join("mutants.out/outcomes.json"),
        partition.to_string(),
    )
    .unwrap();
}

fn write_constructed_evidence(fixture: &Fixture, artifacts: &Path) {
    let campaign = fixture.0.join("build/core-mutation");
    let report = json_file(&campaign.join("mutants.out/outcomes.json"));
    assert_eq!(report["caught"], 3);
    assert_eq!(report["unviable"], 1);
    assert!(!report["end_time"].is_null());
    let all: serde_json::Value = (0..33)
        .map(|index| serde_json::json!({"name":format!("fixture-mutant-{index}"),"file":"core/src/lib.rs"}))
        .collect();
    for index in 0..32 {
        write_partition_fixture(artifacts, &campaign, &all, &report, index);
    }
    assert_eq!(
        json_file(&artifacts.join("core-mutation-42-1-shard-0-of-32/selected-mutants.json")),
        serde_json::json!([{"name":"fixture-mutant-0","file":"core/src/lib.rs"},{"name":"fixture-mutant-32","file":"core/src/lib.rs"}])
    );
    assert_eq!(
        json_file(&artifacts.join("core-mutation-42-1-shard-31-of-32/selected-mutants.json")),
        serde_json::json!([{"name":"fixture-mutant-31","file":"core/src/lib.rs"}])
    );
    let mut covered = BTreeSet::new();
    for index in 0..32 {
        let selected = json_file(&artifacts.join(format!(
            "core-mutation-42-1-shard-{index}-of-32/selected-mutants.json"
        )));
        let selected = selected.as_array().unwrap();
        assert!(!selected.is_empty());
        for mutant in selected {
            assert!(covered.insert(mutant["name"].as_str().unwrap().to_owned()));
        }
    }
    assert_eq!(
        covered,
        all.as_array()
            .unwrap()
            .iter()
            .map(|mutant| mutant["name"].as_str().unwrap().to_owned())
            .collect()
    );
}

fn reject_invalid_partition_evidence(fixture: &Fixture, artifacts: &Path) {
    let last = artifacts.join("core-mutation-42-1-shard-31-of-32");
    reject_missing_evidence(fixture, artifacts, &last);
    for path in [
        "identity.txt",
        "all-selected-mutants.json",
        "selected-mutants.json",
        "mutants.out/outcomes.json",
    ] {
        reject_missing_evidence(fixture, artifacts, &last.join(path));
    }
    for identity in [
        "other-head\n42\n1\n31/32\n",
        "fixture-head\n43\n1\n31/32\n",
        "fixture-head\n42\n2\n31/32\n",
        "fixture-head\n42\n1\n0/32\n",
        "fixture-head\n42\n1\n31/64\n",
    ] {
        reject_changed_evidence(
            fixture,
            artifacts,
            &last.join("identity.txt"),
            identity.as_bytes(),
            "foreign identity",
        );
    }
    reject_changed_evidence(
        fixture,
        artifacts,
        &last.join("all-selected-mutants.json"),
        b"[]",
        "inconsistent complete selection",
    );
}

fn reject_invalid_mutant_evidence(fixture: &Fixture, artifacts: &Path) {
    let first = artifacts.join("core-mutation-42-1-shard-0-of-32");
    let last = artifacts.join("core-mutation-42-1-shard-31-of-32");
    let selected_path = first.join("selected-mutants.json");
    let selected = json_file(&selected_path);
    let foreign_partition = fs::read(last.join("selected-mutants.json")).unwrap();
    reject_changed_evidence(
        fixture,
        artifacts,
        &selected_path,
        &foreign_partition,
        "foreign partition",
    );
    let mut duplicate = selected.clone();
    duplicate.as_array_mut().unwrap().push(selected[0].clone());
    reject_changed_evidence(
        fixture,
        artifacts,
        &selected_path,
        duplicate.to_string().as_bytes(),
        "duplicate selected mutant",
    );
    let report_path = first.join("mutants.out/outcomes.json");
    let original = json_file(&report_path);
    let mutant_index = original["outcomes"]
        .as_array()
        .unwrap()
        .iter()
        .position(|outcome| outcome.pointer("/scenario/Mutant").is_some())
        .unwrap();
    let mut duplicate = original.clone();
    duplicate["outcomes"]
        .as_array_mut()
        .unwrap()
        .push(original["outcomes"][mutant_index].clone());
    reject_changed_evidence(
        fixture,
        artifacts,
        &report_path,
        duplicate.to_string().as_bytes(),
        "duplicate terminal outcome",
    );
    let mut foreign = original.clone();
    foreign["outcomes"][mutant_index]["scenario"]["Mutant"]["name"] = "foreign mutant".into();
    reject_changed_evidence(
        fixture,
        artifacts,
        &report_path,
        foreign.to_string().as_bytes(),
        "foreign terminal outcome",
    );
    let mut incomplete = original.clone();
    incomplete["outcomes"]
        .as_array_mut()
        .unwrap()
        .remove(mutant_index);
    incomplete["total_mutants"] = 1.into();
    incomplete["caught"] = 1.into();
    reject_changed_evidence(
        fixture,
        artifacts,
        &report_path,
        incomplete.to_string().as_bytes(),
        "self-consistent incomplete terminal outcomes",
    );
    let log_path = first.join("mutants.out").join(
        original["outcomes"][mutant_index]["log_path"]
            .as_str()
            .unwrap(),
    );
    reject_missing_evidence(fixture, artifacts, &log_path);
    reject_changed_evidence(
        fixture,
        artifacts,
        &log_path,
        b"error: No space left on device\n",
        "infrastructure failure",
    );
    for (summary, counter) in [("Timeout", "timeout"), ("MissedMutant", "missed")] {
        let mut report = original.clone();
        report["outcomes"][mutant_index]["summary"] = summary.into();
        report["caught"] = (report["caught"].as_u64().unwrap() - 1).into();
        report[counter] = 1.into();
        reject_changed_evidence(
            fixture,
            artifacts,
            &report_path,
            report.to_string().as_bytes(),
            summary,
        );
    }
}

pub(super) fn assert_constructed_partitions(fixture: &Fixture) {
    let artifacts = fixture.0.join("constructed-artifacts");
    fs::create_dir(&artifacts).unwrap();
    write_constructed_evidence(fixture, &artifacts);
    let result = super::successful(&mut aggregate_command(fixture, &artifacts));
    assert!(String::from_utf8_lossy(&result.stderr).contains("core mutation score: 100.00%"));
    reject_invalid_partition_evidence(fixture, &artifacts);
    reject_invalid_mutant_evidence(fixture, &artifacts);
    super::uncoded::assert_causes(fixture, &artifacts);
    super::successful(&mut aggregate_command(fixture, &artifacts));
}
