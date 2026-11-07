// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Verification of pinned private corpus files.

use std::{
    collections::BTreeSet,
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
};

use sha2::{Digest, Sha256};

pub(super) fn verify(
    directory: &Path,
    skipped: &mut Vec<serde_json::Value>,
) -> Result<Option<BTreeSet<String>>, String> {
    let manifest = directory.join("SHA256SUMS");
    let text = match fs::read_to_string(&manifest) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    let mut files = BTreeSet::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let (digest, name) = line
            .split_once("  ")
            .ok_or_else(|| "invalid SHA256SUMS line".to_owned())?;
        let name = name.strip_prefix('*').unwrap_or(name);
        if digest.len() != 64
            || !digest.bytes().all(|byte| byte.is_ascii_hexdigit())
            || !files.insert(name.to_owned())
        {
            return Err("invalid or duplicate SHA256SUMS record".to_owned());
        }
        let path = contained(directory, name)?;
        let mut file = match fs::File::open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                files.remove(name);
                skipped.push(serde_json::json!(format!(
                    "pinned input {name}: unavailable"
                )));
                continue;
            }
            Err(error) => return Err(format!("pinned input {name}: {error}")),
        };
        let mut hasher = Sha256::new();
        let mut buffer = [0; 8192];
        loop {
            let count = file.read(&mut buffer).map_err(|error| error.to_string())?;
            if count == 0 {
                break;
            }
            if let Some(bytes) = buffer.get(..count) {
                hasher.update(bytes);
            }
        }
        if hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
            != digest.to_ascii_lowercase()
        {
            return Err(format!("SHA-256 mismatch: {name}"));
        }
    }
    Ok(Some(files))
}

pub(super) fn contained(directory: &Path, name: &str) -> Result<PathBuf, String> {
    let components = Path::new(name).components().collect::<Vec<_>>();
    if components.is_empty()
        || components
            .iter()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(format!("unsafe corpus file name: {name}"));
    }
    let path = directory.join(name);
    if path.exists()
        && !path
            .canonicalize()
            .map_err(|error| error.to_string())?
            .starts_with(
                directory
                    .canonicalize()
                    .map_err(|error| error.to_string())?,
            )
    {
        return Err(format!("corpus file escapes entry: {name}"));
    }
    Ok(path)
}
