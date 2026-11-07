// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Public workspace build-script identity contracts.

#![expect(
    non_snake_case,
    reason = "Requirement tags contain a double underscore."
)]

use crate::build_code::{source_hashes, violations};
use crate::scratch::Scratch;
use serde_json::{Value, json};

fn workspace_fixture() -> (Scratch, Value, String, Value) {
    let scratch = Scratch::new();
    scratch.write(
        "crates/ocr/Cargo.toml",
        "[package]\nname = \"tallyline-adapter-ocr\"\nversion = \"0.1.0\"\n",
    );
    scratch.write("crates/ocr/build.rs", "abc");
    let metadata = json!({
        "workspace_root": scratch.path(),
        "packages": [{"id":"workspace-ocr", "name":"tallyline-adapter-ocr", "version":"0.1.0",
            "source":null, "manifest_path":scratch.path().join("crates/ocr/Cargo.toml"),
            "targets":[{"kind":["custom-build"],"src_path":scratch.path().join("crates/ocr/build.rs")}]}],
        "workspace_members":["workspace-ocr"],
        "resolve":{"nodes":[{"id":"workspace-ocr","deps":[],"features":[]}]}
    });
    let registry = json!({"schema_version":1,"entries":[{
        "name":"tallyline-adapter-ocr","version":"0.1.0","kind":"custom-build",
        "workspace":{"manifest_path":"crates/ocr/Cargo.toml","source_path":"crates/ocr/build.rs"},
        "source_sha256":"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        "behaviour":"Public hash fixture; no build is executed.","review_scope":"Exact fixture bytes and paths."
    }],"contexts":[]});
    let lock = "[[package]]\nname = \"tallyline-adapter-ocr\"\nversion = \"0.1.0\"\n".to_owned();
    (scratch, metadata, lock, registry)
}

#[test]
fn r_sec_1__a_reviewed_workspace_build_script_uses_real_membership_path_and_source_identity() {
    let (_scratch, metadata, lock, registry) = workspace_fixture();
    let hashes = source_hashes(&metadata.to_string()).unwrap();
    assert!(
        violations(&metadata.to_string(), &lock, &registry.to_string(), &hashes)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn r_sec_1__a_missing_workspace_manifest_is_rejected_before_script_hashing() {
    let (scratch, metadata, _lock, _registry) = workspace_fixture();
    std::fs::remove_file(scratch.path().join("crates/ocr/Cargo.toml")).unwrap();
    assert!(source_hashes(&metadata.to_string()).is_err());
}

#[cfg(unix)]
#[test]
fn r_sec_1__a_symlinked_workspace_package_directory_is_rejected_before_hashing() {
    let (scratch, metadata, _lock, _registry) = workspace_fixture();
    let original = scratch.path().join("crates/ocr");
    let target = scratch.path().join("real-ocr");
    std::fs::rename(&original, &target).unwrap();
    std::os::unix::fs::symlink(target, original).unwrap();
    assert!(source_hashes(&metadata.to_string()).is_err());
}

#[test]
fn r_sec_1__workspace_identity_changes_and_malformed_metadata_are_rejected() {
    let (_scratch, metadata, lock, registry) = workspace_fixture();
    let hashes = source_hashes(&metadata.to_string()).unwrap();
    for (pointer, value) in [
        ("/packages/0/name", json!("renamed")),
        ("/packages/0/version", json!("0.2.0")),
        ("/packages/0/source", json!(crate::lockfile::CRATES_IO)),
        ("/packages/0/source", json!(false)),
        ("/workspace_members", json!([])),
        ("/workspace_members", json!(["foreign"])),
        (
            "/workspace_members",
            json!(["workspace-ocr", "workspace-ocr"]),
        ),
        ("/workspace_root", json!("relative")),
        ("/workspace_root", Value::Null),
        ("/packages/0/manifest_path", json!("foreign/Cargo.toml")),
        ("/packages/0/targets/0/src_path", json!("foreign/build.rs")),
        ("/packages/0/targets/0/src_path", Value::Null),
        ("/packages/0/targets/0/kind", json!(["proc-macro"])),
    ] {
        let mut changed = metadata.clone();
        *changed.pointer_mut(pointer).unwrap() = value;
        let result = violations(&changed.to_string(), &lock, &registry.to_string(), &hashes);
        assert!(result.is_err() || !result.unwrap().is_empty(), "{pointer}");
    }
}

#[test]
fn r_sec_1__workspace_reviews_cannot_borrow_archive_trust_or_erase_source_and_path_bindings() {
    let (_scratch, metadata, lock, registry) = workspace_fixture();
    let hashes = source_hashes(&metadata.to_string()).unwrap();
    for (pointer, value) in [
        ("/entries/0/workspace", Value::Null),
        (
            "/entries/0/workspace/manifest_path",
            json!("/absolute/Cargo.toml"),
        ),
        ("/entries/0/workspace/source_path", json!("../build.rs")),
        (
            "/entries/0/workspace/source_path",
            json!("crates/foreign/build.rs"),
        ),
        (
            "/entries/0/workspace/source_path",
            json!("crates\\ocr\\build.rs"),
        ),
        ("/entries/0/source_sha256", json!("bad")),
        ("/entries/0/kind", json!("proc-macro")),
    ] {
        let mut changed = registry.clone();
        *changed.pointer_mut(pointer).unwrap() = value;
        let result = violations(&metadata.to_string(), &lock, &changed.to_string(), &hashes);
        assert!(result.is_err() || !result.unwrap().is_empty(), "{pointer}");
    }
    for changed_lock in [
        String::new(),
        format!("{lock}checksum = \"{}\"\n", "a".repeat(64)),
    ] {
        let result = violations(
            &metadata.to_string(),
            &changed_lock,
            &registry.to_string(),
            &hashes,
        );
        assert!(result.is_err() || !result.unwrap().is_empty());
    }
    let mut mixed = registry.clone();
    mixed["entries"][0]["archive_sha256"] = json!("a".repeat(64));
    assert!(violations(&metadata.to_string(), &lock, &mixed.to_string(), &hashes).is_err());
    assert!(
        !violations(
            &metadata.to_string(),
            &lock,
            &registry.to_string(),
            &std::collections::BTreeMap::new()
        )
        .unwrap()
        .is_empty()
    );
}

#[test]
fn r_sec_1__changed_or_missing_workspace_script_bytes_are_rejected() {
    let (scratch, metadata, lock, registry) = workspace_fixture();
    scratch.write("crates/ocr/build.rs", "changed");
    let hashes = source_hashes(&metadata.to_string()).unwrap();
    assert!(
        !violations(&metadata.to_string(), &lock, &registry.to_string(), &hashes)
            .unwrap()
            .is_empty()
    );
    std::fs::remove_file(scratch.path().join("crates/ocr/build.rs")).unwrap();
    assert!(source_hashes(&metadata.to_string()).is_err());
}
