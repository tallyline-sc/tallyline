// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Bounds the property-testing dependency profile before compilation.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_json::Value;

use crate::native_assembly;

/// The adopted profile location; private proposed records are not read automatically.
pub const REVIEWED_PATH: &str = "supply-chain/property-testing.json";

const TARGETS: [&str; 2] = ["x86_64-pc-windows-msvc", "x86_64-unknown-linux-gnu"];

/// Checks a conservative graph against the adopted testing profile.
///
/// # Errors
/// Returns malformed metadata or review data.
pub fn violations(
    metadata: &str,
    lock: &str,
    review: Option<&str>,
    target: &str,
    source_trees: &BTreeMap<String, String>,
) -> Result<Vec<String>, String> {
    let document: Value = serde_json::from_str(metadata).map_err(|error| error.to_string())?;
    crate::native::violations(metadata)?;
    let reached = native_assembly::reachable(&document)?;
    let active = reached
        .keys()
        .any(|(name, version)| restricted(name, version));
    if !active {
        return if review.is_some() {
            Err("property review is stale: profile is absent".to_owned())
        } else {
            Ok(Vec::new())
        };
    }
    let Some(review) = review else {
        return Ok(vec![
            "property profile is not approved; proposed data cannot authorise execution".to_owned(),
        ]);
    };
    let review: Value = serde_json::from_str(review).map_err(|error| error.to_string())?;
    if review.get("schema") != Some(&Value::from(1))
        || native_assembly::text(&review, "status")? != "approved"
    {
        return Err("property profile is not an adopted approved record".to_owned());
    }
    native_assembly::expiry(native_assembly::text(&review, "revisit_by")?)?;
    if !TARGETS.contains(&target)
        || feature_set(native_assembly::array(&review, "targets")?)?
            != TARGETS.into_iter().collect()
    {
        return Err("unsupported property profile target".to_owned());
    }
    let packages = native_assembly::index(native_assembly::array(&document, "packages")?)?;
    let mut reviews = BTreeMap::new();
    for entry in native_assembly::array(&review, "packages")? {
        let key = (
            native_assembly::text(entry, "name")?,
            native_assembly::text(entry, "version")?,
        );
        if reviews.insert(key, entry).is_some() {
            return Err("duplicate property package review".to_owned());
        }
    }
    let locked = crate::lockfile::parse(lock).map_err(|error| error.to_string())?;
    let mut seen = BTreeSet::new();
    let mut findings = Vec::new();
    for (key, node) in &reached {
        let shared_rng = key.0 == "getrandom"
            && feature_set(native_assembly::array(node, "features")?)?.contains("sys_rng");
        if !restricted(key.0, key.1) && !reviews.contains_key(key) && !shared_rng {
            continue;
        }
        seen.insert(*key);
        let Some(entry) = reviews.get(key) else {
            findings.push(format!(
                "{} {}: unreviewed property package/version",
                key.0, key.1
            ));
            continue;
        };
        let id = native_assembly::text(node, "id")?;
        let package = packages.get(id).ok_or("missing property package")?;
        let archive = hash(entry, "archive_sha256")?;
        if package["source"] != crate::lockfile::CRATES_IO
            || locked
                .iter()
                .filter(|package| package.name == key.0 && package.version == key.1)
                .count()
                != 1
            || !locked.iter().any(|package| {
                package.name == key.0
                    && package.version == key.1
                    && package.source.as_deref() == Some(crate::lockfile::CRATES_IO)
                    && package.checksum.as_deref() == Some(archive)
            })
        {
            findings.push(format!(
                "{} {}: property archive/source identity changed",
                key.0, key.1
            ));
        }
        if source_trees.get(id).map(String::as_str) != Some(hash(entry, "source_tree_sha256")?) {
            findings.push(format!(
                "{} {}: property source tree changed or missing",
                key.0, key.1
            ));
        }
        if feature_set(native_assembly::array(node, "features")?)?
            != feature_set(native_assembly::array(entry, "features")?)?
        {
            findings.push(format!("{} {}: property features changed", key.0, key.1));
        }
    }
    if seen != reviews.keys().copied().collect() {
        findings.push("property review package set is stale or incomplete".to_owned());
    }
    check_edges(&document, &reached, &packages, &mut findings)?;
    Ok(findings)
}

fn restricted(name: &str, version: &str) -> bool {
    match name {
        "proptest" | "chacha20" | "core_detect" | "rand_xorshift" | "unarray" => true,
        "rand" | "rand_core" => {
            let mut parts = version.split('.');
            match (
                parts.next().and_then(|part| part.parse::<u64>().ok()),
                parts.next().and_then(|part| part.parse::<u64>().ok()),
            ) {
                (Some(major), Some(minor)) => (major, minor) >= (0, 10),
                _ => true,
            }
        }
        _ => false,
    }
}

fn feature_set(values: &[Value]) -> Result<BTreeSet<&str>, String> {
    let set = native_assembly::strings(values)?;
    if set.len() != values.len() || set.contains("") {
        return Err("duplicate or empty property scope string".to_owned());
    }
    Ok(set)
}

fn hash<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    let value = native_assembly::text(value, key)?;
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("invalid property source/archive hash".to_owned());
    }
    Ok(value)
}

fn check_edges(
    document: &Value,
    reached: &BTreeMap<(&str, &str), &Value>,
    packages: &BTreeMap<&str, &Value>,
    findings: &mut Vec<String>,
) -> Result<(), String> {
    let roots = feature_set(native_assembly::array(document, "workspace_members")?)?;
    let mut introduction = 0;
    for ((owner, version), node) in reached {
        for dependency in native_assembly::array(node, "deps")? {
            let package = packages
                .get(native_assembly::text(dependency, "pkg")?)
                .ok_or("unknown property edge")?;
            let name = native_assembly::text(package, "name")?;
            if !restricted(name, native_assembly::text(package, "version")?) {
                continue;
            }
            let kinds = native_assembly::array(dependency, "dep_kinds")?;
            if kinds.is_empty() {
                return Err("missing property edge kinds".to_owned());
            }
            let direct = *owner == "tallyline-recognition"
                && name == "proptest"
                && roots.contains(native_assembly::text(node, "id")?)
                && packages
                    .get(native_assembly::text(node, "id")?)
                    .is_some_and(|package| package["source"].is_null());
            let internal = restricted(owner, version)
                || (*owner == "getrandom" && *version == "0.4.3" && name == "rand_core");
            let expected_kind = if direct {
                Value::from("dev")
            } else {
                Value::Null
            };
            let allowed = (direct || internal)
                && native_assembly::text(dependency, "name")? == name.replace('-', "_")
                && kinds.len() == 1
                && kinds.first().and_then(|kind| kind.get("kind")) == Some(&expected_kind)
                && kinds.first().and_then(|kind| kind.get("target")) == Some(&Value::Null);
            if !allowed {
                findings.push(format!("{owner} -> {name}: property dependency introduction is outside the reviewed dev-only scope"));
            }
            if direct {
                introduction += 1;
            }
        }
    }
    if introduction != 1 {
        findings.push(
            "property profile requires exactly one recognition dev-dependency introduction"
                .to_owned(),
        );
    }
    Ok(())
}

/// Hashes the resolved package sources using the existing vendor tree algorithm.
///
/// # Errors
/// Returns malformed metadata or unreadable source trees.
pub fn source_trees(metadata: &str, review: &str) -> Result<BTreeMap<String, String>, String> {
    let document: Value = serde_json::from_str(metadata).map_err(|error| error.to_string())?;
    let review: Value = serde_json::from_str(review).map_err(|error| error.to_string())?;
    let reviewed: BTreeSet<_> = native_assembly::array(&review, "packages")?
        .iter()
        .map(|entry| {
            Ok((
                native_assembly::text(entry, "name")?,
                native_assembly::text(entry, "version")?,
            ))
        })
        .collect::<Result<_, String>>()?;
    let mut result = BTreeMap::new();
    for package in native_assembly::array(&document, "packages")? {
        if !reviewed.contains(&(
            native_assembly::text(package, "name")?,
            native_assembly::text(package, "version")?,
        )) {
            continue;
        }
        let root = Path::new(native_assembly::text(package, "manifest_path")?)
            .parent()
            .ok_or("missing source root")?;
        check_source_tree(root)?;
        result.insert(
            native_assembly::text(package, "id")?.to_owned(),
            crate::vendor::hash_package_dir(root).map_err(|error| error.to_string())?,
        );
    }
    Ok(result)
}

fn check_source_tree(path: &Path) -> Result<(), String> {
    crate::build_code::reject_links(path)?;
    if path.is_dir() {
        for entry in std::fs::read_dir(path).map_err(|error| error.to_string())? {
            check_source_tree(&entry.map_err(|error| error.to_string())?.path())?;
        }
    }
    Ok(())
}

/// Rejects property packages in an independently collected normal/build tree.
///
/// # Errors
/// Returns malformed target-tree or metadata evidence.
pub fn release_violations(metadata: &str, tree: &str, target: &str) -> Result<Vec<String>, String> {
    if !TARGETS.contains(&target) {
        return Err("unsupported property release target".to_owned());
    }
    let resolved = crate::target_graph::resolved_metadata(metadata, tree)?;
    let document: Value = serde_json::from_str(&resolved).map_err(|error| error.to_string())?;
    Ok(native_assembly::reachable(&document)?
        .keys()
        .filter(|(name, version)| restricted(name, version))
        .map(|(name, version)| {
            format!("{target}: {name} {version} must be absent from the normal/build release graph")
        })
        .collect())
}
