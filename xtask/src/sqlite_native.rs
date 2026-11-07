// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact source and compiler-input boundary for bundled SQLite hardware-timer assembly.

use crate::lockfile::{CRATES_IO, LockedPackage};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// Checks exact reviewed source identities, reachable target features and ambient build inputs.
///
/// # Errors
/// Returns malformed or missing evidence instead of treating it as approved.
pub fn violations(
    registry: &str,
    locked: &[LockedPackage],
    target_graphs: &BTreeMap<String, String>,
    source_hashes: &BTreeMap<String, String>,
    environment: &[(String, String)],
    now_days: u64,
) -> Result<Vec<String>, String> {
    let review: Value = serde_json::from_str(registry).map_err(|error| error.to_string())?;
    if field(&review, "schema")?.as_u64() != Some(1) {
        return Err("invalid SQLite native registry schema".to_owned());
    }
    let mut findings = Vec::new();
    if field(&review, "status")?.as_str() != Some("approved") {
        findings.push("SQLite native source scope is not owner-approved".to_owned());
    }
    let expiry = text(&review, "revisit_by")?;
    if now_days >= date_days(expiry)? {
        findings.push("SQLite native source review expired".to_owned());
    }
    let expected_targets = BTreeSet::from([
        "x86_64-pc-windows-msvc".to_owned(),
        "x86_64-unknown-linux-gnu".to_owned(),
    ]);
    let mut evidence_targets = expected_targets.clone();
    evidence_targets.insert("full".to_owned());
    if strings(field(&review, "targets")?)? != expected_targets
        || target_graphs.keys().cloned().collect::<BTreeSet<_>>() != evidence_targets
    {
        return Err("SQLite supported-target evidence differs".to_owned());
    }
    let files = field(&review, "files")?
        .as_object()
        .ok_or("SQLite source records missing")?;
    let expected_files = BTreeSet::from([
        "build.rs",
        "sqlite3/sqlite3.c",
        "sqlite3/sqlite3.h",
        "sqlite3/sqlite3ext.h",
    ]);
    if files.keys().map(String::as_str).collect::<BTreeSet<_>>() != expected_files {
        return Err("SQLite native source record set differs".to_owned());
    }
    for (path, expected) in files {
        let expected = expected.as_str().ok_or("SQLite source hash malformed")?;
        if expected.len() != 64 || !expected.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("SQLite source hash invalid".to_owned());
        }
        if source_hashes.get(path).map(String::as_str) != Some(expected) {
            findings.push(format!("SQLite reviewed source hash differs: {path}"));
        }
    }
    let sys_version = text(&review, "version")?;
    let wrapper = field(&review, "rusqlite")?;
    for (name, version, archive) in [
        (
            "libsqlite3-sys",
            sys_version,
            text(&review, "archive_sha256")?,
        ),
        (
            "rusqlite",
            text(wrapper, "version")?,
            text(wrapper, "archive_sha256")?,
        ),
    ] {
        let packages: Vec<_> = locked
            .iter()
            .filter(|package| package.name == name)
            .collect();
        if packages.len() != 1
            || !packages.iter().all(|package| {
                package.version == version
                    && package.source.as_deref() == Some(CRATES_IO)
                    && package.checksum.as_deref() == Some(archive)
            })
        {
            findings.push(format!("SQLite locked identity differs: {name}"));
        }
    }
    for (target, graph) in target_graphs {
        let document: Value = serde_json::from_str(graph).map_err(|error| error.to_string())?;
        let packages = field(&document, "packages")?
            .as_array()
            .ok_or("SQLite graph packages missing")?;
        let nodes = field(field(&document, "resolve")?, "nodes")?
            .as_array()
            .ok_or("SQLite resolve nodes missing")?;
        for values in [packages, nodes] {
            let mut ids = BTreeSet::new();
            for value in values {
                if !ids.insert(text(value, "id")?) {
                    return Err("duplicate SQLite evidence metadata ID".to_owned());
                }
            }
        }
        let mut reachable = BTreeSet::new();
        let mut pending: Vec<String> = strings(field(&document, "workspace_members")?)?
            .into_iter()
            .collect();
        if pending.is_empty() {
            return Err("SQLite graph has no workspace roots".to_owned());
        }
        while let Some(id) = pending.pop() {
            if !reachable.insert(id.clone()) {
                continue;
            }
            if !packages
                .iter()
                .any(|package| package.get("id").and_then(Value::as_str) == Some(id.as_str()))
            {
                return Err("SQLite reachable package missing".to_owned());
            }
            let matches: Vec<_> = nodes
                .iter()
                .filter(|node| node.get("id").and_then(Value::as_str) == Some(&id))
                .collect();
            let [node] = matches.as_slice() else {
                return Err("SQLite graph has missing/duplicate reachable node".to_owned());
            };
            let deps = field(node, "deps")?
                .as_array()
                .ok_or("SQLite graph dependency edges missing")?;
            for dependency in deps {
                text(dependency, "name")?;
                pending.push(text(dependency, "pkg")?.to_owned());
            }
        }
        for (name, version, expected) in [
            (
                "libsqlite3-sys",
                sys_version,
                strings(field(&review, "features")?)?,
            ),
            (
                "rusqlite",
                text(wrapper, "version")?,
                strings(field(wrapper, "features")?)?,
            ),
        ] {
            let selected: Vec<_> = packages
                .iter()
                .filter(|package| package.get("name").and_then(Value::as_str) == Some(name))
                .collect();
            let [package] = selected.as_slice() else {
                return Err(format!(
                    "{target}: SQLite package identity missing/ambiguous"
                ));
            };
            let id = text(package, "id")?;
            if package.get("version").and_then(Value::as_str) != Some(version)
                || package.get("source").and_then(Value::as_str) != Some(CRATES_IO)
                || !reachable.contains(id)
            {
                findings.push(format!(
                    "{target}: SQLite package identity/reachability differs: {name}"
                ));
            }
            let matching: Vec<_> = nodes
                .iter()
                .filter(|node| node.get("id").and_then(Value::as_str) == Some(id))
                .collect();
            let [node] = matching.as_slice() else {
                return Err("SQLite package resolve node missing/ambiguous".to_owned());
            };
            if strings(field(node, "features")?)? != expected {
                findings.push(format!("{target}: SQLite selected features differ: {name}"));
            }
        }
    }
    findings.extend(environment_violations(environment));
    Ok(findings)
}

fn overridden(key: &str) -> bool {
    key.starts_with("SQLITE_")
        || key.starts_with("SQLCIPHER_")
        || key.starts_with("LIBSQLITE3_")
        || [
            "CFLAGS",
            "CPPFLAGS",
            "CC",
            "CXX",
            "AR",
            "ARFLAGS",
            "RANLIB",
            "RANLIBFLAGS",
            "RUSTFLAGS",
            "RUSTC",
        ]
        .iter()
        .any(|base| {
            key == *base
                || key.starts_with(&format!("{base}_"))
                || key.ends_with(&format!("_{base}"))
        })
        || matches!(
            key,
            "HOST_CC"
                | "TARGET_CC"
                | "CRATE_CC_NO_DEFAULTS"
                | "CC_KNOWN_WRAPPER_CUSTOM"
                | "CARGO_ENCODED_RUSTFLAGS"
                | "CARGO_BUILD_RUSTFLAGS"
                | "RUSTC_WRAPPER"
                | "RUSTC_WORKSPACE_WRAPPER"
                | "CL"
                | "_CL_"
        )
}

fn text<'a>(value: &'a Value, field: &str) -> Result<&'a str, String> {
    value[field]
        .as_str()
        .ok_or_else(|| format!("SQLite evidence lacks {field}"))
}

fn strings(value: &Value) -> Result<BTreeSet<String>, String> {
    let values = value
        .as_array()
        .ok_or("SQLite evidence string array missing")?;
    let parsed: Result<Vec<_>, _> = values
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_owned)
                .ok_or("SQLite evidence string array malformed".to_owned())
        })
        .collect();
    let parsed = parsed?;
    let set: BTreeSet<_> = parsed.iter().cloned().collect();
    if set.len() != parsed.len() {
        return Err("SQLite evidence duplicate array entry".to_owned());
    }
    Ok(set)
}

fn date_days(value: &str) -> Result<u64, String> {
    let parts = value
        .split('-')
        .map(str::parse::<u64>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "invalid SQLite review date")?;
    let [year, month, day] = parts.as_slice() else {
        return Err("invalid SQLite review date".to_owned());
    };
    if !(1970..=9999).contains(year) || !(1..=12).contains(month) {
        return Err("invalid SQLite review date".to_owned());
    }
    let leap = |year: u64| {
        year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
    };
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
    let days_in_month = months
        .iter()
        .zip(1..)
        .find(|(_, number)| number == month)
        .map(|(days, _)| *days)
        .ok_or("invalid SQLite review month")?;
    if *day == 0 || *day > days_in_month {
        return Err("invalid SQLite review date".to_owned());
    }
    Ok((1970..*year)
        .map(|year| if leap(year) { 366 } else { 365 })
        .sum::<u64>()
        + months
            .iter()
            .zip(1..)
            .filter(|(_, number)| number < month)
            .map(|(days, _)| *days)
            .sum::<u64>()
        + day
        - 1)
}

/// Hashes the exact bundled native sources without executing build code.
///
/// # Errors
/// Returns missing or ambiguous metadata, unreadable files, symlinks or path escape.
pub fn source_hashes(metadata: &str) -> Result<BTreeMap<String, String>, String> {
    let document: Value = serde_json::from_str(metadata).map_err(|error| error.to_string())?;
    let packages = field(&document, "packages")?
        .as_array()
        .ok_or("SQLite package metadata missing")?;
    let selected = packages
        .iter()
        .filter(|package| package.get("name").and_then(Value::as_str) == Some("libsqlite3-sys"))
        .collect::<Vec<_>>();
    let [package] = selected.as_slice() else {
        return Err("SQLite native source package missing or ambiguous".to_owned());
    };
    let manifest = std::path::Path::new(text(package, "manifest_path")?);
    let directory = manifest.parent().ok_or("SQLite manifest parent missing")?;
    for ancestor in directory.ancestors() {
        reject_link(ancestor)?;
    }
    let root = std::fs::canonicalize(directory).map_err(|error| error.to_string())?;
    let mut hashes = BTreeMap::new();
    for relative in [
        "build.rs",
        "sqlite3/sqlite3.c",
        "sqlite3/sqlite3.h",
        "sqlite3/sqlite3ext.h",
    ] {
        let source = directory.join(relative);
        for ancestor in source
            .ancestors()
            .take_while(|ancestor| ancestor.starts_with(directory))
        {
            reject_link(ancestor)?;
        }
        let canonical = std::fs::canonicalize(&source).map_err(|error| error.to_string())?;
        if !canonical.starts_with(&root) {
            return Err("SQLite native source escapes package root".to_owned());
        }
        let bytes = std::fs::read(&canonical).map_err(|error| error.to_string())?;
        hashes.insert(relative.to_owned(), crate::hashlist::sha256_hex(&bytes));
    }
    Ok(hashes)
}

fn reject_link(path: &std::path::Path) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if metadata.file_type().is_symlink() {
        return Err("SQLite native source symlink rejected".to_owned());
    }
    Ok(())
}

/// Extracts explicit Cargo native/compiler inputs for the same ambient-override check.
///
/// # Errors
/// Returns malformed configuration or unsupported environment value shapes.
pub fn configuration_environment(configuration: &str) -> Result<Vec<(String, String)>, String> {
    let document = configuration
        .parse::<toml::Table>()
        .map_err(|error| error.to_string())?;
    let mut environment = Vec::new();
    if let Some(value) = document.get("env") {
        let entries = value.as_table().ok_or("Cargo env is not a table")?;
        for (key, value) in entries {
            let value = value
                .as_str()
                .or_else(|| {
                    value
                        .as_table()
                        .and_then(|entry| entry.get("value"))
                        .and_then(toml::Value::as_str)
                })
                .ok_or("Cargo environment value is not a string")?;
            environment.push((key.clone(), value.to_owned()));
        }
    }
    if document
        .get("build")
        .and_then(toml::Value::as_table)
        .is_some_and(|build| build.contains_key("rustflags"))
    {
        environment.push((
            "RUSTFLAGS".to_owned(),
            "Cargo build configuration".to_owned(),
        ));
    }
    if let Some(value) = document.get("target") {
        for target in value
            .as_table()
            .ok_or("Cargo targets are not a table")?
            .values()
        {
            if target
                .as_table()
                .ok_or("Cargo target configuration is not a table")?
                .contains_key("rustflags")
            {
                environment.push((
                    "RUSTFLAGS".to_owned(),
                    "Cargo target configuration".to_owned(),
                ));
            }
        }
    }
    Ok(environment)
}

/// Returns unreviewed native/compiler environment inputs without reading their values into diagnostics.
#[must_use]
pub fn environment_violations(environment: &[(String, String)]) -> Vec<String> {
    let mut findings = Vec::new();
    for (key, value) in environment {
        let key = key.to_ascii_uppercase();
        if key == "LIBSQLITE3_SYS_USE_PKG_CONFIG" {
            if value != "0" {
                findings.push("SQLite system linking override is forbidden".to_owned());
            }
        } else if overridden(&key) {
            findings.push(format!("SQLite unreviewed compiler/native override: {key}"));
        }
    }
    findings
}

fn field<'a>(value: &'a Value, key: &str) -> Result<&'a Value, String> {
    value
        .get(key)
        .ok_or_else(|| format!("SQLite evidence lacks {key}"))
}
