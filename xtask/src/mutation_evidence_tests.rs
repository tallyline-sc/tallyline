// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! File-backed contracts for mutation-cause admission.

use serde_json::json;

use crate::mutation_evidence;
use crate::scratch::Scratch;

fn report(summary: &str, log: &str) -> String {
    json!({"outcomes":[{
        "scenario":{"Mutant":{"name":"core mutation"}},
        "summary":summary,"log_path":log
    }]})
    .to_string()
}

#[test]
fn caught_infrastructure_failure_is_refused_even_when_the_test_runner_failed() {
    let scratch = Scratch::new();
    scratch.write(
        "log/mutation.log",
        "test loader_contract ... FAILED\nError: Custom { kind: ExecutableFileBusy, error: \"Text file busy\" }\ntest result: FAILED. 4 passed; 1 failed\n",
    );
    assert!(
        mutation_evidence::verify_cause_logs(
            scratch.path(),
            &report("CaughtMutant", "log/mutation.log"),
        )
        .is_err()
    );
}

#[test]
fn compiler_infrastructure_errors_are_not_unviable_rust_mutants() {
    let scratch = Scratch::new();
    scratch.write(
        "log/mutation.log",
        "error: could not write output: disk full\n",
    );
    assert!(
        mutation_evidence::verify_cause_logs(
            scratch.path(),
            &report("Unviable", "log/mutation.log"),
        )
        .is_err()
    );
}

#[test]
fn real_assertions_returned_errors_and_rust_type_errors_are_admitted() {
    let scratch = Scratch::new();
    for (summary, log) in [
        (
            "CaughtMutant",
            "test price_contract ... FAILED\nassertion failed: amount > 0\ntest result: FAILED. 0 passed; 1 failed\n",
        ),
        (
            "CaughtMutant",
            "test price_contract ... FAILED\nError: InvalidAmount\ntest result: FAILED. 0 passed; 1 failed\n",
        ),
        (
            "Unviable",
            "error[E0277]: the trait bound Preview: Default is not satisfied\n",
        ),
    ] {
        scratch.write("log/mutation.log", log);
        assert!(
            mutation_evidence::verify_cause_logs(
                scratch.path(),
                &report(summary, "log/mutation.log"),
            )
            .is_ok()
        );
    }
}

#[test]
fn linker_failure_invalidates_a_run_even_when_another_rust_error_is_present() {
    let scratch = Scratch::new();
    scratch.write(
        "log/mutation.log",
        "error[E0277]: Preview does not implement Default\nerror: linking with `link.exe` failed: exit code: 1104\nLINK : fatal error LNK1104: cannot open output\n",
    );
    assert!(
        mutation_evidence::verify_cause_logs(
            scratch.path(),
            &report("Unviable", "log/mutation.log"),
        )
        .is_err()
    );
}

#[test]
fn cause_logs_are_required_and_cannot_escape_their_fixed_directory() {
    let scratch = Scratch::new();
    for path in [
        "log/missing.log",
        "../foreign.log",
        "foreign.log",
        "log/../foreign.log",
        "log/nested/foreign.log",
        "log/foreign.txt",
    ] {
        assert!(
            mutation_evidence::verify_cause_logs(scratch.path(), &report("CaughtMutant", path))
                .is_err(),
            "Unadmitted cause path {path}",
        );
    }
}

#[test]
fn colour_sequences_preserve_genuine_compiler_diagnostics() {
    let scratch = Scratch::new();
    scratch.write(
        "log/mutation.log",
        "\u{1b}[1m\u{1b}[31merror[E0277]\u{1b}[0m: Preview does not implement Default\n",
    );
    assert!(
        mutation_evidence::verify_cause_logs(
            scratch.path(),
            &report("Unviable", "log/mutation.log"),
        )
        .is_ok()
    );
}

#[test]
fn the_pinned_explicit_empty_listing_is_json_without_a_measured_score() {
    for stderr in [
        b" INFO No mutants to filter\n".as_slice(),
        b" INFO Diff changes no Rust source files\r\n",
        b" INFO Diff file is empty\n",
    ] {
        assert_eq!(
            mutation_evidence::listing_json(Vec::new(), stderr),
            Ok("[]".to_owned()),
        );
    }
    for stderr in [
        b"".as_slice(),
        b"unexpected failure",
        b"INFO No mutants to filter\nerror: broken",
    ] {
        assert!(mutation_evidence::listing_json(Vec::new(), stderr).is_err());
    }
    assert_eq!(
        mutation_evidence::listing_json(b"[]\n".to_vec(), b""),
        Ok("[]\n".to_owned()),
    );
}
