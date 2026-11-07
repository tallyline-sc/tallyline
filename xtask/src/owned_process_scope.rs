// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact declaration and resolved-feature boundary for the owned-process primitive.

use serde_json::Value;
use std::collections::BTreeSet;

const REVIEW: &str = include_str!("../../supply-chain/owned-process-windows.json");

/// Checks complete target-resolved metadata against the exact owned-process Windows packet.
///
/// # Errors
/// Returns malformed graph or review evidence instead of treating absence as approval.
pub fn violations(metadata: &str, target: &str) -> Result<Vec<String>, String> {
    let graph: Value = serde_json::from_str(metadata).map_err(|error| error.to_string())?;
    let review: Value = serde_json::from_str(REVIEW).map_err(|error| error.to_string())?;
    let packages = array(&graph, "packages")?;
    let nodes = array(
        graph.get("resolve").ok_or("missing resolve graph")?,
        "nodes",
    )?;
    let workspace = strings(array(&graph, "workspace_members")?)?;
    let mut findings = Vec::new();
    let declared = strings(array(&review, "declared_features")?)?;
    let resolved = strings(array(&review, "resolved_features")?)?;
    let declarations = array(&review, "permitted_declarations")?;
    let mut approved_edges = BTreeSet::new();
    for owner in packages {
        let Some(dependencies) = owner.get("dependencies").and_then(Value::as_array) else {
            continue;
        };
        for dependency in dependencies {
            if text(dependency, "name")? != "windows-sys" {
                continue;
            }
            let features = strings(array(dependency, "features")?)?;
            if features.is_disjoint(&declared) {
                continue;
            }
            let mut permitted = false;
            for declaration in declarations {
                if declaration_matches(owner, dependency, &workspace, declaration)? {
                    permitted = true;
                    break;
                }
            }
            if !permitted {
                findings.push("owned-process packet feature has an unapproved declaration owner/target/version/features".to_owned());
                continue;
            }
            if !features.contains("Win32_System_JobObjects") {
                continue;
            }
            let node = nodes
                .iter()
                .find(|node| node["id"] == owner["id"])
                .ok_or("missing declaration owner resolve node")?;
            for edge in array(node, "deps")? {
                approved_edges.insert(text(edge, "pkg")?);
            }
        }
    }
    let mut activated = false;
    for package in packages {
        if text(package, "name")? != "windows-sys" {
            continue;
        }
        let Some(node) = nodes.iter().find(|node| node["id"] == package["id"]) else {
            continue;
        };
        let features = strings(array(node, "features")?)?;
        if !features.contains("Win32_System_JobObjects") {
            continue;
        }
        activated = true;
        if target != text(&review, "target")?
            || text(package, "version")? != text(&review, "windows_sys")?
            || !approved_edges.contains(text(package, "id")?)
            || features != resolved
        {
            findings.push("owned-process resolved Windows SDK differs from the exact target/owner/version/feature profile".to_owned());
        }
    }
    if activated {
        let links = packages
            .iter()
            .filter(|package| {
                package["name"] == "windows-link"
                    && nodes.iter().any(|node| node["id"] == package["id"])
            })
            .collect::<Vec<_>>();
        if links.is_empty()
            || links
                .iter()
                .any(|package| package.get("version") != review.get("windows_link"))
        {
            findings.push(
                "owned-process windows-link differs from the exact reviewed version".to_owned(),
            );
        }
    }
    Ok(findings)
}

fn declaration_matches(
    owner: &Value,
    dependency: &Value,
    workspace: &BTreeSet<&str>,
    permitted: &Value,
) -> Result<bool, String> {
    let expected = permitted
        .get("dependency")
        .ok_or("missing permitted declaration")?;
    let fields = [
        "name",
        "source",
        "req",
        "kind",
        "rename",
        "optional",
        "uses_default_features",
        "target",
        "registry",
    ];
    Ok(owner.get("name") == permitted.get("owner")
        && owner.get("version") == permitted.get("owner_version")
        && owner.get("source") == permitted.get("owner_source")
        && permitted.get("workspace_member").and_then(Value::as_bool)
            == Some(workspace.contains(text(owner, "id")?))
        && fields
            .iter()
            .all(|field| dependency.get(field) == expected.get(field))
        && strings(array(dependency, "features")?)? == strings(array(expected, "features")?)?)
}
fn text<'a>(value: &'a Value, name: &str) -> Result<&'a str, String> {
    value
        .get(name)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("missing owned-process metadata string {name}"))
}
fn array<'a>(value: &'a Value, name: &str) -> Result<&'a [Value], String> {
    value
        .get(name)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("missing owned-process metadata array {name}"))
}
fn strings(values: &[Value]) -> Result<BTreeSet<&str>, String> {
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| "invalid owned-process feature".to_owned())
        })
        .collect()
}
