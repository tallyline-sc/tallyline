// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Public metadata fixtures for the reviewed build-time inventory.

use std::collections::BTreeMap;

use crate::build_code::violations;
use serde_json::json;

#[test]
fn an_unreviewed_build_script_is_rejected() {
    let metadata = json!({
        "packages": [{"id":"script", "name":"script", "version":"1.0.0",
            "source":"registry+https://github.com/rust-lang/crates.io-index",
            "targets":[{"kind":["custom-build"],"src_path":"/cache/script/build.rs"}]}],
        "workspace_members":["script"],
        "resolve":{"nodes":[{"id":"script","deps":[],"features":[]}]}
    });
    let registry = json!({"schema_version":1,"entries":[],"contexts":[]});
    assert!(
        !violations(
            &metadata.to_string(),
            "",
            &registry.to_string(),
            &BTreeMap::new()
        )
        .unwrap()
        .is_empty()
    );
}

#[test]
fn an_exact_reviewed_script_is_allowed() {
    let metadata = json!({
        "packages": [{"id":"script", "name":"script", "version":"1.0.0",
            "source":"registry+https://github.com/rust-lang/crates.io-index",
            "targets":[{"kind":["custom-build"],"src_path":"/cache/script/build.rs"}]}],
        "workspace_members":["script"],
        "resolve":{"nodes":[{"id":"script","deps":[],"features":[],"features":[]}]}
    });
    let registry = json!({"schema_version":1,"entries":[{
        "name":"script","version":"1.0.0","kind":"custom-build",
        "archive_sha256":"a".repeat(64),"source_sha256":"b".repeat(64),
        "behaviour":"Emits target cfgs.","review_scope":"Complete script read."
    }],"contexts":[]});
    let lock = format!(
        "[[package]]\nname = \"script\"\nversion = \"1.0.0\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"{}\"\n",
        "a".repeat(64)
    );
    let hashes = BTreeMap::from([("/cache/script/build.rs".to_owned(), "b".repeat(64))]);
    assert!(
        violations(&metadata.to_string(), &lock, &registry.to_string(), &hashes)
            .unwrap()
            .is_empty()
    );
}

fn gl_fixture() -> (
    serde_json::Value,
    String,
    serde_json::Value,
    BTreeMap<String, String>,
) {
    let generator = json!({"id":"generator","name":"gl_generator","version":"0.14.0",
        "source":"registry+https://github.com/rust-lang/crates.io-index","targets":[{"kind":["lib"]}]});
    let mut packages = vec![generator];
    let mut nodes = vec![json!({"id":"generator","deps":[],"features":[]})];
    let mut entries = Vec::new();
    let mut consumers = Vec::new();
    let mut hashes = BTreeMap::new();
    let mut lock = format!(
        "[[package]]\nname = \"gl_generator\"\nversion = \"0.14.0\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"{}\"\n",
        "c".repeat(64)
    );
    for (name, version) in [
        ("glutin_egl_sys", "0.7.1"),
        ("glutin_glx_sys", "0.6.1"),
        ("glutin_wgl_sys", "0.6.1"),
    ] {
        let path = format!("/cache/{name}/build.rs");
        packages.push(json!({"id":name,"name":name,"version":version,
            "source":"registry+https://github.com/rust-lang/crates.io-index",
            "targets":[{"kind":["custom-build"],"src_path":path}]}));
        nodes.push(json!({"id":name,"deps":[{"name":"renamed_generator","pkg":"generator"}],"features":[]}));
        entries.push(json!({"name":name,"version":version,"kind":"custom-build",
            "archive_sha256":"a".repeat(64),"source_sha256":"b".repeat(64),
            "behaviour":"Uses reviewed non-global generator.","review_scope":"Complete script read."}));
        consumers.push(json!({"name":name,"version":version,"archive_sha256":"a".repeat(64),"source_sha256":"b".repeat(64)}));
        hashes.insert(path, "b".repeat(64));
        lock += &format!(
            "\n[[package]]\nname = \"{name}\"\nversion = \"{version}\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"{}\"\n",
            "a".repeat(64)
        );
    }
    (
        json!({"packages":packages,"workspace_members":["glutin_egl_sys"],"resolve":{"nodes":nodes}}),
        lock,
        json!({"schema_version":1,"entries":entries,"contexts":[{"name":"gl_generator","version":"0.14.0","archive_sha256":"c".repeat(64),"consumers":consumers}]}),
        hashes,
    )
}

#[test]
fn an_unknown_gl_generator_consumer_is_rejected() {
    let (mut metadata, lock, registry, hashes) = gl_fixture();
    metadata["packages"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"new","name":"unreviewed","version":"1.0.0","source":null,"targets":[{"kind":["lib"]}]}));
    metadata["resolve"]["nodes"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"new","deps":[{"name":"gl_generator","pkg":"generator"}],"features":[]}));
    assert!(
        !violations(&metadata.to_string(), &lock, &registry.to_string(), &hashes)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn the_reviewed_gl_context_is_allowed() {
    let (metadata, lock, registry, hashes) = gl_fixture();
    assert!(
        violations(&metadata.to_string(), &lock, &registry.to_string(), &hashes)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn changed_target_identity_or_code_needs_review() {
    let (metadata, lock, registry, hashes) = gl_fixture();
    for case in 0..7 {
        let mut metadata = metadata.clone();
        let mut registry = registry.clone();
        let mut hashes = hashes.clone();
        let mut lock = lock.clone();
        match case {
            0 => metadata["packages"][1]["version"] = json!("0.7.2"),
            1 => metadata["packages"][1]["source"] = json!("registry+https://untrusted.invalid"),
            2 => lock = lock.replace(&"a".repeat(64), &"d".repeat(64)),
            3 => {
                hashes.clear();
            }
            4 => {
                registry["entries"].as_array_mut().unwrap().remove(0);
            }
            5 => metadata["packages"][1]["targets"][0]["kind"] = json!(["proc-macro"]),
            _ => {
                metadata["packages"][1]["targets"] = json!([]);
            }
        }
        let result = violations(&metadata.to_string(), &lock, &registry.to_string(), &hashes);
        assert!(
            result.is_err() || !result.unwrap().is_empty(),
            "case {case}"
        );
    }
}

#[test]
fn gl_selection_cannot_be_changed_by_only_updating_the_general_review() {
    let (metadata, lock, mut registry, mut hashes) = gl_fixture();
    registry["entries"][0]["source_sha256"] = json!("d".repeat(64));
    hashes.insert("/cache/glutin_egl_sys/build.rs".to_owned(), "d".repeat(64));
    let findings =
        violations(&metadata.to_string(), &lock, &registry.to_string(), &hashes).unwrap();
    assert!(
        findings
            .iter()
            .any(|finding| finding.contains("GL generator selection source changed"))
    );
}

#[test]
fn gl_version_consumer_set_and_archive_are_pinned_independently() {
    let (metadata, lock, registry, hashes) = gl_fixture();
    for case in 0..5 {
        let mut metadata = metadata.clone();
        let mut registry = registry.clone();
        let mut lock = lock.clone();
        match case {
            0 => {
                metadata["packages"][0]["version"] = json!("0.15.0");
                lock = lock.replace("0.14.0", "0.15.0");
            }
            1 => {
                metadata["packages"][1]["version"] = json!("0.7.2");
                registry["entries"][0]["version"] = json!("0.7.2");
                lock = lock.replace("0.7.1", "0.7.2");
            }
            2 => lock = lock.replace(&"c".repeat(64), &"d".repeat(64)),
            3 => registry["contexts"] = json!([]),
            _ => metadata["resolve"]["nodes"][1]["deps"] = json!([]),
        }
        let result = violations(&metadata.to_string(), &lock, &registry.to_string(), &hashes);
        assert!(
            result.is_err() || !result.unwrap().is_empty(),
            "case {case}"
        );
    }
}

#[test]
fn malformed_inputs_do_not_erase_review_obligations() {
    let (metadata, lock, registry, hashes) = gl_fixture();
    for case in 0..13 {
        let mut metadata = metadata.clone();
        let mut registry = registry.clone();
        match case {
            0 => {
                metadata.as_object_mut().unwrap().remove("packages");
            }
            1 => metadata["packages"][1]["targets"] = json!(null),
            2 => metadata["packages"][1]["targets"][0]["kind"] = json!([null]),
            3 => metadata["workspace_members"] = json!([]),
            4 => metadata["resolve"] = json!(null),
            5 => metadata["resolve"]["nodes"][0]["id"] = json!("unknown"),
            6 => metadata["resolve"]["nodes"][1]["deps"][0]["pkg"] = json!("unknown"),
            7 => metadata["resolve"]["nodes"][1]["deps"] = json!(null),
            8 => {
                let duplicate = registry["entries"][0].clone();
                registry["entries"].as_array_mut().unwrap().push(duplicate);
            }
            9 => registry["entries"][0]["source_sha256"] = json!("bad"),
            10 => registry["entries"][0]["review_scope"] = json!(""),
            11 => registry["schema_version"] = json!(2),
            _ => metadata["packages"][1]["targets"][0]["kind"] = json!([]),
        }
        assert!(
            violations(&metadata.to_string(), &lock, &registry.to_string(), &hashes).is_err(),
            "case {case}"
        );
    }
}
