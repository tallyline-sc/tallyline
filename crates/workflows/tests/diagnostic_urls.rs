// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Privacy contracts across URL-userinfo removal boundaries.

#![expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]

use tallyline_workflows::api::{
    DiagnosticExportApplication, DiagnosticExportFailure, DiagnosticInput, DiagnosticPrivacy,
    DiagnosticSection,
};

const URL: &str = "https://ab:cd@ef.invalid/path";
const USERINFO: &str = "ab:cd@";
const CLEAN: &str = "value=https://ef.invalid/path;status=200";
const OMITTED: &str = "[private diagnostic line omitted]";

#[test]
fn r_nf_6__userinfo_removal_keeps_crossing_private_values_whole() {
    for identity in [
        URL,
        "https://ab:",
        "ab:cd@ef.invalid",
        "cd@ef.invalid",
        "HTTPS://AB:CD@EF.INVALID/PATH",
    ] {
        assert_eq!(
            preview(identity, &format!("value={URL};status=200")).unwrap(),
            OMITTED,
            "private value crosses the removed userinfo: {identity:?}",
        );
    }
}

#[test]
fn r_nf_6__every_url_substring_obeys_the_private_removal_boundary() {
    let input = format!("value={URL};status=200");
    let removed_start = input.find(USERINFO).unwrap();
    let removed_end = removed_start + USERINFO.len();
    for start in 0..URL.len() {
        for end in start + 1..=URL.len() {
            let identity = URL.get(start..end).unwrap();
            let crosses =
                input
                    .as_bytes()
                    .windows(identity.len())
                    .enumerate()
                    .any(|(offset, bytes)| {
                        bytes.eq_ignore_ascii_case(identity.as_bytes())
                            && offset < removed_end
                            && removed_start < offset + bytes.len()
                            && !(removed_start <= offset && offset + bytes.len() <= removed_end)
                    });
            let expected = if crosses {
                OMITTED.to_owned()
            } else {
                preview(identity, CLEAN).unwrap()
            };
            assert_eq!(
                preview(identity, &input).unwrap(),
                expected,
                "original private-value boundary: {identity:?}",
            );
        }
    }
}

#[test]
fn r_nf_6__fully_removed_or_untouched_private_values_preserve_useful_context() {
    for (identity, expected) in [
        ("ab:cd@", CLEAN),
        ("cd", CLEAN),
        ("https://", "value=[redacted]ef.invalid/path;status=200"),
        ("ef.invalid", "value=https://[redacted]/path;status=200"),
        ("https://ef.invalid/path", "value=[redacted];status=200"),
    ] {
        assert_eq!(
            preview(identity, &format!("value={URL};status=200")).unwrap(),
            expected,
            "{identity:?}",
        );
    }
}

#[test]
fn r_nf_6__userinfo_offsets_remain_original_utf8_boundaries_across_urls() {
    for prefix in [
        "",
        "λ ",
        "first=https://x:y@other.invalid/z second=",
        "🙂 first=https://x:y@other.invalid/z second=",
    ] {
        let input = format!("{prefix}{URL};status=200");
        assert_eq!(
            preview("CD@EF.INVALID", &input).unwrap(),
            OMITTED,
            "{prefix:?}"
        );
    }
}

fn preview(identity: &str, text: &str) -> Result<String, DiagnosticExportFailure> {
    let privacy = DiagnosticPrivacy::new(&[identity], &[identity])?;
    let input = DiagnosticInput::new(DiagnosticSection::Logs, text)?;
    let preview = DiagnosticExportApplication::prepare(vec![input], &privacy)?;
    preview
        .entries()
        .first()
        .map(|entry| entry.text().to_owned())
        .ok_or(DiagnosticExportFailure::InvalidSections)
}
