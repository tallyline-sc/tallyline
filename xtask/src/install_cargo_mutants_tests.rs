// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Real download, extraction and Cargo dispatch contracts for the installer.

use std::env;
use std::fs;
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use super::Release;
use crate::scratch::Scratch;

const SERVER_TIMEOUT: Duration = Duration::from_secs(30);
const SERVER_POLL_INTERVAL: Duration = Duration::from_millis(10);

struct HttpServer {
    stopped: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<usize>>,
}

impl HttpServer {
    fn join(mut self) -> thread::Result<usize> {
        self.stop()
    }

    fn stop(&mut self) -> thread::Result<usize> {
        self.stopped.store(true, Ordering::Release);
        self.worker.take().map_or(Ok(0), thread::JoinHandle::join)
    }
}

impl Drop for HttpServer {
    fn drop(&mut self) {
        self.stop().expect("valid installer fixture");
    }
}
struct Fixture {
    scratch: Scratch,
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let scratch = Scratch::new();
        let root = scratch.path().join("literal space & $ ; ' directory");
        fs::create_dir(&root).expect("valid installer fixture");
        fs::write(root.join("github-path"), b"existing-path\n").expect("valid installer fixture");
        Self { scratch, root }
    }

    fn archive(&self, behaviour: &str) -> Vec<u8> {
        let package = self.scratch.path().join("fixture");
        fs::create_dir(&package).expect("valid installer fixture");
        fs::write(package.join("Cargo.toml"), "[package]\nname = \"cargo-mutants\"\nversion = \"0.0.0\"\nedition = \"2024\"\n[workspace]\n[[bin]]\nname = \"cargo-mutants\"\npath = \"main.rs\"\n").expect("valid installer fixture");
        fs::write(package.join("main.rs"), FIXTURE).expect("valid installer fixture");
        let build = self.scratch.path().join("build");
        let output = Command::new(env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
            .args(["build", "--offline", "--manifest-path"])
            .arg(package.join("Cargo.toml"))
            .env("CARGO_TARGET_DIR", &build)
            .env("CARGO_BUILD_BUILD_DIR", &build)
            .output()
            .expect("valid installer fixture");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let contents = self.scratch.path().join("contents");
        fs::create_dir(&contents).expect("valid installer fixture");
        if behaviour != "missing" {
            fs::copy(
                build.join("debug").join(executable()),
                contents.join(executable()),
            )
            .expect("valid installer fixture");
        }
        fs::write(contents.join("behaviour"), behaviour).expect("valid installer fixture");
        let archive = self.scratch.path().join(if cfg!(windows) {
            "fixture.zip"
        } else {
            "fixture.tar.gz"
        });
        let output = Command::new(if cfg!(windows) { "tar.exe" } else { "tar" })
            .args(if cfg!(windows) {
                ["-a", "-cf"]
            } else {
                ["-z", "-cf"]
            })
            .arg(&archive)
            .arg("-C")
            .arg(&contents)
            .arg(".")
            .output()
            .expect("valid installer fixture");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        fs::read(archive).expect("valid installer fixture")
    }

    fn output(&self) -> PathBuf {
        self.root.join("github-path")
    }

    fn serve(&self, archive: Vec<u8>) -> (Release, HttpServer) {
        self.serve_status(archive, "200 OK")
    }

    fn serve_status(&self, archive: Vec<u8>, status: &'static str) -> (Release, HttpServer) {
        self.serve_statuses(archive, vec![status])
    }

    fn serve_statuses(
        &self,
        archive: Vec<u8>,
        statuses: Vec<&'static str>,
    ) -> (Release, HttpServer) {
        use std::io::{Read, Write};

        let listener = TcpListener::bind("127.0.0.1:0").expect("valid installer fixture");
        listener
            .set_nonblocking(true)
            .expect("valid installer fixture");
        let address = listener.local_addr().expect("valid installer fixture");
        use sha2::{Digest, Sha256};
        let digest: [u8; 32] = Sha256::digest(&archive).into();
        assert!(!statuses.is_empty(), "fixture requires an HTTP response");
        let stopped = Arc::new(AtomicBool::new(false));
        let worker_stopped = Arc::clone(&stopped);
        let worker = thread::spawn(move || {
            let start = Instant::now();
            let mut requests = 0;
            loop {
                let mut stream = loop {
                    if worker_stopped.load(Ordering::Acquire) {
                        return requests;
                    }
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(
                                start.elapsed() < SERVER_TIMEOUT,
                                "fixture request timed out"
                            );
                            thread::sleep(SERVER_POLL_INTERVAL);
                        }
                        Err(error) => panic!("cannot accept fixture request: {error}"),
                    }
                };
                stream
                    .set_nonblocking(false)
                    .expect("valid installer fixture");
                stream
                    .set_read_timeout(Some(SERVER_TIMEOUT))
                    .expect("valid installer fixture");
                stream
                    .set_write_timeout(Some(SERVER_TIMEOUT))
                    .expect("valid installer fixture");
                let mut request = [0; 4096];
                let count = stream.read(&mut request).expect("valid installer fixture");
                assert!(count > 0);
                let status = statuses[requests.min(statuses.len() - 1)];
                requests += 1;
                let body = if status == "200 OK" {
                    archive.as_slice()
                } else {
                    b"unavailable"
                };
                write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .expect("valid installer fixture");
                stream.write_all(body).expect("valid installer fixture");
            }
        });
        (
            Release::fixture(format!("http://{address}/fixture"), digest),
            HttpServer {
                stopped,
                worker: Some(worker),
            },
        )
    }
}

fn executable() -> &'static str {
    if cfg!(windows) {
        "cargo-mutants.exe"
    } else {
        "cargo-mutants"
    }
}

fn assert_verified_install(fixture: &Fixture) {
    let bin = fixture
        .root
        .join("cargo-mutants")
        .join("bin")
        .canonicalize()
        .expect("valid installer fixture");
    assert_eq!(
        fs::read_to_string(fixture.output()).expect("valid installer fixture"),
        format!("existing-path\n{}\n", bin.display())
    );
    assert!(bin.join(executable()).is_file());
    assert_eq!(
        fs::read(bin.join("ran")).expect("valid installer fixture"),
        b"cargo dispatch"
    );
    assert!(!fixture.root.join("cargo-mutants/archive").exists());
}

#[test]
fn verified_install_publishes_one_literal_directory_after_real_cargo_dispatch() {
    let fixture = Fixture::new();
    let (release, server) = fixture.serve(fixture.archive("success"));
    super::install(&release, &fixture.root, &fixture.output()).expect("valid installer fixture");
    server.join().expect("valid installer fixture");
    assert_verified_install(&fixture);
}

#[test]
fn transient_http_errors_recover_before_verified_path_publication() {
    let fixture = Fixture::new();
    let (release, server) = fixture.serve_statuses(
        fixture.archive("success"),
        vec!["503 Service Unavailable", "504 Gateway Timeout", "200 OK"],
    );
    super::install(&release, &fixture.root, &fixture.output())
        .expect("transient HTTP errors must recover within the bounded download");
    server.join().expect("valid installer fixture");
    assert_verified_install(&fixture);
}

#[test]
fn unterminated_existing_path_is_preserved_as_a_distinct_line() {
    let fixture = Fixture::new();
    fs::write(fixture.output(), b"existing-path").expect("valid installer fixture");
    let (release, server) = fixture.serve(fixture.archive("success"));
    super::install(&release, &fixture.root, &fixture.output()).expect("valid installer fixture");
    server.join().expect("valid installer fixture");
    let bin = fixture
        .root
        .join("cargo-mutants/bin")
        .canonicalize()
        .expect("valid installer fixture");
    assert_eq!(
        fs::read_to_string(fixture.output()).expect("valid installer fixture"),
        format!("existing-path\n{}\n", bin.display()),
        "existing PATH bytes and the installed directory must occupy separate lines"
    );
}
#[test]
fn empty_or_terminated_path_preserves_bytes_without_an_extra_separator() {
    for existing in ["", "existing-path\n", "existing-path\r\n"] {
        let fixture = Fixture::new();
        fs::write(fixture.output(), existing).expect("valid installer fixture");
        let (release, server) = fixture.serve(fixture.archive("success"));
        super::install(&release, &fixture.root, &fixture.output())
            .expect("valid installer fixture");
        server.join().expect("valid installer fixture");
        let bin = fixture
            .root
            .join("cargo-mutants/bin")
            .canonicalize()
            .expect("valid installer fixture");
        assert_eq!(
            fs::read_to_string(fixture.output()).expect("valid installer fixture"),
            format!("{existing}{}\n", bin.display()),
            "empty and LF/CRLF-terminated output must retain its existing boundary"
        );
    }
}
const FIXTURE: &str = r#"
use std::env;
use std::fs;
use std::io::{self, Write};
use std::process;
fn main() {
    if env::args().skip(1).collect::<Vec<_>>() != ["mutants", "--version"] { process::exit(91); }
    let root = env::current_exe().expect("valid installer fixture").parent().expect("valid installer fixture").to_path_buf();
    fs::write(root.join("ran"), b"cargo dispatch").expect("valid installer fixture");
    fs::write(root.parent().expect("valid installer fixture").parent().expect("valid installer fixture").join("executed"), b"executed").expect("valid installer fixture");
    let behaviour = fs::read_to_string(root.join("behaviour")).expect("valid installer fixture");
    match behaviour.as_str() {
        "flood" => { io::stderr().write_all(&vec![b'x'; 70_000]).expect("valid installer fixture"); println!("cargo-mutants 27.1.0"); }
        "cleanup" => {
            fs::write(root.join("obstruction"), b"owned obstruction").expect("valid installer fixture");
            let start = std::time::Instant::now();
            while !root.parent().expect("valid installer fixture").parent().expect("valid installer fixture").join("continue").exists() {
                if start.elapsed() > std::time::Duration::from_secs(10) { process::exit(92); }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            process::exit(7);
        }
        "nonzero" => process::exit(7),
        "no-ending" => io::stdout().write_all(b"cargo-mutants 27.1.0").expect("valid installer fixture"),
        "space" => println!("cargo-mutants 27.1.0 "),
        "carriage" => io::stdout().write_all(b"cargo-mutants 27.1.0\r").expect("valid installer fixture"),
        "wrong" => println!("cargo-mutants 0.0.0"),
        "multiline" => println!("cargo-mutants 27.1.0\nextra"),
        "invalid" => io::stdout().write_all(&[255]).expect("valid installer fixture"),
        "crlf" => io::stdout().write_all(b"cargo-mutants 27.1.0\r\n").expect("valid installer fixture"),
        _ => println!("cargo-mutants 27.1.0"),
    }
}
"#;
#[test]
fn excessive_native_output_fails_without_publication_and_removes_owned_output() {
    let fixture = Fixture::new();
    let (release, server) = fixture.serve(fixture.archive("flood"));
    assert!(super::install(&release, &fixture.root, &fixture.output()).is_err());
    server.join().expect("valid installer fixture");
    assert_eq!(
        fs::read(fixture.output()).expect("valid installer fixture"),
        b"existing-path\n"
    );
    assert!(!fixture.root.join("cargo-mutants").exists());
}
fn assert_failure(fixture: &Fixture, release: &Release, phase: &str) {
    let error = super::install(release, &fixture.root, &fixture.output())
        .expect_err("invalid installation must fail");
    assert!(error.to_string().contains(phase), "{error}");
    assert_eq!(
        fs::read(fixture.output()).expect("valid installer fixture"),
        b"existing-path\n"
    );
    assert!(!fixture.root.join("cargo-mutants").exists());
}

#[test]
fn wrong_digest_refuses_before_extraction_or_execution() {
    let fixture = Fixture::new();
    let (release, server) = fixture.serve(fixture.archive("success"));
    let corrupt = Release::fixture(release.url, [0; 32]);
    assert_failure(&fixture, &corrupt, "digest mismatch");
    server.join().expect("valid installer fixture");
    assert!(!fixture.root.join("executed").exists());
}

#[test]
fn invalid_archive_is_cleaned_without_executing_or_publishing() {
    let fixture = Fixture::new();
    let (release, server) = fixture.serve(b"not an archive".to_vec());
    assert_failure(&fixture, &release, "extraction failed");
    server.join().expect("valid installer fixture");
    assert!(!fixture.root.join("executed").exists());
}

#[test]
fn exhausted_transient_http_errors_are_cleaned_without_publication() {
    let fixture = Fixture::new();
    let (release, server) =
        fixture.serve_status(b"unavailable".to_vec(), "503 Service Unavailable");
    assert_failure(&fixture, &release, "download failed");
    assert_eq!(
        server.join().expect("valid installer fixture"),
        4,
        "the live HTTP endpoint must receive only the initial attempt and three retries"
    );
}

#[test]
fn missing_executable_is_cleaned_without_publication() {
    let fixture = Fixture::new();
    let (release, server) = fixture.serve(fixture.archive("missing"));
    assert_failure(&fixture, &release, "expected executable");
    server.join().expect("valid installer fixture");
}

#[test]
fn failed_or_inexact_version_is_cleaned_without_publication() {
    for (behaviour, phase) in [
        ("nonzero", "version failed"),
        ("wrong", "version mismatch"),
        ("multiline", "version mismatch"),
        ("invalid", "decode version"),
        ("space", "version mismatch"),
        ("carriage", "version mismatch"),
    ] {
        let fixture = Fixture::new();
        let (release, server) = fixture.serve(fixture.archive(behaviour));
        assert_failure(&fixture, &release, phase);
        server.join().expect("valid installer fixture");
        assert!(fixture.root.join("executed").is_file());
    }
}

#[test]
fn exact_version_accepts_crlf_or_no_line_ending() {
    for behaviour in ["crlf", "no-ending"] {
        let fixture = Fixture::new();
        let (release, server) = fixture.serve(fixture.archive(behaviour));
        super::install(&release, &fixture.root, &fixture.output())
            .expect("valid installer fixture");
        server.join().expect("valid installer fixture");
        assert!(fixture.root.join("executed").is_file());
    }
}

#[test]
fn existing_installation_is_preserved_without_download() {
    let fixture = Fixture::new();
    let owned = fixture.root.join("cargo-mutants");
    fs::create_dir(&owned).expect("valid installer fixture");
    fs::write(owned.join("foreign"), b"preserve").expect("valid installer fixture");
    let release = Release::fixture("http://127.0.0.1:9/unused".into(), [0; 32]);
    let error = super::install(&release, &fixture.root, &fixture.output())
        .expect_err("invalid installation must fail");
    assert!(error.to_string().contains("create fresh"));
    assert_eq!(
        fs::read(owned.join("foreign")).expect("valid installer fixture"),
        b"preserve"
    );
    assert_eq!(
        fs::read(fixture.output()).expect("valid installer fixture"),
        b"existing-path\n"
    );
}

#[test]
fn obstructed_output_is_preserved_and_owned_installation_is_cleaned() {
    let fixture = Fixture::new();
    fs::remove_file(fixture.output()).expect("valid installer fixture");
    fs::create_dir(fixture.output()).expect("valid installer fixture");
    fs::write(fixture.output().join("foreign"), b"preserve").expect("valid installer fixture");
    let (release, server) = fixture.serve(fixture.archive("success"));
    let error = super::install(&release, &fixture.root, &fixture.output())
        .expect_err("invalid installation must fail");
    server.join().expect("valid installer fixture");
    assert!(error.to_string().contains("GITHUB_PATH"));
    assert_eq!(
        fs::read(fixture.output().join("foreign")).expect("valid installer fixture"),
        b"preserve"
    );
    assert!(!fixture.root.join("cargo-mutants").exists());
}

#[test]
fn production_release_selection_rejects_unsupported_or_mismatched_hosts() {
    for (os, arch) in [
        ("Windows", "ARM64"),
        ("Linux", "ARM64"),
        ("macOS", "X64"),
        ("windows", "X64"),
    ] {
        assert!(Release::for_runner(os, arch).is_err());
    }
    if cfg!(windows) {
        assert!(Release::for_runner("Linux", "X64").is_err());
    }
    if cfg!(target_os = "linux") {
        assert!(Release::for_runner("Windows", "X64").is_err());
    }
}

#[cfg(windows)]
#[test]
fn cleanup_failure_reports_both_errors_and_preserves_foreign_output() {
    use std::fs::OpenOptions;
    use std::os::windows::fs::OpenOptionsExt;
    use std::time::Instant;

    let fixture = Fixture::new();
    let (release, server) = fixture.serve(fixture.archive("cleanup"));
    thread::scope(|scope| {
        let installation =
            scope.spawn(|| super::install(&release, &fixture.root, &fixture.output()));
        let obstruction = fixture.root.join("cargo-mutants/bin/obstruction");
        let start = Instant::now();
        while !obstruction.exists() {
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "fixture did not start"
            );
            thread::sleep(Duration::from_millis(10));
        }
        let lock = OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(obstruction)
            .expect("valid installer fixture");
        fs::write(fixture.root.join("continue"), b"continue").expect("valid installer fixture");
        let error = installation
            .join()
            .expect("valid installer fixture")
            .expect_err("invalid installation must fail");
        assert!(error.to_string().contains("version failed"), "{error}");
        assert!(error.to_string().contains("cleanup failed"), "{error}");
        assert_eq!(
            fs::read(fixture.output()).expect("valid installer fixture"),
            b"existing-path\n"
        );
        drop(lock);
    });
    server.join().expect("valid installer fixture");
}
