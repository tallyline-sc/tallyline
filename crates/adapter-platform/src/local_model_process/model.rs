// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Explicit owned-child commands, observations and release outcomes.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;

pub use super::generation::OwnedProcessGeneration;

pub(super) const MAX_WINDOWS_UNITS: usize = 32767;

/// A fixed executable, cleared child environment and explicitly deletable private state.
pub struct OwnedProcessCommand {
    pub(super) executable: PathBuf,
    pub(super) arguments: Vec<OsString>,
    pub(super) private: PathBuf,
    pub(super) environment: BTreeMap<OsString, OsString>,
    pub(super) capture: usize,
}

impl OwnedProcessCommand {
    /// Validates an absolute executable and private directory with bounded command data.
    /// Cleanup may delete only the supplied private directory, which must be caller-owned.
    ///
    /// # Errors
    /// Refuses relative, root, overlapping or missing paths and oversized inputs.
    pub fn new(
        executable: PathBuf,
        arguments: Vec<OsString>,
        private: PathBuf,
        environment: BTreeMap<OsString, OsString>,
        capture: usize,
    ) -> io::Result<Self> {
        const MAX_ARGUMENTS: usize = 128;
        const MAX_ENVIRONMENT: usize = 128;
        const MAX_CAPTURE: usize = 1024 * 1024;
        if !executable.is_absolute()
            || !private.is_absolute()
            || private.parent().is_none()
            || arguments.len() > MAX_ARGUMENTS
            || environment.len() > MAX_ENVIRONMENT
            || capture == 0
            || capture > MAX_CAPTURE
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid owned-child bounds",
            ));
        }
        let mut units = 0_usize;
        let strings = arguments.iter().map(AsRef::as_ref).chain(
            environment
                .iter()
                .flat_map(|(key, value)| [key.as_os_str(), value.as_os_str()]),
        );
        for value in strings {
            let count = value.encode_wide().take(MAX_WINDOWS_UNITS).count();
            units = units.checked_add(count).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "owned command size overflow")
            })?;
            if units >= MAX_WINDOWS_UNITS {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "owned command data exceed the UTF-16 bound",
                ));
            }
        }
        let executable = std::fs::canonicalize(executable)?;
        let private = std::fs::canonicalize(private)?;
        if !executable.is_file()
            || !private.is_dir()
            || executable.starts_with(&private)
            || private.parent().is_none()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid owned-child paths",
            ));
        }
        if !environment.contains_key(&OsString::from("SystemRoot")) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "SystemRoot is required",
            ));
        }
        Ok(Self {
            executable,
            arguments,
            private,
            environment,
            capture,
        })
    }
}

/// Job committed-memory, CPU hard-cap and simultaneous-process bounds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnedProcessLimits {
    pub(super) commit: usize,
    pub(super) cpu: u8,
    pub(super) processes: u32,
}

impl OwnedProcessLimits {
    /// Constructs explicit positive job bounds without choosing product defaults.
    ///
    /// # Errors
    /// Refuses zero memory, CPU outside 1–100 or process counts outside 1–256.
    pub fn new(commit: usize, cpu: u8, processes: u32) -> io::Result<Self> {
        if commit == 0 || !(1..=100).contains(&cpu) || !(1..=256).contains(&processes) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid owned-job limits",
            ));
        }
        Ok(Self {
            commit,
            cpu,
            processes,
        })
    }
}

/// The admission state of one retained owned child.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnedProcessState {
    /// Its initial thread has not been resumed.
    Suspended,
    /// Its initial thread has been resumed exactly once.
    Running,
    /// No further execution or observation admission is permitted.
    Revoked,
}

/// A refusal to resume an owned child.
pub enum ResumeRefusal {
    /// Cancellation permanently revoked this lifetime.
    Revoked,
    /// The initial thread has already been resumed.
    AlreadyRunning,
    /// The retained identity or suspend count could not be verified.
    Unverified(io::Error),
}
impl fmt::Display for ResumeRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for ResumeRefusal {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unverified(error) => Some(error),
            Self::Revoked | Self::AlreadyRunning => None,
        }
    }
}

/// A retained-root observation taken while the root is live in its configured job.
#[derive(Debug)]
pub struct OwnedProcessObservation {
    pub(super) state: OwnedProcessState,
    pub(super) generation: OwnedProcessGeneration,
    pub(super) active: u32,
    pub(super) live_member: bool,
    pub(super) limits: OwnedProcessLimits,
}
impl OwnedProcessObservation {
    /// Returns the execution admission state at observation.
    #[must_use]
    pub const fn state(&self) -> OwnedProcessState {
        self.state
    }
    /// Returns this opaque adapter-local lifetime token.
    #[must_use]
    pub const fn generation(&self) -> &OwnedProcessGeneration {
        &self.generation
    }
    /// Reports the successful retained-handle membership and liveness observation.
    #[must_use]
    pub fn root_is_live_member(&self) -> bool {
        self.live_member
    }
    /// Returns the current active count cross-checked against complete membership.
    #[must_use]
    pub const fn active_processes(&self) -> u32 {
        self.active
    }
    /// Returns the configured limits read back during observation.
    #[must_use]
    pub const fn limits(&self) -> OwnedProcessLimits {
        self.limits
    }
}

/// The completeness of explicit release observations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CleanupOutcome {
    /// All required observations and requested deletion completed without errors.
    ObservedReleased,
    /// One or more resources or observations remain unresolved.
    Incomplete,
}

/// A failed release stage, retained separately from the startup failure.
pub enum CleanupFailure {
    /// The one total monotonic budget expired.
    Deadline,
    /// A job query, termination or identity operation failed.
    Process(io::Error),
    /// A pipe operation failed.
    Pipe(io::Error),
    /// Captured output exceeded its explicit per-stream bound.
    OutputLimit,
    /// Explicit handle closure failed.
    Close(io::Error),
    /// Requested private-state deletion failed.
    Delete(io::Error),
}

/// Observations and bounded output retained after an explicit cleanup attempt.
pub struct CleanupReport {
    pub(super) root: Option<()>,
    pub(super) counts: Option<(u32, u32)>,
    pub(super) stdout_eof: Option<()>,
    pub(super) stderr_eof: Option<()>,
    pub(super) handles: Option<()>,
    pub(super) deleted: Option<()>,
    pub(super) stdout: Vec<u8>,
    pub(super) stderr: Vec<u8>,
    pub(super) failures: Vec<CleanupFailure>,
}
impl CleanupReport {
    /// Classifies the actual observations and any retained failures.
    #[must_use]
    pub fn outcome(&self) -> CleanupOutcome {
        if self.root.is_some()
            && self.counts == Some((0, 0))
            && self.stdout_eof.is_some()
            && self.stderr_eof.is_some()
            && self.handles.is_some()
            && self.deleted.is_some()
            && self.failures.is_empty()
        {
            CleanupOutcome::ObservedReleased
        } else {
            CleanupOutcome::Incomplete
        }
    }
    /// Reports a signalled retained root process.
    #[must_use]
    pub const fn root_exited(&self) -> bool {
        self.root.is_some()
    }
    /// Returns the last queried active count, or None when unavailable.
    #[must_use]
    pub const fn observed_process_counts(&self) -> Option<(u32, u32)> {
        self.counts
    }
    /// Returns the queried active count, or None when unavailable.
    #[must_use]
    pub fn active_processes(&self) -> Option<u32> {
        self.counts.map(|p| p.0)
    }
    /// Returns the queried member count, or None when unavailable.
    #[must_use]
    pub fn member_processes(&self) -> Option<u32> {
        self.counts.map(|p| p.1)
    }
    /// Reports observed stdout pipe EOF.
    #[must_use]
    pub const fn stdout_eof(&self) -> bool {
        self.stdout_eof.is_some()
    }
    /// Reports observed stderr pipe EOF.
    #[must_use]
    pub const fn stderr_eof(&self) -> bool {
        self.stderr_eof.is_some()
    }
    /// Reports successful explicit closure of all owned handles.
    #[must_use]
    pub const fn owned_handles_closed(&self) -> bool {
        self.handles.is_some()
    }
    /// Returns the bounded captured stdout bytes.
    #[must_use]
    pub fn stdout(&self) -> &[u8] {
        &self.stdout
    }
    /// Returns the bounded captured stderr bytes.
    #[must_use]
    pub fn stderr(&self) -> &[u8] {
        &self.stderr
    }
    /// Returns all recorded release failures.
    #[must_use]
    pub fn failures(&self) -> &[CleanupFailure] {
        &self.failures
    }
}

/// A primary startup failure with any explicit cleanup result preserved.
pub struct OwnedProcessFailure {
    pub(super) primary: io::Error,
    pub(super) cleanup: Option<Result<Box<CleanupReport>, io::Error>>,
}
impl OwnedProcessFailure {
    /// Returns the primary operation failure.
    #[must_use]
    pub const fn primary(&self) -> &io::Error {
        &self.primary
    }
    /// Returns cleanup observations when a child had already been created.
    #[must_use]
    pub fn cleanup(&self) -> Option<Result<&CleanupReport, &io::Error>> {
        self.cleanup
            .as_ref()
            .map(|result| result.as_ref().map(AsRef::as_ref))
    }
}
impl From<io::Error> for OwnedProcessFailure {
    fn from(primary: io::Error) -> Self {
        Self {
            primary,
            cleanup: None,
        }
    }
}
impl std::error::Error for OwnedProcessFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.primary)
    }
}
