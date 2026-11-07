// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Recorded own-process lifecycle fixtures without native readers.

use super::{docker, files};
use std::{
    path::PathBuf,
    process::Command,
    time::{Duration, Instant},
};

pub(super) fn fixture(mode: &str) -> Result<String, String> {
    match mode {
        "deadline" | "blocked-stdin" | "cancel" => {
            std::thread::sleep(Duration::from_secs(5));
            Err("fixture deadline was not enforced".to_owned())
        }
        "admission" => {
            use std::io::Read;
            let mut bytes = Vec::new();
            std::io::stdin()
                .take(1024)
                .read_to_end(&mut bytes)
                .map_err(|_| "recorded stdin unreadable")?;
            Ok(format!(
                "received={};text={}",
                bytes.len(),
                String::from_utf8(bytes).map_err(|_| "recorded stdin is not UTF-8")?
            ))
        }
        "row" => Ok("x".repeat(65 * 1024)),
        "output" => Ok("x".repeat(1024 * 1024)),
        "error" => Err("recorded process refusal".to_owned()),
        "environment" => {
            if [
                "DOCKER_HOST",
                "DOCKER_CONTEXT",
                "CFLAGS",
                "TESSDATA_PREFIX",
                "OMP_NUM_THREADS",
            ]
            .iter()
            .any(|key| std::env::var_os(key).is_some())
            {
                Err("poisoned child environment".to_owned())
            } else {
                Ok("clean".to_owned())
            }
        }
        _ => Err("unknown recorded process fixture".to_owned()),
    }
}

pub(super) fn run(mode: &str) -> Result<(), String> {
    if !matches!(
        mode,
        "deadline"
            | "blocked-stdin"
            | "output"
            | "error"
            | "environment"
            | "cancel"
            | "row"
            | "stderr"
            | "runtime-stderr"
            | "admission-reject"
            | "admission-accept"
            | "admission-limit-mismatch"
    ) {
        return Err("unknown recorded process contract".to_owned());
    }
    let executable = std::env::current_exe().map_err(|_| "fixture executable unavailable")?;
    let mut command = Command::new(executable);
    command.env_clear().args([
        "fixture",
        if matches!(
            mode,
            "admission-reject" | "admission-accept" | "admission-limit-mismatch"
        ) {
            "admission"
        } else if matches!(mode, "stderr" | "runtime-stderr") {
            "error"
        } else {
            mode
        },
    ]);
    let cancelled = PathBuf::from(format!("research-contract-cancel-{}", std::process::id()));
    if cancelled.exists() {
        return Err("fixture cancellation path must be fresh".to_owned());
    }
    if mode == "cancel" {
        files::write(&cancelled, b"public fixture cancellation")?;
    }
    let input = if mode == "blocked-stdin" {
        vec![0; 2 * 1024 * 1024]
    } else {
        Vec::new()
    };
    let input = if matches!(
        mode,
        "admission-reject" | "admission-accept" | "admission-limit-mismatch"
    ) {
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        sender
            .send(recorded_admission(mode)?)
            .map_err(|_| "fixture admission unavailable")?;
        docker::Input::AfterLocalProof {
            bytes: b"RECORDED_PRIVATE_BYTES".to_vec(),
            decision: receiver,
        }
    } else {
        docker::Input::Immediate(input)
    };
    let output = docker::bounded(
        command,
        input,
        64 * 1024,
        if mode == "row" { Some(1024) } else { None },
        if mode == "stderr" {
            docker::Diagnostics::SourceBuild
        } else {
            docker::Diagnostics::Null
        },
        Instant::now()
            + Duration::from_millis(if matches!(mode, "deadline" | "blocked-stdin") {
                150
            } else {
                2000
            }),
        &cancelled,
    )?;
    if mode == "cancel" {
        std::fs::remove_file(&cancelled)
            .map_err(|_| "owned cancellation fixture cleanup failed")?;
    }
    let passed = match mode {
        "deadline" | "blocked-stdin" => output.failure.as_deref() == Some("wall deadline exceeded"),
        "cancel" => output.failure.as_deref() == Some("operator cancellation"),
        "row" => output.failure.as_deref() == Some("stdout row limit exceeded"),
        "output" => output.failure.as_deref() == Some("stdout limit exceeded"),
        "stderr" => {
            output.code == Some(1)
                && output.failure.is_none()
                && String::from_utf8_lossy(&output.stderr).contains("recorded process refusal")
        }
        "admission-reject" | "admission-limit-mismatch" => {
            output.failure.as_deref() == Some("private stdin admission refused")
                && output.input_bytes == Some(0)
                && output.bytes == b"received=0;text=\n"
        }
        "admission-accept" => {
            output.failure.is_none()
                && output.code == Some(0)
                && output.input_bytes == Some(22)
                && output.bytes == b"received=22;text=RECORDED_PRIVATE_BYTES\n"
        }
        "runtime-stderr" => {
            output.code == Some(1) && output.failure.is_none() && output.stderr.is_empty()
        }
        "error" => output.code == Some(1) && output.failure.is_none(),
        "environment" => {
            output.code == Some(0) && output.failure.is_none() && output.bytes == b"clean\n"
        }
        _ => false,
    };
    if !passed {
        return Err(format!("recorded lifecycle contract failed: {mode}"));
    }
    Ok(())
}

fn recorded_admission(mode: &str) -> Result<docker::Admission, String> {
    if mode == "admission-accept" {
        return Ok(docker::Admission::Permit);
    }
    if mode == "admission-limit-mismatch" {
        let path = PathBuf::from(format!("research-contract-cgroup-{}", std::process::id()));
        files::write(&path,b"populated 1\nfrozen 0\n200000 100000\n4294967296\n0\n1\n783780\n8187904\nlow 0\nhigh 0\nmax 0\noom 0\noom_kill 0\noom_group_kill 0\npopulated 1\nfrozen 0\n")?;
        let result = docker::recorded_resource(&path, "8g");
        std::fs::remove_file(path).map_err(|_| "owned resource fixture cleanup failed")?;
        match result {
            Err(error) if error.starts_with("owned cgroup hard limits differ") => {
                return Ok(docker::Admission::Refuse);
            }
            Err(error) => return Err(error),
            Ok(_) => return Err("recorded alive limit mismatch was accepted".to_owned()),
        }
    }
    Ok(docker::Admission::Refuse)
}
