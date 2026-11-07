// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Confirmed diagnostics exports against actual temporary files.

#![expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]

use std::path::PathBuf;
use tallyline_adapter_files::api::{DiagnosticFileExport, DiagnosticLogFiles};
use tallyline_workflows::api::{
    ConfirmedDiagnosticExport, DiagnosticExportApplication, DiagnosticInput, DiagnosticPrivacy,
    DiagnosticSection,
};

fn export() -> Result<ConfirmedDiagnosticExport, tallyline_workflows::api::DiagnosticExportFailure>
{
    let privacy = DiagnosticPrivacy::new(&["private-source-credential"], &["PrivateOwner"])?;
    Ok(DiagnosticExportApplication::prepare(
        vec![DiagnosticInput::new(
            DiagnosticSection::Logs,
            "INFO key=private-source-credential user=PrivateOwner host=api.uexcorp.uk",
        )?],
        &privacy,
    )?
    .confirm_public_attachment_warning())
}

#[test]
fn r_nf_6_r_doc_2__confirmed_entries_are_saved_privately_without_overwriting_existing_data() {
    let directory = directory("exclusive").unwrap();
    let destination = directory.join("support.json");
    let confirmed = export().unwrap();
    let mut sink = DiagnosticFileExport::new(destination.clone());
    assert!(!destination.exists());
    DiagnosticExportApplication::save(&confirmed, &mut sink).unwrap();
    let original = std::fs::read(&destination).unwrap();
    let document: serde_json::Value = serde_json::from_slice(&original).unwrap();
    assert_eq!(document["format_version"], 1);
    assert_eq!(document["files"].as_array().unwrap().len(), 1);
    assert_eq!(document["files"][0]["name"], "logs.txt");
    assert!(
        document["files"][0]["text"]
            .as_str()
            .unwrap()
            .contains("api.uexcorp.uk")
    );
    let text = String::from_utf8(original.clone()).unwrap();
    assert!(!text.contains("private-source-credential"));
    assert!(!text.contains("PrivateOwner"));
    assert!(!text.contains("Game.log"));
    assert!(DiagnosticExportApplication::save(&confirmed, &mut sink).is_err());
    assert_eq!(std::fs::read(&destination).unwrap(), original);
    assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&destination)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    remove_directory(directory).unwrap();
}

#[test]
fn r_nf_6__an_invalid_destination_returns_no_path_or_source_content() {
    let directory = directory("unavailable").unwrap();
    let mut sink = DiagnosticFileExport::new(directory.clone());
    let error = DiagnosticExportApplication::save(&export().unwrap(), &mut sink).unwrap_err();
    let diagnostic = format!("{error:?}");
    assert!(!diagnostic.contains("private"));
    assert!(!diagnostic.contains("tallyline-support-export"));
    assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 0);
    remove_directory(directory).unwrap();
}

#[test]
fn r_nf_6__log_collection_never_reads_game_logs_or_unknown_filenames() {
    let directory = directory("allowlist").unwrap();
    std::fs::write(directory.join("Game.log"), "PRIVATE GAME DATA\n").unwrap();
    std::fs::write(directory.join("foreign.log"), "PRIVATE FOREIGN DATA\n").unwrap();
    let source = DiagnosticLogFiles::new(directory.clone());
    assert!(source.collect().unwrap().is_none());
    std::fs::write(directory.join("tallyline.log"), "INFO approved app line\n").unwrap();
    let input = source.collect().unwrap().unwrap();
    let privacy = DiagnosticPrivacy::new(&[], &[]).unwrap();
    let preview = DiagnosticExportApplication::prepare(vec![input], &privacy).unwrap();
    let text = preview.entries().first().unwrap().text();
    assert!(text.contains("INFO approved app line"));
    assert!(!text.contains("PRIVATE"));
    assert!(!text.contains("Game.log"));
    assert!(!text.contains("foreign.log"));
    remove_directory(directory).unwrap();
}

#[test]
fn r_nf_6__bounded_log_tails_discard_partial_first_and_last_records() {
    let directory = directory("complete-lines").unwrap();
    let unfinished = "SECRET-FINAL-FRAGMENT";
    let retained_bytes = 64 * 1024 - unfinished.len() - 12;
    let line = "INFO kept\n";
    let mut complete = line.repeat(retained_bytes / line.len());
    let remaining = retained_bytes - complete.len();
    if remaining > 0 {
        complete.push_str(&" ".repeat(remaining - 1));
        complete.push('\n');
    }
    let source = format!("header\nignored-prefix-start PARTIAL-PRIVATE\n{complete}{unfinished}");
    std::fs::write(directory.join("tallyline.log"), source).unwrap();
    let input = DiagnosticLogFiles::new(directory.clone())
        .collect()
        .unwrap()
        .unwrap();
    let preview = DiagnosticExportApplication::prepare(
        vec![input],
        &DiagnosticPrivacy::new(&[], &[]).unwrap(),
    )
    .unwrap();
    let text = preview.entries().first().unwrap().text();
    assert!(text.contains("INFO kept"));
    assert!(!text.contains("PRIVATE"));
    assert!(!text.contains(unfinished));
    assert!(text.len() < 65 * 1024);
    remove_directory(directory).unwrap();
}

#[test]
fn r_nf_6__non_file_and_invalid_utf8_log_sources_fail_without_reflecting_content() {
    let directory = directory("invalid-log").unwrap();
    let log = directory.join("tallyline.log");
    let source = DiagnosticLogFiles::new(directory.clone());
    std::fs::create_dir(&log).unwrap();
    assert!(source.collect().is_err());
    std::fs::remove_dir(&log).unwrap();
    std::fs::write(&log, b"private-invalid-utf8-\xff\n").unwrap();
    let error = source.collect().unwrap_err();
    assert!(!format!("{error:?}").contains("private"));
    assert!(!format!("{error:?}").contains("invalid-log"));
    remove_directory(directory).unwrap();
}

#[cfg(unix)]
#[test]
fn r_nf_6__linked_application_logs_never_admit_game_log_content() {
    let directory = directory("linked-log").unwrap();
    let game_log = directory.join("Game.log");
    std::fs::write(&game_log, "PRIVATE GAME DATA\n").unwrap();
    std::os::unix::fs::symlink(&game_log, directory.join("tallyline.log")).unwrap();
    assert!(
        DiagnosticLogFiles::new(directory.clone())
            .collect()
            .is_err()
    );
    assert_eq!(
        std::fs::read_to_string(&game_log).unwrap(),
        "PRIVATE GAME DATA\n"
    );
    remove_directory(directory).unwrap();
}

fn directory(label: &str) -> std::io::Result<PathBuf> {
    let root = std::env::temp_dir();
    for attempt in 0..100 {
        let directory = root.join(format!(
            "tallyline-support-export-{}-{label}-{attempt}",
            std::process::id()
        ));
        match std::fs::create_dir(&directory) {
            Ok(()) => return Ok(directory),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::other(
        "cannot reserve an exclusive export fixture",
    ))
}

fn remove_directory(directory: PathBuf) -> std::io::Result<()> {
    let root = std::env::temp_dir().canonicalize()?;
    let resolved = directory.canonicalize()?;
    assert_eq!(resolved.parent(), Some(root.as_path()));
    assert!(
        resolved
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|name| name.starts_with("tallyline-support-export-"))
    );
    std::fs::remove_dir_all(resolved)
}
