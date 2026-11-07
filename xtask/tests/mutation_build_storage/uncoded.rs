// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Public aggregate admission of actual uncoded compiler causes.

use std::fs;
use std::path::Path;

use super::Fixture;
use super::campaign;

fn compiler_log(fixture: &Fixture) -> String {
    let campaign = fixture.0.join("build/core-mutation/mutants.out");
    let report = campaign::json_file(&campaign.join("outcomes.json"));
    let outcome = report["outcomes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|outcome| outcome["summary"] == "Unviable")
        .unwrap();
    assert_eq!(
        outcome["scenario"]["Mutant"]["name"],
        "core/src/lib.rs:4:9: replace && with || in chained"
    );
    assert_eq!(outcome["scenario"]["Mutant"]["file"], "core/src/lib.rs");
    let log = fs::read_to_string(campaign.join(outcome["log_path"].as_str().unwrap())).unwrap();
    assert!(!log.lines().any(|line| line.starts_with("error[E")));
    log.replace('\\', "/")
}

pub(super) fn assert_causes(fixture: &Fixture, artifacts: &Path) {
    let first = artifacts.join("core-mutation-42-1-shard-0-of-32/mutants.out");
    let report_path = first.join("outcomes.json");
    let original = fs::read(&report_path).unwrap();
    let mut report = campaign::json_file(&report_path);
    report["outcomes"][1]["summary"] = "Unviable".into();
    report["outcomes"][1]["log_path"] = "log/uncoded.log".into();
    report["caught"] = 1.into();
    report["unviable"] = 1.into();
    fs::write(&report_path, report.to_string()).unwrap();
    let log_path = first.join("log/uncoded.log");
    let log = compiler_log(fixture);
    fs::write(&log_path, &log).unwrap();
    super::successful(&mut campaign::aggregate_command(fixture, artifacts));

    let start = log
        .find("error: `||` operators are not supported in let chain conditions")
        .unwrap();
    let block = log[start..].lines().take(5).collect::<Vec<_>>().join("\n") + "\n";
    assert_eq!(
        block.lines().nth(1).unwrap().trim_start(),
        "--> core/src/lib.rs:4:9"
    );
    fs::write(&log_path, &block).unwrap();
    super::successful(&mut campaign::aggregate_command(fixture, artifacts));
    campaign::reject_changed_evidence(
        fixture,
        artifacts,
        &log_path,
        block.replace("|         ^^", "| ^").as_bytes(),
        "caret does not match source location",
    );
    reject_invalid_logs(fixture, artifacts, &log_path, &block);
    campaign::reject_missing_evidence(fixture, artifacts, &log_path);
    for path in [
        "../uncoded.log",
        "log/../uncoded.log",
        "log/nested/uncoded.log",
    ] {
        let mut invalid = report.clone();
        invalid["outcomes"][1]["log_path"] = path.into();
        campaign::reject_changed_evidence(
            fixture,
            artifacts,
            &report_path,
            invalid.to_string().as_bytes(),
            "escaping log path",
        );
    }
    let mut missing_source = report.clone();
    missing_source["outcomes"][1]["scenario"]["Mutant"]
        .as_object_mut()
        .unwrap()
        .remove("file");
    campaign::reject_changed_evidence(
        fixture,
        artifacts,
        &report_path,
        missing_source.to_string().as_bytes(),
        "missing mutated source path",
    );
    let mut failed_baseline = report.clone();
    failed_baseline["outcomes"][0]["summary"] = "Unviable".into();
    campaign::reject_changed_evidence(
        fixture,
        artifacts,
        &report_path,
        failed_baseline.to_string().as_bytes(),
        "failed baseline",
    );
    #[cfg(unix)]
    reject_symlink(fixture, artifacts, &log_path);
    super::successful(&mut campaign::aggregate_command(fixture, artifacts));
    fs::write(report_path, original).unwrap();
    fs::remove_file(log_path).unwrap();
}

fn reject_invalid_logs(fixture: &Fixture, artifacts: &Path, log_path: &Path, block: &str) {
    for missing in 1..5 {
        let incomplete = block
            .lines()
            .enumerate()
            .filter_map(|(index, line)| (index != missing).then_some(line))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        campaign::reject_changed_evidence(
            fixture,
            artifacts,
            log_path,
            incomplete.as_bytes(),
            "incomplete rendered compiler block",
        );
    }
    let quoted = block
        .lines()
        .map(|line| format!("1 | {line}\n"))
        .collect::<String>();
    for invalid in [
        "error: invalid source\n".to_owned(),
        "error: could not compile `tallyline-shared-kernel` (lib) due to 1 previous error\n"
            .to_owned(),
        block.replace("core/src/lib.rs", "other/src/lib.rs"),
        block.replace("core/src/lib.rs", "core/../src/lib.rs"),
        block.replace(":4:9", ":0:9"),
        block.replace(":4:9", ":4:0"),
        block.replace(":4:9", ":4:column"),
        block.replace("4 |", "3 |"),
        block.replace("^^", "--"),
        quoted,
    ] {
        campaign::reject_changed_evidence(
            fixture,
            artifacts,
            log_path,
            invalid.as_bytes(),
            "unbound or unrecognised compiler cause",
        );
    }
    for marker in [
        "ExecutableFileBusy",
        "Text file busy",
        "No space left on device",
        "error: linking with",
        "LNK1104",
        "error[E0463]",
        "error[E0514]",
    ] {
        campaign::reject_changed_evidence(
            fixture,
            artifacts,
            log_path,
            format!("{block}{marker}\n").as_bytes(),
            "mixed compiler and infrastructure failure",
        );
    }
    campaign::reject_changed_evidence(fixture, artifacts, log_path, &[0xff], "non-UTF-8 cause");
    campaign::reject_changed_evidence(
        fixture,
        artifacts,
        log_path,
        &vec![b' '; 16 * 1024 * 1024 + 1],
        "oversized cause",
    );
    campaign::reject_changed_evidence(
        fixture,
        artifacts,
        log_path,
        format!("{block}\u{1b}[2J").as_bytes(),
        "unsupported escape",
    );
    let coloured = block.replacen("error:", "\u{1b}[31merror:\u{1b}[0m", 1);
    fs::write(log_path, coloured).unwrap();
    super::successful(&mut campaign::aggregate_command(fixture, artifacts));
    fs::write(log_path, block).unwrap();
}

#[cfg(unix)]
fn reject_symlink(fixture: &Fixture, artifacts: &Path, log_path: &Path) {
    use std::os::unix::fs;

    let retained = log_path.with_extension("retained");
    std::fs::rename(log_path, &retained).unwrap();
    fs::symlink(&retained, log_path).unwrap();
    let output = campaign::aggregate_command(fixture, artifacts).output();
    std::fs::remove_file(log_path).unwrap();
    std::fs::rename(retained, log_path).unwrap();
    campaign::assert_rejected(&output.unwrap(), "symlink cause log");
}
