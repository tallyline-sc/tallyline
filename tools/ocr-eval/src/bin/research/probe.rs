// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cargo-built public-only isolation contracts.

use super::{
    docker::{Docker, Kind, Spec},
    elf, files,
};
use serde_json::json;
use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};

const PROBE_SOURCE: &str = include_str!("../isolation-probe.rs");
const MANIFEST: &str = "[package]\nname = \"ocr-research-isolation-probe\"\nversion = \"0.0.0\"\nedition = \"2024\"\nrust-version = \"1.99.0\"\nlicense = \"GPL-3.0-or-later\"\n[workspace]\n[lints.rust]\nunsafe_code = \"forbid\"\n";
const LOCK: &str =
    "version = 4\n\n[[package]]\nname = \"ocr-research-isolation-probe\"\nversion = \"0.0.0\"\n";

pub(super) fn run(directory: &Path) -> Result<(), String> {
    let parent = directory
        .parent()
        .ok_or("public probe directory needs an existing parent")?;
    files::reject_aliases(parent)?;
    fs::create_dir(directory).map_err(|_| "public probe directory must be fresh")?;
    for name in [
        "docker-config",
        "evidence",
        "work",
        "worker",
        "model",
        "probe-src",
    ] {
        fs::create_dir(directory.join(name))
            .map_err(|_| "public probe directory creation failed")?;
    }
    fs::create_dir(directory.join("work/tmp")).map_err(|_| "public probe temp creation failed")?;
    fs::create_dir(directory.join("probe-src/src"))
        .map_err(|_| "public probe source creation failed")?;
    files::write(&directory.join("docker-config/config.json"), b"{}")?;
    files::write(&directory.join("probe-src/Cargo.toml"), MANIFEST.as_bytes())?;
    files::write(&directory.join("probe-src/Cargo.lock"), LOCK.as_bytes())?;
    files::write(
        &directory.join("probe-src/src/main.rs"),
        PROBE_SOURCE.as_bytes(),
    )?;
    let docker = Docker::new(directory)?;
    let started = Instant::now();
    let spec = Spec {
        workdir: "/",
        kind: Kind::ProbeBuild,
        memory: 4_294_967_296,
        mounts: vec![
            (directory.join("probe-src"), "/probe", false),
            (directory.join("work"), "/work", true),
        ],
        argv: [
            "/usr/local/cargo/bin/cargo",
            "build",
            "--locked",
            "--offline",
            "--release",
            "--manifest-path",
            "/probe/Cargo.toml",
            "--target-dir",
            "/work/target",
            "--jobs",
            "2",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        started,
    };
    let attempt = docker.run(
        &spec,
        Vec::new(),
        started + Duration::from_secs(180),
        8 * 1024 * 1024,
    )?;
    files::write(
        &directory.join("evidence/probe-build.stderr"),
        &attempt.output.stderr,
    )?;
    files::write_json(
        &directory.join("evidence/probe-build.json"),
        &attempt.receipt,
    )?;
    if attempt.output.code != Some(0) || attempt.output.failure.is_some() {
        return Err("safe Rust public probe Cargo build failed".to_owned());
    }
    let elf_spec = Spec {
        argv: [
            "/usr/bin/readelf",
            "-h",
            "-d",
            "-l",
            "--version-info",
            "/work/target/release/ocr-research-isolation-probe",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        ..spec
    };
    let elf = docker.run(
        &elf_spec,
        Vec::new(),
        Instant::now() + Duration::from_secs(30),
        8 * 1024 * 1024,
    )?;
    if elf.output.code != Some(0) || elf.output.failure.is_some() {
        return Err("safe Rust probe ELF inspection failed".to_owned());
    }
    elf::validate(&elf.output.bytes)?;
    files::write(&directory.join("evidence/probe-elf.txt"), &elf.output.bytes)?;
    let probe = files::read(
        &directory.join("work/target/release/ocr-research-isolation-probe"),
        16 * 1024 * 1024,
    )?;
    files::write(&directory.join("worker/probe"), &probe)?;
    let build_control_started = Instant::now();
    let build_control = Spec {
        kind: Kind::Build,
        memory: 4_294_967_296,
        mounts: vec![
            (directory.join("probe-src"), "/probe", false),
            (directory.join("work"), "/work", true),
        ],
        argv: ["/usr/bin/cat", "/proc/sys/kernel/osrelease"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        started: build_control_started,
        workdir: "/",
    };
    let control = docker.run(
        &build_control,
        Vec::new(),
        build_control_started + Duration::from_secs(30),
        1024,
    )?;
    files::write_json(
        &directory.join("evidence/build-metadata-control.json"),
        &control.receipt,
    )?;
    if control.output.code != Some(0)
        || control.output.failure.is_some()
        || control.output.bytes != b"6.18.33.2-microsoft-standard-WSL2\n"
    {
        return Err("public model-free build metadata control failed".to_owned());
    }
    let baseline_started = Instant::now();
    let baseline = Spec {
        workdir: "/",
        kind: Kind::ControlRuntime,
        memory: 4_294_967_296,
        mounts: vec![
            (directory.join("worker"), "/worker", false),
            (directory.join("model"), "/model", false),
        ],
        argv: ["/usr/bin/nice", "-n", "5", "/worker/probe", "baseline"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        started: baseline_started,
    };
    let control = docker.run(
        &baseline,
        Vec::new(),
        baseline_started + Duration::from_secs(30),
        1024,
    )?;
    if control.output.code != Some(0)
        || control.output.failure.is_some()
        || control.output.bytes != b"safe-rust-no-write-control\n"
        || super::docker::normalise_diff(
            control
                .receipt
                .get("filesystem_diff")
                .and_then(serde_json::Value::as_str)
                .ok_or("control diff missing")?
                .as_bytes(),
        )? != vec!["A /model".to_owned(), "A /worker".to_owned()]
    {
        return Err("exact no-write Docker startup baseline differs".to_owned());
    }
    files::write_json(
        &directory.join("evidence/startup-control.json"),
        &control.receipt,
    )?;
    files::write(
        &directory.join("runtime-startup-diff.txt"),
        b"A /model\nA /worker\n",
    )?;
    let mut results = Vec::new();
    for memory in [4_294_967_296u64, 8_589_934_592] {
        for (mode, seconds) in [
            ("snapshot", 30u64),
            ("deadline", 10),
            ("output", 30),
            ("oom", 120),
        ] {
            let started = Instant::now();
            let spec = Spec {
                workdir: "/",
                kind: Kind::Runtime,
                memory,
                mounts: vec![
                    (directory.join("worker"), "/worker", false),
                    (directory.join("model"), "/model", false),
                ],
                argv: ["/usr/bin/nice", "-n", "5", "/worker/probe", mode]
                    .into_iter()
                    .map(str::to_owned)
                    .collect(),
                started,
            };
            let attempt = docker.run(
                &spec,
                Vec::new(),
                started + Duration::from_secs(seconds),
                8 * 1024 * 1024,
            )?;
            let label = format!("{memory}-{mode}");
            files::write(
                &directory.join("evidence").join(format!("{label}.stdout")),
                &attempt.output.bytes,
            )?;
            files::write_json(
                &directory.join("evidence").join(format!("{label}.json")),
                &attempt.receipt,
            )?;
            let passed = match mode {
                "snapshot" => {
                    attempt.output.code == Some(0)
                        && attempt.output.failure.is_none()
                        && String::from_utf8_lossy(&attempt.output.bytes)
                            .contains("isolation=passed")
                }
                "deadline" => attempt.output.failure.as_deref() == Some("wall deadline exceeded"),
                "output" => attempt.output.failure.as_deref() == Some("stdout limit exceeded"),
                "oom" => attempt.receipt.pointer("/final_state/OOMKilled") == Some(&json!(true)),
                _ => return Err("unknown fixed public probe condition".to_owned()),
            };
            results.push(json!({"condition":label,"passed":passed,"failure":attempt.output.failure,"receipt_sha256":files::hash(&files::read(&directory.join("evidence").join(format!("{label}.json")),16*1024*1024)?)}));
            if !passed {
                files::write_json(
                    &directory.join("probe.failed.json"),
                    &json!({"results":results}),
                )?;
                return Err(format!("public isolation probe failed: {label}"));
            }
        }
    }
    let mut admission_receipts = Vec::new();
    for memory in [4_294_967_296u64, 8_589_934_592] {
        let started = Instant::now();
        let spec = Spec {
            kind: Kind::Runtime,
            memory,
            mounts: vec![
                (directory.join("worker"), "/worker", false),
                (directory.join("model"), "/model", false),
            ],
            argv: ["/usr/bin/nice", "-n", "5", "/worker/probe", "stdin"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            started,
            workdir: "/",
        };
        let attempt = docker.run(
            &spec,
            b"PUBLIC_STDIN_PROOF_FIXTURE".to_vec(),
            started + Duration::from_secs(30),
            1024 * 1024,
        )?;
        let label = format!("{memory}-stdin-admission");
        files::write_json(
            &directory.join("evidence").join(format!("{label}.json")),
            &attempt.receipt,
        )?;
        if attempt.output.code != Some(0)
            || attempt.output.failure.is_some()
            || !attempt.output.bytes.starts_with(b"isolation=passed\n")
            || !attempt
                .output
                .bytes
                .ends_with(b"public stdin proof fixture passed\n")
            || attempt
                .receipt
                .pointer("/stdin_admission/state")
                .and_then(serde_json::Value::as_str)
                != Some("admitted")
            || attempt.receipt.get("input_bytes_written") != Some(&json!(26))
        {
            return Err(format!(
                "public owned-cgroup-before-stdin proof failed: {label}"
            ));
        }
        admission_receipts.push(json!({"condition":label,"sha256":files::hash(&files::read(&directory.join("evidence").join(format!("{label}.json")),16*1024*1024)?)}));
    }
    let controller = std::env::current_exe().map_err(|_| "controller identity unavailable")?;
    let receipt = json!({"all_public_isolation_gates":true,"source_sha256":files::hash(PROBE_SOURCE.as_bytes()),"probe_sha256":files::hash(&probe),"controller_sha256":files::hash(&files::read(&controller,256*1024*1024)?),"results":results,"stdin_admission_receipts":admission_receipts,"probe_elf_sha256":files::hash(&files::read(&directory.join("evidence/probe-elf.txt"),16*1024*1024)?),"build_control_receipt_sha256":files::hash(&files::read(&directory.join("evidence/build-metadata-control.json"),16*1024*1024)?),"startup_control_sha256":files::hash(&files::read(&directory.join("evidence/startup-control.json"),16*1024*1024)?),"models_mounted":false,"physical_weak_pc":false});
    files::write_json(&directory.join("probe.passed.json"), &receipt)
}

pub(super) fn verify(directory: &Path, controller: &str) -> Result<serde_json::Value, String> {
    let receipt = super::plan::json(&directory.join("probe.passed.json"))?;
    if receipt.get("all_public_isolation_gates") != Some(&json!(true))
        || receipt.get("models_mounted") != Some(&json!(false))
        || super::plan::text(&receipt, "controller_sha256")? != controller
        || super::plan::text(&receipt, "source_sha256")? != files::hash(PROBE_SOURCE.as_bytes())
    {
        return Err("matching public isolation evidence is unavailable".to_owned());
    }
    super::plan::digest(
        &directory.join("worker/probe"),
        super::plan::text(&receipt, "probe_sha256")?,
        "public probe",
    )?;
    super::plan::digest(
        &directory.join("evidence/probe-elf.txt"),
        super::plan::text(&receipt, "probe_elf_sha256")?,
        "public probe ELF",
    )?;
    super::plan::digest(
        &directory.join("evidence/startup-control.json"),
        super::plan::text(&receipt, "startup_control_sha256")?,
        "public startup control",
    )?;
    super::plan::digest(
        &directory.join("evidence/build-metadata-control.json"),
        super::plan::text(&receipt, "build_control_receipt_sha256")?,
        "public build metadata control",
    )?;
    let admission = super::plan::array(&receipt, "stdin_admission_receipts")?;
    let mut names = std::collections::BTreeSet::from([
        "4294967296-stdin-admission",
        "8589934592-stdin-admission",
    ]);
    if admission.len() != 2 {
        return Err("public stdin admission condition count differs".to_owned());
    }
    for entry in admission {
        let condition = super::plan::text(entry, "condition")?;
        if !names.remove(condition) {
            return Err("public stdin admission condition differs".to_owned());
        }
        super::plan::digest(
            &directory.join("evidence").join(format!("{condition}.json")),
            super::plan::text(entry, "sha256")?,
            "public stdin admission receipt",
        )?;
    }
    let mut expected = std::collections::BTreeSet::new();
    for memory in [4_294_967_296u64, 8_589_934_592] {
        for mode in ["snapshot", "deadline", "output", "oom"] {
            expected.insert(format!("{memory}-{mode}"));
        }
    }
    let results = super::plan::array(&receipt, "results")?;
    if results.len() != 8 {
        return Err("public isolation condition count differs".to_owned());
    }
    for result in results {
        let condition = super::plan::text(result, "condition")?;
        if !expected.remove(condition) || result.get("passed") != Some(&json!(true)) {
            return Err("public isolation conditions differ".to_owned());
        }
        super::plan::digest(
            &directory.join("evidence").join(format!("{condition}.json")),
            super::plan::text(result, "receipt_sha256")?,
            "public isolation receipt",
        )?;
    }
    Ok(receipt)
}
