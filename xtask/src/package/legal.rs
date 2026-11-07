// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Physical legal material bound to the complete offline attribution catalogue.

use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Component, Path};

pub(super) const MAX_NOTICE_BYTES: u64 = 16 * 1024 * 1024;

pub(super) fn verify(root: &Path, catalogue: &Value, original: &[u8]) -> io::Result<()> {
    let components = catalogue["components"].as_array().ok_or_else(invalid)?;
    let mut expected = BTreeMap::new();
    let mut registry = BTreeMap::new();
    for component in components {
        if let Some(declared) = component.get("declared_licence") {
            let declared = match declared {
                Value::Null => "-",
                Value::String(text) => text.as_str(),
                Value::Bool(_) | Value::Number(_) | Value::Array(_) | Value::Object(_) => {
                    return Err(invalid());
                }
            };
            let key = (
                text(component, "name")?.to_owned(),
                text(component, "version")?.to_owned(),
            );
            if registry.insert(key, declared.to_owned()).is_some() {
                return Err(invalid());
            }
        }
        for notice in component["notices"].as_array().ok_or_else(invalid)? {
            let path = text(notice, "path")?;
            if path.contains(['\\', ':'])
                || Path::new(path)
                    .components()
                    .any(|part| !matches!(part, Component::Normal(_)))
            {
                return Err(invalid());
            }
            if path.starts_with("published-source/") {
                continue;
            }
            let hash = crate::hashlist::sha256_hex(text(notice, "text")?.as_bytes());
            if expected
                .insert(path.to_owned(), hash.clone())
                .is_some_and(|previous| previous != hash)
            {
                return Err(invalid());
            }
        }
    }
    expected.insert(
        "catalogue.json".into(),
        crate::hashlist::sha256_hex(original),
    );
    expected.insert("crates/index.tsv".into(), String::new());
    let map_path = root.join("source-map.json");
    if map_path.try_exists()? {
        let metadata = fs::symlink_metadata(&map_path)?;
        if !metadata.file_type().is_file() || metadata.len() > MAX_NOTICE_BYTES {
            return Err(invalid());
        }
        let bytes = fs::read(map_path)?;
        super::source_map::validate_metadata(catalogue, &bytes)?;
        expected.insert(
            "source-map.json".into(),
            crate::hashlist::sha256_hex(&bytes),
        );
    }

    let mut directories = BTreeSet::new();
    for name in expected.keys() {
        for ancestor in Path::new(name).ancestors().skip(1) {
            directories.insert(ancestor.to_string_lossy().replace('\\', "/"));
        }
    }
    let mut found = BTreeSet::new();
    collect(root, root, &expected, &directories, &registry, &mut found)?;
    if found != expected.keys().cloned().collect() {
        return Err(invalid());
    }
    Ok(())
}

fn collect(
    root: &Path,
    directory: &Path,
    expected: &BTreeMap<String, String>,
    directories: &BTreeSet<String>,
    registry: &BTreeMap<(String, String), String>,
    found: &mut BTreeSet<String>,
) -> io::Result<()> {
    for (child, _) in super::archive::children(directory)? {
        let path = child.path();
        let relative = path
            .strip_prefix(root)
            .map_err(io::Error::other)?
            .to_str()
            .ok_or_else(invalid)?
            .replace('\\', "/");
        let metadata = fs::symlink_metadata(&path)?;
        let kind = metadata.file_type();
        if kind.is_symlink() {
            return Err(invalid());
        }
        if kind.is_dir() {
            if !directories.contains(&relative) {
                return Err(invalid());
            }
            collect(root, &path, expected, directories, registry, found)?;
        } else if kind.is_file() {
            let hash = expected.get(&relative).ok_or_else(invalid)?;
            if metadata.len() > MAX_NOTICE_BYTES || super::payload::forbidden_runtime(&path) {
                return Err(invalid());
            }
            let bytes = fs::read(&path)?;
            if relative == "crates/index.tsv" {
                verify_index(
                    std::str::from_utf8(&bytes).map_err(|_| invalid())?,
                    registry,
                )?;
            } else if crate::hashlist::sha256_hex(&bytes) != *hash {
                return Err(invalid());
            }
            found.insert(relative);
        } else {
            return Err(invalid());
        }
    }
    Ok(())
}

fn verify_index(index: &str, registry: &BTreeMap<(String, String), String>) -> io::Result<()> {
    let mut lines = index.lines();
    if lines.next() != Some("crate\tversion\tdeclared\tdetected") {
        return Err(invalid());
    }
    let mut seen = BTreeSet::new();
    for line in lines {
        let fields = line.split('\t').collect::<Vec<_>>();
        let [name, version, declared, detected] = fields.as_slice() else {
            return Err(invalid());
        };
        let key = ((*name).to_owned(), (*version).to_owned());
        if detected.is_empty()
            || !seen.insert(key.clone())
            || registry.get(&key).map(String::as_str) != Some(*declared)
        {
            return Err(invalid());
        }
    }
    if seen.len() != registry.len() {
        return Err(invalid());
    }
    Ok(())
}

fn text<'a>(value: &'a Value, key: &str) -> io::Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .ok_or_else(invalid)
}

fn invalid() -> io::Error {
    io::Error::other("legal catalogue, index and physical notice inventory differ")
}

pub(super) fn verify_profile(root: &Path, target: super::Target) -> io::Result<()> {
    verify_source_inputs(None)?;
    let profiles: Value = serde_json::from_str(include_str!(
        "../../../supply-chain/package-legal-profiles.json"
    ))?;
    let bytes = fs::read(root.join("legal/catalogue.json"))?;
    if profiles
        .get("targets")
        .and_then(|targets| targets.get(target.name()))
        .and_then(|profile| profile.get("catalogue_sha256"))
        .and_then(Value::as_str)
        != Some(crate::hashlist::sha256_hex(&bytes).as_str())
    {
        return Err(io::Error::other("approved target legal catalogue mismatch"));
    }
    Ok(())
}

pub(super) fn verify_source_inputs(root: Option<&Path>) -> io::Result<()> {
    let profiles: Value = serde_json::from_str(include_str!(
        "../../../supply-chain/package-legal-profiles.json"
    ))?;
    let sources = super::source_inputs::compiled()?;
    let expected = profiles
        .get("source_inputs")
        .and_then(Value::as_object)
        .filter(|inputs| inputs.len() == sources.len())
        .ok_or_else(|| io::Error::other("packaging legal source bindings are malformed"))?;
    for (name, compiled) in sources {
        let material = if let Some(root) = root {
            let mut path = root.to_path_buf();
            if !fs::symlink_metadata(&path)?.file_type().is_dir() {
                return Err(io::Error::other(
                    "packaging source root must be a directory",
                ));
            }
            for component in Path::new(name).components() {
                path.push(component);
                if fs::symlink_metadata(&path)?.file_type().is_symlink() {
                    return Err(io::Error::other(
                        "packaging legal source input must not be linked",
                    ));
                }
            }
            let metadata = fs::symlink_metadata(&path)?;
            if !metadata.file_type().is_file() || metadata.len() > MAX_NOTICE_BYTES {
                return Err(io::Error::other(
                    "packaging legal source input is not a bounded regular file",
                ));
            }
            fs::read(path)?
        } else {
            compiled.to_vec()
        };
        let canonical = std::str::from_utf8(&material)
            .map_err(io::Error::other)?
            .replace("\r\n", "\n");
        if expected.get(name).and_then(Value::as_str)
            != Some(crate::hashlist::sha256_hex(canonical.as_bytes()).as_str())
        {
            return Err(io::Error::other(format!(
                "stale packaging legal source binding: {name}"
            )));
        }
    }
    Ok(())
}
