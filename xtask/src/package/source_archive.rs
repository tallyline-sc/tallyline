// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Bounded data-only inspection of the fixed source archive profile.

use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

pub(super) fn verify(archive: &Path, expected: &BTreeMap<String, String>) -> io::Result<()> {
    let mut reader = flate2::read::MultiGzDecoder::new(File::open(archive)?);
    let mut seen = BTreeSet::new();
    let mut directories = BTreeSet::new();
    let mut found = BTreeSet::new();
    loop {
        let mut header = [0_u8; 512];
        reader.read_exact(&mut header)?;
        if header.iter().all(|byte| *byte == 0) {
            reader.read_exact(&mut header)?;
            let mut tail = [0_u8; 1];
            if header.iter().any(|byte| *byte != 0) || reader.read(&mut tail)? != 0 {
                return Err(invalid());
            }
            break;
        }
        let checksum: u64 = header
            .iter()
            .enumerate()
            .map(|(index, byte)| {
                if (148..156).contains(&index) {
                    32
                } else {
                    u64::from(*byte)
                }
            })
            .sum();
        if number(&header, 148, 156)? != checksum
            || bytes(&header, 257, 263)? != b"ustar\0"
            || bytes(&header, 263, 265)? != b"00"
            || number(&header, 108, 116)? != 0
            || number(&header, 116, 124)? != 0
            || number(&header, 136, 148)? != 0
            || !text(&header, 157, 257)?.is_empty()
            || text(&header, 265, 297)? != "root"
            || text(&header, 297, 329)? != "root"
            || number(&header, 329, 337)? != 0
            || number(&header, 337, 345)? != 0
            || bytes(&header, 500, 512)?.iter().any(|byte| *byte != 0)
        {
            return Err(invalid());
        }
        let leaf = text(&header, 0, 100)?;
        let prefix = text(&header, 345, 500)?;
        let name = if prefix.is_empty() {
            leaf
        } else {
            format!("{prefix}/{leaf}")
        };
        let size = number(&header, 124, 136)?;
        let mode = number(&header, 100, 108)?;
        let kind = header.get(156).copied().ok_or_else(invalid)?;
        let directory = kind == b'5';
        let name = if directory {
            name.strip_suffix('/').ok_or_else(invalid)?
        } else {
            name.as_str()
        };
        let relative = if name == "tallyline" && directory {
            ""
        } else {
            name.strip_prefix("tallyline/")
                .filter(|name| !name.is_empty())
                .ok_or_else(invalid)?
        };
        if !relative.is_empty() && !relative.split('/').all(super::archive::portable_name) {
            return Err(invalid());
        }
        if !seen.insert(relative.to_uppercase()) {
            return Err(invalid());
        }
        if !relative.is_empty() {
            let parent = relative.rsplit_once('/').map_or("", |(parent, _)| parent);
            if !directories.contains(parent) {
                return Err(invalid());
            }
        }
        if directory {
            if mode != 0o755 || size != 0 {
                return Err(invalid());
            }
            directories.insert(relative.to_owned());
        } else if kind == b'0' && mode == 0o644 {
            let actual = hash_data(&mut reader, size)?;
            if let Some(hash) = expected.get(relative) {
                if *hash != actual {
                    return Err(invalid());
                }
                found.insert(relative.to_owned());
            }
            let padding = (512 - size % 512) % 512;
            let mut buffer = [0_u8; 512];
            let padding = usize::try_from(padding).map_err(io::Error::other)?;
            let tail = buffer.get_mut(..padding).ok_or_else(invalid)?;
            reader.read_exact(tail)?;
            if tail.iter().any(|byte| *byte != 0) {
                return Err(invalid());
            }
        } else {
            return Err(invalid());
        }
    }
    if found != expected.keys().cloned().collect() {
        return Err(invalid());
    }
    Ok(())
}

fn bytes(header: &[u8], start: usize, end: usize) -> io::Result<&[u8]> {
    header.get(start..end).ok_or_else(invalid)
}

fn text(header: &[u8], start: usize, end: usize) -> io::Result<String> {
    let field = bytes(header, start, end)?;
    let length = field
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(field.len());
    if field
        .get(length..)
        .ok_or_else(invalid)?
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(invalid());
    }
    std::str::from_utf8(field.get(..length).ok_or_else(invalid)?)
        .map(str::to_owned)
        .map_err(|_| invalid())
}

fn number(header: &[u8], start: usize, end: usize) -> io::Result<u64> {
    let value = std::str::from_utf8(bytes(header, start, end)?).map_err(|_| invalid())?;
    u64::from_str_radix(value.trim_matches(['\0', ' ']), 8).map_err(|_| invalid())
}

fn hash_data(reader: &mut impl Read, mut remaining: u64) -> io::Result<String> {
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 65536];
    while remaining != 0 {
        let count = usize::try_from(remaining.min(65536)).map_err(io::Error::other)?;
        let chunk = buffer.get_mut(..count).ok_or_else(invalid)?;
        reader.read_exact(chunk)?;
        digest.update(chunk);
        remaining -= u64::try_from(count).map_err(io::Error::other)?;
    }
    Ok(digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn invalid() -> io::Error {
    io::Error::other(
        "source archive path, metadata or file evidence differs from the fixed data-only profile",
    )
}
