// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Public approval-boundary fixtures for literal inline assembly.

use crate::native_assembly::{SourceFile, violations};
use serde_json::{Value, json};

fn metadata() -> Value {
    json!({"packages": [{"id":"root", "name":"app", "version":"0.0.0", "source":null},
        {"id":"dep", "name":"sample", "version":"1.0.0", "source":crate::lockfile::CRATES_IO}],
        "workspace_members":["root"], "resolve":{"nodes":[
            {"id":"root","features":[],"deps":[{"pkg":"dep"}]},
            {"id":"dep","features":["std"],"deps":[]}]}})
}

fn review() -> Value {
    json!({"schema":1,"status":"approved","revisit_by":"2027-04-13",
        "targets":["windows","linux"],"packages":[{"name":"sample","version":"1.0.0",
        "archive_sha256":"a".repeat(64),"full_features":["std"],
        "targets":{"windows":["std"],"linux":["std"]},
        "files":{"src/lib.rs":"b".repeat(64)}}]})
}

fn findings(metadata: &Value, review: &Value, files: &[SourceFile]) -> Result<Vec<String>, String> {
    let lock = format!(
        "[[package]]\nname = \"sample\"\nversion = \"1.0.0\"\nsource = \"{}\"\nchecksum = \"{}\"\n",
        crate::lockfile::CRATES_IO,
        "a".repeat(64)
    );
    let targets = ["windows", "linux"]
        .map(|target| (target.to_owned(), metadata.to_string()))
        .into();
    violations(
        &metadata.to_string(),
        &lock,
        &review.to_string(),
        files,
        &targets,
    )
}

fn files() -> Vec<SourceFile> {
    vec![SourceFile {
        package_id: "dep".to_owned(),
        path: "src/lib.rs".to_owned(),
        sha256: "b".repeat(64),
    }]
}

#[test]
fn exact_approved_scope_passes() {
    assert!(
        findings(&metadata(), &review(), &files())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn unknown_package_version_hash_and_forbidden_feature_fail() {
    for field in ["name", "version"] {
        let mut input = metadata();
        input["packages"][1][field] = json!("unreviewed");
        assert!(!findings(&input, &review(), &files()).unwrap().is_empty());
    }
    let mut wrong_archive = review();
    wrong_archive["packages"][0]["archive_sha256"] = json!("c".repeat(64));
    assert!(
        !findings(&metadata(), &wrong_archive, &files())
            .unwrap()
            .is_empty()
    );
    let mut changed = files();
    changed[0].sha256 = "c".repeat(64);
    assert!(
        !findings(&metadata(), &review(), &changed)
            .unwrap()
            .is_empty()
    );
    let mut input = metadata();
    input["resolve"]["nodes"][1]["features"] = json!(["std", "hardware-lock-elision"]);
    assert!(!findings(&input, &review(), &files()).unwrap().is_empty());
}

#[test]
fn proposed_missing_and_stale_reviews_fail_closed() {
    let mut proposed = review();
    proposed["status"] = json!("proposed");
    assert!(
        !findings(&metadata(), &proposed, &files())
            .unwrap()
            .is_empty()
    );
    assert!(!findings(&metadata(), &review(), &[]).unwrap().is_empty());
    let mut extra = files();
    extra.push(SourceFile {
        package_id: "dep".to_owned(),
        path: "src/new.rs".to_owned(),
        sha256: "b".repeat(64),
    });
    assert!(!findings(&metadata(), &review(), &extra).unwrap().is_empty());
    proposed = review();
    proposed["packages"] = json!([]);
    assert!(
        !findings(&metadata(), &proposed, &files())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn excluded_scope_cannot_become_reachable() {
    let mut approved = review();
    approved["packages"][0]["targets"]["windows"] = Value::Null;
    assert!(
        !findings(&metadata(), &approved, &files())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn cached_source_collection_is_conservative_and_hash_bound() {
    let scratch = crate::scratch::Scratch::new();
    scratch.write("Cargo.toml", "[package]");
    scratch.write("src/a.rs", "core::arch::asm \n ! (\"nop\");");
    scratch.write("src/b.rs", "// global_asm ! is deliberately included");
    scratch.write("src/c.rs", "some_asm!(); assembly();");
    let mut input = metadata();
    input["packages"][1]["manifest_path"] = json!(scratch.path().join("Cargo.toml"));
    let found = crate::native_assembly::source_files(&input.to_string()).unwrap();
    assert_eq!(found.len(), 2);
    assert!(found.iter().any(|file| file.path == "src/a.rs"
        && file.sha256 == crate::hashlist::sha256_hex(b"core::arch::asm \n ! (\"nop\");")));
}

#[test]
fn expired_or_malformed_scope_cannot_pass() {
    let mut expired = review();
    expired["revisit_by"] = json!("1970-01-01");
    assert!(findings(&metadata(), &expired, &files()).is_err());
    assert!(violations("{}", "", "{}", &[], &Default::default()).is_err());
}

#[test]
fn target_tree_exclusion_requires_evidence_and_rejects_real_activation() {
    let mut approved = review();
    approved["packages"][0]["resolved_targets"] = json!({"windows":null,"linux":["std"]});
    assert!(findings(&metadata(), &approved, &files()).is_err());
}

#[test]
fn resolved_target_selectors_preserve_exclusion_and_pin_provenance() {
    let mut approved = review();
    approved["packages"][0]["resolved_targets"] = json!({"windows":null,"linux":["std"]});
    let windows =
        crate::target_graph::resolved_metadata(&metadata().to_string(), "0app v0.0.0|\n").unwrap();
    let linux = crate::target_graph::resolved_metadata(
        &metadata().to_string(),
        "0app v0.0.0|\n1sample v1.0.0|std\n",
    )
    .unwrap();
    let mut targets: std::collections::BTreeMap<_, _> = [("windows", windows), ("linux", linux)]
        .map(|(target, graph)| {
            let mut input = metadata();
            input["tallyline_target_tree"] = json!({"cargo":crate::target_graph::REVIEWED_CARGO,
            "host":"x86_64-pc-windows-msvc","target":target,
            "graph":serde_json::from_str::<Value>(&graph).unwrap()});
            (target.to_owned(), input.to_string())
        })
        .into();
    let lock = format!(
        "[[package]]\nname = \"sample\"\nversion = \"1.0.0\"\nsource = \"{}\"\nchecksum = \"{}\"\n",
        crate::lockfile::CRATES_IO,
        "a".repeat(64)
    );
    let check = |targets: &std::collections::BTreeMap<String, String>| {
        violations(
            &metadata().to_string(),
            &lock,
            &approved.to_string(),
            &files(),
            targets,
        )
    };
    assert!(check(&targets).unwrap().is_empty());
    let good = targets.clone();
    for (field, value) in [
        ("cargo", json!("cargo 1.100.0")),
        ("host", json!("aarch64-unknown-linux-gnu")),
        ("target", json!("wrong")),
        ("graph", json!({})),
    ] {
        let mut input: Value = serde_json::from_str(&good["windows"]).unwrap();
        input["tallyline_target_tree"][field] = value;
        targets.insert("windows".to_owned(), input.to_string());
        assert!(check(&targets).is_err(), "{field}");
    }
    let mut input: Value = serde_json::from_str(&good["windows"]).unwrap();
    input["tallyline_target_tree"]["graph"] = metadata();
    targets = good;
    targets.insert("windows".to_owned(), input.to_string());
    assert!(
        check(&targets)
            .unwrap()
            .iter()
            .any(|finding| finding.contains("resolved assembly activation"))
    );
}

#[test]
fn credential_ffi_sources_require_explicit_hash_bound_review_without_literal_assembly() {
    let scratch = crate::scratch::Scratch::new();
    scratch.write("Cargo.toml", "[package]");
    scratch.write("src/utils.rs", "pub fn native_credential_call() {}");
    scratch.write("src/cred.rs", "pub struct Credential;");
    scratch.write("src/store.rs", "pub struct Store;");
    let mut input = metadata();
    input["packages"][1]["name"] = json!("windows-native-keyring-store");
    input["packages"][1]["manifest_path"] = json!(scratch.path().join("Cargo.toml"));
    let found = crate::native_assembly::source_files(&input.to_string()).unwrap();
    let paths: std::collections::BTreeSet<_> =
        found.iter().map(|file| file.path.as_str()).collect();
    assert_eq!(
        paths,
        ["src/cred.rs", "src/store.rs", "src/utils.rs"].into()
    );
}
