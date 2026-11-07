// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Mutation selection and report contracts.

use crate::mutation::{Options, Shard, selected_score};
use serde_json::json;

#[test]
fn parses_shards_and_diff_in_either_order_without_changing_unsharded_calls() {
    assert_eq!(Options::parse(&[]).unwrap().shard, None);
    for args in [
        vec!["--shard", "0/8", "--in-diff", "core.diff"],
        vec!["--in-diff", "core.diff", "--shard", "0/8"],
    ] {
        let options = Options::parse(&args).unwrap();
        assert_eq!(options.shard.unwrap().to_string(), "0/8");
        assert_eq!(options.diff.unwrap().to_str(), Some("core.diff"));
    }
    assert_eq!(
        "7/8".parse::<Shard>().unwrap().report_directory(),
        "shard-7-of-8"
    );
}

#[test]
fn rejects_bad_shards_and_ambiguous_arguments() {
    for value in [
        "", "0", "0/0", "8/8", "9/8", "-1/8", "1/2/3", "a/8", "1/+8", "01/8",
    ] {
        assert!(value.parse::<Shard>().is_err(), "{value}");
    }
    for args in [
        vec!["--shard"],
        vec!["--shard", "0/8", "--shard", "1/8"],
        vec!["--in-diff", "a", "--in-diff", "b"],
        vec!["--unknown"],
        vec!["--in-diff", "--shard"],
    ] {
        assert!(Options::parse(&args).is_err(), "{args:?}");
    }
}

#[test]
fn rejects_self_consistent_partial_duplicate_or_foreign_outcomes() {
    let selected = json!([{"name":"first"}, {"name":"second"}]);
    let mut report = json!({"cargo_mutants_version":"27.1.0", "end_time":"2026-11-02T00:00:00Z", "total_mutants":2,"caught":2,"missed":0,"unviable":0,"timeout":0,"success":0,"outcomes":[{"scenario":"Baseline","summary":"Success"},{"scenario":{"Mutant":{"name":"first"}},"summary":"CaughtMutant"},{"scenario":{"Mutant":{"name":"second"}},"summary":"CaughtMutant"}]});
    assert_eq!(
        selected_score(&report.to_string(), &selected.to_string()),
        Ok(100.0)
    );
    report["outcomes"][2]["scenario"]["Mutant"]["name"] = json!("first");
    assert!(selected_score(&report.to_string(), &selected.to_string()).is_err());
    report["outcomes"][2]["scenario"]["Mutant"]["name"] = json!("foreign");
    assert!(selected_score(&report.to_string(), &selected.to_string()).is_err());
    report["outcomes"].as_array_mut().unwrap().pop();
    report["total_mutants"] = json!(1);
    report["caught"] = json!(1);
    assert!(selected_score(&report.to_string(), &selected.to_string()).is_err());
}

#[test]
fn aggregates_viable_and_unviable_shards_but_rejects_all_unviable_and_missing_evidence() {
    use crate::mutation::{RunIdentity, aggregate};
    let scratch = crate::scratch::Scratch::new();
    let identity = RunIdentity {
        head: "head",
        run: "42",
        attempt: "1",
    };
    let full = json!([{"name":"first"}, {"name":"second"}, {"name":"third"}]);
    for index in 0..32 {
        let directory = format!("core-mutation-42-1-shard-{index}-of-32");
        scratch.write(
            &format!("{directory}/identity.txt"),
            &format!("head\n42\n1\n{index}/32\n"),
        );
        scratch.write(
            &format!("{directory}/all-selected-mutants.json"),
            &full.to_string(),
        );
        let selected = if index < 3 {
            json!([full[index].clone()])
        } else {
            json!([])
        };
        scratch.write(
            &format!("{directory}/selected-mutants.json"),
            &selected.to_string(),
        );
        if index < 3 {
            scratch.write(
                &format!("{directory}/mutants.out/outcomes.json"),
                &completed_report(full[index].clone(), index == 2).to_string(),
            );
            scratch.write(
                &format!("{directory}/mutants.out/log/cause.log"),
                if index == 2 {
                    "error[E0277]: Preview does not implement Default\n"
                } else {
                    "test domain_contract ... FAILED\nassertion failed: expected value\ntest result: FAILED. 0 passed; 1 failed\n"
                },
            );
        }
    }
    assert_eq!(aggregate(scratch.path(), &identity), Ok(Some(100.0)));
    scratch.write(
        "core-mutation-42-1-shard-0-of-32/mutants.out/log/cause.log",
        "test unrelated_loader ... FAILED\nError: ExecutableFileBusy\ntest result: FAILED. 0 passed; 1 failed\n",
    );
    assert!(
        aggregate(scratch.path(), &identity).is_err(),
        "Infrastructure failure must not count as a caught core mutation."
    );
    scratch.write(
        "core-mutation-42-1-shard-0-of-32/mutants.out/log/cause.log",
        "test domain_contract ... FAILED\nassertion failed: expected value\ntest result: FAILED. 0 passed; 1 failed\n",
    );
    scratch.write(
        "core-mutation-42-1-shard-0-of-32/selected-mutants.json",
        &json!([full[1].clone()]).to_string(),
    );
    assert!(aggregate(scratch.path(), &identity).is_err());
    scratch.write(
        "core-mutation-42-1-shard-0-of-32/selected-mutants.json",
        &json!([full[0].clone()]).to_string(),
    );
    let foreign_run = RunIdentity {
        head: "head",
        run: "43",
        attempt: "1",
    };
    assert!(aggregate(scratch.path(), &foreign_run).is_err());
    scratch.write(
        "core-mutation-42-1-shard-31-of-32/all-selected-mutants.json",
        "[]",
    );
    assert!(aggregate(scratch.path(), &identity).is_err());
    scratch.write(
        "core-mutation-42-1-shard-31-of-32/all-selected-mutants.json",
        &full.to_string(),
    );

    for index in 0..2 {
        scratch.write(
            &format!("core-mutation-42-1-shard-{index}-of-32/mutants.out/outcomes.json"),
            &completed_report(full[index].clone(), true).to_string(),
        );
        scratch.write(
            &format!("core-mutation-42-1-shard-{index}-of-32/mutants.out/log/cause.log"),
            "error[E0277]: Preview does not implement Default\n",
        );
    }
    assert!(aggregate(scratch.path(), &identity).is_err());
    scratch.write(
        "core-mutation-42-1-shard-0-of-32/mutants.out/outcomes.json",
        &completed_report(full[0].clone(), false).to_string(),
    );
    scratch.write(
        "core-mutation-42-1-shard-0-of-32/mutants.out/log/cause.log",
        "test domain_contract ... FAILED\nassertion failed: expected value\ntest result: FAILED. 0 passed; 1 failed\n",
    );
    scratch.write(
        "core-mutation-42-1-shard-31-of-32/identity.txt",
        "other-head\n42\n1\n31/32\n",
    );
    assert!(aggregate(scratch.path(), &identity).is_err());
    scratch.write(
        "core-mutation-42-1-shard-31-of-32/identity.txt",
        "head\n42\n1\n31/32\n",
    );
    std::fs::remove_file(
        scratch
            .path()
            .join("core-mutation-42-1-shard-31-of-32/selected-mutants.json"),
    )
    .unwrap();
    assert!(aggregate(scratch.path(), &identity).is_err());
}

fn completed_report(mutant: serde_json::Value, unviable: bool) -> serde_json::Value {
    json!({"cargo_mutants_version":"27.1.0", "end_time":"2026-11-02T00:00:00Z", "total_mutants":1,"caught":usize::from(!unviable),"missed":0,"unviable":usize::from(unviable),"timeout":0,"success":0,"outcomes":[{"scenario":"Baseline","summary":"Success"},{"scenario":{"Mutant":mutant},"summary":if unviable {"Unviable"} else {"CaughtMutant"},"log_path":"log/cause.log"}]})
}

#[test]
fn all_thirty_two_empty_selections_skip_without_a_measured_score() {
    let scratch = crate::scratch::Scratch::new();
    let identity = crate::mutation::RunIdentity {
        head: "head",
        run: "42",
        attempt: "1",
    };
    for index in 0..32 {
        let directory = format!("core-mutation-42-1-shard-{index}-of-32");
        scratch.write(
            &format!("{directory}/identity.txt"),
            &format!("head\n42\n1\n{index}/32\n"),
        );
        scratch.write(&format!("{directory}/all-selected-mutants.json"), "[]");
        scratch.write(&format!("{directory}/selected-mutants.json"), "[]");
    }
    assert_eq!(
        crate::mutation::aggregate(scratch.path(), &identity),
        Ok(None)
    );
    scratch.write(
        "core-mutation-42-1-shard-0-of-32/mutants.out/outcomes.json",
        "{}",
    );
    assert!(crate::mutation::aggregate(scratch.path(), &identity).is_err());
}
