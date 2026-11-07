// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exclusive private development-artifact filesystem contracts.

use std::path::PathBuf;
use tallyline_adapter_files::api::write_private_artifact;

#[test]
fn private_artifact_preserves_exact_bytes_without_overwriting_existing_data() {
    let directory = directory("exclusive").unwrap();
    let path = directory.join("raw.json");
    write_private_artifact(&path, b"first-private-artifact").unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"first-private-artifact");
    assert!(write_private_artifact(&path, b"replacement").is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"first-private-artifact");
    std::fs::remove_dir_all(directory).unwrap();
}

#[cfg(unix)]
#[test]
fn private_artifact_is_created_owner_only() {
    use std::os::unix::fs::PermissionsExt;
    let directory = directory("permissions").unwrap();
    let path = directory.join("raw.json");
    write_private_artifact(&path, b"private").unwrap();
    assert_eq!(
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    std::fs::remove_dir_all(directory).unwrap();
}

fn directory(label: &str) -> std::io::Result<PathBuf> {
    for attempt in 0..100 {
        let path = std::env::temp_dir().join(format!(
            "tallyline-private-artifact-{}-{label}-{attempt}",
            std::process::id()
        ));
        match std::fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::other(
        "cannot reserve private artifact fixture",
    ))
}
