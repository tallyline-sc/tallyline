// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Headless process-start telemetry preparation contracts.

#![expect(
    non_snake_case,
    reason = "Requirement tags contain a double underscore."
)]

use std::process::Command;

use tallyline_adapter_platform::api::model::OcrProcessStartup;
use tallyline_adapter_platform::api::prepare_ocr_process;

#[test]
fn r_nf_12__missing_mitigation_relaunches_the_same_headless_probe() -> std::io::Result<()> {
    run_probe("prepared", ProbeEnvironment::Missing)
}

#[test]
fn r_nf_12__an_unsuccessful_first_bootstrap_cannot_loop() -> std::io::Result<()> {
    run_probe("refused", ProbeEnvironment::Failed)
}

#[test]
fn r_nf_12__existing_mitigation_needs_no_relaunch_even_with_a_bootstrap_marker()
-> std::io::Result<()> {
    run_probe("prepared", ProbeEnvironment::Prepared)
}

enum ProbeEnvironment {
    Missing,
    Prepared,
    Failed,
}

fn run_probe(expected: &str, environment: ProbeEnvironment) -> std::io::Result<()> {
    let marker = std::env::temp_dir().join(format!(
        "tallyline-ocr-bootstrap-{}-{expected}-{}",
        std::process::id(),
        match environment {
            ProbeEnvironment::Missing => "missing",
            ProbeEnvironment::Prepared => "prepared",
            ProbeEnvironment::Failed => "failed",
        }
    ));
    let mut command = Command::new(std::env::current_exe()?);
    command
        .args(["--exact", "bootstrap_probe", "--nocapture"])
        .env_remove("ORT_DISABLE_TELEMETRY")
        .env_remove("TALLYLINE_ORT_BOOTSTRAPPED")
        .env("TALLYLINE_OCR_PROBE_RESULT", expected)
        .env("TALLYLINE_OCR_PROBE_FILE", &marker);
    match environment {
        ProbeEnvironment::Missing => {}
        ProbeEnvironment::Prepared => {
            command
                .env("ORT_DISABLE_TELEMETRY", "1")
                .env("TALLYLINE_ORT_BOOTSTRAPPED", "1");
        }
        ProbeEnvironment::Failed => {
            command.env("TALLYLINE_ORT_BOOTSTRAPPED", "1");
        }
    }
    assert!(command.status()?.success());
    let actual = std::fs::read_to_string(&marker)?;
    std::fs::remove_file(marker)?;
    assert_eq!(actual, expected);
    Ok(())
}

#[test]
fn bootstrap_probe() -> std::io::Result<()> {
    let Some(expected) = std::env::var_os("TALLYLINE_OCR_PROBE_RESULT") else {
        return Ok(());
    };
    let actual = match prepare_ocr_process() {
        Ok(OcrProcessStartup::Prepared) => {
            assert_eq!(std::env::var_os("ORT_DISABLE_TELEMETRY"), Some("1".into()));
            "prepared"
        }
        Ok(OcrProcessStartup::Relaunched(status)) => {
            assert!(status.success());
            return Ok(());
        }
        Err(_) => "refused",
    };
    assert_eq!(std::ffi::OsStr::new(actual), expected);
    let marker = std::env::var_os("TALLYLINE_OCR_PROBE_FILE")
        .ok_or_else(|| std::io::Error::other("missing public probe marker"))?;
    use std::io::Write;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(marker)?
        .write_all(actual.as_bytes())
}
#[test]
fn r_nf_12__child_exit_classification_preserves_application_codes_and_refuses_native_crash_codes() {
    use tallyline_adapter_platform::api::model::OcrChildExit;
    for code in [0, 7, 255] {
        #[cfg(windows)]
        let status = {
            use std::os::windows::process::ExitStatusExt;
            std::process::ExitStatus::from_raw(u32::from(code))
        };
        #[cfg(unix)]
        let status = {
            use std::os::unix::process::ExitStatusExt;
            std::process::ExitStatus::from_raw(i32::from(code) << 8)
        };
        assert_eq!(OcrChildExit::from_status(&status), OcrChildExit::Code(code));
    }
    #[cfg(windows)]
    let abnormal = {
        use std::os::windows::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(u32::MAX)
    };
    #[cfg(unix)]
    let abnormal = {
        use std::os::unix::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(9)
    };
    assert_eq!(OcrChildExit::from_status(&abnormal), OcrChildExit::Abnormal);
}
