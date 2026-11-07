// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Checks the approved target exclusions and disabled native shim feature.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

/// Checks full, target-filtered Cargo metadata for the approved native restrictions.
///
/// # Errors
/// Returns an error when the metadata cannot establish a complete reachable graph.
pub fn violations(metadata: &str) -> Result<Vec<String>, String> {
    check(metadata, false)
}

/// Rejects credential-native scope changes in resolved host/target occurrences.
///
/// # Errors
/// Returns malformed or incomplete graph evidence.
pub fn resolved_credential_violations(metadata: &str) -> Result<Vec<String>, String> {
    check(metadata, true)
}

fn check(metadata: &str, resolved: bool) -> Result<Vec<String>, String> {
    let document: Value = serde_json::from_str(metadata).map_err(|error| error.to_string())?;
    let packages = indexed(array(&document, "packages")?)?;
    let resolve = document
        .get("resolve")
        .ok_or("missing metadata resolve graph")?;
    let nodes = indexed(array(resolve, "nodes")?)?;
    let roots = array(&document, "workspace_members")?;
    if roots.is_empty() {
        return Err("cargo metadata has no workspace members".to_owned());
    }
    let mut pending = roots.iter().map(string).collect::<Result<Vec<_>, _>>()?;
    let mut visited = BTreeSet::new();
    let mut findings = Vec::new();
    while let Some(id) = pending.pop() {
        if !visited.insert(id) {
            continue;
        }
        let package = packages
            .get(id)
            .ok_or_else(|| format!("missing package {id}"))?;
        let node = nodes
            .get(id)
            .ok_or_else(|| format!("missing resolve node {id}"))?;
        let name = field(package, "name")?;
        let version = field(package, "version")?;
        let features = array(node, "features")?
            .iter()
            .map(string)
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(finding) = crate::ort_scope::violation(name, version, &features) {
            findings.push(finding);
        }
        if matches!(
            name,
            "android-activity"
                | "objc-sys"
                | "android-properties"
                | "core-graphics"
                | "dispatch"
                | "system-configuration"
                | "system-configuration-sys"
                | "memoffset"
                | "uds_windows"
        ) {
            findings.push(format!("{name} {version} must be absent from the supported target graph; new owner approval is required"));
        }
        if name == "rust_decimal" && !features.is_empty() {
            findings.push(format!("{name} {version} must keep all default and optional features disabled; source/purity review is required"));
        }
        if name == "send_wrapper" {
            findings.push(format!("{name} {version} must be absent from the supported target graph; its unresolved Pin/drop contract requires a new source review and owner approval"));
        }
        if name == "wayland-backend" && features.contains(&"log") {
            findings.push(format!("{name} {version} feature log enables native C shims; new owner approval is required"));
        }
        if let Some((reviewed_version, reviewed_features)) = credential_scope(name)
            && (version != reviewed_version
                || features.iter().copied().collect::<BTreeSet<_>>()
                    != reviewed_features.iter().copied().collect())
        {
            findings.push(format!("{name} {version}: credential source version/features differ from the exact reviewed scope; fresh source/graph review is required before adoption"));
        }
        if resolved
            && name == "windows-sys"
            && features.iter().any(|feature| {
                matches!(
                    *feature,
                    "Win32_NetworkManagement"
                        | "Win32_NetworkManagement_IpHelper"
                        | "Win32_Security_Authorization"
                )
            })
        {
            findings.push(format!("{name} {version}: off-target zbus Windows transport/NSS helper features must remain absent from resolved host/target occurrences"));
        }
        for dependency in array(node, "deps")? {
            field(dependency, "name")?;
            let dependency_id = field(dependency, "pkg")?;
            let dependency_package = packages
                .get(dependency_id)
                .ok_or_else(|| format!("missing package {dependency_id}"))?;
            if let Some(finding) =
                crate::ort_scope::edge_violation(name, field(dependency_package, "name")?)
            {
                findings.push(finding);
            }
            pending.push(dependency_id);
        }
    }
    Ok(findings)
}

fn credential_scope(name: &str) -> Option<(&'static str, &'static [&'static str])> {
    Some(match name {
        "keyring-core" => ("1.0.1", &[]),
        "windows-native-keyring-store" => ("1.1.0", &[]),
        "secret-service" => (
            "5.2.0",
            &["crypto-rust", "rt-tokio", "rt-tokio-crypto-rust"],
        ),
        "zbus" => ("5.19.0", &["blocking-api", "tokio"]),
        "signal-hook-registry" => ("1.4.8", &[]),
        "cmov" => ("0.5.4", &[]),
        "event-listener" => ("5.4.2", &["default", "parking", "std"]),
        "num-bigint" => ("0.4.8", &["std"]),
        _ => return None,
    })
}
fn indexed(values: &[Value]) -> Result<BTreeMap<&str, &Value>, String> {
    let mut indexed = BTreeMap::new();
    for value in values {
        let id = field(value, "id")?;
        if indexed.insert(id, value).is_some() {
            return Err(format!("duplicate metadata ID {id}"));
        }
    }
    Ok(indexed)
}

fn array<'a>(value: &'a Value, key: &str) -> Result<&'a [Value], String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("missing or malformed metadata array {key}"))
}

fn field<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value
        .get(key)
        .ok_or_else(|| format!("missing metadata field {key}"))
        .and_then(string)
}

fn string(value: &Value) -> Result<&str, String> {
    value
        .as_str()
        .filter(|text| !text.is_empty())
        .ok_or_else(|| "metadata string is empty or malformed".to_owned())
}
