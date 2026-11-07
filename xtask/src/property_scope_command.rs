// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Collects property-profile evidence without compiling the inspected graph.

use std::fs;
use std::io;
use std::path::Path;

use crate::property_scope;

/// Checks the workspace profile and independently collected target trees.
///
/// # Errors
/// Returns unreadable records, failed Cargo collection or malformed evidence.
pub fn workspace_violations(
    root: &Path,
    target: &str,
    cargo: impl Fn(&[&str]) -> io::Result<String>,
) -> io::Result<Vec<String>> {
    let metadata = cargo(&["metadata", "--locked", "--format-version", "1"])?;
    let profile = match fs::read_to_string(root.join(property_scope::REVIEWED_PATH)) {
        Ok(text) => Some(text),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    let lock = fs::read_to_string(root.join("Cargo.lock"))?;
    let sources = profile
        .as_deref()
        .map(|profile| property_scope::source_trees(&metadata, profile))
        .transpose()
        .map_err(io::Error::other)?
        .unwrap_or_default();
    let mut findings =
        property_scope::violations(&metadata, &lock, profile.as_deref(), target, &sources)
            .map_err(io::Error::other)?;
    if profile.is_some() && findings.is_empty() {
        let test_tree = cargo(&tree_arguments(target, "normal,build,dev"))?;
        let resolved = crate::target_graph::resolved_metadata(&metadata, &test_tree)
            .map_err(io::Error::other)?;
        findings.extend(
            property_scope::violations(&resolved, &lock, profile.as_deref(), target, &sources)
                .map_err(io::Error::other)?,
        );
        let release_tree = cargo(&tree_arguments(target, "normal,build"))?;
        findings.extend(
            property_scope::release_violations(&metadata, &release_tree, target)
                .map_err(io::Error::other)?,
        );
    }
    Ok(findings)
}

fn tree_arguments<'a>(target: &'a str, edges: &'a str) -> [&'a str; 14] {
    [
        "tree",
        "--locked",
        "--offline",
        "--workspace",
        "--target",
        target,
        "--edges",
        edges,
        "--prefix",
        "depth",
        "--format",
        "{p}|{f}",
        "--color",
        "never",
    ]
}

/// Inspects supplied conservative graph evidence without running Cargo.
///
/// # Errors
/// Returns unreadable files or malformed/unapproved profile evidence.
pub fn inspect(
    metadata: &Path,
    lock: &Path,
    target: &str,
    review: Option<&Path>,
) -> io::Result<Vec<String>> {
    let metadata = fs::read_to_string(metadata)?;
    let review = review.map(fs::read_to_string).transpose()?;
    let sources = review
        .as_deref()
        .map(|review| property_scope::source_trees(&metadata, review))
        .transpose()
        .map_err(io::Error::other)?
        .unwrap_or_default();
    property_scope::violations(
        &metadata,
        &fs::read_to_string(lock)?,
        review.as_deref(),
        target,
        &sources,
    )
    .map_err(io::Error::other)
}
