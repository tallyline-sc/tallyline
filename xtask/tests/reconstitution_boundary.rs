// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Reconstitution is a storage boundary rather than a command shortcut.

use xtask::guard::SourceFile;
use xtask::reconstitution::violations;
fn source(path: &str, text: &str) -> SourceFile {
    SourceFile {
        path: path.to_owned(),
        text: text.to_owned(),
    }
}
#[test]
fn commands_cannot_reconstitute_through_aliases_function_values_or_macros() {
    for text in [
        "Report::reconstitute(stored);",
        "Alias :: reconstitute (stored);",
        "let create = <Alias>::reconstitute; create(stored);",
        "macro_rules! restore { () => { Alias::reconstitute(stored) } }",
        "Alias::r#reconstitute(stored);",
    ] {
        assert_eq!(
            violations(&[source("crates/workflows/src/release.rs", text)]).len(),
            1
        );
        assert_eq!(
            violations(&[source("crates/app/src/worker.rs", text)]).len(),
            1
        );
        assert_eq!(
            violations(&[source(
                "crates/submission/src/submission_lifecycle.rs",
                text
            )])
            .len(),
            1
        );
        assert!(violations(&[source("crates/adapter-storage/src/queue.rs", text)]).is_empty());
        assert!(
            violations(&[source("crates/submission/tests/reconstitution.rs", text)]).is_empty()
        );
    }
}
#[test]
fn owning_definitions_do_not_allow_command_calls_in_the_same_file() {
    for path in [
        "crates/reporting/src/report.rs",
        "crates/submission/src/job/reconstitution.rs",
        "crates/reference-data/src/prior_history.rs",
        "crates/submission/src/submission_lifecycle.rs",
    ] {
        assert!(
            violations(&[source(
                path,
                "pub fn reconstitute(stored: Stored) -> Self { build(stored) }"
            )])
            .is_empty()
        );
        assert_eq!(
            violations(&[source(path, "fn command() { Self::reconstitute(stored); }")]).len(),
            1
        );
    }
    assert_eq!(
        violations(&[source(
            "crates/submission/src/job/submission_job.rs",
            "pub fn reconstitute(stored: Stored) -> Self { build(stored) }"
        )])
        .len(),
        1
    );
    assert_eq!(
        violations(&[source(
            "crates/workflows/src/release.rs",
            "fn reconstitute() {}"
        )])
        .len(),
        1
    );
}
#[test]
fn documentation_literals_and_longer_identifiers_are_not_calls() {
    let text = r####"
//! Report::reconstitute is reserved for storage.
/* nested /* Report::reconstitute() */ ignored */
let normal = "escaped \" Report::reconstitute()";
let raw = r###"\" Report::reconstitute()"###;
let bytes = br#"Report::reconstitute()"#;
let cstring = cr#"Report::reconstitute()"#;
let can_reconstitute = true;
"####;
    assert!(violations(&[source("crates/app/src/worker.rs", text)]).is_empty());
    let text = format!("{text}\nReport::reconstitute(stored);");
    let findings = violations(&[source("crates/app/src/worker.rs", &text)]);
    assert_eq!(findings.len(), 1);
    assert!(findings[0].contains("crates/app/src/worker.rs:10"));
}

#[test]
fn character_literals_and_lifetimes_cannot_hide_a_reconstruction_call() {
    for text in [
        r#"let quote = '"'; Report::reconstitute(stored);"#,
        r#"let quote = b'"'; Report::reconstitute(stored);"#,
        r#"let quote = '\''; Report::reconstitute(stored);"#,
        r#"let quote = '\u{22}'; Report::reconstitute(stored);"#,
        "let value: &'a str = \"fine\"; Report::reconstitute(stored);",
    ] {
        assert_eq!(
            violations(&[source("crates/app/src/worker.rs", text)]).len(),
            1
        );
    }
}
