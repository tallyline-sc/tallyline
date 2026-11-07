// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Validates the reviewed build-time code inventory.

mod source;
mod workspace;

pub(crate) use source::reject_links;

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::lockfile::{self, CRATES_IO, LockedPackage};

/// The reviewed build-time code registry.
pub const REVIEWED_PATH: &str = "supply-chain/build-code.json";

/// Checks full Cargo metadata, lock identity and reviewed source hashes.
///
/// # Errors
/// Returns an error for malformed inputs.
pub fn violations(
    metadata: &str,
    lock: &str,
    reviewed: &str,
    source_hashes: &BTreeMap<String, String>,
) -> Result<Vec<String>, String> {
    let document: Value = serde_json::from_str(metadata).map_err(|error| error.to_string())?;
    let registry: Value = serde_json::from_str(reviewed).map_err(|error| error.to_string())?;
    if registry.get("schema_version").and_then(Value::as_u64) != Some(1) {
        return Err("unsupported build-code registry schema".to_owned());
    }
    let packages = indexed(array(&document, "packages")?)?;
    let reviews = reviews_by_target(array(&registry, "entries")?)?;
    let locked = lockfile::parse(lock).map_err(|error| error.to_string())?;
    let mut selected = BTreeSet::new();
    let mut findings = Vec::new();
    for package in packages.values() {
        let name = field(package, "name")?;
        let version = field(package, "version")?;
        let targets = array(package, "targets")?;
        if targets.is_empty() {
            return Err("package has no targets".to_owned());
        }
        for target in targets {
            let kinds = strings(array(target, "kind")?)?;
            if kinds.is_empty() {
                return Err("target has no kind".to_owned());
            }
            for kind in kinds {
                if !matches!(kind, "custom-build" | "proc-macro") {
                    continue;
                }
                let key = (name, version, kind);
                if !selected.insert(key) {
                    return Err(format!(
                        "duplicate build-time target {name} {version} {kind}"
                    ));
                }
                let Some(review) = reviews.get(&key) else {
                    findings.push(format!(
                        "unreviewed build-time target {name} {version} {kind}"
                    ));
                    continue;
                };
                if review.get("workspace").is_some() {
                    workspace::check(&document, package, target, review, &locked, &mut findings)?;
                } else {
                    check_identity(package, review, &locked, &mut findings)?;
                }
                let source = field(target, "src_path")?;
                let expected = hash(review, "source_sha256")?;
                if source_hashes.get(source).map(String::as_str) != Some(expected) {
                    findings.push(format!(
                        "{name} {version} {kind}: reviewed source hash differs or is missing"
                    ));
                }
            }
        }
    }
    for key in reviews.keys() {
        if !selected.contains(key) {
            findings.push(format!(
                "stale build-time review {} {} {}",
                key.0, key.1, key.2
            ));
        }
    }
    check_contexts(
        &document,
        &registry,
        &packages,
        &locked,
        source_hashes,
        &mut findings,
    )?;
    Ok(findings)
}

/// Hashes the local entrypoint of every build-time target in Cargo metadata.
///
/// # Errors
/// Returns an error for malformed metadata, escaping paths, links or unreadable source files.
pub fn source_hashes(metadata: &str) -> Result<BTreeMap<String, String>, String> {
    let document: Value = serde_json::from_str(metadata).map_err(|error| error.to_string())?;
    let mut hashes = BTreeMap::new();
    for package in array(&document, "packages")? {
        let targets = array(package, "targets")?;
        if targets.is_empty() {
            return Err("package has no targets".to_owned());
        }
        for target in targets {
            if !strings(array(target, "kind")?)?
                .iter()
                .any(|kind| matches!(*kind, "custom-build" | "proc-macro"))
            {
                continue;
            }
            let (path, digest) = source::hash(&document, package, target)?;
            hashes.insert(path, digest);
        }
    }
    Ok(hashes)
}
fn check_contexts(
    document: &Value,
    registry: &Value,
    packages: &BTreeMap<&str, &Value>,
    locked: &[LockedPackage],
    source_hashes: &BTreeMap<String, String>,
    findings: &mut Vec<String>,
) -> Result<(), String> {
    let nodes = indexed(array(
        document.get("resolve").ok_or("missing resolve graph")?,
        "nodes",
    )?)?;
    if packages.keys().ne(nodes.keys()) {
        return Err("full metadata package and resolve IDs differ".to_owned());
    }
    let roots = workspace_members(document)?;
    if roots.iter().any(|id| !packages.contains_key(id)) {
        return Err("missing or unknown workspace members".to_owned());
    }
    for node in nodes.values() {
        strings(array(node, "features")?)?;
        for dependency in array(node, "deps")? {
            field(dependency, "name")?;
            let id = field(dependency, "pkg")?;
            if !packages.contains_key(id) {
                return Err(format!("unknown dependency package {id}"));
            }
        }
    }
    let contexts = array(registry, "contexts")?;
    if contexts.len() > 1
        || contexts
            .iter()
            .any(|context| context["name"] != "gl_generator")
    {
        return Err("unknown or duplicate build-code context".to_owned());
    }
    let generators: Vec<_> = packages
        .iter()
        .filter(|(_, package)| package["name"] == "gl_generator")
        .collect();
    let Some(context) = contexts.first() else {
        if !generators.is_empty() {
            findings.push("gl_generator requires its reviewed consumer context".to_owned());
        }
        return Ok(());
    };
    let [(generator_id, generator)] = generators.as_slice() else {
        findings.push("stale or ambiguous gl_generator context".to_owned());
        return Ok(());
    };

    if field(generator, "version")? != field(context, "version")? {
        findings.push("gl_generator version requires a new review".to_owned());
    }
    check_identity(generator, context, locked, findings)?;
    let mut expected = BTreeMap::new();
    for consumer in array(context, "consumers")? {
        let key = (field(consumer, "name")?, field(consumer, "version")?);
        hash(consumer, "archive_sha256")?;
        hash(consumer, "source_sha256")?;
        if expected.insert(key, consumer).is_some() {
            return Err("duplicate reviewed GL consumer".to_owned());
        }
    }
    if expected.is_empty() {
        return Err("empty reviewed GL consumer set".to_owned());
    }
    let mut actual = BTreeSet::new();
    for (id, node) in &nodes {
        if !array(node, "deps")?
            .iter()
            .any(|dependency| dependency["pkg"].as_str() == Some(*generator_id))
        {
            continue;
        }
        let package = packages[id];
        let key = (field(package, "name")?, field(package, "version")?);
        actual.insert(key);
        let Some(review) = expected.get(&key) else {
            findings.push(format!(
                "unreviewed gl_generator consumer {} {}",
                key.0, key.1
            ));
            continue;
        };
        check_identity(package, review, locked, findings)?;
        let scripts: Vec<_> = array(package, "targets")?
            .iter()
            .filter(|target| {
                target["kind"]
                    .as_array()
                    .is_some_and(|kinds| kinds.iter().any(|kind| kind == "custom-build"))
            })
            .collect();
        if let [script] = scripts.as_slice() {
            if source_hashes
                .get(field(script, "src_path")?)
                .map(String::as_str)
                != Some(hash(review, "source_sha256")?)
            {
                findings.push(format!(
                    "{} {}: GL generator selection source changed",
                    key.0, key.1
                ));
            }
        } else {
            findings.push(format!(
                "{} {}: reviewed GL build script missing or ambiguous",
                key.0, key.1
            ));
        }
    }
    if actual.iter().ne(expected.keys()) {
        findings
            .push("gl_generator reverse consumers differ from the reviewed exact set".to_owned());
    }
    Ok(())
}
fn reviews_by_target(values: &[Value]) -> Result<BTreeMap<(&str, &str, &str), &Value>, String> {
    let mut reviews = BTreeMap::new();
    for review in values {
        let key = (
            field(review, "name")?,
            field(review, "version")?,
            field(review, "kind")?,
        );
        if !matches!(key.2, "custom-build" | "proc-macro") {
            return Err(format!("invalid reviewed target kind {}", key.2));
        }
        field(review, "behaviour")?;
        field(review, "review_scope")?;
        if review.get("workspace").is_some() {
            workspace::validate_review(review)?;
        } else {
            hash(review, "archive_sha256")?;
        }
        hash(review, "source_sha256")?;
        if reviews.insert(key, review).is_some() {
            return Err(format!(
                "duplicate build-time review {} {} {}",
                key.0, key.1, key.2
            ));
        }
    }
    Ok(reviews)
}

fn check_identity(
    package: &Value,
    review: &Value,
    locked: &[LockedPackage],
    findings: &mut Vec<String>,
) -> Result<(), String> {
    let name = field(package, "name")?;
    let version = field(package, "version")?;
    let checksum = hash(review, "archive_sha256")?;
    let matching: Vec<_> = locked
        .iter()
        .filter(|item| item.name == name && item.version == version)
        .collect();
    let matches = matches!(matching.as_slice(), [item] if item.source.as_deref() == Some(CRATES_IO) && item.checksum.as_deref() == Some(checksum));
    if field(package, "source")? != CRATES_IO || !matches {
        findings.push(format!(
            "{name} {version}: reviewed registry/archive identity differs"
        ));
    }
    Ok(())
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
        .ok_or_else(|| format!("missing or malformed array {key}"))
}

fn strings(values: &[Value]) -> Result<Vec<&str>, String> {
    values.iter().map(string).collect()
}

fn field<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value
        .get(key)
        .ok_or_else(|| format!("missing field {key}"))
        .and_then(string)
}

fn string(value: &Value) -> Result<&str, String> {
    value
        .as_str()
        .filter(|text| !text.is_empty())
        .ok_or_else(|| "empty or malformed string".to_owned())
}

fn hash<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    let text = field(value, key)?;
    if text.len() != 64
        || !text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!("invalid SHA-256 field {key}"));
    }
    Ok(text)
}

fn workspace_members(document: &Value) -> Result<BTreeSet<&str>, String> {
    let members = strings(array(document, "workspace_members")?)?;
    let unique: BTreeSet<_> = members.iter().copied().collect();
    if unique.is_empty() || unique.len() != members.len() {
        return Err("workspace members are missing or duplicated".to_owned());
    }
    Ok(unique)
}
