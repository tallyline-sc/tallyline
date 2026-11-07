// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact credential native sources and conservative literal assembly inventory.

use crate::lockfile::CRATES_IO;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// The owner-approved assembly inventory; proposed data must stay outside the repository.
pub const REVIEWED_PATH: &str = "supply-chain/native-assembly.json";

/// One Rust source file covered by the native inventory.
#[derive(Clone, Debug)]
pub struct SourceFile {
    /// Full Cargo package ID.
    pub package_id: String,
    /// Forward-slash path relative to the package root.
    pub path: String,
    /// SHA-256 of the complete source file.
    pub sha256: String,
}

/// Collect hashes of explicit credential native sources and literal assembly indicators.
///
/// Comments and strings count conservatively; renamed/generated macros may escape this heuristic.
/// Symlinks and paths outside the canonical package root are rejected.
///
/// # Errors
/// Returns an error for malformed metadata, symlinks, unreadable sources or path escape.
pub fn source_files(metadata: &str) -> Result<Vec<SourceFile>, String> {
    let metadata: Value = serde_json::from_str(metadata).map_err(|e| e.to_string())?;
    let mut files = Vec::new();
    for package in array(&metadata, "packages")? {
        if package["source"].is_null() {
            continue;
        }
        if text(package, "source")? != CRATES_IO {
            return Err("assembly inventory requires crates.io sources".to_owned());
        }
        let manifest = Path::new(text(package, "manifest_path")?);
        let root = manifest.parent().ok_or("manifest has no parent")?;
        for ancestor in root.ancestors() {
            reject_link(ancestor)?;
        }
        let canonical = fs::canonicalize(root).map_err(|e| e.to_string())?;
        collect(
            root,
            &canonical,
            text(package, "id")?,
            text(package, "name")?,
            &mut files,
        )?;
    }
    Ok(files)
}

fn reject_link(path: &Path) -> Result<(), String> {
    let info = fs::symlink_metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if info.file_type().is_symlink() {
        return Err(format!("symlink rejected: {}", path.display()));
    }
    Ok(())
}

fn collect(
    path: &Path,
    root: &Path,
    id: &str,
    name: &str,
    files: &mut Vec<SourceFile>,
) -> Result<(), String> {
    let kind = fs::symlink_metadata(path)
        .map_err(|e| e.to_string())?
        .file_type();
    if kind.is_symlink() {
        return Err(format!("symlink rejected: {}", path.display()));
    }
    if !kind.is_dir() && !path.extension().is_some_and(|extension| extension == "rs") {
        return Ok(());
    }
    let canonical = fs::canonicalize(path).map_err(|e| e.to_string())?;
    let relative = canonical
        .strip_prefix(root)
        .map_err(|_| "source path escapes package root")?;
    if kind.is_dir() {
        for entry in fs::read_dir(path).map_err(|e| e.to_string())? {
            collect(
                &entry.map_err(|e| e.to_string())?.path(),
                root,
                id,
                name,
                files,
            )?;
        }
    } else if path.extension().is_some_and(|extension| extension == "rs") {
        let bytes = fs::read(path).map_err(|e| e.to_string())?;
        let relative = relative.to_string_lossy().replace('\\', "/");
        if indicator(&bytes) || credential_source(name, &relative) {
            files.push(SourceFile {
                package_id: id.to_owned(),
                path: relative,
                sha256: crate::hashlist::sha256_hex(&bytes),
            });
        }
    }
    Ok(())
}

fn credential_source(name: &str, path: &str) -> bool {
    match name {
        "windows-native-keyring-store" => matches!(
            path,
            "src/utils.rs" | "src/cred.rs" | "src/store.rs" | "src/lib.rs" | "src/tests.rs"
        ),
        "zbus" => matches!(
            path,
            "src/connection/socket/unix.rs"
                | "src/fdo/peer.rs"
                | "src/win32.rs"
                | "src/connection/builder.rs"
                | "src/connection/handshake/client.rs"
                | "src/address/mod.rs"
        ),
        "signal-hook-registry" => matches!(
            path,
            "src/lib.rs" | "src/half_lock.rs" | "tests/unregister_signal.rs"
        ),
        _ => false,
    }
}
fn indicator(bytes: &[u8]) -> bool {
    bytes
        .split(|b| !b.is_ascii_alphanumeric() && *b != b'_')
        .any(|word| {
            if !matches!(word, b"asm" | b"global_asm" | b"llvm_asm") {
                return false;
            }
            let offset = word.as_ptr() as usize - bytes.as_ptr() as usize + word.len();
            bytes
                .get(offset..)
                .unwrap_or_default()
                .iter()
                .find(|b| !b.is_ascii_whitespace())
                == Some(&b'!')
        })
}

/// Compare assembly indicators, locked identities and exact reachable target features with approval.
///
/// # Errors
/// Returns an error for malformed metadata, expired/unapproved review data or duplicate entries.
pub fn violations(
    metadata: &str,
    lockfile: &str,
    review: &str,
    files: &[SourceFile],
    targets: &BTreeMap<String, String>,
) -> Result<Vec<String>, String> {
    let document: Value = serde_json::from_str(metadata).map_err(|e| e.to_string())?;
    let review: Value = serde_json::from_str(review).map_err(|e| e.to_string())?;
    if review.get("schema") != Some(&Value::from(1)) {
        return Err("invalid assembly registry schema".to_owned());
    }
    let approved = text(&review, "status")? == "approved";
    expiry(text(&review, "revisit_by")?)?;
    let expected_targets = strings(array(&review, "targets")?)?;
    if expected_targets.is_empty()
        || expected_targets != targets.keys().map(String::as_str).collect()
    {
        return Err("assembly target set differs from reviewed scope".to_owned());
    }
    let packages = index(array(&document, "packages")?)?;
    let nodes = index(array(
        document.get("resolve").ok_or("missing resolve graph")?,
        "nodes",
    )?)?;
    let roots = strings(array(&document, "workspace_members")?)?;
    if roots.is_empty()
        || packages.keys().ne(nodes.keys())
        || roots.iter().any(|id| !packages.contains_key(id))
    {
        return Err("incomplete full assembly metadata graph".to_owned());
    }
    let locked = crate::lockfile::parse(lockfile).map_err(|e| e.to_string())?;
    let mut reviews = BTreeMap::new();
    for entry in array(&review, "packages")? {
        let key = (text(entry, "name")?, text(entry, "version")?);
        if reviews.insert(key, entry).is_some() {
            return Err("duplicate assembly review".to_owned());
        }
    }
    let mut observed = BTreeMap::<_, BTreeMap<&str, &str>>::new();
    for file in files {
        if !packages.contains_key(file.package_id.as_str()) {
            return Err("source observation has unknown package ID".to_owned());
        }
        if observed
            .entry(file.package_id.as_str())
            .or_default()
            .insert(&file.path, &file.sha256)
            .is_some()
        {
            return Err("duplicate assembly source observation".to_owned());
        }
    }
    let mut findings = if approved {
        Vec::new()
    } else {
        vec!["assembly registry is not owner-approved; proposed data cannot pass".to_owned()]
    };
    let mut seen = BTreeSet::new();
    for (id, sources) in observed {
        let package = packages.get(id).ok_or("unknown observed package")?;
        let key = (text(package, "name")?, text(package, "version")?);
        seen.insert(key);
        let Some(entry) = reviews.get(&key) else {
            findings.push(format!("unreviewed assembly package {} {}", key.0, key.1));
            continue;
        };
        let archive = text(entry, "archive_sha256")?;
        let identity_matches = package["source"] == CRATES_IO
            && locked.iter().any(|p| {
                p.name == key.0
                    && p.version == key.1
                    && p.source.as_deref() == Some(CRATES_IO)
                    && p.checksum.as_deref() == Some(archive)
            });
        if !identity_matches {
            findings.push(format!(
                "{} {}: assembly archive identity changed",
                key.0, key.1
            ));
        }
        let expected = entry["files"].as_object().ok_or("missing reviewed files")?;
        let expected: BTreeMap<_, _> = expected
            .iter()
            .map(|(p, h)| {
                h.as_str()
                    .map(|h| (p.as_str(), h))
                    .ok_or("invalid source hash")
            })
            .collect::<Result<_, _>>()?;
        if sources != expected || expected.is_empty() {
            findings.push(format!(
                "{} {}: assembly source paths/hashes changed",
                key.0, key.1
            ));
        }
        let node = nodes
            .get(id)
            .ok_or("assembly package missing resolve node")?;
        if strings(array(node, "features")?)? != strings(array(entry, "full_features")?)? {
            findings.push(format!(
                "{} {}: full assembly features changed",
                key.0, key.1
            ));
        }
    }
    if seen != reviews.keys().copied().collect() {
        findings.push("assembly review package set is stale or incomplete".to_owned());
    }
    for (target, metadata) in targets {
        check_target(target, metadata, &reviews, &mut findings)?;
    }
    Ok(findings)
}

fn check_target(
    target: &str,
    metadata: &str,
    reviews: &BTreeMap<(&str, &str), &Value>,
    findings: &mut Vec<String>,
) -> Result<(), String> {
    let document: Value = serde_json::from_str(metadata).map_err(|e| e.to_string())?;
    let reached = reachable(&document)?;
    let resolved = document
        .get("tallyline_target_tree")
        .map(|proof| {
            if text(proof, "cargo")? != crate::target_graph::REVIEWED_CARGO
                || text(proof, "target")? != target
                || !matches!(
                    text(proof, "host")?,
                    "x86_64-pc-windows-msvc" | "x86_64-unknown-linux-gnu"
                )
            {
                return Err("unreviewed Cargo target-tree provenance".to_owned());
            }
            reachable(proof.get("graph").ok_or("missing target-tree graph")?)
        })
        .transpose()?;
    for (key, entry) in reviews {
        let expected = entry["targets"]
            .get(target)
            .ok_or("missing reviewed target")?;
        let matches = matches_activation(expected, reached.get(key).copied())?;
        if !matches {
            findings.push(format!(
                "{target}: {} {} assembly activation/features changed",
                key.0, key.1
            ));
        }
        if let Some(expected) = entry.get("resolved_targets") {
            let expected = expected
                .get(target)
                .ok_or("missing resolved target selector")?;
            let resolved = resolved
                .as_ref()
                .ok_or("missing required Cargo target-tree evidence")?;
            if !matches_activation(expected, resolved.get(key).copied())? {
                findings.push(format!(
                    "{target}: {} {} resolved assembly activation/features changed",
                    key.0, key.1
                ));
            }
        }
    }
    Ok(())
}

fn matches_activation(expected: &Value, node: Option<&Value>) -> Result<bool, String> {
    Ok(match (expected, node) {
        (Value::Null, None) => true,
        (Value::Array(features), Some(node)) => {
            strings(features)? == strings(array(node, "features")?)?
        }
        (Value::Null | Value::Array(_), _) => false,
        _ => return Err("malformed target activation selector".to_owned()),
    })
}

pub(crate) fn reachable(document: &Value) -> Result<BTreeMap<(&str, &str), &Value>, String> {
    let packages = index(array(document, "packages")?)?;
    let nodes = index(array(
        document.get("resolve").ok_or("missing resolve graph")?,
        "nodes",
    )?)?;
    let mut pending: Vec<_> = strings(array(document, "workspace_members")?)?
        .into_iter()
        .collect();
    if pending.is_empty() {
        return Err("missing workspace roots".to_owned());
    }
    let mut reached = BTreeMap::new();
    let mut seen = BTreeSet::new();
    while let Some(id) = pending.pop() {
        if !seen.insert(id) {
            continue;
        }
        let node = nodes.get(id).ok_or("unknown reachable node")?;
        let package = packages.get(id).ok_or("unknown reachable package")?;
        if reached
            .insert((text(package, "name")?, text(package, "version")?), *node)
            .is_some()
        {
            return Err("ambiguous reachable package identity".to_owned());
        }
        for dependency in array(node, "deps")? {
            pending.push(text(dependency, "pkg")?);
        }
    }
    Ok(reached)
}

pub(crate) fn expiry(date: &str) -> Result<(), String> {
    let parts: Vec<u64> = date
        .split('-')
        .map(str::parse)
        .collect::<Result<_, _>>()
        .map_err(|_| "invalid review date")?;
    let [year, month, day] = parts.as_slice() else {
        return Err("invalid review date".to_owned());
    };
    if !(1970..=9999).contains(year) || !(1..=12).contains(month) {
        return Err("invalid review date".to_owned());
    }
    let leap = |y: u64| y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400));
    let months = [
        31,
        if leap(*year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let prior = 0;
    let prior = months
        .iter()
        .zip(1_u64..)
        .filter(|(_, m)| m < month)
        .fold(prior, |sum, (days, _)| sum + days);
    let month_days = months
        .iter()
        .zip(1_u64..)
        .find(|(_, m)| m == month)
        .map(|(d, _)| *d)
        .ok_or("invalid month")?;
    if *day == 0 || *day > month_days {
        return Err("invalid review date".to_owned());
    }
    let days = (1970..*year)
        .map(|y| if leap(y) { 366 } else { 365 })
        .sum::<u64>()
        + prior
        + day
        - 1;
    let today = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs()
        / 86_400;
    if today >= days {
        return Err("assembly approval has reached its revisit date".to_owned());
    }
    Ok(())
}

pub(crate) fn index(values: &[Value]) -> Result<BTreeMap<&str, &Value>, String> {
    let mut result = BTreeMap::new();
    for value in values {
        if result.insert(text(value, "id")?, value).is_some() {
            return Err("duplicate metadata ID".to_owned());
        }
    }
    Ok(result)
}

pub(crate) fn array<'a>(value: &'a Value, key: &str) -> Result<&'a [Value], String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("missing array {key}"))
}

pub(crate) fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("missing string {key}"))
}

pub(crate) fn strings(values: &[Value]) -> Result<BTreeSet<&str>, String> {
    values
        .iter()
        .map(|v| v.as_str().ok_or_else(|| "invalid string array".to_owned()))
        .collect()
}
