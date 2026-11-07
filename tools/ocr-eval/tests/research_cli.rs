// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Real-file contracts for the isolated research command.

use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> std::io::Result<Self> {
        Self::with_timestamp(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
        )
    }
    fn with_timestamp(timestamp: u128) -> std::io::Result<Self> {
        static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "research-contract-{}-{timestamp}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed),
        ));
        Self::create(path)
    }
    fn create(path: PathBuf) -> std::io::Result<Self> {
        fs::create_dir(&path)?;
        Ok(Self(path))
    }
    fn run(&self, args: &[&str]) -> std::io::Result<std::process::Output> {
        Command::new(env!("CARGO_BIN_EXE_tesseract-research"))
            .args(args)
            .env("DOCKER_HOST", "tcp://192.0.2.1:2375")
            .env("DOCKER_CONTEXT", "RECORDED_POISON")
            .env("TESSDATA_PREFIX", "RECORDED_POISON")
            .env("CFLAGS", "RECORDED_POISON")
            .current_dir(&self.0)
            .output()
    }
    fn bitmap(&self) -> std::io::Result<()> {
        let mut bytes = vec![
            66, 77, 70, 0, 0, 0, 0, 0, 0, 0, 54, 0, 0, 0, 40, 0, 0, 0, 2, 0, 0, 0, 2, 0, 0, 0, 1,
            0, 24, 0, 0, 0, 0, 0, 16, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ];
        bytes.extend([255, 0, 0, 255, 255, 255, 0, 0, 0, 0, 255, 0, 255, 0, 0, 0]);
        fs::write(self.0.join("input.bmp"), bytes)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let result = fs::remove_dir_all(&self.0);
        assert!(
            result.is_ok(),
            "fixture cleanup failed for {}: {result:?}; remaining entries: {:?}",
            self.0.display(),
            fs::read_dir(&self.0).map(|entries| entries.collect::<Vec<_>>())
        );
    }
}

#[test]
fn equal_clock_readings_keep_research_cli_working_directories_independent() {
    let first = Fixture::with_timestamp(0).unwrap();
    let second = Fixture::with_timestamp(0).unwrap();
    fs::write(
        second.0.join("build-evidence.json"),
        br#"{"generated_artifacts":[]}"#,
    )
    .unwrap();
    drop(first);
    let output = second.run(&["verify-evidence", "."]).unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn existing_research_fixture_directory_is_refused_without_adopting_its_files() {
    let parent = Fixture::new().unwrap();
    let reserved = parent.0.join("reserved");
    fs::create_dir(&reserved).unwrap();
    fs::write(reserved.join("owner.txt"), b"another fixture").unwrap();
    let reservation = Fixture::create(reserved.clone());
    assert!(matches!(reservation, Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists));
    assert_eq!(
        fs::read(reserved.join("owner.txt")).unwrap(),
        b"another fixture"
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use the prescribed separator."
)]
fn r_sec_1__research_contract_files_follow_system_temporary_storage() {
    let fixture = Fixture::new().unwrap();
    assert!(fixture.0.starts_with(std::env::temp_dir()));
}

#[test]
fn recorded_tsv_preserves_literal_currency_sign_and_unit() {
    let fixture = Fixture::new().unwrap();
    fixture.bitmap().unwrap();
    fs::write(fixture.0.join("recorded.tsv"), "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n1\t1\t0\t0\t0\t0\t0\t0\t2\t2\t-1\t\n5\t1\t1\t1\t1\t1\t0\t0\t2\t1\t87.125\t−1,234.50¤/SCU\n").unwrap();
    let output = fixture
        .run(&["tsv", "input.bmp", "recorded.tsv", "parsed.json"])
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let parsed: serde_json::Value =
        serde_json::from_slice(&fs::read(fixture.0.join("parsed.json")).unwrap()).unwrap();
    assert_eq!(parsed["words"][0]["text"], "−1,234.50¤/SCU");
    assert!(!String::from_utf8(output.stdout).unwrap().contains("SCU"));
}

#[test]
fn preparation_rejects_unfrozen_metadata_before_creating_owned_state() {
    let fixture = Fixture::new().unwrap();
    fs::create_dir(fixture.0.join("campaign")).unwrap();
    fs::create_dir(fixture.0.join("proposal")).unwrap();
    fs::write(fixture.0.join("proposal/scope.json"), b"{}").unwrap();
    let output = fixture.run(&["prepare", "campaign", "proposal"]).unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("scope digest mismatch"));
    assert!(!fixture.0.join("campaign/controller").exists());
}

#[test]
fn bounded_recorded_process_contracts_reap_deadline_output_and_blocked_stdin() {
    let fixture = Fixture::new().unwrap();
    for mode in [
        "deadline",
        "output",
        "blocked-stdin",
        "error",
        "environment",
        "row",
        "cancel",
        "stderr",
        "runtime-stderr",
        "admission-reject",
        "admission-accept",
        "admission-limit-mismatch",
    ] {
        let output = fixture.run(&["contract", mode]).unwrap();
        assert!(
            output.status.success(),
            "{mode}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("contract passed"));
    }
}

fn png_chunk(kind: &[u8; 4], data: &[u8], output: &mut Vec<u8>) {
    output.extend((data.len() as u32).to_be_bytes());
    output.extend(kind);
    output.extend(data);
    let mut crc = 0xffff_ffffu32;
    for byte in kind.iter().chain(data) {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ if crc & 1 == 1 { 0xedb8_8320 } else { 0 };
        }
    }
    output.extend((!crc).to_be_bytes());
}

fn tiny_png() -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    png_chunk(
        b"IHDR",
        &[0, 0, 0, 2, 0, 0, 0, 2, 8, 2, 0, 0, 0],
        &mut bytes,
    );
    let pixels = [0u8, 255, 0, 0, 0, 255, 0, 0, 0, 0, 255, 255, 255, 255];
    let mut zlib = vec![0x78, 0x01, 0x01, 14, 0, 0xf1, 0xff];
    zlib.extend(pixels);
    let (mut a, mut b) = (1u32, 0u32);
    for byte in pixels {
        a = (a + u32::from(byte)) % 65521;
        b = (b + a) % 65521;
    }
    zlib.extend(((b << 16) | a).to_be_bytes());
    png_chunk(b"IDAT", &zlib, &mut bytes);
    png_chunk(b"IEND", &[], &mut bytes);
    bytes
}

#[test]
fn png_conversion_preserves_worked_bgr_bottom_up_rows_and_zero_padding() {
    let fixture = Fixture::new().unwrap();
    fs::write(fixture.0.join("colours.png"), tiny_png()).unwrap();
    let output = fixture.run(&["bmp", "colours.png", "colours.bmp"]).unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let bytes = fs::read(fixture.0.join("colours.bmp")).unwrap();
    assert_eq!(bytes.len(), 70);
    assert_eq!(
        bytes.get(54..),
        Some([255, 0, 0, 255, 255, 255, 0, 0, 0, 0, 255, 0, 255, 0, 0, 0].as_slice())
    );
    assert!(
        !fixture
            .run(&["bmp", "colours.png", "colours.bmp"])
            .unwrap()
            .status
            .success()
    );
}

#[test]
fn malformed_bmp_and_tsv_are_refused_without_creating_parsed_output() {
    let fixture = Fixture::new().unwrap();
    fixture.bitmap().unwrap();
    let header = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n";
    for (index, body) in [
        "1\t1\t0\t0\t0\t0\t0\t0\t2\t2\t-1\t\n5\t1\t1\t1\t1\t1\t1\t0\t2\t1\t87\t12¤\n",
        "1\t1\t0\t0\t0\t0\t0\t0\t2\t2\t-1\t\n5\t1\t1\t1\t1\t1\t0\t0\t2\t1\tNaN\t12¤\n",
        "1\t1\t0\t0\t0\t0\t0\t0\t2\t2\t-1\t\n2\t1\t1\t1\t0\t0\t0\t0\t2\t1\t-1\t\n",
    ]
    .iter()
    .enumerate()
    {
        fs::write(fixture.0.join("bad.tsv"), format!("{header}{body}")).unwrap();
        let destination = format!("bad-{index}.json");
        assert!(
            !fixture
                .run(&["tsv", "input.bmp", "bad.tsv", &destination])
                .unwrap()
                .status
                .success(),
            "invalid TSV{index} accepted"
        );
        assert!(!fixture.0.join(destination).exists());
    }
    fs::write(fixture.0.join("bad.tsv"), [0xff]).unwrap();
    assert!(
        !fixture
            .run(&["tsv", "input.bmp", "bad.tsv", "invalid-utf8.json"])
            .unwrap()
            .status
            .success()
    );
    assert!(!fixture.0.join("invalid-utf8.json").exists());
    fs::write(
        fixture.0.join("valid.tsv"),
        format!("{header}1\t1\t0\t0\t0\t0\t0\t0\t2\t2\t-1\t\n"),
    )
    .unwrap();
    let control = fixture
        .run(&["tsv", "input.bmp", "valid.tsv", "valid-control.json"])
        .unwrap();
    assert!(
        control.status.success(),
        "{}",
        String::from_utf8_lossy(&control.stderr)
    );
    let parsed: serde_json::Value =
        serde_json::from_slice(&fs::read(fixture.0.join("valid-control.json")).unwrap()).unwrap();
    assert_eq!(parsed["width"], 2);
    assert_eq!(parsed["height"], 2);
    assert_eq!(parsed["words"], serde_json::json!([]));
    let original = fs::read(fixture.0.join("input.bmp")).unwrap();
    for (index, (offset, value, diagnostic)) in [
        (18usize, 255u8, "bitmap length or palette is invalid"),
        (28, 32, "only canonical 24bpp BI_RGB BMP is accepted"),
        (30, 1, "only canonical 24bpp BI_RGB BMP is accepted"),
        (60, 1, "bitmap padding must be zero"),
    ]
    .iter()
    .enumerate()
    {
        let mut changed = original.clone();
        *changed.get_mut(*offset).unwrap() = *value;
        fs::write(fixture.0.join("bad.bmp"), changed).unwrap();
        let destination = format!("bad-bmp-{index}.json");
        let output = fixture
            .run(&["tsv", "bad.bmp", "valid.tsv", &destination])
            .unwrap();
        assert!(!output.status.success(), "invalid BMP{index} accepted");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(diagnostic),
            "BMP{index}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!fixture.0.join(destination).exists());
    }
}

#[test]
fn changed_generated_native_evidence_is_refused_before_any_process_start() {
    let fixture = Fixture::new().unwrap();
    fs::create_dir(fixture.0.join("work")).unwrap();
    fs::write(fixture.0.join("work/config_auto.h"), b"abc").unwrap();
    fs::write(fixture.0.join("build-evidence.json"),br#"{"generated_artifacts":[{"path":"work/config_auto.h","sha256":"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"}]}"#).unwrap();
    assert!(
        fixture
            .run(&["verify-evidence", "."])
            .unwrap()
            .status
            .success()
    );
    fs::write(fixture.0.join("work/config_auto.h"), b"abd").unwrap();
    let output = fixture.run(&["verify-evidence", "."]).unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("generated build evidence digest mismatch")
    );
    assert!(!fixture.0.join("synthetic.started").exists());
}

#[test]
fn missing_and_tampered_models_are_refused_at_the_file_boundary() {
    let fixture = Fixture::new().unwrap();
    let missing = fixture
        .run(&["verify-model", "best", "missing.traineddata"])
        .unwrap();
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("unavailable"));
    fs::write(
        fixture.0.join("tampered.traineddata"),
        b"recorded native bytes; not a model",
    )
    .unwrap();
    let tampered = fixture
        .run(&["verify-model", "fast", "tampered.traineddata"])
        .unwrap();
    assert!(!tampered.status.success());
    assert!(String::from_utf8_lossy(&tampered.stderr).contains("model digest mismatch"));
}

#[test]
fn preprocessing_plan_refuses_shells_plugins_and_foreign_outputs_without_execution() {
    let fixture = Fixture::new().unwrap();
    let mut plan = String::from("tallyline-preprocess-v1\n");
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/preprocessing-commands.json")).unwrap();
    let mut index = 0;
    for case in cases {
        let directory = case["directory"].as_str().unwrap();
        let arguments = case["preprocessed_arguments"]
            .as_array()
            .unwrap()
            .iter()
            .map(|arg| arg.as_str().unwrap())
            .collect::<Vec<_>>()
            .join("\t");
        for _ in 0..case["copies"].as_u64().unwrap() {
            plan.push_str(
                &format!("{directory}\t{arguments}\n").replace("INDEX", &index.to_string()),
            );
            index += 1;
        }
    }
    assert_eq!(index, 334);
    assert_eq!(plan.matches("\n/work/build-lept/src\t").count(), 182);
    assert_eq!(plan.matches("\n/work/build-tess\t").count(), 152);
    fs::write(fixture.0.join("preprocess.plan"), &plan).unwrap();
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_isolation-probe"))
            .args(["check-preprocess-plan", "preprocess.plan"])
            .current_dir(&fixture.0)
            .output()
            .unwrap()
    };
    let accepted = run();
    assert!(
        accepted.status.success(),
        "{}",
        String::from_utf8_lossy(&accepted.stderr)
    );
    assert_eq!(
        String::from_utf8(accepted.stdout).unwrap(),
        "approved preprocessing plan validated; no process started\n"
    );
    for changed in [
        plan.replace("/usr/bin/x86_64-linux-gnu-gcc-12", "/bin/sh"),
        plan.replace("\t-E\t", "\t-fplugin=/work/foreign.so\t-E\t"),
        plan.replace("/work/preprocessed-0.ii", "/tmp/foreign.ii"),
        plan.replace("\t-E\t", "\t-E;touch foreign\t"),
        plan.replace("/src/leptonica-1.87.0/src/adaptmap.c", "/src/foreign.c"),
        plan.replace(
            "/src/leptonica-1.87.0/src/adaptmap.c",
            "/src/leptonica-1.87.0/src/../foreign.c",
        ),
        plan.replace("\t-E\t", "\t-c\t"),
        plan.replace("\t-dD\n", "\n"),
        plan.replace("/work/preprocessed-0.ii", "/work/preprocessed-1.ii"),
        plan.replacen("\t-E\t", &format!("\t{}\t-E\t", "x".repeat(4097)), 1),
        plan.replacen("\t-E\t", &format!("\t{}-E\t", "-DTEST\t".repeat(128)), 1),
        plan.lines().take(334).collect::<Vec<_>>().join("\n"),
        format!(
            "{plan}{}\n",
            plan.lines()
                .nth(1)
                .unwrap()
                .replace("preprocessed-0.ii", "preprocessed-334.ii")
        ),
    ] {
        fs::write(fixture.0.join("preprocess.plan"), changed).unwrap();
        assert!(!run().status.success());
    }
    for (approved, rejected) in [
        ("/work/build-lept/src", "/work/build-lept"),
        ("/work/build-lept/src", "/work/build-lept/src/.."),
        ("/work/build-lept/src", "/work/build-lept/src/child"),
        ("/work/build-lept/src", "/work/build-lept/src-extra"),
        ("/work/build-lept/src", "/tmp/build-lept/src"),
        ("/work/build-tess", "/work/build-tess/.."),
        ("/work/build-tess", "/work/build-tess/child"),
        ("/work/build-tess", "/work/build-tess-extra"),
        ("/work/build-tess", "/tmp/build-tess"),
    ] {
        let changed = plan.replace(&format!("\n{approved}\t"), &format!("\n{rejected}\t"));
        assert_ne!(changed, plan);
        fs::write(fixture.0.join("preprocess.plan"), changed).unwrap();
        assert!(!run().status.success(), "accepted directory {rejected}");
    }
    fs::write(
        fixture.0.join("preprocess.plan"),
        "x".repeat(8 * 1024 * 1024 + 1),
    )
    .unwrap();
    let oversized = run();
    assert!(!oversized.status.success());
    assert_eq!(
        String::from_utf8(oversized.stdout).unwrap(),
        "isolation-probe: preprocessing plan exceeds bounds\n"
    );
    assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 1);
}

#[test]
fn recorded_owned_cgroup_peak_is_a_lower_bound_with_exact_limits() {
    let fixture = Fixture::new().unwrap();
    fs::write(fixture.0.join("cgroup.txt"),b"populated 1\nfrozen 0\n200000 100000\n4294967296\n0\n1\n783780\n8187904\nlow 0\nhigh 0\nmax 0\noom 0\noom_kill 0\noom_group_kill 0\npopulated 1\nfrozen 0\n").unwrap();
    let output = fixture.run(&["resource", "cgroup.txt", "4g"]).unwrap();
    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["observed_memory_peak_bytes"], 8187904);
    assert_eq!(value["whole_attempt_peak_known"], false);
    assert!(
        !fixture
            .run(&["resource", "cgroup.txt", "8g"])
            .unwrap()
            .status
            .success()
    );
}

#[test]
fn unpopulated_starting_cgroup_cannot_be_misclassified_as_a_live_limit_refusal() {
    let fixture = Fixture::new().unwrap();
    fs::write(fixture.0.join("starting.txt"),b"populated 0\nfrozen 0\nmax 100000\nmax\nmax\nmax\n0\nlow 0\nhigh 0\nmax 0\noom 0\noom_kill 0\noom_group_kill 0\npopulated 0\nfrozen 0\n").unwrap();
    let output = fixture.run(&["resource", "starting.txt", "8g"]).unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("owned cgroup has no live population")
    );
}

#[test]
fn created_container_cannot_refuse_or_admit_using_pre_running_kernel_limits() {
    let fixture = Fixture::new().unwrap();
    fs::write(fixture.0.join("initial.txt"),b"populated 1\nfrozen 0\nmax 100000\nmax\nmax\nmax\n783780\n8187904\nlow 0\nhigh 0\nmax 0\noom 0\noom_kill 0\noom_group_kill 0\npopulated 1\nfrozen 0\n").unwrap();
    fs::write(fixture.0.join("state.json"),br#"{"State":{"Status":"created","Running":false,"Paused":false,"Restarting":false,"Dead":false}}"#).unwrap();
    let output = fixture
        .run(&["resource", "initial.txt", "8g", "state.json"])
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("owned container has not entered running state")
    );
    fs::write(fixture.0.join("state.json"),br#"{"State":{"Status":"running","Running":true,"Paused":false,"Restarting":false,"Dead":false}}"#).unwrap();
    let output = fixture
        .run(&["resource", "initial.txt", "8g", "state.json"])
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("owned cgroup hard limits differ"));
}
