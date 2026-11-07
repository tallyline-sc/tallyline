// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Compiled application identity before native bootstrap.

#![expect(
    non_snake_case,
    reason = "Requirement tags use the prescribed separator."
)]

use tallyline_app::api::model::{BuildInformation, BuildKind, BuildTarget};

#[test]
fn r_nf_11_r_api_7__build_identity_uses_the_ci_tag_instead_of_cargo_package_version() {
    let info = BuildInformation::new(
        Some("v9.8.7"),
        Some("0123456789abcdef0123456789abcdef01234567"),
        BuildTarget::WindowsX64,
    )
    .unwrap();
    assert_eq!(info.kind(), BuildKind::Release);
    assert_eq!(info.version(), "9.8.7");
    assert_eq!(info.release_tag(), Some("v9.8.7"));
    assert_eq!(
        info.source_commit(),
        Some("0123456789abcdef0123456789abcdef01234567")
    );
}

#[test]
fn r_nf_11__development_identity_is_fixed_and_contains_no_release_claim() {
    let info = BuildInformation::new(None, None, BuildTarget::LinuxX64).unwrap();
    assert_eq!(info.kind(), BuildKind::Development);
    assert_eq!(info.version(), "0.0.0");
    assert_eq!(info.release_tag(), None);
    assert_eq!(info.source_commit(), None);
}

#[test]
fn r_nf_11__partial_or_invalid_release_metadata_is_rejected() {
    for (tag, commit) in [
        (Some("v9.8.7"), None),
        (None, Some("0123456789abcdef0123456789abcdef01234567")),
        (
            Some("v09.8.7"),
            Some("0123456789abcdef0123456789abcdef01234567"),
        ),
        (
            Some("v9.8.7-rc.1"),
            Some("0123456789abcdef0123456789abcdef01234567"),
        ),
        (Some("v9.8.7"), Some("not-a-source-commit")),
        (Some(""), Some("")),
    ] {
        assert!(BuildInformation::new(tag, commit, BuildTarget::WindowsX64).is_err());
    }
}

#[test]
fn r_nf_11_r_sec_8__headless_identity_returns_before_the_native_bootstrap_port() {
    let info = BuildInformation::new(
        Some("v9.8.7"),
        Some("0123456789abcdef0123456789abcdef01234567"),
        BuildTarget::WindowsX64,
    )
    .unwrap();
    let called = std::cell::Cell::new(false);
    let mut output = Vec::new();
    let status = tallyline_app::api::enter_application(
        [std::ffi::OsString::from("--build-info")],
        Ok(info),
        &mut output,
        |_| {
            called.set(true);
            Ok(std::process::ExitCode::SUCCESS)
        },
    )
    .unwrap();
    assert_eq!(status, std::process::ExitCode::SUCCESS);
    assert!(!called.get());
    assert!(output.len() <= 1024);
    let report: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(report["schema_version"], 1);
    assert_eq!(report["kind"], "release");
    assert_eq!(report["version"], "9.8.7");
    assert_eq!(report["release_tag"], "v9.8.7");
    assert_eq!(
        report["source_commit"],
        "0123456789abcdef0123456789abcdef01234567"
    );
    assert_eq!(report["target"], "x86_64-pc-windows-msvc");
}

#[test]
fn r_nf_11_r_sec_8__invalid_metadata_or_query_never_enters_native_bootstrap() {
    for (arguments, info) in [
        (
            Vec::<std::ffi::OsString>::new(),
            BuildInformation::new(Some("v9.8.7"), None, BuildTarget::WindowsX64),
        ),
        (
            vec!["--build-info".into(), "--assets".into()],
            BuildInformation::new(None, None, BuildTarget::WindowsX64),
        ),
        (
            vec!["--assets".into(), "missing".into(), "--build-info".into()],
            BuildInformation::new(None, None, BuildTarget::WindowsX64),
        ),
    ] {
        let called = std::cell::Cell::new(false);
        let mut output = Vec::new();
        let result = tallyline_app::api::enter_application(arguments, info, &mut output, |_| {
            called.set(true);
            Ok(std::process::ExitCode::SUCCESS)
        });
        assert!(result.is_err());
        assert!(!called.get());
        assert!(output.is_empty());
    }
}

#[test]
fn r_nf_11_r_sec_8__actual_headless_process_needs_no_resources_or_user_state() {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "tallyline-build-info-{}-{unique}",
        std::process::id()
    ));
    std::fs::create_dir(&directory).unwrap();
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_tallyline"));
    command
        .arg("--build-info")
        .current_dir(&directory)
        .env_clear()
        .env("PATH", "")
        .env("TALLYLINE_RELEASE_TAG", "v99.98.97")
        .env(
            "TALLYLINE_RELEASE_COMMIT",
            "ffffffffffffffffffffffffffffffffffffffff",
        )
        .env("HOME", &directory)
        .env("USERPROFILE", &directory)
        .env("LOCALAPPDATA", directory.join("local"))
        .env("APPDATA", directory.join("roaming"))
        .env("XDG_DATA_HOME", directory.join("data"))
        .env("XDG_CONFIG_HOME", directory.join("config"))
        .env("XDG_STATE_HOME", directory.join("state"))
        .env("XDG_CACHE_HOME", directory.join("cache"))
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    if let Some(windows) = std::env::var_os("SystemRoot") {
        command.env("SystemRoot", windows);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = command.spawn().unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if std::time::Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("Owned headless build-info process exceeded its deadline");
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.len() <= 1024);
    assert!(output.stderr.is_empty());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let info = tallyline_app::api::compiled_build_information().unwrap();
    assert_eq!(report["schema_version"], 1);
    assert_eq!(report["product"], "tallyline");
    assert_eq!(report["version"], info.version());
    assert_eq!(report["target"], info.target().triple());
    assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 0);
}
