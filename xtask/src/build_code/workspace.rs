// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact workspace-owned build-script review identities.

use std::path::Path;

use serde_json::Value;

use super::{field, workspace_members};
use crate::lockfile::LockedPackage;

pub(super) fn validate_review(review: &Value) -> Result<(), String> {
    if field(review, "kind")? != "custom-build" || review.get("archive_sha256").is_some() {
        return Err(
            "workspace review must name a build script without a registry archive".to_owned(),
        );
    }
    let workspace = review
        .get("workspace")
        .and_then(Value::as_object)
        .ok_or("malformed workspace script identity")?;
    if workspace.len() != 2
        || !workspace.contains_key("manifest_path")
        || !workspace.contains_key("source_path")
    {
        return Err("workspace script identity needs exact manifest and source paths".to_owned());
    }
    let manifest = relative(field(&review["workspace"], "manifest_path")?)?;
    let source = relative(field(&review["workspace"], "source_path")?)?;
    if manifest.file_name().and_then(|name| name.to_str()) != Some("Cargo.toml")
        || source.extension().and_then(|extension| extension.to_str()) != Some("rs")
        || source
            .strip_prefix(
                manifest
                    .parent()
                    .ok_or("workspace manifest has no parent")?,
            )
            .is_err()
    {
        return Err("workspace source must remain inside its reviewed package".to_owned());
    }
    Ok(())
}

pub(super) fn check(
    document: &Value,
    package: &Value,
    target: &Value,
    review: &Value,
    locked: &[LockedPackage],
    findings: &mut Vec<String>,
) -> Result<(), String> {
    let root = Path::new(field(document, "workspace_root")?);
    if !root.is_absolute() {
        return Err("workspace root is not absolute".to_owned());
    }
    let manifest = root.join(field(&review["workspace"], "manifest_path")?);
    let source = root.join(field(&review["workspace"], "source_path")?);
    let members = workspace_members(document)?;
    let name = field(package, "name")?;
    let version = field(package, "version")?;
    let matching: Vec<_> = locked
        .iter()
        .filter(|item| item.name == name && item.version == version)
        .collect();
    let locked_workspace =
        matches!(matching.as_slice(), [item] if item.source.is_none() && item.checksum.is_none());
    if package.get("source") != Some(&Value::Null)
        || !members.contains(field(package, "id")?)
        || Path::new(field(package, "manifest_path")?) != manifest
        || Path::new(field(target, "src_path")?) != source
        || !locked_workspace
    {
        findings.push(format!(
            "{name} {version}: reviewed workspace membership/path identity differs"
        ));
    }
    Ok(())
}

fn relative(text: &str) -> Result<&Path, String> {
    if text.contains(['\\', ':'])
        || text
            .split('/')
            .any(|part| part.is_empty() || matches!(part, "." | ".."))
    {
        return Err("workspace review path is not a normal relative path".to_owned());
    }
    let path = Path::new(text);
    if path
        .components()
        .any(|part| !matches!(part, std::path::Component::Normal(_)))
    {
        return Err("workspace review path is not a normal relative path".to_owned());
    }
    Ok(path)
}

pub(super) fn check_source_owner(document: &Value, package: &Value) -> Result<(), String> {
    let root = Path::new(field(document, "workspace_root")?);
    let manifest = Path::new(field(package, "manifest_path")?);
    let members = workspace_members(document)?;
    if !root.is_absolute() || !members.contains(field(package, "id")?) {
        return Err("build script does not belong to an absolute workspace".to_owned());
    }
    let relative = manifest
        .strip_prefix(root)
        .map_err(|_| "workspace manifest lies outside its root")?;
    if relative
        .components()
        .any(|part| !matches!(part, std::path::Component::Normal(_)))
        || manifest.file_name().and_then(|name| name.to_str()) != Some("Cargo.toml")
    {
        return Err("workspace manifest path is not normal".to_owned());
    }
    let metadata = std::fs::symlink_metadata(manifest).map_err(|error| error.to_string())?;
    if !metadata.file_type().is_file() {
        return Err("workspace manifest is not a regular file".to_owned());
    }
    Ok(())
}
