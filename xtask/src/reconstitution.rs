// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Reserves persistence reconstruction identifiers for the storage adapter.

use crate::guard::SourceFile;

/// Finds reconstruction identifiers outside storage and owning constructor definitions.
///
/// The reserved identifier also catches aliases, function values and macro bodies. Comments
/// and string literals carry no calls. Integration test targets remain outside this production rule.
#[must_use]
pub fn violations(files: &[SourceFile]) -> Vec<String> {
    let mut findings = Vec::new();
    for file in files {
        let Some((context, relative)) = file
            .path
            .strip_prefix("crates/")
            .and_then(|path| path.split_once('/'))
        else {
            continue;
        };
        if !relative.ends_with(".rs")
            || !relative.starts_with("src/")
            || context == "adapter-storage"
        {
            continue;
        }
        let definition = matches!(
            file.path.as_str(),
            "crates/reporting/src/report.rs"
                | "crates/submission/src/job/reconstitution.rs"
                | "crates/reference-data/src/prior_history.rs"
                | "crates/submission/src/submission_lifecycle.rs"
        );
        for offset in reserved_identifiers(&file.text, definition) {
            let line = file.text.as_bytes().get(..offset).map_or(1, |prefix| {
                prefix.iter().filter(|byte| **byte == b'\n').count() + 1
            });
            findings.push(format!(
                "{}:{line}: reconstitute is reserved for adapter-storage",
                file.path
            ));
        }
    }
    findings
}
fn reserved_identifiers(text: &str, definition: bool) -> Vec<usize> {
    let bytes = text.as_bytes();
    let mut findings = Vec::new();
    let mut offset = 0;
    let mut after_fn = false;
    while let Some(byte) = bytes.get(offset).copied() {
        if byte.is_ascii_whitespace() {
            offset += 1;
            continue;
        }
        if bytes.get(offset..offset + 2) == Some(b"//") {
            while bytes.get(offset).is_some_and(|byte| *byte != b'\n') {
                offset += 1;
            }
            continue;
        }
        if bytes.get(offset..offset + 2) == Some(b"/*") {
            offset = block_comment_end(bytes, offset);
            continue;
        }
        if let Some((start, hashes)) = raw_string_start(bytes, offset) {
            offset = raw_string_end(bytes, start, hashes);
            after_fn = false;
            continue;
        }
        if byte == b'\''
            && let Some(end) = character_end(bytes, offset + 1)
        {
            offset = end;
            after_fn = false;
            continue;
        }
        if byte == b'"' {
            offset = quoted_end(bytes, offset + 1);
            after_fn = false;
            continue;
        }
        if identifier_start(byte) {
            let start = offset;
            while bytes
                .get(offset)
                .is_some_and(|byte| identifier_continue(*byte))
            {
                offset += 1;
            }
            let word = bytes.get(start..offset);
            if word == Some(b"reconstitute") && !(definition && after_fn) {
                findings.push(start);
            }
            after_fn = word == Some(b"fn");
        } else {
            after_fn = false;
            offset += 1;
        }
    }
    findings
}
fn identifier_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_' || byte >= 128
}
fn identifier_continue(byte: u8) -> bool {
    identifier_start(byte) || byte.is_ascii_digit()
}
fn block_comment_end(bytes: &[u8], mut offset: usize) -> usize {
    offset += 2;
    let mut depth = 1;
    while offset < bytes.len() && depth > 0 {
        if bytes.get(offset..offset + 2) == Some(b"/*") {
            depth += 1;
            offset += 2;
        } else if bytes.get(offset..offset + 2) == Some(b"*/") {
            depth -= 1;
            offset += 2;
        } else {
            offset += 1;
        }
    }
    offset
}
fn quoted_end(bytes: &[u8], mut offset: usize) -> usize {
    while let Some(byte) = bytes.get(offset) {
        match byte {
            b'\\' => offset += 2,
            b'"' => return offset + 1,
            _ => offset += 1,
        }
    }
    offset
}
fn raw_string_start(bytes: &[u8], mut offset: usize) -> Option<(usize, usize)> {
    if matches!(bytes.get(offset), Some(b'b' | b'c')) {
        offset += 1;
    }
    if bytes.get(offset) != Some(&b'r') {
        return None;
    }
    offset += 1;
    let mut hashes = 0;
    while bytes.get(offset) == Some(&b'#') {
        hashes += 1;
        offset += 1;
    }
    (bytes.get(offset) == Some(&b'"')).then_some((offset + 1, hashes))
}
fn raw_string_end(bytes: &[u8], mut offset: usize, hashes: usize) -> usize {
    while let Some(byte) = bytes.get(offset) {
        if *byte == b'"'
            && bytes
                .get(offset + 1..offset + 1 + hashes)
                .is_some_and(|suffix| suffix.iter().all(|byte| *byte == b'#'))
        {
            return offset + 1 + hashes;
        }
        offset += 1;
    }
    offset
}

fn character_end(bytes: &[u8], mut offset: usize) -> Option<usize> {
    if bytes.get(offset) == Some(&b'\\') {
        offset += 1;
        match bytes.get(offset)? {
            b'u' if bytes.get(offset + 1) == Some(&b'{') => {
                offset += 2;
                while bytes
                    .get(offset)
                    .is_some_and(|byte| byte.is_ascii_hexdigit() || *byte == b'_')
                {
                    offset += 1;
                }
                if bytes.get(offset) != Some(&b'}') {
                    return None;
                }
                offset += 1;
            }
            b'x' => {
                if !bytes
                    .get(offset + 1..offset + 3)?
                    .iter()
                    .all(u8::is_ascii_hexdigit)
                {
                    return None;
                }
                offset += 3;
            }
            _ => offset += 1,
        }
    } else {
        let width = match bytes.get(offset)? {
            0..=127 => 1,
            194..=223 => 2,
            224..=239 => 3,
            240..=244 => 4,
            _ => return None,
        };
        offset += width;
    }
    (bytes.get(offset) == Some(&b'\'')).then_some(offset + 1)
}
