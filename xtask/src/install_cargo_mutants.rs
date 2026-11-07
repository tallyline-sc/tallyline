// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Installation of the fixed cargo-mutants release on hosted runners.

use std::env;
use std::fmt::Display;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::Path;
use std::process::{Command, ExitStatus, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use sha2::Sha256;

const CONNECT_TIMEOUT_SECONDS: &str = "30";
const DOWNLOAD_TIMEOUT_SECONDS: &str = "120";
const DOWNLOAD_RETRIES: &str = "3";
const RETRY_WINDOW_SECONDS: &str = "20";
const MAX_DOWNLOAD_BYTES: &str = "67108864";
const MAX_OUTPUT_BYTES: u64 = 65_536;
const POLL_INTERVAL: Duration = Duration::from_millis(10);
const PROCESS_TIMEOUT: Duration = Duration::from_secs(150);

const VERSION: &str = "cargo-mutants 27.1.0";
const WINDOWS_ASSET: &str = "cargo-mutants-x86_64-pc-windows-msvc.zip";
const LINUX_ASSET: &str = "cargo-mutants-x86_64-unknown-linux-gnu.tar.gz";
const RELEASE_URL: &str = "https://github.com/sourcefrog/cargo-mutants/releases/download/v27.1.0/";

/// An approved platform release of cargo-mutants.
pub struct Release {
    url: String,
    digest: [u8; 32],
    executable: &'static str,
    protocol: &'static str,
}

impl Release {
    /// Selects the fixed release for a matching Windows or Linux X64 host.
    ///
    /// # Errors
    /// Returns an error for unsupported runners or a host mismatch.
    pub fn for_runner(os: &str, arch: &str) -> io::Result<Self> {
        if arch != "X64" || env::consts::ARCH != "x86_64" {
            return Err(io::Error::other(
                "cargo-mutants requires an X64 runner and host",
            ));
        }
        let (asset, digest, executable) = match os {
            "Windows" if cfg!(target_os = "windows") => (
                WINDOWS_ASSET,
                [
                    0x2a, 0x2f, 0x00, 0xe4, 0x7d, 0x4b, 0x45, 0x82, 0x62, 0xa4, 0x15, 0x01, 0xb0,
                    0x82, 0x0a, 0xa2, 0x60, 0x15, 0xfd, 0x35, 0x77, 0x99, 0x03, 0xd2, 0xc8, 0xb3,
                    0x0b, 0x29, 0x93, 0xf3, 0x67, 0x91,
                ],
                "cargo-mutants.exe",
            ),
            "Linux" if cfg!(target_os = "linux") => (
                LINUX_ASSET,
                [
                    0xdf, 0xe6, 0xdc, 0x37, 0xd0, 0x34, 0x2c, 0x89, 0x1d, 0x28, 0x29, 0xb5, 0xa6,
                    0x95, 0xaa, 0x57, 0xc2, 0xd0, 0xed, 0xec, 0xef, 0x7e, 0x7d, 0x03, 0x99, 0xa3,
                    0x0c, 0xc6, 0xe2, 0x06, 0x41, 0x1e,
                ],
                "cargo-mutants",
            ),
            _ => {
                return Err(io::Error::other(
                    "unsupported cargo-mutants runner OS or host mismatch",
                ));
            }
        };
        Ok(Self {
            url: format!("{RELEASE_URL}{asset}"),
            digest,
            executable,
            protocol: "=https",
        })
    }

    #[cfg(test)]
    fn fixture(url: String, digest: [u8; 32]) -> Self {
        Self {
            url,
            digest,
            executable: if cfg!(windows) {
                "cargo-mutants.exe"
            } else {
                "cargo-mutants"
            },
            protocol: "=http",
        }
    }
}

/// Installs the approved release using the GitHub runner environment.
///
/// # Errors
/// Returns an error for invalid configuration or a failed installation.
pub fn run() -> io::Result<()> {
    let os = required("RUNNER_OS")?;
    let arch = required("RUNNER_ARCH")?;
    let release = Release::for_runner(&os, &arch)?;
    let temp = required("RUNNER_TEMP")?;
    let output = required("GITHUB_PATH")?;
    install(&release, Path::new(&temp), Path::new(&output))
}

fn required(name: &str) -> io::Result<String> {
    env::var(name)
        .ok()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| io::Error::other(format!("missing or non-UTF-8 {name}")))
}

/// Installs and verifies a release before publishing its executable directory.
///
/// # Errors
/// Returns an error when installation, publication or owned cleanup fails.
pub fn install(release: &Release, runner_temp: &Path, github_path: &Path) -> io::Result<()> {
    let temp = runner_temp
        .canonicalize()
        .map_err(|error| phase("resolve RUNNER_TEMP", error))?;
    if !temp.is_dir() {
        return Err(io::Error::other("RUNNER_TEMP is not a directory"));
    }
    let owned = temp.join("cargo-mutants");
    fs::create_dir(&owned).map_err(|error| phase("create fresh cargo-mutants directory", error))?;
    match install_owned(release, &owned, github_path) {
        Ok(()) => Ok(()),
        Err(error) => match fs::remove_dir_all(&owned) {
            Ok(()) => Err(error),
            Err(cleanup) => Err(io::Error::other(format!(
                "{error}; cargo-mutants cleanup failed: {cleanup}"
            ))),
        },
    }
}

fn install_owned(release: &Release, owned: &Path, github_path: &Path) -> io::Result<()> {
    let archive = owned.join("archive");
    let bin = owned.join("bin");
    fs::create_dir(&bin).map_err(|error| phase("create executable directory", error))?;
    download(release, owned, &archive)?;
    verify_digest(release, &archive)?;
    extract(owned, &archive, &bin)?;
    verify_version(release, owned, &bin)?;
    for name in ["stdout", "stderr", "archive"] {
        fs::remove_file(owned.join(name))
            .map_err(|error| phase("remove temporary output", error))?;
    }
    publish_path(&bin, github_path)
}

fn download(release: &Release, owned: &Path, archive: &Path) -> io::Result<()> {
    let output = checked_output(
        Command::new(if cfg!(windows) { "curl.exe" } else { "curl" })
            .args([
                "--disable",
                "--fail",
                "--silent",
                "--show-error",
                "--location",
                "--proto",
                release.protocol,
                "--proto-redir",
                release.protocol,
                "--connect-timeout",
                CONNECT_TIMEOUT_SECONDS,
                "--max-time",
                DOWNLOAD_TIMEOUT_SECONDS,
                "--retry",
                DOWNLOAD_RETRIES,
                "--retry-max-time",
                RETRY_WINDOW_SECONDS,
                "--max-filesize",
                MAX_DOWNLOAD_BYTES,
                "--output",
            ])
            .arg(archive)
            .arg(&release.url),
        owned,
        "download",
    )?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "cargo-mutants download failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(())
}

fn verify_digest(release: &Release, archive: &Path) -> io::Result<()> {
    use std::io::Read;

    use sha2::Digest;

    let mut input = File::open(archive).map_err(|error| phase("open archive for digest", error))?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 8192];
    loop {
        let read = input
            .read(&mut buffer)
            .map_err(|error| phase("hash archive", error))?;
        if read == 0 {
            break;
        }
        hash.update(
            buffer
                .get(..read)
                .ok_or_else(|| io::Error::other("invalid archive read length"))?,
        );
    }
    if hash.finalize().as_slice() != release.digest {
        return Err(io::Error::other("cargo-mutants archive digest mismatch"));
    }
    Ok(())
}

fn extract(owned: &Path, archive: &Path, bin: &Path) -> io::Result<()> {
    let output = checked_output(
        Command::new(if cfg!(windows) { "tar.exe" } else { "tar" })
            .arg(if cfg!(windows) { "-xf" } else { "-xzf" })
            .arg(archive)
            .arg("-C")
            .arg(bin),
        owned,
        "extract archive",
    )?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "cargo-mutants extraction failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(())
}

fn verify_version(release: &Release, owned: &Path, bin: &Path) -> io::Result<()> {
    if !fs::symlink_metadata(bin.join(release.executable))
        .map_err(|error| phase("expected executable", error))?
        .file_type()
        .is_file()
    {
        return Err(io::Error::other(
            "cargo-mutants expected executable is not a regular file",
        ));
    }
    let path = env::var_os("PATH").ok_or_else(|| io::Error::other("missing PATH"))?;
    let child_path =
        env::join_paths(std::iter::once(bin.to_path_buf()).chain(env::split_paths(&path)))
            .map_err(|error| phase("prepare child PATH", error))?;
    let output = checked_output(
        Command::new(env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
            .args(["--locked", "mutants", "--version"])
            .env("PATH", child_path),
        owned,
        "verify version",
    )?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "cargo-mutants version failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    let version =
        String::from_utf8(output.stdout).map_err(|error| phase("decode version", error))?;
    let version = version
        .strip_suffix("\r\n")
        .or_else(|| version.strip_suffix('\n'))
        .unwrap_or(&version);
    if version != VERSION {
        return Err(io::Error::other("cargo-mutants version mismatch"));
    }
    Ok(())
}

fn publish_path(bin: &Path, github_path: &Path) -> io::Result<()> {
    use std::io::{Read, Seek, SeekFrom, Write};

    let directory = bin
        .to_str()
        .ok_or_else(|| io::Error::other("executable directory is not UTF-8"))?;
    if directory.contains(['\r', '\n']) {
        return Err(io::Error::other(
            "executable directory contains a line ending",
        ));
    }
    let mut output = OpenOptions::new()
        .read(true)
        .append(true)
        .open(github_path)
        .map_err(|error| phase("open GITHUB_PATH", error))?;
    let length = output
        .metadata()
        .map_err(|error| phase("inspect GITHUB_PATH", error))?
        .len();
    if length > 0 {
        output
            .seek(SeekFrom::End(-1))
            .map_err(|error| phase("seek GITHUB_PATH boundary", error))?;
        let mut last = [0];
        output
            .read_exact(&mut last)
            .map_err(|error| phase("read GITHUB_PATH boundary", error))?;
        if last != *b"\n" {
            output
                .write_all(b"\n")
                .map_err(|error| phase("terminate GITHUB_PATH line", error))?;
        }
    }
    writeln!(output, "{directory}").map_err(|error| phase("publish GITHUB_PATH", error))
}

fn checked_output(command: &mut Command, owned: &Path, name: &str) -> io::Result<Output> {
    let stdout = File::create(owned.join("stdout")).map_err(|error| phase(name, error))?;
    let stderr = File::create(owned.join("stderr")).map_err(|error| phase(name, error))?;
    let mut child = command
        .stdin(Stdio::null())
        .stdout(stdout.try_clone()?)
        .stderr(stderr.try_clone()?)
        .spawn()
        .map_err(|error| phase(name, error))?;
    let start = Instant::now();
    let result = (|| -> io::Result<ExitStatus> {
        loop {
            let status = child.try_wait()?;
            if stdout.metadata()?.len() > MAX_OUTPUT_BYTES
                || stderr.metadata()?.len() > MAX_OUTPUT_BYTES
            {
                return Err(io::Error::other("native output exceeded 65536 bytes"));
            }
            if let Some(status) = status {
                return Ok(status);
            }
            if start.elapsed() >= PROCESS_TIMEOUT {
                return Err(io::Error::other("native process exceeded 150 seconds"));
            }
            thread::sleep(POLL_INTERVAL);
        }
    })();
    let status = match result {
        Ok(status) => status,
        Err(error) => {
            if child.try_wait()?.is_none() {
                child.kill().map_err(|cleanup| {
                    phase(name, format!("{error}; terminate child: {cleanup}"))
                })?;
            }
            child
                .wait()
                .map_err(|cleanup| phase(name, format!("{error}; reap child: {cleanup}")))?;
            return Err(phase(name, error));
        }
    };
    Ok(Output {
        status,
        stdout: fs::read(owned.join("stdout"))?,
        stderr: fs::read(owned.join("stderr"))?,
    })
}

fn phase(name: &str, error: impl Display) -> io::Error {
    io::Error::other(format!("cargo-mutants {name}: {error}"))
}

#[cfg(test)]
#[path = "install_cargo_mutants_tests.rs"]
mod tests;
