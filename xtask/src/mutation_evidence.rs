// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Retained failure causes for completed mutation outcomes.

use std::fs;
use std::path::{Component, Path};

use serde_json::Value;

use crate::mutation_uncoded;

const MAX_CAUSE_LOG_BYTES: u64 = 16 * 1024 * 1024;

/// Decodes a successful pinned-tool listing without inventing missing evidence.
///
/// # Errors
/// Refuses invalid JSON or an unrecognised empty-listing protocol.
pub fn listing_json(stdout: Vec<u8>, stderr: &[u8]) -> Result<String, String> {
    let text = String::from_utf8(stdout).map_err(|error| error.to_string())?;
    if text.is_empty() {
        let diagnostic = std::str::from_utf8(stderr).map_err(|error| error.to_string())?;
        if matches!(
            visible_text(diagnostic)?.trim(),
            "INFO No mutants to filter"
                | "INFO Diff changes no Rust source files"
                | "INFO Diff file is empty"
        ) {
            return Ok("[]".to_owned());
        }
        return Err(
            "mutation listing is empty without the pinned empty-selection message".to_owned(),
        );
    }
    let document: Value = serde_json::from_str(&text).map_err(|error| error.to_string())?;
    if !document.is_array() {
        return Err("mutation listing is not an array".to_owned());
    }
    Ok(text)
}

/// Refuses missing or infrastructure-only causes before mutation scoring.
///
/// # Errors
/// Returns a closed explanation when retained logs do not establish a test or compiler failure.
pub fn verify_cause_logs(directory: &Path, report: &str) -> Result<(), String> {
    let report: Value = serde_json::from_str(report).map_err(|error| error.to_string())?;
    let outcomes = report
        .get("outcomes")
        .and_then(Value::as_array)
        .ok_or("mutation report has no outcomes")?;
    for outcome in outcomes {
        let summary = outcome
            .get("summary")
            .and_then(Value::as_str)
            .ok_or("mutation outcome has no summary")?;
        match summary {
            "CaughtMutant" | "Unviable" => verify_failure(directory, outcome, summary)?,
            "Success" | "MissedMutant" => continue,
            _ => return Err("mutation outcome has an unknown summary".to_owned()),
        }
    }
    Ok(())
}

fn verify_failure(directory: &Path, outcome: &Value, summary: &str) -> Result<(), String> {
    let relative = outcome
        .get("log_path")
        .and_then(Value::as_str)
        .ok_or("mutation failure has no retained cause log")?;
    let text = visible_text(&read_cause_log(directory, relative)?)?;
    for marker in [
        "ExecutableFileBusy",
        "Text file busy",
        "No space left on device",
        "error: linking with",
        "LNK1104",
        "error[E0463]",
        "error[E0514]",
    ] {
        if text.contains(marker) {
            return Err("mutation cause contains an infrastructure failure".to_owned());
        }
    }
    match summary {
        "CaughtMutant" => {
            let failed_test = text
                .lines()
                .any(|line| line.starts_with("test ") && line.ends_with(" ... FAILED"));
            let failed_result = text
                .lines()
                .any(|line| line.starts_with("test result: FAILED."));
            if !failed_test || !failed_result {
                return Err("caught mutation has no retained failed-test result".to_owned());
            }
        }
        "Unviable" => {
            if !text.lines().any(rust_error) && !mutation_uncoded::rust_error(&text, outcome) {
                return Err("unviable mutation has no Rust compiler diagnostic".to_owned());
            }
        }
        _ => return Err("mutation failure has an unknown summary".to_owned()),
    }
    Ok(())
}

fn rust_error(line: &str) -> bool {
    let Some(rest) = line.strip_prefix("error[E") else {
        return false;
    };
    let Some((code, detail)) = rest.split_once(']') else {
        return false;
    };
    code.len() == 4 && code.bytes().all(|byte| byte.is_ascii_digit()) && detail.starts_with(':')
}

fn read_cause_log(directory: &Path, relative: &str) -> Result<String, String> {
    use std::io::Read;

    let relative = Path::new(relative);
    let components: Vec<_> = relative.components().collect();
    if !matches!(components.as_slice(), [Component::Normal(parent), Component::Normal(_)] if *parent == "log")
        || relative
            .extension()
            .is_none_or(|extension| extension != "log")
    {
        return Err("mutation cause path is outside the fixed log directory".to_owned());
    }
    for path in [directory.to_owned(), directory.join("log")] {
        let metadata =
            fs::symlink_metadata(path).map_err(|_| "mutation cause directory is unavailable")?;
        if metadata.is_symlink() || !metadata.is_dir() {
            return Err("mutation cause directory is not a regular directory".to_owned());
        }
    }
    let path = directory.join(relative);
    let metadata = fs::symlink_metadata(&path).map_err(|_| "mutation cause log is unavailable")?;
    if metadata.is_symlink() || !metadata.is_file() || metadata.len() > MAX_CAUSE_LOG_BYTES {
        return Err("mutation cause log is not a bounded regular file".to_owned());
    }
    let mut text = String::new();
    fs::File::open(path)
        .map_err(|_| "mutation cause log cannot be opened")?
        .take(MAX_CAUSE_LOG_BYTES + 1)
        .read_to_string(&mut text)
        .map_err(|_| "mutation cause log is not readable UTF-8")?;
    if text.len() as u64 > MAX_CAUSE_LOG_BYTES {
        return Err("mutation cause log exceeded its byte bound".to_owned());
    }
    Ok(text)
}

fn visible_text(text: &str) -> Result<String, String> {
    let mut output = String::new();
    let mut remaining = text;
    while let Some((plain, styled)) = remaining.split_once("\u{1b}[") {
        output.push_str(plain);
        let Some((codes, rest)) = styled.split_once('m') else {
            return Err("mutation cause has an incomplete colour sequence".to_owned());
        };
        if !codes
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte == b';')
        {
            return Err("mutation cause has an unsupported colour sequence".to_owned());
        }
        remaining = rest;
    }
    output.push_str(remaining);
    if output.contains('\u{1b}') {
        return Err("mutation cause has an unsupported escape sequence".to_owned());
    }
    Ok(output)
}
