// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Public loader gates without initialising a native library.

#![expect(
    non_snake_case,
    reason = "Requirement tags contain a double underscore."
)]

use tallyline_adapter_ocr::api::model::{
    OcrRuntime, OcrRuntimeLoadToken, RuntimeHostChecks, RuntimeHostError, RuntimeLoadError,
};

#[test]
fn r_nf_12__the_loader_refuses_missing_startup_mitigation_before_host_or_ort_calls()
-> std::io::Result<()> {
    let child = publish_probe(|| {
        std::process::Command::new(std::env::current_exe()?)
            .args(["--exact", "loader_probe", "--nocapture"])
            .env_remove("ORT_DISABLE_TELEMETRY")
            .env("TALLYLINE_LOADER_PROBE", "missing-telemetry")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
    })?;
    let result = child.wait_with_output()?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    Ok(())
}

#[test]
fn r_sec_7_r_sec_8__a_missing_or_same_size_wrong_sibling_cannot_reach_host_checks()
-> std::io::Result<()> {
    run_prepared_probe("missing-library", false)?;
    run_prepared_probe("wrong-library", false)
}

#[test]
fn r_sec_7_r_sec_8__verified_bytes_reach_search_policy_and_its_refusal_stops_native_loading()
-> std::io::Result<()> {
    run_prepared_probe("verified-refused", true)
}

#[test]
fn loader_probe() {
    let Some(mode) = std::env::var_os("TALLYLINE_LOADER_PROBE") else {
        return;
    };
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let before = std::sync::Arc::clone(&calls);
    let after = std::sync::Arc::clone(&calls);
    let checks = RuntimeHostChecks::new(
        move |_| {
            before.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Err(RuntimeHostError::SearchPolicy)
        },
        move |_| {
            after.fetch_add(100, std::sync::atomic::Ordering::SeqCst);
            Err(RuntimeHostError::MappedLibrary)
        },
    );
    let mut token = OcrRuntimeLoadToken::new();
    let result = OcrRuntime::load(&mut token, checks);
    assert!(!token.native_load_attempted());
    let expected_calls = match mode.to_str().unwrap() {
        "missing-telemetry" => {
            assert!(matches!(
                result,
                Err(RuntimeLoadError::TelemetryNotPrepared)
            ));
            0
        }
        "missing-library" => {
            assert!(matches!(result, Err(RuntimeLoadError::Read)));
            0
        }
        "wrong-library" => {
            assert!(matches!(result, Err(RuntimeLoadError::HashMismatch)));
            0
        }
        "verified-refused" => {
            assert!(matches!(
                result,
                Err(RuntimeLoadError::Host(RuntimeHostError::SearchPolicy))
            ));
            1
        }
        other => panic!("unexpected loader probe {other}"),
    };
    assert_eq!(
        calls.load(std::sync::atomic::Ordering::SeqCst),
        expected_calls
    );
}

#[cfg(windows)]
const LIBRARY: (&str, &str, u64) = ("windows", "onnxruntime.dll", 17_253_176);
#[cfg(target_os = "linux")]
const LIBRARY: (&str, &str, u64) = ("linux", "libonnxruntime.so.1.31.0", 29_345_384);

fn run_prepared_probe(mode: &str, verified: bool) -> std::io::Result<()> {
    run_owned_probe(mode, |directory| {
        if verified {
            let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("assets/runtime")
                .join(LIBRARY.0)
                .join(LIBRARY.1);
            fixture_stage(
                "copy verified library",
                std::fs::copy(source, directory.join(LIBRARY.1)),
            )?;
        } else if mode == "wrong-library" {
            fixture_stage(
                "size invalid library",
                fixture_stage(
                    "create invalid library",
                    std::fs::File::create(directory.join(LIBRARY.1)),
                )?
                .set_len(LIBRARY.2),
            )?;
        }
        Ok(())
    })
}

fn run_owned_probe(
    mode: &str,
    prepare: impl FnOnce(&std::path::Path) -> std::io::Result<()>,
) -> std::io::Result<()> {
    let base = fixture_stage(
        "canonicalise temporary root",
        std::env::temp_dir().canonicalize(),
    )?;
    let mut directory = None;
    for attempt in 0..100 {
        let path = base.join(format!(
            "tallyline-loader-{}-{mode}-{attempt}",
            std::process::id()
        ));
        match std::fs::create_dir(&path) {
            Ok(()) => {
                directory = Some(path);
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return fixture_stage("reserve directory", Err(error)),
        }
    }
    let directory =
        directory.ok_or_else(|| std::io::Error::other("cannot reserve loader probe directory"))?;
    let child = (|| {
        let child = publish_probe(|| {
            let executable = directory.join(if cfg!(windows) { "probe.exe" } else { "probe" });
            fixture_stage(
                "copy executable",
                std::fs::copy(
                    fixture_stage("locate executable", std::env::current_exe())?,
                    &executable,
                ),
            )?;
            prepare(&directory)?;
            fixture_stage(
                "start child",
                std::process::Command::new(executable)
                    .args(["--exact", "loader_probe", "--nocapture"])
                    .env("ORT_DISABLE_TELEMETRY", "1")
                    .env("TALLYLINE_LOADER_PROBE", mode)
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .spawn(),
            )
        })?;
        let outcome = fixture_stage("wait child", child.wait_with_output())?;
        if outcome.status.success() {
            Ok(())
        } else {
            Err(std::io::Error::other(format!(
                "loader fixture child status {}: {} {}",
                outcome.status,
                String::from_utf8_lossy(&outcome.stdout),
                String::from_utf8_lossy(&outcome.stderr)
            )))
        }
    })();
    let cleanup = (|| {
        let actual = fixture_stage("canonicalise cleanup directory", directory.canonicalize())?;
        let prefix = format!("tallyline-loader-{}-", std::process::id());
        if !actual.starts_with(&base)
            || !actual
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(&prefix))
        {
            return Err(std::io::Error::other(
                "loader cleanup escaped owned directory",
            ));
        }
        fixture_stage("remove owned directory", std::fs::remove_dir_all(actual))
    })();
    match (child, cleanup) {
        (Err(primary), Err(cleanup)) => Err(std::io::Error::new(
            primary.kind(),
            format!("{primary}; additional cleanup failure: {cleanup}"),
        )),
        (Err(primary), Ok(())) => Err(primary),
        (Ok(()), result) => result,
    }
}

/// Keeps executable-copy write handles out of every probe's process-creation window.
fn publish_probe<T>(operation: impl FnOnce() -> std::io::Result<T>) -> std::io::Result<T> {
    static PUBLICATION: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _publication = PUBLICATION
        .lock()
        .map_err(|error| std::io::Error::other(format!("loader publication lock: {error}")))?;
    operation()
}

fn fixture_stage<T>(operation: &'static str, result: std::io::Result<T>) -> std::io::Result<T> {
    result.map_err(|error| {
        std::io::Error::new(
            error.kind(),
            format!("loader fixture {operation}: {error:?}"),
        )
    })
}

#[test]
fn child_failure_is_returned_with_its_own_diagnostic_after_cleanup() {
    let error = run_prepared_probe("invalid-child", true).unwrap_err();
    let message = error.to_string();
    assert!(message.contains("loader fixture child status"), "{message}");
    assert!(
        message.contains("unexpected loader probe invalid-child"),
        "{message}"
    );
}

#[test]
fn setup_failure_removes_its_owned_directory_and_retains_the_io_error() {
    let mut owned = None;
    let error = run_owned_probe("setup-failure", |directory| {
        owned = Some(directory.to_owned());
        fixture_stage(
            "copy missing fixture library",
            std::fs::copy(directory.join("absent-library"), directory.join(LIBRARY.1)),
        )?;
        Ok(())
    })
    .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    assert!(error.to_string().contains("copy missing fixture library"));
    let directory = owned.unwrap();
    assert!(
        !directory.exists(),
        "setup failure retained {}",
        directory.display()
    );
}

#[test]
fn spawn_failure_removes_its_owned_directory_and_retains_the_io_error() {
    let mut owned = None;
    let error = run_owned_probe("spawn-failure", |directory| {
        owned = Some(directory.to_owned());
        std::fs::remove_file(directory.join(if cfg!(windows) { "probe.exe" } else { "probe" }))
    })
    .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    assert!(error.to_string().contains("loader fixture start child"));
    let directory = owned.unwrap();
    assert!(
        !directory.exists(),
        "spawn failure retained {}",
        directory.display()
    );
}

#[test]
fn concurrent_preparation_and_startup_keep_the_loader_refusals() -> std::io::Result<()> {
    std::thread::scope(|scope| {
        let prepared = scope.spawn(|| -> std::io::Result<()> {
            for _ in 0..4 {
                run_prepared_probe("missing-library", false)?;
                run_prepared_probe("wrong-library", false)?;
                run_prepared_probe("verified-refused", true)?;
            }
            Ok(())
        });
        let startup = scope.spawn(|| -> std::io::Result<()> {
            for _ in 0..12 {
                r_nf_12__the_loader_refuses_missing_startup_mitigation_before_host_or_ort_calls()?;
            }
            Ok(())
        });
        for handle in [prepared, startup] {
            handle
                .join()
                .map_err(|_| std::io::Error::other("loader fixture worker panicked"))??;
        }
        Ok(())
    })
}
