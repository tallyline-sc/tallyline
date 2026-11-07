// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Fixed public-contract command grammar without credentialled operations.

use super::contract_cli::{ContractMode, command, write_snapshot};
use tallyline_adapter_uex::api::model::PublicContractSnapshot;

fn arguments(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn r_api_5_r_doc_2__snapshot_command_has_no_account_path_or_alternative_host() {
    assert_eq!(
        command(&arguments(&["contract-snapshot"]))
            .unwrap()
            .unwrap(),
        ContractMode::Snapshot,
    );
    assert_eq!(
        command(&arguments(&["--host", "uk", "contract-snapshot"]))
            .unwrap()
            .unwrap(),
        ContractMode::Snapshot,
    );
    for args in [
        vec!["--host", "space", "contract-snapshot"],
        vec!["account", "contract-snapshot"],
        vec!["contract-snapshot", "data_submit"],
        vec!["--confirm-expert-host", "contract-snapshot"],
        vec!["--host", "never-emit-private-host", "contract-snapshot"],
    ] {
        let reason = command(&arguments(&args)).unwrap().unwrap_err();
        assert!(!reason.contains("never-emit"));
    }
    assert!(command(&arguments(&["account"])).is_none());
    assert!(command(&arguments(&["game-versions"])).is_none());
}

#[test]
fn r_api_5_r_doc_2__the_check_command_is_an_explicit_public_operation() {
    assert!(command(&arguments(&["contract-check"])).is_some_and(|result| result.is_ok()));
    assert!(
        command(&arguments(&["--host", "space", "contract-check"]))
            .is_some_and(|result| result.is_err())
    );
}

#[test]
fn r_api_5_r_doc_2__an_observed_difference_fails_the_check_and_names_only_the_field() {
    let baseline = include_str!("../contracts/public-v1.json");
    let reference = PublicContractSnapshot::from_json(baseline).unwrap();
    let mode = command(&arguments(&["contract-check"])).unwrap().unwrap();
    let mut clean_output = Vec::new();
    assert!(write_snapshot(mode, &reference, &mut clean_output).is_ok());
    let mut changed: serde_json::Value = serde_json::from_str(baseline).unwrap();
    changed["endpoints"][0]["fields"]["data.pending_ci_probe"] = serde_json::json!(["number"]);
    let changed = PublicContractSnapshot::from_json(&changed.to_string()).unwrap();
    let mut output = Vec::new();
    assert!(write_snapshot(mode, &changed, &mut output).is_err());
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("field added"));
    assert!(output.contains("data.pending_ci_probe"));
}
