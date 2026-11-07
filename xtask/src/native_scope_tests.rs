// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Public metadata fixtures for the approved native restrictions.

use crate::native::violations;
use serde_json::{Value, json};

#[test]
fn secret_service_other_target_dependencies_cannot_be_activated_even_when_renamed() {
    for (name, version) in [
        ("memoffset", "0.9.1"),
        ("uds_windows", "1.2.1"),
        ("memoffset", "99.0.0"),
        ("uds_windows", "99.0.0"),
    ] {
        let input = metadata(name, version, &[], true);
        assert!(
            violations(&input.to_string())
                .unwrap()
                .iter()
                .any(|finding| finding.contains(name))
        );
        let excluded = metadata(name, version, &[], false);
        assert!(violations(&excluded.to_string()).unwrap().is_empty());
    }
}

fn metadata(name: &str, version: &str, features: &[&str], reachable: bool) -> Value {
    json!({
        "packages": [
            {"id": "root", "name": "tallyline-app", "version": "0.0.0"},
            {"id": "restricted", "name": name, "version": version}
        ],
        "workspace_members": ["root"],
        "resolve": {"nodes": [
            {"id": "root", "features": [], "deps": if reachable {
                vec![json!({"name": "renamed", "pkg": "restricted"})]
            } else { vec![] }},
            {"id": "restricted", "features": features, "deps": []}
        ]}
    })
}

#[test]
fn enabled_wayland_log_is_rejected_through_a_renamed_dependency() {
    let input = metadata("wayland-backend", "0.3.17", &["log"], true);
    assert!(
        violations(&input.to_string())
            .unwrap()
            .iter()
            .any(|finding| finding.contains("wayland-backend 0.3.17") && finding.contains("log"))
    );
}

#[test]
fn excluded_packages_and_inactive_wayland_log_are_allowed() {
    for (name, version, features, reachable) in [
        ("wayland-backend", "0.3.17", vec!["client_system"], true),
        ("wayland-backend", "0.3.17", vec!["log"], false),
        ("android-activity", "0.6.1", vec!["game-activity"], false),
        ("objc-sys", "0.3.5", vec!["std"], false),
    ] {
        let input = metadata(name, version, &features, reachable);
        assert!(violations(&input.to_string()).unwrap().is_empty(), "{name}");
    }
}

#[test]
fn mobile_native_packages_leaking_into_a_supported_target_are_rejected() {
    for (name, version) in [("android-activity", "0.6.1"), ("objc-sys", "0.3.5")] {
        let input = metadata(name, version, &[], true);
        assert!(
            violations(&input.to_string())
                .unwrap()
                .iter()
                .any(|finding| finding.contains(name)),
            "{name}"
        );
    }
}

#[test]
fn transitive_native_activation_is_rejected_from_every_workspace_root() {
    let mut input = metadata("wayland-backend", "0.3.17", &["log"], true);
    input["packages"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id": "second", "name": "xtask", "version": "0.0.0"}));
    input["workspace_members"] = json!(["second"]);
    input["resolve"]["nodes"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id": "second", "features": [], "deps": [{"name": "app", "pkg": "root"}]}));
    assert_eq!(violations(&input.to_string()).unwrap().len(), 1);
}

#[test]
fn malformed_metadata_cannot_become_an_empty_selection() {
    for input in [
        "not json",
        "{}",
        r#"{"packages":[],"workspace_members":[],"resolve":{"nodes":[]}}"#,
    ] {
        assert!(violations(input).is_err(), "{input}");
    }
    let good = metadata("wayland-backend", "0.3.17", &[], true);
    for path in [
        "/packages/1/name",
        "/packages/1/version",
        "/packages/1/id",
        "/workspace_members/0",
        "/resolve/nodes/0/id",
        "/resolve/nodes/1/features",
        "/resolve/nodes/0/deps",
        "/resolve/nodes/0/deps/0/pkg",
        "/resolve/nodes/0/deps/0/name",
    ] {
        let mut input = good.clone();
        *input.pointer_mut(path).unwrap() = Value::Null;
        assert!(violations(&input.to_string()).is_err(), "{path}");
    }
}

#[test]
fn missing_graph_nodes_and_duplicate_ids_are_rejected() {
    let good = metadata("wayland-backend", "0.3.17", &[], true);
    for path in ["/packages", "/resolve/nodes"] {
        let mut input = good.clone();
        input
            .pointer_mut(path)
            .unwrap()
            .as_array_mut()
            .unwrap()
            .pop();
        assert!(violations(&input.to_string()).is_err(), "{path}");
        let mut input = good.clone();
        let values = input.pointer_mut(path).unwrap().as_array_mut().unwrap();
        values.push(values[0].clone());
        assert!(violations(&input.to_string()).is_err(), "{path}");
    }
}

#[test]
fn pure_rust_links_markers_do_not_imply_native_code() {
    for (name, version, links) in [
        ("rayon-core", "1.13.0", "rayon-core"),
        ("wasm-bindgen-shared", "0.2.130", "wasm_bindgen"),
    ] {
        let mut input = metadata(name, version, &[], true);
        input["packages"][1]["links"] = json!(links);
        assert!(violations(&input.to_string()).unwrap().is_empty());
    }
}

#[test]
fn declared_features_do_not_activate_native_shims() {
    let mut input = metadata("wayland-backend", "0.3.17", &[], true);
    input["packages"][1]["features"] = json!({"log": ["dep:log"]});
    assert!(violations(&input.to_string()).unwrap().is_empty());
}

#[test]
fn malformed_active_features_and_missing_workspace_roots_fail_closed() {
    let good = metadata("wayland-backend", "0.3.17", &[], true);
    for features in [json!("log"), json!([null]), json!([42]), json!([""])] {
        let mut input = good.clone();
        input["resolve"]["nodes"][1]["features"] = features;
        assert!(violations(&input.to_string()).is_err());
    }
    let mut input = good;
    input["workspace_members"] = json!(["missing"]);
    assert!(violations(&input.to_string()).is_err());
}

#[test]
fn packages_excluded_from_the_resolve_graph_are_allowed() {
    for (name, version) in [("android-activity", "0.6.1"), ("objc-sys", "0.3.5")] {
        let mut input = metadata(name, version, &[], false);
        input["resolve"]["nodes"].as_array_mut().unwrap().pop();
        assert!(violations(&input.to_string()).unwrap().is_empty());
    }
}

#[test]
fn android_properties_must_stay_outside_supported_graphs() {
    for version in ["0.2.2", "0.3.0"] {
        let excluded = metadata("android-properties", version, &[], false);
        assert!(violations(&excluded.to_string()).unwrap().is_empty());
        let reachable = metadata("android-properties", version, &["unexpected"], true);
        assert!(
            violations(&reachable.to_string())
                .unwrap()
                .iter()
                .any(|finding| finding.contains("android-properties"))
        );
    }
}

#[test]
fn apple_ffi_bindings_must_stay_outside_supported_graphs() {
    for (name, version) in [
        ("core-graphics", "0.23.2"),
        ("dispatch", "0.2.0"),
        ("core-graphics", "99.0.0"),
        ("dispatch", "99.0.0"),
    ] {
        let excluded = metadata(name, version, &[], false);
        assert!(
            violations(&excluded.to_string()).unwrap().is_empty(),
            "{name}"
        );
        let reachable = metadata(name, version, &["unexpected"], true);
        assert!(
            violations(&reachable.to_string())
                .unwrap()
                .iter()
                .any(|finding| finding.contains(name)),
            "{name}"
        );
    }
}

#[test]
fn apple_system_proxy_bindings_must_remain_absent_even_through_renamed_edges() {
    for (name, version) in [
        ("system-configuration", "0.7.0"),
        ("system-configuration-sys", "0.6.0"),
        ("system-configuration", "99.0.0"),
        ("system-configuration-sys", "99.0.0"),
    ] {
        let excluded = metadata(name, version, &[], false);
        assert!(violations(&excluded.to_string()).unwrap().is_empty());
        let reachable = metadata(name, version, &["unexpected"], true);
        assert!(
            violations(&reachable.to_string())
                .unwrap()
                .iter()
                .any(|finding| finding.contains(name)),
            "{name} {version}"
        );
    }
}

#[test]
fn decimal_runtime_scope_rejects_every_unreviewed_feature() {
    assert!(
        violations(&metadata("rust_decimal", "1.43.0", &[], true).to_string())
            .unwrap()
            .is_empty()
    );
    for feature in ["std", "serde", "db-postgres"] {
        assert!(
            violations(&metadata("rust_decimal", "1.43.0", &[feature], true).to_string())
                .unwrap()
                .iter()
                .any(|finding| finding.contains("rust_decimal")),
            "{feature}"
        );
    }
}

#[test]
fn send_wrapper_activation_is_rejected_for_every_version_and_workspace_root() {
    for version in ["0.6.0", "99.0.0"] {
        let excluded = metadata("send_wrapper", version, &[], false);
        assert!(violations(&excluded.to_string()).unwrap().is_empty());
        let mut reachable = metadata("send_wrapper", version, &["futures"], true);
        reachable["packages"].as_array_mut().unwrap().push(json!({
            "id": "second", "name": "xtask", "version": "0.0.0"
        }));
        reachable["workspace_members"] = json!(["second"]);
        reachable["resolve"]["nodes"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "id": "second", "features": [], "deps": [{"name": "renamed_root", "pkg": "root"}]
            }));
        assert!(
            violations(&reachable.to_string())
                .unwrap()
                .iter()
                .any(|finding| { finding.contains("send_wrapper") && finding.contains(version) })
        );
    }
}

#[test]
fn credential_dependencies_reject_unreviewed_versions_and_optional_features() {
    for (name, version, features) in [
        ("keyring-core", "1.0.1", vec![]),
        ("windows-native-keyring-store", "1.1.0", vec![]),
        (
            "secret-service",
            "5.2.0",
            vec!["crypto-rust", "rt-tokio", "rt-tokio-crypto-rust"],
        ),
        ("zbus", "5.19.0", vec!["blocking-api", "tokio"]),
        ("signal-hook-registry", "1.4.8", vec![]),
        ("cmov", "0.5.4", vec![]),
        ("event-listener", "5.4.2", vec!["default", "parking", "std"]),
        ("num-bigint", "0.4.8", vec!["std"]),
    ] {
        assert!(
            violations(&metadata(name, version, &features, true).to_string())
                .unwrap()
                .is_empty(),
            "{name}"
        );
        for (version, features) in [("99.0.0", features.clone()), (version, vec!["unreviewed"])] {
            assert!(
                violations(&metadata(name, version, &features, true).to_string())
                    .unwrap()
                    .iter()
                    .any(|finding| finding.contains(name)),
                "{name} {version}"
            );
        }
    }
}

#[test]
fn off_target_zbus_windows_helpers_cannot_activate_in_resolved_target_units() {
    for feature in [
        "Win32_NetworkManagement",
        "Win32_NetworkManagement_IpHelper",
        "Win32_Security_Authorization",
    ] {
        let input = metadata("windows-sys", "0.61.2", &[feature], true).to_string();
        assert!(violations(&input).unwrap().is_empty());
        assert!(
            !crate::native::resolved_credential_violations(&input)
                .unwrap()
                .is_empty(),
            "{feature}"
        );
    }
}

#[test]
fn ort_native_scope_rejects_features_outside_the_exact_accepted_binding() {
    for (name, index) in [("ort", 1), ("ort-sys", 2)] {
        for feature in [
            "default",
            "download-binaries",
            "copy-dylibs",
            "cuda",
            "training",
            "unknown",
        ] {
            let mut input = runtime_metadata();
            input["resolve"]["nodes"][index]["features"]
                .as_array_mut()
                .unwrap()
                .push(json!(feature));
            assert!(
                violations(&input.to_string())
                    .unwrap()
                    .iter()
                    .any(|finding| finding.contains(name)),
                "{name}/{feature}"
            );
        }
    }
}
fn runtime_metadata() -> Value {
    let api: Vec<_> = (17..=31).map(|api| format!("api-{api}")).collect();
    let mut wrapper = api.clone();
    wrapper.extend(["std", "load-dynamic", "preload-dylibs"].map(str::to_owned));
    let mut system = api;
    system.extend(["std", "disable-linking"].map(str::to_owned));
    json!({
        "packages": [
            {"id": "root", "name": "tallyline-adapter-ocr", "version": "0.1.0"},
            {"id": "wrapper", "name": "ort", "version": "2.0.0-rc.14"},
            {"id": "system", "name": "ort-sys", "version": "2.0.0-rc.14"}
        ],
        "workspace_members": ["root"],
        "resolve": {"nodes": [
            {"id": "root", "features": [], "deps": [{"name": "renamed", "pkg": "wrapper"}]},
            {"id": "wrapper", "features": wrapper, "deps": [{"name": "renamed_sys", "pkg": "system"}]},
            {"id": "system", "features": system, "deps": []}
        ]}
    })
}

#[test]
fn ort_native_scope_keeps_the_binding_private_even_when_renamed() {
    let good = runtime_metadata();
    assert!(violations(&good.to_string()).unwrap().is_empty());
    let mut outside = good.clone();
    outside["packages"][0]["name"] = json!("tallyline-app");
    assert!(
        violations(&outside.to_string())
            .unwrap()
            .iter()
            .any(|finding| finding.contains("only tallyline-adapter-ocr"))
    );
    let mut bypass = good;
    bypass["resolve"]["nodes"][0]["deps"]
        .as_array_mut()
        .unwrap()
        .push(json!({"name": "bypass", "pkg": "system"}));
    assert!(
        violations(&bypass.to_string())
            .unwrap()
            .iter()
            .any(|finding| finding.contains("only ort"))
    );
}

#[test]
fn ort_native_scope_requires_the_reviewed_version_and_complete_feature_closure() {
    for index in [1, 2] {
        for version in ["2.0.0-rc.13", "2.0.0-rc.15", "2.0.0"] {
            let mut input = runtime_metadata();
            input["packages"][index]["version"] = json!(version);
            assert!(
                !violations(&input.to_string()).unwrap().is_empty(),
                "{version}"
            );
        }
        let good = runtime_metadata();
        let features = good["resolve"]["nodes"][index]["features"]
            .as_array()
            .unwrap();
        for removed in 0..features.len() {
            let mut input = good.clone();
            input["resolve"]["nodes"][index]["features"]
                .as_array_mut()
                .unwrap()
                .remove(removed);
            assert!(
                !violations(&input.to_string()).unwrap().is_empty(),
                "removed {removed}"
            );
        }
        let mut duplicate = good;
        duplicate["resolve"]["nodes"][index]["features"]
            .as_array_mut()
            .unwrap()
            .push(json!("std"));
        assert!(!violations(&duplicate.to_string()).unwrap().is_empty());
    }
}

#[test]
fn owned_process_job_scope_requires_exact_owner_target_version_and_features() {
    let mut input = metadata("windows-sys", "0.61.2", &["Win32_System_JobObjects"], true);
    input["packages"][0]["name"] = json!("tallyline-adapter-platform");
    input["packages"][0]["version"] = json!("0.1.0");
    input["packages"][0]["source"] = Value::Null;
    input["packages"][0]["dependencies"] = json!([{
        "name": "windows-sys", "req": "=0.61.2", "uses_default_features": false,
        "features": ["Win32_Security", "Win32_System_JobObjects", "Win32_System_Pipes", "Win32_System_IO"],
        "target": "x86_64-pc-windows-msvc",
        "kind": null, "rename": null, "optional": false, "registry": null,
        "source": "registry+https://github.com/rust-lang/crates.io-index"
    }]);
    let review: Value = serde_json::from_str(include_str!(
        "../../supply-chain/owned-process-windows.json"
    ))
    .unwrap();
    input["resolve"]["nodes"][1]["features"] = review["resolved_features"].clone();
    input["packages"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id": "link", "name": "windows-link", "version": "0.2.1"}));
    input["resolve"]["nodes"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id": "link", "features": [], "deps": []}));
    let inspect = |value: &Value, target: &str| {
        crate::owned_process_scope::violations(&value.to_string(), target).unwrap()
    };
    assert!(inspect(&input, "x86_64-pc-windows-msvc").is_empty());
    let mut additional_owner = input.clone();
    additional_owner["packages"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "id": "foreign", "name": "tallyline-app", "version": "0.0.0", "source": null,
            "dependencies": [{
                "name": "windows-sys", "req": "=0.61.2", "uses_default_features": false,
                "features": ["Win32_System_Pipes"], "target": "x86_64-pc-windows-msvc",
                "kind": null, "rename": null, "optional": false, "registry": null,
            "source": "registry+https://github.com/rust-lang/crates.io-index"
            }]
        }));
    additional_owner["workspace_members"]
        .as_array_mut()
        .unwrap()
        .push(json!("foreign"));
    assert!(
        !inspect(&additional_owner, "x86_64-pc-windows-msvc").is_empty(),
        "an additional Pipes-only owner must be rejected while the approved edge remains"
    );
    assert!(!inspect(&input, "x86_64-unknown-linux-gnu").is_empty());
    for feature in ["Win32_Security", "Win32_System_Pipes", "Win32_System_IO"] {
        for owner in ["tallyline-app", "tallyline-adapter-platform"] {
            let mut changed = additional_owner.clone();
            changed["packages"][3]["name"] = json!(owner);
            changed["packages"][3]["version"] = json!("0.1.0");
            changed["packages"][3]["dependencies"][0]["features"] = json!([feature]);
            changed["packages"][3]["dependencies"][0]["target"] = json!("cfg(windows)");
            assert!(
                !inspect(&changed, "x86_64-pc-windows-msvc").is_empty(),
                "{feature}: {owner}"
            );
        }
    }
    for (field, bad) in [
        ("target", json!("cfg(windows)")),
        ("req", json!("=0.61.1")),
        ("uses_default_features", json!(true)),
        (
            "features",
            json!(["Win32_System_JobObjects", "Win32_Security_Authorization"]),
        ),
        ("source", json!("git+https://example.invalid/sdk")),
    ] {
        let mut changed = input.clone();
        changed["packages"][0]["dependencies"][0][field] = bad;
        assert!(
            !inspect(&changed, "x86_64-pc-windows-msvc").is_empty(),
            "{field}"
        );
    }
    for owner in ["renamed-platform", "zbus", "tallyline-app"] {
        let mut changed = input.clone();
        changed["packages"][0]["name"] = json!(owner);
        assert!(
            !inspect(&changed, "x86_64-pc-windows-msvc").is_empty(),
            "{owner}"
        );
    }
    let mut foreign_owner = input.clone();
    foreign_owner["packages"][0]["source"] =
        json!("registry+https://github.com/rust-lang/crates.io-index");
    assert!(!inspect(&foreign_owner, "x86_64-pc-windows-msvc").is_empty());
    let mut nonmember_owner = input.clone();
    nonmember_owner["workspace_members"] = json!(["restricted"]);
    assert!(!inspect(&nonmember_owner, "x86_64-pc-windows-msvc").is_empty());
    let mut wrong_version = input.clone();
    wrong_version["packages"][1]["version"] = json!("99.0.0");
    assert!(!inspect(&wrong_version, "x86_64-pc-windows-msvc").is_empty());
    let mut extra = input.clone();
    extra["resolve"]["nodes"][1]["features"] = json!(["Win32_System_JobObjects", "unexpected"]);
    assert!(!inspect(&extra, "x86_64-pc-windows-msvc").is_empty());
}
