// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact approved SQLite native-source boundary fixtures.

use crate::{lockfile, sqlite_native};
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn record() -> Value {
    serde_json::from_str(include_str!("../../supply-chain/sqlite-native.json")).unwrap()
}

fn graph(review: &Value) -> String {
    let source = lockfile::CRATES_IO;
    json!({"workspace_members":["root"],"packages":[
        {"id":"root","name":"fixture","version":"0.0.0","source":null},
        {"id":"wrapper","name":"rusqlite","version":review["rusqlite"]["version"],"source":source},
        {"id":"sys","name":"libsqlite3-sys","version":review["version"],"source":source}
    ],"resolve":{"nodes":[
        {"id":"root","features":[],"deps":[{"name":"renamed_sqlite","pkg":"wrapper"}]},
        {"id":"wrapper","features":review["rusqlite"]["features"],"deps":[{"name":"ffi","pkg":"sys"}]},
        {"id":"sys","features":review["features"],"deps":[]}
    ]}}).to_string()
}

struct Evidence {
    locked: Vec<lockfile::LockedPackage>,
    graphs: BTreeMap<String, String>,
    hashes: BTreeMap<String, String>,
}
impl Evidence {
    fn new(review: &Value) -> Self {
        let locked = [
            ("rusqlite", &review["rusqlite"]),
            ("libsqlite3-sys", review),
        ]
        .into_iter()
        .map(|(name, entry)| lockfile::LockedPackage {
            name: name.to_owned(),
            version: entry["version"].as_str().unwrap().to_owned(),
            source: Some(lockfile::CRATES_IO.to_owned()),
            checksum: Some(entry["archive_sha256"].as_str().unwrap().to_owned()),
        })
        .collect();
        let graphs = ["full", "x86_64-pc-windows-msvc", "x86_64-unknown-linux-gnu"]
            .into_iter()
            .map(|target| (target.to_owned(), graph(review)))
            .collect();
        let hashes = review["files"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(path, hash)| (path.clone(), hash.as_str().unwrap().to_owned()))
            .collect();
        Self {
            locked,
            graphs,
            hashes,
        }
    }
    fn findings(
        &self,
        review: &Value,
        environment: &[(String, String)],
    ) -> Result<Vec<String>, String> {
        sqlite_native::violations(
            &review.to_string(),
            &self.locked,
            &self.graphs,
            &self.hashes,
            environment,
            20_758,
        )
    }
}
fn findings(review: &Value, environment: &[(String, String)]) -> Result<Vec<String>, String> {
    Evidence::new(review).findings(review, environment)
}
#[test]
fn exact_approved_scope_passes_and_native_compiler_overrides_fail() {
    let review = record();
    assert!(findings(&review, &[]).unwrap().is_empty());
    for key in [
        "CFLAGS",
        "CFLAGS_x86_64_unknown_linux_gnu",
        "TARGET_CFLAGS",
        "LIBSQLITE3_FLAGS",
        "RUSTFLAGS",
        "CARGO_ENCODED_RUSTFLAGS",
        "RUSTC_WRAPPER",
        "CL",
        "_CL_",
    ] {
        assert!(
            !findings(&review, &[(key.to_owned(), "unreviewed".to_owned())])
                .unwrap()
                .is_empty()
        );
    }
    assert!(
        !findings(
            &review,
            &[("LIBSQLITE3_SYS_USE_PKG_CONFIG".to_owned(), "1".to_owned())]
        )
        .unwrap()
        .is_empty()
    );
    assert!(
        findings(
            &review,
            &[("LIBSQLITE3_SYS_USE_PKG_CONFIG".to_owned(), "0".to_owned())]
        )
        .unwrap()
        .is_empty()
    );
}

#[test]
fn reviewed_source_collection_hashes_complete_files_and_rejects_missing_evidence() {
    let scratch = crate::scratch::Scratch::new();
    for path in [
        "build.rs",
        "sqlite3/sqlite3.c",
        "sqlite3/sqlite3.h",
        "sqlite3/sqlite3ext.h",
    ] {
        scratch.write(path, "abc");
    }
    let metadata=json!({"packages":[{"name":"libsqlite3-sys","manifest_path":scratch.path().join("Cargo.toml").to_str().unwrap()}]}).to_string();
    let hashes = sqlite_native::source_hashes(&metadata).unwrap();
    assert_eq!(hashes.len(), 4);
    assert!(
        hashes
            .values()
            .all(|hash| hash == "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
    );
    std::fs::remove_file(scratch.path().join("sqlite3/sqlite3.h")).unwrap();
    assert!(sqlite_native::source_hashes(&metadata).is_err());
}

#[test]
fn cargo_configuration_cannot_hide_target_or_forced_native_overrides() {
    let explicit=sqlite_native::configuration_environment("[env]\nCFLAGS = { value = '-DSQLITE_ENABLE_STMT_SCANSTATUS', force = true }\nLIBSQLITE3_SYS_USE_PKG_CONFIG = '0'\n").unwrap();
    assert!(!findings(&record(), &explicit).unwrap().is_empty());
    let target = sqlite_native::configuration_environment(
        "[target.x86_64-unknown-linux-gnu]\nrustflags = ['--cfg', 'feature=\"session\"']\n",
    )
    .unwrap();
    assert!(!findings(&record(), &target).unwrap().is_empty());
    let ordinary =
        sqlite_native::configuration_environment("[env]\nAWS_LC_SYS_USE_SYSTEM = '0'\n").unwrap();
    assert!(findings(&record(), &ordinary).unwrap().is_empty());
    assert!(sqlite_native::configuration_environment("[env]\nCFLAGS = 42\n").is_err());
}

#[test]
fn sqlite_source_scope_runs_before_other_checks_and_workspace_build_steps() {
    let steps = crate::check::plan();
    assert_eq!(steps.first(), Some(&crate::check::Step::SupplyChainGuard));
    assert_eq!(steps.get(1), Some(&crate::check::Step::SqliteNative));
}

#[test]
fn duplicate_reachable_metadata_id_is_rejected_instead_of_choosing_a_package() {
    let review = record();
    let mut evidence = Evidence::new(&review);
    let target = "x86_64-unknown-linux-gnu";
    let mut graph: Value = serde_json::from_str(evidence.graphs.get(target).unwrap()).unwrap();
    let root = graph["packages"]
        .as_array()
        .unwrap()
        .first()
        .unwrap()
        .clone();
    graph["packages"].as_array_mut().unwrap().push(root);
    evidence.graphs.insert(target.to_owned(), graph.to_string());
    assert!(evidence.findings(&review, &[]).is_err());
}

#[test]
fn unapproved_expired_changed_or_incomplete_scope_never_passes() {
    let review = record();
    let evidence = Evidence::new(&review);
    let mut proposed = review.clone();
    proposed["status"] = "proposed".into();
    assert!(!evidence.findings(&proposed, &[]).unwrap().is_empty());
    let mut expired = review.clone();
    expired["revisit_by"] = "2026-11-01".into();
    assert!(!evidence.findings(&expired, &[]).unwrap().is_empty());
    expired["revisit_by"] = "2027-13-01".into();
    assert!(evidence.findings(&expired, &[]).is_err());
    let mut changed = Evidence::new(&review);
    changed
        .hashes
        .insert("sqlite3/sqlite3.c".to_owned(), "0".repeat(64));
    assert!(!changed.findings(&review, &[]).unwrap().is_empty());
    let mut changed = Evidence::new(&review);
    changed.locked.first_mut().unwrap().checksum = Some("0".repeat(64));
    assert!(!changed.findings(&review, &[]).unwrap().is_empty());
    let mut changed = Evidence::new(&review);
    let target = "x86_64-unknown-linux-gnu";
    let mut graph: Value = serde_json::from_str(changed.graphs.get(target).unwrap()).unwrap();
    for node in graph["resolve"]["nodes"].as_array_mut().unwrap() {
        if node["id"] == "sys" {
            node["features"]
                .as_array_mut()
                .unwrap()
                .push("session".into());
        }
    }
    changed.graphs.insert(target.to_owned(), graph.to_string());
    assert!(!changed.findings(&review, &[]).unwrap().is_empty());
    changed.graphs.remove(target);
    assert!(changed.findings(&review, &[]).is_err());
    changed.graphs.insert(target.to_owned(), "{}".to_owned());
    assert!(changed.findings(&review, &[]).is_err());
}

#[cfg(unix)]
#[test]
fn native_source_symlink_is_rejected() {
    let scratch = crate::scratch::Scratch::new();
    for path in ["build.rs", "sqlite3/sqlite3.c", "sqlite3/sqlite3.h"] {
        scratch.write(path, "abc");
    }
    std::os::unix::fs::symlink(
        scratch.path().join("sqlite3/sqlite3.h"),
        scratch.path().join("sqlite3/sqlite3ext.h"),
    )
    .unwrap();
    let metadata=json!({"packages":[{"name":"libsqlite3-sys","manifest_path":scratch.path().join("Cargo.toml").to_str().unwrap()}]}).to_string();
    assert!(sqlite_native::source_hashes(&metadata).is_err());
}
