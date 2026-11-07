// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Bounded regular-file research artefacts.

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::Path,
};

pub(super) fn read(path: &Path, maximum: usize) -> Result<Vec<u8>, String> {
    reject_aliases(path)?;
    let file = fs::File::open(path).map_err(|_| "input file unavailable")?;
    let metadata = file.metadata().map_err(|_| "input metadata unavailable")?;
    if !metadata.is_file() || metadata.len() > maximum as u64 {
        return Err("input file exceeds bounds".to_owned());
    }
    let mut bytes = Vec::new();
    file.take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "input read failed")?;
    if bytes.len() > maximum {
        return Err("input file grew beyond bounds".to_owned());
    }
    Ok(bytes)
}

pub(super) fn reject_aliases(path: &Path) -> Result<(), String> {
    let absolute = std::path::absolute(path).map_err(|_| "invalid input path")?;
    for ancestor in absolute.ancestors() {
        let metadata = fs::symlink_metadata(ancestor).map_err(|_| "input ancestor unavailable")?;
        if metadata.file_type().is_symlink() {
            return Err("input aliases are forbidden".to_owned());
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                return Err("input reparse points are forbidden".to_owned());
            }
        }
    }
    Ok(())
}

pub(super) fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    reject_aliases(parent)?;
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|_| "output must be a fresh owned regular file")?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| "output write failed".to_owned())
}

pub(super) fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    write(
        path,
        &serde_json::to_vec_pretty(value).map_err(|_| "output encoding failed")?,
    )
}

pub(super) fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
