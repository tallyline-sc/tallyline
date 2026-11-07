// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Data-only admission of externally sealed compiled metadata and executable bytes.

use super::Target;
use serde_json::{Value, json};
use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;

const MAX_RECEIPT_BYTES: u64 = 8192;
const MAX_APPLICATION_BYTES: u64 = 512 * 1024 * 1024;

pub(super) fn read(
    application: &Path,
    receipt: &Path,
    seal: &str,
    target: Target,
    tag: &str,
) -> io::Result<Value> {
    let bytes = bounded_file(receipt, MAX_RECEIPT_BYTES)?;
    if !hexadecimal(seal, 64) || crate::hashlist::sha256_hex(&bytes) != seal {
        return Err(io::Error::other("prebuilt identity receipt seal mismatch"));
    }
    let value: Value = serde_json::from_slice(&bytes)?;
    if serde_json::to_vec_pretty(&value)? != bytes {
        return Err(io::Error::other(
            "prebuilt identity receipt is not canonical JSON",
        ));
    }
    validate(&value, application, target, tag)?;
    Ok(value)
}

pub(super) fn validate(
    receipt: &Value,
    application: &Path,
    target: Target,
    tag: &str,
) -> io::Result<()> {
    fields(
        receipt,
        &[
            "schema_version",
            "purpose",
            "application_sha256",
            "build_info",
        ],
    )?;
    let report = &receipt["build_info"];
    fields(
        report,
        &[
            "schema_version",
            "product",
            "kind",
            "version",
            "release_tag",
            "source_commit",
            "target",
        ],
    )?;
    let version = super::release_version(tag)?;
    if receipt["schema_version"].as_u64() != Some(1)
        || report["schema_version"].as_u64() != Some(1)
        || report["product"].as_str() != Some("tallyline")
        || report["target"].as_str() != Some(target.triple())
        || report["version"].as_str() != Some(version)
    {
        return Err(io::Error::other(
            "prebuilt metadata differs from package identity",
        ));
    }
    let consistent = match report["kind"].as_str() {
        Some("development") => {
            receipt["purpose"].as_str() == Some("development")
                && version == "0.0.0"
                && report["release_tag"].is_null()
                && report["source_commit"].is_null()
        }
        Some("release") => {
            receipt["purpose"].as_str() == Some("release-metadata")
                && report["release_tag"].as_str() == Some(tag)
                && report["source_commit"]
                    .as_str()
                    .is_some_and(|commit| hexadecimal(commit, 40))
        }
        _ => false,
    };
    if !consistent {
        return Err(io::Error::other(
            "inconsistent prebuilt metadata classification",
        ));
    }
    let digest = receipt["application_sha256"]
        .as_str()
        .filter(|digest| hexadecimal(digest, 64))
        .ok_or_else(|| io::Error::other("invalid prebuilt executable digest"))?;
    if crate::hashlist::sha256_hex(&bounded_file(application, MAX_APPLICATION_BYTES)?) != digest {
        return Err(io::Error::other(
            "prebuilt executable differs from admitted metadata",
        ));
    }
    Ok(())
}

fn bounded_file(path: &Path, maximum: u64) -> io::Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() || metadata.len() == 0 || metadata.len() > maximum {
        return Err(io::Error::other(
            "prebuilt identity input is not a bounded regular file",
        ));
    }
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(maximum + 1)
        .read_to_end(&mut bytes)?;
    if bytes.is_empty() || u64::try_from(bytes.len()).map_err(io::Error::other)? > maximum {
        return Err(io::Error::other(
            "prebuilt identity input exceeds its byte bound",
        ));
    }
    Ok(bytes)
}

fn fields(value: &Value, expected: &[&str]) -> io::Result<()> {
    if !value.as_object().is_some_and(|object| {
        object.len() == expected.len() && expected.iter().all(|field| object.contains_key(*field))
    }) {
        return Err(io::Error::other(
            "unknown or missing prebuilt identity field",
        ));
    }
    Ok(())
}

fn hexadecimal(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(super) fn write(
    application: &Path,
    report: &Path,
    output: &Path,
    target: Target,
    tag: &str,
) -> io::Result<String> {
    let report: Value = serde_json::from_slice(&bounded_file(report, 1024)?)?;
    let purpose = match report.get("kind").and_then(Value::as_str) {
        Some("development") => "development",
        Some("release") => "release-metadata",
        _ => return Err(io::Error::other("unknown compiled metadata kind")),
    };
    let digest = crate::hashlist::sha256_hex(&bounded_file(application, MAX_APPLICATION_BYTES)?);
    let receipt = json!({"schema_version":1,"purpose":purpose,"application_sha256":digest,"build_info":report});
    validate(&receipt, application, target, tag)?;
    let bytes = serde_json::to_vec_pretty(&receipt)?;
    let mut file = fs::File::options()
        .write(true)
        .create_new(true)
        .open(output)?;
    if let Err(error) = file.write_all(&bytes).and_then(|()| file.sync_all()) {
        drop(file);
        fs::remove_file(output).map_err(|cleanup| {
            io::Error::other(format!(
                "receipt write failed ({error}); partial output cleanup failed ({cleanup})"
            ))
        })?;
        return Err(error);
    }
    Ok(crate::hashlist::sha256_hex(&bytes))
}
