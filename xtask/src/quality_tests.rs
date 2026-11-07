// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Quality gates through public metadata and check-plan seams.

use crate::quality::core_packages;

#[test]
fn selects_only_documented_core_workspace_members() {
    let metadata = r#"{"workspace_members":["core","ui"],"packages":[{"id":"core","name":"tallyline-reporting"},{"id":"ui","name":"tallyline-ui"},{"id":"external","name":"tallyline-game"}]}"#;
    assert_eq!(core_packages(metadata).unwrap(), ["tallyline-reporting"]);
}

#[test]
fn check_plan_includes_core_coverage() {
    assert!(
        crate::check::plan()
            .iter()
            .any(|step| step.name() == "core coverage")
    );
}
fn mutation_report(caught: usize, missed: usize) -> serde_json::Value {
    let mut outcomes = vec![serde_json::json!({"scenario":"Baseline", "summary":"Success"})];
    outcomes.extend(
        (0..caught).map(|_| serde_json::json!({"scenario":{"Mutant":{}},"summary":"CaughtMutant"})),
    );
    outcomes.extend(
        (0..missed).map(|_| serde_json::json!({"scenario":{"Mutant":{}},"summary":"MissedMutant"})),
    );
    serde_json::json!({"cargo_mutants_version":"27.1.0","end_time":"2026-11-01T00:00:00Z","total_mutants":caught+missed,"caught":caught,"missed":missed,"unviable":0,"timeout":0,"success":0,"outcomes":outcomes})
}

#[test]
fn rejects_mutation_score_below_frozen_floor() {
    assert!(crate::quality::mutation_score(&mutation_report(99, 1).to_string()).is_err());
}

#[test]
fn accepts_boundary_mutation_score_and_rejects_incomplete_reports() {
    assert_eq!(
        crate::quality::mutation_score(&mutation_report(10, 0).to_string()),
        Ok(100.0)
    );
    for key in ["end_time", "caught", "outcomes", "cargo_mutants_version"] {
        let mut report = mutation_report(10, 0);
        report.as_object_mut().unwrap().remove(key);
        assert!(
            crate::quality::mutation_score(&report.to_string()).is_err(),
            "{key}"
        );
    }
    let mut report = mutation_report(10, 0);
    report["timeout"] = 1.into();
    assert!(crate::quality::mutation_score(&report.to_string()).is_err());
    let mut report = mutation_report(10, 0);
    report["outcomes"][0]["summary"] = "Failure".into();
    assert!(crate::quality::mutation_score(&report.to_string()).is_err());
}

#[test]
fn metadata_cannot_silently_hide_core_members() {
    for metadata in [
        "{}",
        r#"{"packages":[],"workspace_members":["missing"]}"#,
        r#"{"packages":[],"workspace_members":[42]}"#,
    ] {
        assert!(core_packages(metadata).is_err());
    }
    assert!(
        core_packages(r#"{"packages":[],"workspace_members":[]}"#)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn permits_ui_to_use_the_approved_design_system() {
    let metadata = r#"{"workspace_members":["ui","egui"],"packages":[{"id":"ui","name":"tallyline-ui","dependencies":[{"name":"tallyline-egui"}]},{"id":"egui","name":"tallyline-egui","dependencies":[]}]}"#;
    assert!(crate::architecture::violations(metadata).is_empty());
}

#[test]
fn excludes_unviable_mutants_but_rejects_disagreeing_totals() {
    let mut report = mutation_report(10, 0);
    report["unviable"] = 5.into();
    report["total_mutants"] = 15.into();
    for _ in 0..5 {
        report["outcomes"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"scenario":{"Mutant":{}}, "summary":"Unviable"}));
    }
    assert_eq!(
        crate::quality::mutation_score(&report.to_string()),
        Ok(100.0)
    );
    report["total_mutants"] = 16.into();
    assert!(crate::quality::mutation_score(&report.to_string()).is_err());
    assert!(crate::quality::mutation_score(&mutation_report(0, 0).to_string()).is_err());
}

#[test]
fn rejects_duplicate_and_malformed_package_identities() {
    for metadata in [
        r#"{"packages":[{"id":"a","name":"tallyline-game"},{"id":"a","name":"tallyline-game"}],"workspace_members":["a"]}"#,
        r#"{"packages":[{"id":"a","name":"tallyline-game"}],"workspace_members":["a","a"]}"#,
        r#"{"packages":[{"id":"a"}],"workspace_members":["a"]}"#,
    ] {
        assert!(core_packages(metadata).is_err());
    }
}
