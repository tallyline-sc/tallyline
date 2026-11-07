// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact-byte leak detection for unpacked payloads and captured outputs.

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

pub(super) fn leaks(
    root: &Path,
    token_file: &Path,
    allowed: Option<&Path>,
) -> io::Result<Vec<PathBuf>> {
    let token = fs::read(token_file)?;
    let patterns = representations(&token)?;
    if allowed.is_some_and(|path| {
        path.as_os_str().is_empty()
            || path
                .components()
                .any(|part| !matches!(part, Component::Normal(_)))
    }) {
        return Err(io::Error::other(
            "allowed token path must be a normal relative file path",
        ));
    }
    let mut findings = Vec::new();
    collect(root, root, allowed, &token, &patterns, &mut findings)?;
    if let Some(path) = allowed
        && fs::read(root.join(path))? != token
    {
        return Err(io::Error::other(
            "expected token data file does not match the fixture",
        ));
    }
    findings.sort();
    Ok(findings)
}

fn collect(
    root: &Path,
    path: &Path,
    allowed: Option<&Path>,
    token: &[u8],
    patterns: &[Vec<u8>],
    findings: &mut Vec<PathBuf>,
) -> io::Result<()> {
    let kind = fs::symlink_metadata(path)?.file_type();
    if kind.is_symlink() {
        return Err(io::Error::other("token scan rejects symbolic links"));
    }
    if kind.is_dir() {
        for child in fs::read_dir(path)? {
            collect(root, &child?.path(), allowed, token, patterns, findings)?;
        }
    } else if kind.is_file() {
        let relative = path.strip_prefix(root).map_err(io::Error::other)?;
        let bytes = fs::read(path)?;
        if allowed == Some(relative) && bytes == token {
            return Ok(());
        }
        if patterns
            .iter()
            .any(|pattern| bytes.windows(pattern.len()).any(|part| part == pattern))
        {
            findings.push(relative.to_owned());
        }
    } else {
        return Err(io::Error::other("token scan input is not a regular file"));
    }
    Ok(())
}

fn representations(token: &[u8]) -> io::Result<[Vec<u8>; 3]> {
    if token.is_empty() {
        return Err(io::Error::other("token fixture must not be empty"));
    }
    let text =
        std::str::from_utf8(token).map_err(|_| io::Error::other("token fixture must be UTF-8"))?;
    Ok([
        token.to_owned(),
        text.encode_utf16().flat_map(u16::to_le_bytes).collect(),
        text.encode_utf16().flat_map(u16::to_be_bytes).collect(),
    ])
}

pub(super) fn contains_fixture(bytes: &[u8], fixture: &Path) -> io::Result<bool> {
    let fixture = super::read_fixture(fixture)?;
    Ok(representations(&fixture)?
        .iter()
        .any(|pattern| bytes.windows(pattern.len()).any(|part| part == pattern)))
}
