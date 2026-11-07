// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Public contracts for suspended owned-child execution and cancellation.

#![cfg(all(target_os = "windows", target_arch = "x86_64", target_env = "msvc"))]
#![expect(
    non_snake_case,
    reason = "Requirement tags contain a double underscore."
)]

use std::collections::BTreeMap;
use std::error::Error;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use tallyline_adapter_platform::api::OwnedProcess;
use tallyline_adapter_platform::api::model::{
    CleanupOutcome, OwnedProcessCommand, OwnedProcessLimits, OwnedProcessState, ResumeRefusal,
};
use tempfile::TempDir;

type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

const CLEANUP_BUDGET: Duration = Duration::from_secs(5);
const FIXTURE_EXECUTION_BUDGET: Duration = Duration::from_secs(5);
const MARKER_POLL: Duration = Duration::from_millis(10);
const JOB_COMMIT_BYTES: usize = 1024 * 1024 * 1024;
const CPU_RATE_PERCENT: u8 = 25;
const ACTIVE_PROCESS_LIMIT: u32 = 4;
const CAPTURE_BYTES_PER_STREAM: usize = 16 * 1024;
const STDOUT_SENTINEL: &[u8] = b"owned-child-stdout";
const STDERR_SENTINEL: &[u8] = b"owned-child-stderr";
const EXECUTED_MARKER: &[u8] = b"executed";
const MARKER_READ_BYTES: u64 = 32;

#[test]
fn r_vlm_7_r_nf_3__cancelling_a_suspended_owned_child_prevents_execution_and_releases_resources()
-> TestResult {
    let fixture = Fixture::new()?;
    let mut child = fixture.create_suspended()?;
    std::thread::sleep(Duration::from_millis(100));
    let suspended = child.observe()?;
    assert_eq!(suspended.state(), OwnedProcessState::Suspended);
    assert!(suspended.root_is_live_member());
    assert_eq!(suspended.generation(), child.generation());
    assert!(!fixture.marker.try_exists()?);

    let cleanup_started = Instant::now();
    child.cancel();
    assert_eq!(child.state(), OwnedProcessState::Revoked);
    assert!(matches!(child.resume(), Err(ResumeRefusal::Revoked)));
    let cleanup = child.cleanup(CLEANUP_BUDGET)?;
    assert!(cleanup_started.elapsed() <= CLEANUP_BUDGET);
    assert_eq!(cleanup.outcome(), CleanupOutcome::ObservedReleased);
    assert!(cleanup.root_exited());
    assert_eq!(cleanup.active_processes(), Some(0));
    assert_eq!(cleanup.member_processes(), Some(0));
    assert!(cleanup.stdout_eof());
    assert!(cleanup.stderr_eof());
    assert!(cleanup.stdout().is_empty());
    assert!(cleanup.stderr().is_empty());
    assert!(cleanup.owned_handles_closed());
    assert!(!fixture.private.try_exists()?);
    assert!(!fixture.marker.try_exists()?);
    assert!(matches!(child.resume(), Err(ResumeRefusal::Revoked)));
    fixture.directory.close()?;
    Ok(())
}

#[test]
fn r_vlm_7_r_nf_3__resuming_an_owned_child_executes_the_fixture_and_cleanup_releases_resources()
-> TestResult {
    let fixture = Fixture::new()?;
    let mut child = fixture.create_suspended()?;
    assert!(!fixture.marker.try_exists()?);
    child.resume()?;
    let executed = fixture.wait_for_execution();
    let running = child.observe();

    let cleanup_started = Instant::now();
    child.cancel();
    assert_eq!(child.state(), OwnedProcessState::Revoked);
    let cleanup = child.cleanup(CLEANUP_BUDGET)?;
    assert!(cleanup_started.elapsed() <= CLEANUP_BUDGET);
    executed?;
    let running = running?;
    assert_eq!(running.state(), OwnedProcessState::Running);
    assert!(running.root_is_live_member());
    assert_eq!(running.generation(), child.generation());
    assert_eq!(cleanup.outcome(), CleanupOutcome::ObservedReleased);
    assert!(cleanup.root_exited());
    assert_eq!(cleanup.active_processes(), Some(0));
    assert_eq!(cleanup.member_processes(), Some(0));
    assert!(cleanup.stdout_eof());
    assert!(cleanup.stderr_eof());
    assert!(cleanup.stdout().len() <= CAPTURE_BYTES_PER_STREAM);
    assert!(cleanup.stderr().len() <= CAPTURE_BYTES_PER_STREAM);
    assert!(
        cleanup
            .stdout()
            .windows(STDOUT_SENTINEL.len())
            .any(|s| s == STDOUT_SENTINEL)
    );
    assert!(
        cleanup
            .stderr()
            .windows(STDERR_SENTINEL.len())
            .any(|s| s == STDERR_SENTINEL)
    );
    for diagnostic in [format!("{cleanup:?}"), format!("{cleanup:#?}")] {
        assert!(diagnostic.contains("stdout_bytes") && diagnostic.contains("stderr_bytes"));
        assert!(diagnostic.contains("ObservedReleased"));
        let compact: String = diagnostic.chars().filter(|c| !c.is_whitespace()).collect();
        for payload in [STDOUT_SENTINEL, STDERR_SENTINEL] {
            assert!(!diagnostic.contains(std::str::from_utf8(payload)?));
            let byte_list: String = format!("{payload:?}")
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect();
            assert!(
                !compact.contains(&byte_list),
                "captured output bytes must not appear in diagnostics"
            );
        }
    }
    assert!(cleanup.owned_handles_closed());
    assert!(!fixture.private.try_exists()?);
    assert_eq!(read_marker(&fixture.marker)?, EXECUTED_MARKER);
    assert!(matches!(child.resume(), Err(ResumeRefusal::Revoked)));
    fixture.directory.close()?;
    Ok(())
}

#[test]
fn r_vlm_7_r_nf_3__an_executed_descendant_is_included_and_released_with_the_owned_job() -> TestResult
{
    let fixture = Fixture::new()?;
    let descendant_marker = fixture.marker.with_extension("descendant");
    let mut child = fixture.create_with("descendant", CAPTURE_BYTES_PER_STREAM)?;
    let suspended_count = child.observe()?.active_processes();
    child.resume()?;
    let executed = fixture.wait_for_execution();
    let observed = child.observe();
    child.cancel();
    let started = Instant::now();
    let cleanup = child.cleanup(CLEANUP_BUDGET)?;
    assert!(started.elapsed() <= CLEANUP_BUDGET);
    executed?;
    assert_eq!(read_marker(&descendant_marker)?, EXECUTED_MARKER);
    assert!(
        observed?.active_processes() > suspended_count,
        "the root and executed descendant must both belong to the job"
    );
    assert_eq!(cleanup.outcome(), CleanupOutcome::ObservedReleased);
    assert_eq!(cleanup.observed_process_counts(), Some((0, 0)));
    assert!(cleanup.root_exited());
    assert!(cleanup.stdout_eof() && cleanup.stderr_eof() && cleanup.owned_handles_closed());
    assert!(!fixture.private.try_exists()?);
    fixture.directory.close()?;
    Ok(())
}

#[test]
fn r_vlm_7_r_nf_3__capture_overflow_retains_bounded_output_and_incomplete_release() -> TestResult {
    use tallyline_adapter_platform::api::model::CleanupFailure;
    const CAPTURE_CAP: usize = 128;
    let fixture = Fixture::new()?;
    let mut child = fixture.create_with("output", CAPTURE_CAP)?;
    child.resume()?;
    let executed = fixture.wait_for_execution();
    child.cancel();
    let cleanup = child.cleanup(CLEANUP_BUDGET)?;
    executed?;
    assert_eq!(cleanup.outcome(), CleanupOutcome::Incomplete);
    assert_eq!(cleanup.stdout().len(), CAPTURE_CAP);
    assert!(cleanup.stderr().len() <= CAPTURE_CAP);
    assert!(
        cleanup
            .failures()
            .iter()
            .any(|failure| matches!(failure, CleanupFailure::OutputLimit))
    );
    assert_eq!(cleanup.observed_process_counts(), Some((0, 0)));
    assert!(cleanup.root_exited() && cleanup.stdout_eof() && cleanup.stderr_eof());
    assert!(cleanup.owned_handles_closed());
    fixture.directory.close()?;
    Ok(())
}

#[test]
fn r_vlm_7_r_nf_3__private_state_deletion_failure_is_preserved_after_process_release() -> TestResult
{
    use tallyline_adapter_platform::api::model::CleanupFailure;
    let fixture = Fixture::new()?;
    let mut child = fixture.create_suspended()?;
    fs::remove_dir(&fixture.private)?;
    fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&fixture.private)?;
    child.cancel();
    let cleanup = child.cleanup(CLEANUP_BUDGET)?;
    assert_eq!(cleanup.outcome(), CleanupOutcome::Incomplete);
    assert!(
        cleanup
            .failures()
            .iter()
            .any(|failure| matches!(failure, CleanupFailure::Delete(_)))
    );
    assert_eq!(cleanup.observed_process_counts(), Some((0, 0)));
    assert!(cleanup.root_exited() && cleanup.stdout_eof() && cleanup.stderr_eof());
    assert!(cleanup.owned_handles_closed());
    assert!(fixture.private.is_file());
    assert!(!fixture.marker.try_exists()?);
    fixture.directory.close()?;
    Ok(())
}
#[test]
fn r_vlm_7__retained_observations_distinguish_replacement_owned_lifetimes() -> TestResult {
    let first_fixture = Fixture::new()?;
    let mut first = first_fixture.create_suspended()?;
    let retained = first.observe()?;
    let token = first.generation().to_owned();
    assert_eq!(retained.generation(), &token);
    first.cancel();
    assert_eq!(
        first.cleanup(CLEANUP_BUDGET)?.outcome(),
        CleanupOutcome::ObservedReleased
    );
    drop(first);
    first_fixture.directory.close()?;

    let second_fixture = Fixture::new()?;
    let mut second = second_fixture.create_suspended()?;
    assert_ne!(retained.generation(), second.generation());
    assert_eq!(retained.generation(), &token);
    assert_eq!(second.observe()?.generation(), second.generation());
    second.cancel();
    assert_eq!(
        second.cleanup(CLEANUP_BUDGET)?.outcome(),
        CleanupOutcome::ObservedReleased
    );
    second_fixture.directory.close()?;
    Ok(())
}
#[test]
fn r_nf_4__owned_failure_diagnostics_summarise_nested_errors() {
    use tallyline_adapter_platform::api::model::{CleanupFailure, OwnedProcessFailure};
    const PAYLOAD: &str = "synthetic-captured-output-environment-command-payload";
    let startup = OwnedProcessFailure::from(io::Error::other(PAYLOAD));
    let cleanup = CleanupFailure::Pipe(io::Error::other(PAYLOAD));
    let resume = ResumeRefusal::Unverified(io::Error::other(PAYLOAD));
    for diagnostic in [
        format!("{startup:?}"),
        format!("{startup:#?}"),
        startup.to_string(),
        format!("{cleanup:?}"),
        format!("{cleanup:#?}"),
        format!("{resume:?}"),
        format!("{resume:#?}"),
        resume.to_string(),
    ] {
        assert!(!diagnostic.contains(PAYLOAD));
        assert!(diagnostic.contains("Other"));
    }
}
#[test]
fn r_vlm_7_r_nf_3__oversized_command_data_is_refused_before_child_creation() -> TestResult {
    let fixture = Fixture::new()?;
    let mut environment = BTreeMap::new();
    environment.insert(
        "SystemRoot".into(),
        std::env::var_os("SystemRoot").ok_or("trusted SystemRoot missing")?,
    );
    let result = OwnedProcessCommand::new(
        std::env::current_exe()?,
        vec!["\\".repeat(32768).into()],
        fixture.private.clone(),
        environment,
        CAPTURE_BYTES_PER_STREAM,
    );
    assert!(matches!(result, Err(error) if error.kind() == io::ErrorKind::InvalidInput));
    assert!(!fixture.marker.try_exists()?);
    fixture.directory.close()?;
    Ok(())
}
struct Fixture {
    directory: TempDir,
    private: PathBuf,
    marker: PathBuf,
}

impl Fixture {
    fn new() -> TestResult<Self> {
        let directory = tempfile::Builder::new()
            .prefix("owned-process-")
            .tempdir()?;
        let evidence = directory.path().join("evidence");
        let private = directory.path().join("private");
        fs::create_dir(&evidence)?;
        fs::create_dir(&private)?;
        Ok(Self {
            directory,
            private,
            marker: evidence.join("executed"),
        })
    }

    fn create_suspended(&self) -> TestResult<OwnedProcess> {
        self.create_with("plain", CAPTURE_BYTES_PER_STREAM)
    }

    fn create_with(&self, mode: &str, cap: usize) -> TestResult<OwnedProcess> {
        let mut environment: BTreeMap<OsString, OsString> = BTreeMap::new();
        environment.insert("TALLYLINE_OWNED_FIXTURE_MODE".into(), mode.into());
        environment.insert(
            "TALLYLINE_OWNED_FIXTURE_MARKER".into(),
            self.marker.as_os_str().to_owned(),
        );
        environment.insert("TEMP".into(), self.private.as_os_str().to_owned());
        environment.insert("TMP".into(), self.private.as_os_str().to_owned());
        let system_root = PathBuf::from(
            std::env::var_os("SystemRoot").ok_or("fixture requires trusted SystemRoot")?,
        );
        if !system_root.is_absolute() || !system_root.join("System32").is_dir() {
            return Err("fixture requires an absolute trusted SystemRoot".into());
        }
        environment.insert("SystemRoot".into(), system_root.into_os_string());
        let command = OwnedProcessCommand::new(
            std::env::current_exe()?,
            vec![
                "--exact".into(),
                "owned_process_fixture".into(),
                "--nocapture".into(),
                "--test-threads=1".into(),
            ],
            self.private.clone(),
            environment,
            cap,
        )?;
        let limits =
            OwnedProcessLimits::new(JOB_COMMIT_BYTES, CPU_RATE_PERCENT, ACTIVE_PROCESS_LIMIT)?;
        Ok(OwnedProcess::create_suspended(command, limits)?)
    }

    fn wait_for_execution(&self) -> TestResult {
        let deadline = Instant::now() + FIXTURE_EXECUTION_BUDGET;
        loop {
            if self.marker.try_exists()? {
                assert_eq!(read_marker(&self.marker)?, EXECUTED_MARKER);
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(
                    "resumed fixture did not publish execution marker within its budget".into(),
                );
            }
            std::thread::sleep(MARKER_POLL);
        }
    }
}

fn read_marker(marker: &std::path::Path) -> io::Result<Vec<u8>> {
    use std::io::Read;
    let file = fs::File::open(marker)?;
    let mut contents = Vec::new();
    file.take(MARKER_READ_BYTES).read_to_end(&mut contents)?;
    Ok(contents)
}

#[test]
fn owned_process_fixture() -> TestResult {
    let Some(marker) = std::env::var_os("TALLYLINE_OWNED_FIXTURE_MARKER") else {
        return Ok(());
    };
    use std::io::{Read, Write};
    let marker = PathBuf::from(marker);
    let pending = marker.with_extension("pending");
    let mode = std::env::var("TALLYLINE_OWNED_FIXTURE_MODE")?;
    let mut descendant = None;
    if mode == "descendant" {
        let private = std::env::current_dir()?.join("descendant-state");
        fs::create_dir(&private)?;
        let nested = Fixture {
            directory: tempfile::Builder::new()
                .prefix("nested-evidence-")
                .tempdir_in(std::env::current_dir()?)?,
            private,
            marker: marker.with_extension("descendant"),
        };
        let mut child = nested.create_suspended()?;
        child.resume()?;
        nested.wait_for_execution()?;
        descendant = Some((child, nested));
    }
    let mut stdout = io::stdout();
    stdout.write_all(STDOUT_SENTINEL)?;
    if mode == "output" {
        stdout.write_all(&[b'x'; 2048])?;
    }
    stdout.flush()?;
    let mut stderr = io::stderr();
    stderr.write_all(STDERR_SENTINEL)?;
    stderr.flush()?;
    let mut output = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&pending)?;
    output.write_all(EXECUTED_MARKER)?;
    output.sync_all()?;
    drop(output);
    fs::rename(pending, marker)?;
    let mut input = [0_u8; 1];
    io::stdin().read_exact(&mut input)?;
    drop(descendant);
    Err("owned fixture unexpectedly received stdin or its writer closed before cancellation".into())
}
