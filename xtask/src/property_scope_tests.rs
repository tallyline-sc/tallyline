// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exercises property profile admission through public metadata and lock evidence.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::property_scope;

const TARGET: &str = "x86_64-pc-windows-msvc";

fn metadata() -> Value {
    json!({
        "packages": [
            {"id":"root", "name":"tallyline-recognition", "version":"0.0.0", "source":null},
            {"id":"property", "name":"proptest", "version":"1.12.0", "source":crate::lockfile::CRATES_IO}
        ],
        "workspace_members":["root"],
        "resolve":{"nodes":[
            {"id":"root", "features":[], "deps":[{"name":"proptest","pkg":"property", "dep_kinds":[{"kind":"dev", "target":null}]}]},
            {"id":"property", "features":["regex-syntax","std"], "deps":[]}
        ]}
    })
}

#[test]
fn an_unapproved_property_profile_is_refused_before_execution() {
    assert!(
        !property_scope::violations(
            &metadata().to_string(),
            "version = 4",
            None,
            TARGET,
            &BTreeMap::new()
        )
        .expect("well-formed fixture")
        .is_empty()
    );
}

fn review() -> Value {
    json!({"schema":1, "status":"approved", "revisit_by":"2099-01-01",
        "targets":[TARGET,"x86_64-unknown-linux-gnu"],
        "packages":[{"name":"proptest", "version":"1.12.0", "archive_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "source_tree_sha256":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", "features":["regex-syntax","std"]}]
    })
}

fn lock() -> String {
    format!(
        "version = 4\n[[package]]\nname = \"proptest\"\nversion = \"1.12.0\"\nsource = \"{}\"\nchecksum = \"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"\n",
        crate::lockfile::CRATES_IO
    )
}

fn check(metadata: &Value, record: &Value) -> Result<Vec<String>, String> {
    property_scope::violations(
        &metadata.to_string(),
        &lock(),
        Some(&record.to_string()),
        TARGET,
        &BTreeMap::from([(
            "property".to_owned(),
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_owned(),
        )]),
    )
}

#[test]
fn an_exact_reviewed_dev_profile_is_admitted() {
    assert!(
        check(&metadata(), &review())
            .expect("reviewed fixture")
            .is_empty()
    );
}

#[test]
fn future_features_cannot_inherit_unreviewed_runner_capabilities() {
    for feature in [
        "fork",
        "timeout",
        "hardware-rng",
        "handle-panics",
        "attr-macro",
        "future-feature",
    ] {
        let mut input = metadata();
        input["resolve"]["nodes"][1]["features"]
            .as_array_mut()
            .expect("features")
            .push(json!(feature));
        assert!(
            check(&input, &review())
                .expect("complete graph")
                .iter()
                .any(|finding| finding.contains("features changed")),
            "{feature}"
        );
    }
}

#[test]
fn future_versions_and_foreign_sources_are_refused() {
    for (field, value) in [
        ("version", "1.13.0"),
        ("source", "git+https://example.invalid/proptest"),
    ] {
        let mut input = metadata();
        input["packages"][1][field] = json!(value);
        assert!(
            !check(&input, &review()).expect("complete graph").is_empty(),
            "{field}"
        );
    }
}

#[test]
fn changed_archive_and_source_bytes_are_refused() {
    let mut record = review();
    record["packages"][0]["archive_sha256"] =
        json!("cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc");
    assert!(
        check(&metadata(), &record)
            .expect("complete graph")
            .iter()
            .any(|finding| finding.contains("archive/source"))
    );
    record = review();
    record["packages"][0]["source_tree_sha256"] =
        json!("cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc");
    assert!(
        check(&metadata(), &record)
            .expect("complete graph")
            .iter()
            .any(|finding| finding.contains("source tree"))
    );
}

#[test]
fn normal_build_and_renamed_introductions_are_refused() {
    for kind in [Value::Null, json!("build")] {
        let mut input = metadata();
        input["resolve"]["nodes"][0]["deps"][0]["dep_kinds"][0]["kind"] = kind;
        assert!(!check(&input, &review()).expect("complete graph").is_empty());
    }
    let mut input = metadata();
    input["resolve"]["nodes"][0]["deps"][0]["name"] = json!("renamed_testing");
    assert!(!check(&input, &review()).expect("complete graph").is_empty());
}

#[test]
fn an_alternative_workspace_root_cannot_introduce_a_transitive_testing_dependency() {
    let mut input = metadata();
    input["packages"]
        .as_array_mut()
        .expect("packages")
        .push(json!({"id":"other", "name":"tallyline-app", "version":"0.0.0", "source":null}));
    input["workspace_members"]
        .as_array_mut()
        .expect("roots")
        .push(json!("other"));
    input["resolve"]["nodes"].as_array_mut().expect("nodes").push(json!({"id":"other","features":[],"deps":[{"pkg":"property","name":"hidden","dep_kinds":[{"kind":null,"target":null}]}]}));
    assert!(!check(&input, &review()).expect("complete graph").is_empty());
}

#[test]
fn malformed_graphs_and_reviews_fail_closed() {
    for path in ["packages", "workspace_members", "resolve"] {
        let mut input = metadata();
        input.as_object_mut().expect("object").remove(path);
        assert!(check(&input, &review()).is_err(), "{path}");
    }
    let mut input = metadata();
    input["resolve"]["nodes"][0]["deps"][0]["pkg"] = json!("missing");
    assert!(check(&input, &review()).is_err());
    let mut input = metadata();
    input["resolve"]["nodes"][0]["deps"][0]
        .as_object_mut()
        .expect("edge")
        .remove("dep_kinds");
    assert!(check(&input, &review()).is_err());
    for status in ["PROPOSED_PRIVATE_AI_REVIEW_ONLY", "proposed"] {
        let mut record = review();
        record["status"] = json!(status);
        assert!(check(&metadata(), &record).is_err());
    }
}

#[test]
fn unsupported_targets_cannot_activate_the_profile() {
    assert!(
        property_scope::violations(
            &metadata().to_string(),
            &lock(),
            Some(&review().to_string()),
            "aarch64-unknown-linux-gnu",
            &BTreeMap::new()
        )
        .is_err()
    );
}

#[test]
fn unconnected_older_rand_versions_do_not_become_a_blanket_ban() {
    let mut input = metadata();
    input["packages"][1]["name"] = json!("rand");
    input["packages"][1]["version"] = json!("0.9.2");
    assert!(
        property_scope::violations(
            &input.to_string(),
            "version = 4",
            None,
            TARGET,
            &BTreeMap::new()
        )
        .expect("older unrelated dependency")
        .is_empty()
    );
}

#[test]
fn normal_build_tree_evidence_must_exclude_testing_packages() {
    let input = metadata().to_string();
    assert!(
        property_scope::release_violations(&input, "0tallyline-recognition v0.0.0|\n", TARGET)
            .expect("dev-free graph")
            .is_empty()
    );
    assert!(
        !property_scope::release_violations(
            &input,
            "0tallyline-recognition v0.0.0|\n1proptest v1.12.0|regex-syntax,std\n",
            TARGET
        )
        .expect("leaked graph")
        .is_empty()
    );
    assert!(
        property_scope::release_violations(&input, "0proptest v1.12.0|regex-syntax,std\n", TARGET)
            .is_err()
    );
}

#[test]
fn source_collection_observes_actual_package_bytes() {
    let scratch = crate::scratch::Scratch::new();
    scratch.write("Cargo.toml", "[package]\nname = \"proptest\"\n");
    scratch.write("src/lib.rs", "pub fn example() {}\n");
    let mut input = metadata();
    input["packages"][1]["manifest_path"] = json!(scratch.path().join("Cargo.toml"));
    let before = property_scope::source_trees(&input.to_string(), &review().to_string())
        .expect("regular source tree");
    scratch.write("src/lib.rs", "pub fn different() {}\n");
    let after = property_scope::source_trees(&input.to_string(), &review().to_string())
        .expect("changed source tree");
    assert_ne!(before, after);
}

#[test]
fn target_tree_activation_must_match_the_exact_reviewed_features() {
    let input = crate::target_graph::resolved_metadata(
        &metadata().to_string(),
        "0tallyline-recognition v0.0.0|\n1proptest v1.12.0|std\n",
    )
    .expect("complete target tree");
    let input: Value = serde_json::from_str(&input).expect("projected metadata");
    assert!(
        check(&input, &review())
            .expect("reviewed target")
            .iter()
            .any(|finding| finding.contains("features changed"))
    );
}

#[test]
fn workspace_refusal_precedes_further_cargo_collection() {
    let scratch = crate::scratch::Scratch::new();
    scratch.write("Cargo.lock", "version = 4\n");
    let findings = crate::property_scope_command::workspace_violations(
        scratch.path(),
        TARGET,
        |args| {
            assert_eq!(args, ["metadata", "--locked", "--format-version", "1"], "collect the conservative host/target union before target trees; unapproved data must stop before any other Cargo request");
            Ok(metadata().to_string())
        },
    )
    .expect("well-formed unapproved graph");
    assert!(!findings.is_empty());
}

#[test]
fn shared_getrandom_sys_rng_cannot_escape_the_exact_profile_record() {
    let mut input = metadata();
    input["packages"].as_array_mut().expect("packages").push(json!({"id":"entropy","name":"getrandom","version":"0.4.3","source":crate::lockfile::CRATES_IO}));
    input["resolve"]["nodes"][1]["deps"]
        .as_array_mut()
        .expect("dependencies")
        .push(
            json!({"name":"getrandom","pkg":"entropy","dep_kinds":[{"kind":null,"target":null}]}),
        );
    input["resolve"]["nodes"]
        .as_array_mut()
        .expect("nodes")
        .push(json!({"id":"entropy","features":["std","sys_rng"],"deps":[]}));
    assert!(
        check(&input, &review())
            .expect("complete graph")
            .iter()
            .any(|finding| finding.contains("getrandom") && finding.contains("unreviewed"))
    );
}
