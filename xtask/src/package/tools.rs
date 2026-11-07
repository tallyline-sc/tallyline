// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Approved delivered-byte identities, checked without executing tools.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const WIX_MANIFEST: &str = include_str!("../../../supply-chain/wix/tool-manifest.json");
const WIX_MANIFEST_SHA256: &str =
    "3f9834599d030a8af18e5d926dd2de07dd04394109681103414500c9adabf794";
const PACKAGER_SHA256: &str = "59a00a50ce45bb3d8940ff86b4b982269e28ad2b1e2ad2a3b78e3a654bd68593";

/// Checks the complete approved packaging-tool tree without executing any supplied file.
///
/// The Windows identity is the reviewed source-built WiX profile and private runtime.
/// The Linux identity is the reviewed cargo-packager binary built with Rust 1.99.0.
/// No extra files, links, missing files or changed bytes are accepted.
///
/// # Errors
/// Returns unsupported platforms, an unapproved inventory or filesystem failures.
pub fn verify(target: &str, root: &Path) -> io::Result<()> {
    let expected = match target {
        "windows" => {
            if crate::hashlist::sha256_hex(WIX_MANIFEST.as_bytes()) != WIX_MANIFEST_SHA256 {
                return Err(rejected());
            }
            let entries: serde_json::Value = serde_json::from_str(WIX_MANIFEST)?;
            let mut expected = BTreeMap::new();
            for entry in entries.as_array().ok_or_else(rejected)? {
                let path = entry["path"].as_str().ok_or_else(rejected)?;
                let hash = entry["sha256"].as_str().ok_or_else(rejected)?;
                expected.insert(PathBuf::from(path), hash.to_owned());
            }
            expected
        }
        "linux" => BTreeMap::from([(PathBuf::from("cargo-packager"), PACKAGER_SHA256.to_owned())]),
        _ => return Err(rejected()),
    };
    let mut actual = BTreeMap::new();
    collect(root, root, &mut actual)?;
    if actual != expected {
        return Err(rejected());
    }
    Ok(())
}

fn collect(root: &Path, path: &Path, files: &mut BTreeMap<PathBuf, String>) -> io::Result<()> {
    let kind = fs::symlink_metadata(path)?.file_type();
    if kind.is_symlink() {
        return Err(rejected());
    }
    if kind.is_dir() {
        for child in fs::read_dir(path)? {
            collect(root, &child?.path(), files)?;
        }
    } else if kind.is_file() {
        let relative = path.strip_prefix(root).map_err(io::Error::other)?;
        files.insert(
            relative.to_owned(),
            crate::hashlist::sha256_hex(&fs::read(path)?),
        );
    } else {
        return Err(rejected());
    }
    Ok(())
}

fn rejected() -> io::Error {
    io::Error::other("input differs from the approved packaging tool identity")
}
