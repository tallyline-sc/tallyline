// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Corpus metadata inspection without recognition or live reference data.

use std::{collections::BTreeSet, fs, io::Read, path::Path};

use super::field;
use serde_json::{Value, json};

/// Inspects public entries and an optional private corpus without running recognition.
///
/// # Errors
/// Returns an error for invalid annotations, unsafe paths or unavailable LFS objects.
pub fn inspect(public: &Path, private: Option<&Path>) -> Result<Value, String> {
    let mut entries = Vec::new();
    let mut skipped = Vec::new();
    inspect_directory(public, None, &mut entries, &mut skipped)?;
    match private {
        Some(path) => inspect_directory(path, Some(public), &mut entries, &mut skipped)?,
        None => skipped.push(json!("private corpus: TALLYLINE_CORPUS_DIR is unset")),
    }
    let classes = super::classes::report(&entries);
    Ok(
        json!({"entries":entries,"skipped":skipped,"classes":classes,
        "recognition":"skipped: recognition is not implemented"}),
    )
}

fn inspect_directory(
    path: &Path,
    public: Option<&Path>,
    entries: &mut Vec<Value>,
    skipped: &mut Vec<Value>,
) -> Result<(), String> {
    if !path.exists() {
        skipped.push(json!("corpus directory: unavailable"));
        return Ok(());
    }
    let mut directories = fs::read_dir(path)
        .map_err(|error| error.to_string())?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    directories.sort();
    let root = path.canonicalize().map_err(|error| error.to_string())?;
    for directory in directories.into_iter().filter(|path| path.is_dir()) {
        if !directory
            .canonicalize()
            .map_err(|error| error.to_string())?
            .starts_with(&root)
        {
            return Err("corpus entry directory escapes corpus root".to_owned());
        }
        let pins = if public.is_some() {
            let Some(pins) = super::pinned::verify(&directory, skipped)? else {
                skipped.push(json!("private entry: SHA256SUMS unavailable"));
                continue;
            };
            Some(pins)
        } else {
            None
        };
        if let Some(expected) = read_expected(&directory, public, pins.as_ref(), skipped)? {
            let available = check_images(&directory, &expected, pins.as_ref(), skipped)?;
            let reference = super::reference::inspect(&directory, &expected, pins.as_ref())?;
            merge_entry(entries, expected, available, reference)?;
        }
    }
    Ok(())
}

fn read_expected(
    directory: &Path,
    public: Option<&Path>,
    pins: Option<&BTreeSet<String>>,
    skipped: &mut Vec<Value>,
) -> Result<Option<Value>, String> {
    let mut path = super::pinned::contained(directory, "expected.json")?;
    if path.exists() {
        if pins.is_some_and(|pins| !pins.contains("expected.json")) {
            return Err("private expected.json is not pinned in SHA256SUMS".to_owned());
        }
    } else if let Some(public) = public {
        let name = directory
            .file_name()
            .ok_or_else(|| "entry has no directory name".to_owned())?;
        path = public.join(name).join("expected.json");
    }
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            skipped.push(json!("entry annotations: expected.json unavailable"));
            return Ok(None);
        }
        Err(error) => return Err(error.to_string()),
    };
    super::validate_json(&text)?;
    let value: Value = serde_json::from_str(&text).map_err(|error| error.to_string())?;
    if field(&value, "schemaVersion") != &json!(2) {
        return Err("inspect requires schema version 2; migrate conditions explicitly".to_owned());
    }
    if directory.file_name().and_then(|name| name.to_str()) != field(&value, "id").as_str() {
        return Err("entry id must match its directory name".to_owned());
    }
    Ok(Some(value))
}

fn merge_entry(
    entries: &mut Vec<Value>,
    expected: Value,
    mut available: Vec<String>,
    reference: Value,
) -> Result<(), String> {
    if let Some(existing) = entries
        .iter_mut()
        .find(|entry| field(field(entry, "expected"), "id") == field(&expected, "id"))
    {
        if field(existing, "expected") != &expected {
            return Err("public and private annotations for the same entry disagree".to_owned());
        }
        if let Some(names) = field(existing, "availableCaptures").as_array() {
            available.extend(names.iter().filter_map(Value::as_str).map(str::to_owned));
        }
        available.sort();
        available.dedup();
        *existing =
            json!({"expected":expected,"availableCaptures":available,"reference":reference});
    } else {
        entries
            .push(json!({"expected":expected,"availableCaptures":available,"reference":reference}));
    }
    Ok(())
}

fn check_images(
    directory: &Path,
    entry: &Value,
    pins: Option<&BTreeSet<String>>,
    skipped: &mut Vec<Value>,
) -> Result<Vec<String>, String> {
    let mut available = Vec::new();
    if let Some(captures) = field(entry, "captures").as_object() {
        for name in captures.keys() {
            let path = super::pinned::contained(directory, name)?;
            if pins.is_some_and(|pins| !pins.contains(name)) {
                skipped.push(json!(format!(
                    "private image {name}: not pinned in SHA256SUMS"
                )));
                continue;
            }
            match fs::File::open(&path) {
                Ok(mut file) => {
                    let mut prefix = [0; 128];
                    let count = file.read(&mut prefix).map_err(|error| error.to_string())?;
                    if prefix.get(..count).is_some_and(|bytes| {
                        bytes.starts_with(b"version https://git-lfs.github.com/spec/v1")
                    }) {
                        return Err(format!("image {name}: LFS pointer; run git lfs pull"));
                    }
                    available.push(name.clone());
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    skipped.push(json!(format!("image {name}: unavailable")))
                }
                Err(error) => return Err(format!("image {name}: {error}")),
            }
        }
    }
    Ok(available)
}
