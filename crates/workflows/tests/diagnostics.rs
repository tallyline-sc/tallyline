// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Allowlisted support previews and explicit export decisions.

#![expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]

use tallyline_workflows::api::{
    ConfirmedDiagnosticExport, DiagnosticExportApplication, DiagnosticExportFailure,
    DiagnosticExportSink, DiagnosticInput, DiagnosticPrivacy, DiagnosticSection,
};

#[derive(Default)]
struct Sink {
    writes: usize,
    text: String,
}

impl DiagnosticExportSink for Sink {
    fn write(&mut self, export: &ConfirmedDiagnosticExport) -> Result<(), DiagnosticExportFailure> {
        self.writes += 1;
        self.text = export
            .entries()
            .iter()
            .map(|entry| entry.text())
            .collect::<Vec<_>>()
            .join("\n");
        Ok(())
    }
}

#[test]
fn r_nf_6_r_doc_2__a_preview_is_allowlisted_and_masked_before_the_explicit_save() {
    let privacy = DiagnosticPrivacy::new(
        &["test-user-secret", "private-app-token-9221"],
        &["PrivateRunner", "LocalOwner"],
    )
    .unwrap();
    let image_payload = "QUJDREVGR0hJSktMTU5PUFFSU1RVVldYWVo=".repeat(8);
    let inputs = vec![
        DiagnosticInput::new(
            DiagnosticSection::Build,
            "Tallyline development; Rust 1.99.0",
        )
        .unwrap(),
        DiagnosticInput::new(
            DiagnosticSection::Preferences,
            "folder=C:\\Users\\LocalOwner\\Captures; user=PrivateRunner",
        )
        .unwrap(),
        DiagnosticInput::new(
            DiagnosticSection::Connection,
            "GET https://proxy-user:proxy-password@api.uexcorp.uk/2.0/ status=403",
        )
        .unwrap(),
        DiagnosticInput::new(
            DiagnosticSection::Logs,
            format!(
                "DEBUG secret-key=test-user-secret token=private-app-token-9221\n\
                 INFO user=privaterunner folder=C:\\Users\\LOCALOWNER\\Captures\n\
                 DEBUG submit screenshot=\"{image_payload}\"\n\
                 DEBUG vlm images=[\"{image_payload}\"]\n\
                 email=private@example.invalid discord=private-discord-name\n\
                 INFO retry host=api.uexcorp.uk code=403"
            ),
        )
        .unwrap(),
    ];
    let preview = DiagnosticExportApplication::prepare(inputs, &privacy).unwrap();
    assert_eq!(preview.entries().len(), 4);
    assert_eq!(
        preview
            .entries()
            .iter()
            .map(|entry| entry.name())
            .collect::<Vec<_>>(),
        ["build.txt", "settings.txt", "connection.txt", "logs.txt"],
    );
    let text = preview
        .entries()
        .iter()
        .map(|entry| entry.text())
        .collect::<Vec<_>>()
        .join("\n");
    for prohibited in [
        "test-user-secret",
        "private-app-token-9221",
        "PrivateRunner",
        "privaterunner",
        "LocalOwner",
        "LOCALOWNER",
        "proxy-user",
        "proxy-password",
        "private@example.invalid",
        "private-discord-name",
        image_payload.as_str(),
    ] {
        assert!(!text.contains(prohibited), "retained prohibited field");
    }
    assert!(text.contains("api.uexcorp.uk"));
    assert!(text.contains("403"));
    assert_eq!(
        preview.entries().get(1).unwrap().text(),
        "folder=C:\\Users\\[redacted]\\Captures; user=[redacted]",
    );
    assert_eq!(
        preview.entries().get(2).unwrap().text(),
        "GET https://api.uexcorp.uk/2.0/ status=403",
    );
    let mut sink = Sink::default();
    assert_eq!(sink.writes, 0);
    let confirmed = preview.confirm_public_attachment_warning();
    DiagnosticExportApplication::save(&confirmed, &mut sink).unwrap();
    assert_eq!(sink.writes, 1);
    assert_eq!(sink.text, text);
}

#[test]
fn r_nf_6__unsafe_privacy_context_and_unbounded_or_duplicate_input_are_refused() {
    let privacy = DiagnosticPrivacy::new(&[], &[]).unwrap();
    for value in [
        String::new(),
        "private\ncredential".to_owned(),
        "x".repeat(8_192),
    ] {
        assert!(DiagnosticPrivacy::new(&[value.as_str()], &[]).is_err());
    }
    assert!(DiagnosticInput::new(DiagnosticSection::Logs, "x".repeat(4 * 1024 * 1024)).is_err());
    assert!(DiagnosticExportApplication::prepare(Vec::new(), &privacy).is_err());
    assert!(
        DiagnosticExportApplication::prepare(
            vec![
                DiagnosticInput::new(DiagnosticSection::Build, "one").unwrap(),
                DiagnosticInput::new(DiagnosticSection::Build, "two").unwrap(),
            ],
            &privacy,
        )
        .is_err()
    );
}

#[test]
fn r_nf_6__unlabelled_payloads_and_oversized_log_lines_are_not_exposed() {
    let privacy = DiagnosticPrivacy::new(&[], &[]).unwrap();
    let unlabelled = "QWxwaGFiZXRpY0JpbmFyeURhdGE=".repeat(8);
    let oversized = format!("private-line:{}", "ordinary text ".repeat(1_024));
    let preview = DiagnosticExportApplication::prepare(
        vec![
            DiagnosticInput::new(
                DiagnosticSection::Logs,
                format!("DEBUG value={unlabelled}\n{oversized}\nINFO status=503"),
            )
            .unwrap(),
        ],
        &privacy,
    )
    .unwrap();
    let text = preview.entries().first().unwrap().text();
    assert!(!text.contains(&unlabelled));
    assert!(!text.contains("private-line"));
    assert!(text.contains("INFO status=503"));
}

#[test]
fn r_nf_6__unicode_identity_case_changes_and_overlapping_private_values_are_masked() {
    let privacy =
        DiagnosticPrivacy::new(&["private-token", "private-token-long"], &["İsmail"]).unwrap();
    assert!(!format!("{privacy:?}").contains("token"));
    let input = DiagnosticInput::new(
        DiagnosticSection::Logs,
        "INFO İSMAIL i̇smail private-token-long private-token status=200",
    )
    .unwrap();
    assert!(!format!("{input:?}").contains("token"));
    let preview = DiagnosticExportApplication::prepare(vec![input], &privacy).unwrap();
    let text = preview.entries().first().unwrap().text();
    for prohibited in ["İSMAIL", "i̇smail", "private-token", "long"] {
        assert!(!text.contains(prohibited));
    }
    assert!(text.contains("status=200"));
    assert_eq!(
        text,
        "INFO [redacted] [redacted] [redacted] [redacted] status=200",
    );
}

#[test]
fn r_nf_6__privacy_and_section_budgets_include_their_documented_boundary() {
    assert!(DiagnosticPrivacy::new(&["a"; 32], &[]).is_ok());
    assert!(DiagnosticPrivacy::new(&["a"; 33], &[]).is_err());
    let maximum_rule = "a".repeat(4_096);
    assert!(DiagnosticPrivacy::new(&[maximum_rule.as_str(); 4], &[]).is_ok());
    assert!(DiagnosticPrivacy::new(&[maximum_rule.as_str(); 5], &[]).is_err());
    assert!(DiagnosticPrivacy::new(&["a".repeat(4_097).as_str()], &[]).is_err());
    assert!(DiagnosticInput::new(DiagnosticSection::Logs, " ".repeat(512 * 1024)).is_ok());
    assert!(DiagnosticInput::new(DiagnosticSection::Logs, " ".repeat(512 * 1024 + 1)).is_err());
}

#[test]
fn r_nf_6__ordinary_short_tokens_and_complete_bounded_lines_remain_useful() {
    let privacy = DiagnosticPrivacy::new(&[], &[]).unwrap();
    let exact_line = format!("marker{}", " ".repeat(4_096 - "marker".len()));
    let short_token = "a".repeat(79);
    let oversized_token = "a".repeat(80);
    let preview = DiagnosticExportApplication::prepare(
        vec![
            DiagnosticInput::new(
                DiagnosticSection::Logs,
                format!("{exact_line}\nINFO #{short_token}#\nINFO #{oversized_token}#"),
            )
            .unwrap(),
        ],
        &privacy,
    )
    .unwrap();
    let text = preview.entries().first().unwrap().text();
    assert!(text.contains(&exact_line));
    assert!(text.contains(&format!("#{short_token}#")));
    assert!(!text.contains(&oversized_token));
}

#[test]
fn r_nf_6__masking_a_short_identity_cannot_expand_a_section_beyond_its_output_budget() {
    let input = format!("{}\n", "a ".repeat(1_024)).repeat(50);
    let privacy = DiagnosticPrivacy::new(&[], &["a"]).unwrap();
    let input = DiagnosticInput::new(DiagnosticSection::Logs, input).unwrap();
    assert_eq!(
        DiagnosticExportApplication::prepare(vec![input], &privacy).unwrap_err(),
        DiagnosticExportFailure::InputTooLarge,
    );
}

#[test]
fn r_nf_6__an_exactly_full_masked_section_is_accepted_without_truncation() {
    let text = format!("{}\n", "x ".repeat(1_024)).repeat(255) + &" ".repeat(1_793);
    assert_eq!(text.len(), 512 * 1024);
    let privacy = DiagnosticPrivacy::new(&[], &[]).unwrap();
    let input = DiagnosticInput::new(DiagnosticSection::Logs, text.clone()).unwrap();
    let preview = DiagnosticExportApplication::prepare(vec![input], &privacy).unwrap();
    assert_eq!(preview.entries().first().unwrap().text(), text);
}

#[test]
fn r_nf_6__private_debug_values_keep_a_useful_redacted_type_summary() {
    let privacy = DiagnosticPrivacy::new(&["private-token"], &["PrivateRunner"]).unwrap();
    let input = DiagnosticInput::new(DiagnosticSection::Logs, "private source text").unwrap();
    for (debug, name) in [
        (format!("{input:?}"), "DiagnosticInput"),
        (format!("{privacy:?}"), "DiagnosticPrivacy"),
    ] {
        assert!(debug.contains(name));
        assert!(debug.contains("redacted"));
        for prohibited in ["private-token", "PrivateRunner", "private source text"] {
            assert!(!debug.contains(prohibited));
        }
    }
}

#[test]
fn r_nf_6__url_userinfo_removal_preserves_each_authority_and_its_context() {
    let privacy = DiagnosticPrivacy::new(&[], &[]).unwrap();
    for separator in ["/", "?", "#", " "] {
        let text = format!(
            "first=https://one:first-pass@first.example.invalid{separator}context \
             second=https://two:second-pass@second.example.invalid/p status=200"
        );
        let input = DiagnosticInput::new(DiagnosticSection::Connection, text).unwrap();
        let preview = DiagnosticExportApplication::prepare(vec![input], &privacy).unwrap();
        assert_eq!(
            preview.entries().first().unwrap().text(),
            format!(
                "first=https://first.example.invalid{separator}context \
                 second=https://second.example.invalid/p status=200"
            ),
        );
    }
}

#[test]
fn r_nf_6__unicode_case_variants_of_known_private_values_are_masked() {
    for (identity, variants) in [
        ("ΟΣ", vec!["ΟΣ", "ος", "οσ", "Ος", "οΣ"]),
        ("ΟΣΟΣ", vec!["ΟΣΟΣ", "οσος", "οσοσ", "ΟΣος"]),
        (
            "Stra\u{df}e",
            vec!["Stra\u{df}e", "STRASSE", "stra\u{df}e", "STRAẞE"],
        ),
        ("ẞ", vec!["ẞ", "\u{df}", "SS", "ss"]),
        ("İ", vec!["İ", "i\u{0307}", "I\u{0307}"]),
        ("ﬃ", vec!["ﬃ", "FFI", "ffi"]),
    ] {
        let privacy = DiagnosticPrivacy::new(&[identity], &[identity]).unwrap();
        for variant in variants {
            let input = DiagnosticInput::new(
                DiagnosticSection::Logs,
                format!("identity={variant}; host=api.uexcorp.uk"),
            )
            .unwrap();
            let preview = DiagnosticExportApplication::prepare(vec![input], &privacy).unwrap();
            assert_eq!(
                preview.entries().first().unwrap().text(),
                "identity=[redacted]; host=api.uexcorp.uk",
                "private case variant {identity:?} / {variant:?}",
            );
        }
    }
}

#[test]
fn r_nf_6__self_overlapping_private_values_leave_no_unmasked_fragment() {
    for (identity, value) in [
        ("aba", "ababa"),
        ("aaa", "aaaaa"),
        ("ababa", "ababababa"),
        ("aabaa", "aabaabaa"),
        ("ΣΣ", "σσσ"),
        ("éé", "ÉéÉ"),
        ("ẞẞ", "\u{df}ss\u{df}"),
    ] {
        let privacy = DiagnosticPrivacy::new(&[], &[identity]).unwrap();
        let input = DiagnosticInput::new(DiagnosticSection::Logs, format!("λ[{value}]Ω")).unwrap();
        let preview = DiagnosticExportApplication::prepare(vec![input], &privacy).unwrap();
        let actual = preview.entries().first().unwrap().text();
        assert!(actual.contains("[redacted]"));
        assert_eq!(
            actual.replace("[redacted]", ""),
            "λ[]Ω",
            "{identity:?} / {value:?}"
        );
    }
}

#[test]
fn r_nf_6__overlap_masking_preserves_exactly_the_unrelated_neighbours() {
    for identity_length in 1..=4 {
        for identity_bits in 0..(1 << identity_length) {
            let identity = binary_text(identity_length, identity_bits);
            let privacy = DiagnosticPrivacy::new(&[], &[&identity]).unwrap();
            for input_length in 0..=7 {
                for input_bits in 0..(1 << input_length) {
                    let input = binary_text(input_length, input_bits);
                    let mut covered = vec![false; input.len()];
                    for start in 0..input.len() {
                        if input
                            .get(start..)
                            .is_some_and(|tail| tail.starts_with(&identity))
                        {
                            for byte in covered.iter_mut().skip(start).take(identity.len()) {
                                *byte = true;
                            }
                        }
                    }
                    let unrelated: String = input
                        .chars()
                        .zip(&covered)
                        .filter_map(|(character, covered)| (!covered).then_some(character))
                        .collect();
                    let value =
                        DiagnosticInput::new(DiagnosticSection::Logs, format!("λ[{input}]Ω"))
                            .unwrap();
                    let preview =
                        DiagnosticExportApplication::prepare(vec![value], &privacy).unwrap();
                    let actual = preview.entries().first().unwrap().text();
                    assert_eq!(
                        actual.replace("[redacted]", ""),
                        format!("λ[{unrelated}]Ω"),
                        "{identity:?} / {input:?}"
                    );
                    assert_eq!(
                        actual.contains("[redacted]"),
                        covered.iter().any(|value| *value)
                    );
                }
            }
        }
    }
}

fn binary_text(length: u32, bits: u32) -> String {
    (0..length)
        .map(|index| if bits & (1 << index) == 0 { 'a' } else { 'b' })
        .collect()
}
