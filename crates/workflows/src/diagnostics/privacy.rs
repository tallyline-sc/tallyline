// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Privacy masking before support evidence can be previewed or saved.

use std::fmt;
use std::ops::Range;

use super::DiagnosticExportFailure;
use super::masking::{self, MaskingRule};

const OMITTED_LINE: &str = "[private diagnostic line omitted]";
const MAX_RULES: usize = 32;
const MAX_RULE_BYTES: usize = 4_096;
const MAX_TOTAL_RULE_BYTES: usize = 16 * 1024;
const MAX_LINE_BYTES: usize = 4_096;
const MAX_ENCODED_RUN: usize = 80;

/// Private values excluded from support evidence, without printable state.
pub struct DiagnosticPrivacy {
    rules: Vec<MaskingRule>,
}

impl DiagnosticPrivacy {
    /// Captures the active credentials and UEX/OS identities for this export.
    ///
    /// # Errors
    /// Refuses an unusable or oversized privacy context.
    pub fn new(secrets: &[&str], identities: &[&str]) -> Result<Self, DiagnosticExportFailure> {
        if secrets.len().saturating_add(identities.len()) > MAX_RULES {
            return Err(DiagnosticExportFailure::InvalidPrivacy);
        }
        let mut total = 0_usize;
        for value in secrets.iter().chain(identities) {
            total = total.saturating_add(value.len());
            if value.is_empty()
                || value.len() > MAX_RULE_BYTES
                || value.chars().any(char::is_control)
                || total > MAX_TOTAL_RULE_BYTES
            {
                return Err(DiagnosticExportFailure::InvalidPrivacy);
            }
        }
        let rules = masking::prepare_rules(secrets.iter().chain(identities).copied());
        Ok(Self { rules })
    }

    pub(super) fn redact(&self, input: &str) -> String {
        input
            .lines()
            .map(|line| {
                if line.len() > MAX_LINE_BYTES {
                    return OMITTED_LINE.to_owned();
                }
                let urls = without_url_userinfo(line);
                if masking::crosses_removed_boundary(line, &self.rules, &urls.removed) {
                    return OMITTED_LINE.to_owned();
                }
                let line = urls.text;
                let lower = line.to_lowercase();
                if line.contains('@')
                    || has_encoded_run(&line)
                    || ["screenshot", "images", "base64", "discord", "raster"]
                        .iter()
                        .any(|marker| lower.contains(marker))
                {
                    OMITTED_LINE.to_owned()
                } else {
                    masking::redact(&line, &self.rules)
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn has_encoded_run(input: &str) -> bool {
    let mut run = 0;
    input.bytes().any(|byte| {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'=' | b'_' | b'-') {
            run += 1;
        } else {
            run = 0;
        }
        run >= MAX_ENCODED_RUN
    })
}

impl fmt::Debug for DiagnosticPrivacy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("DiagnosticPrivacy(<redacted>)")
    }
}

struct SanitisedUrls {
    text: String,
    removed: Vec<Range<usize>>,
}

fn without_url_userinfo(input: &str) -> SanitisedUrls {
    let mut output = String::new();
    let mut removed = Vec::new();
    let mut rest = input;
    while let Some(scheme_end) = rest.find("://") {
        let authority_start = scheme_end + 3;
        let original_start = input.len() - rest.len() + authority_start;
        output.push_str(&rest[..authority_start]);
        rest = &rest[authority_start..];
        let authority_end = rest
            .find(|character: char| {
                character.is_whitespace() || matches!(character, '/' | '?' | '#')
            })
            .unwrap_or(rest.len());
        let authority = &rest[..authority_end];
        let host = if let Some((userinfo, host)) = authority.rsplit_once('@') {
            removed.push(original_start..original_start + userinfo.len() + 1);
            host
        } else {
            authority
        };
        output.push_str(host);
        rest = &rest[authority_end..];
    }
    output.push_str(rest);
    SanitisedUrls {
        text: output,
        removed,
    }
}
