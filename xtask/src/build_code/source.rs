// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Filesystem proof for reviewed build-time entrypoint bytes.

use std::path::{Component, Path};

use serde_json::Value;

use super::{field, workspace};
use crate::lockfile::CRATES_IO;

pub(super) fn hash(
    document: &Value,
    package: &Value,
    target: &Value,
) -> Result<(String, String), String> {
    match package.get("source") {
        Some(Value::Null) => workspace::check_source_owner(document, package)?,
        Some(Value::String(source)) if source == CRATES_IO => {}
        Some(
            Value::String(_)
            | Value::Bool(_)
            | Value::Number(_)
            | Value::Array(_)
            | Value::Object(_),
        )
        | None => {
            return Err("build-time source has no supported package identity".to_owned());
        }
    }
    let path_text = field(target, "src_path")?;
    let path = Path::new(path_text);
    let manifest = Path::new(field(package, "manifest_path")?);
    if !path.is_absolute() || !manifest.is_absolute() {
        return Err("build-time source and manifest paths must be absolute".to_owned());
    }
    let root = manifest.parent().ok_or("manifest has no parent")?;
    let relative = path.strip_prefix(root).map_err(|error| error.to_string())?;
    if relative
        .components()
        .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(format!("build-time source escapes package: {path_text}"));
    }
    reject_links(manifest)?;
    reject_links(path)?;
    let canonical_root = root.canonicalize().map_err(|error| error.to_string())?;
    let canonical_source = path.canonicalize().map_err(|error| error.to_string())?;
    if canonical_source.strip_prefix(canonical_root).is_err() {
        return Err(format!("build-time source escapes package: {path_text}"));
    }
    let metadata = std::fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if !metadata.file_type().is_file() {
        return Err(format!(
            "build-time source is not a regular file: {path_text}"
        ));
    }
    let contents = std::fs::read(path).map_err(|error| error.to_string())?;
    Ok((path_text.to_owned(), crate::hashlist::sha256_hex(&contents)))
}

pub(crate) fn reject_links(path: &Path) -> Result<(), String> {
    for ancestor in path.ancestors() {
        let metadata = std::fs::symlink_metadata(ancestor).map_err(|error| error.to_string())?;
        if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
            return Err(format!(
                "build-time source link is rejected: {}",
                ancestor.display()
            ));
        }
    }
    Ok(())
}

#[cfg(windows)]
fn is_reparse_point(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 1024;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_: &std::fs::Metadata) -> bool {
    false
}
