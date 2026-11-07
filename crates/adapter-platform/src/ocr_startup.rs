// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Safe process-start OCR telemetry preparation.

use std::ffi::OsStr;
use std::io;
use std::process::{Command, ExitStatus};

const TELEMETRY_VARIABLE: &str = "ORT_DISABLE_TELEMETRY";
const BOOTSTRAP_VARIABLE: &str = "TALLYLINE_ORT_BOOTSTRAPPED";

/// Whether this process may initialise the local OCR runtime.
#[derive(Debug)]
pub enum OcrProcessStartup {
    /// The process inherited the required mitigation and needs no relaunch.
    Prepared,
    /// The Windows bootstrap waited for the prepared child's complete lifetime.
    Relaunched(ExitStatus),
}

/// Establishes OCR telemetry mitigation through at most one relaunch of this executable.
///
/// Call before instance election, logging or worker construction; Unix success replaces
/// the process without running stack destructors. No shell or parent environment mutation is used.
///
/// # Errors
/// Returns an error if a previous bootstrap did not establish mitigation, or the
/// absolute current executable cannot be resolved, replaced or started.
pub fn prepare_ocr_process() -> io::Result<OcrProcessStartup> {
    if std::env::var_os(TELEMETRY_VARIABLE).as_deref() == Some(OsStr::new("1")) {
        return Ok(OcrProcessStartup::Prepared);
    }
    if std::env::var_os(BOOTSTRAP_VARIABLE).is_some() {
        return Err(io::Error::other("OCR telemetry startup preparation failed"));
    }
    let executable = std::env::current_exe()?.canonicalize()?;
    if !executable.is_absolute() {
        return Err(io::Error::other("OCR startup executable is not absolute"));
    }
    let mut arguments = std::env::args_os();
    let original_program = arguments.next();
    let mut command = Command::new(executable);
    command
        .args(arguments)
        .env(TELEMETRY_VARIABLE, "1")
        .env(BOOTSTRAP_VARIABLE, "1");
    relaunch(command, original_program)
}

#[cfg(unix)]
fn relaunch(
    mut command: Command,
    original_program: Option<std::ffi::OsString>,
) -> io::Result<OcrProcessStartup> {
    use std::os::unix::process::CommandExt;
    if let Some(program) = original_program {
        command.arg0(program);
    }
    Err(command.exec())
}

#[cfg(windows)]
fn relaunch(mut command: Command, _: Option<std::ffi::OsString>) -> io::Result<OcrProcessStartup> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map(OcrProcessStartup::Relaunched)
}
/// A child exit outcome representable by the portable application entry point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OcrChildExit {
    /// An ordinary application exit code from zero through 255.
    Code(u8),
    /// A native crash code or signal that must remain a failure.
    Abnormal,
}

impl OcrChildExit {
    /// Preserves ordinary application codes and classifies all other outcomes as abnormal.
    #[must_use]
    pub fn from_status(status: &ExitStatus) -> Self {
        match status.code().and_then(|code| u8::try_from(code).ok()) {
            Some(code) => Self::Code(code),
            None => Self::Abnormal,
        }
    }
}
