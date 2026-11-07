// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Content-free diagnostics for owned-child observations and failures.

use super::model::{CleanupFailure, CleanupReport, OwnedProcessFailure, ResumeRefusal};
use std::fmt;
use std::io;

struct ErrorSummary<'a>(&'a io::Error);

impl fmt::Debug for ErrorSummary<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IoError")
            .field("kind", &self.0.kind())
            .field("os_code", &self.0.raw_os_error())
            .finish()
    }
}

impl fmt::Debug for CleanupReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CleanupReport")
            .field("outcome", &self.outcome())
            .field("root_exited", &self.root_exited())
            .field("process_counts", &self.observed_process_counts())
            .field("stdout_eof", &self.stdout_eof())
            .field("stderr_eof", &self.stderr_eof())
            .field("handles_closed", &self.owned_handles_closed())
            .field("private_deleted", &self.deleted.is_some())
            .field("stdout_bytes", &self.stdout.len())
            .field("stderr_bytes", &self.stderr.len())
            .field("failures", &self.failures)
            .finish()
    }
}

impl fmt::Debug for CleanupFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Deadline => f.write_str("Deadline"),
            Self::OutputLimit => f.write_str("OutputLimit"),
            Self::Process(error) => f
                .debug_tuple("Process")
                .field(&ErrorSummary(error))
                .finish(),
            Self::Pipe(error) => f.debug_tuple("Pipe").field(&ErrorSummary(error)).finish(),
            Self::Close(error) => f.debug_tuple("Close").field(&ErrorSummary(error)).finish(),
            Self::Delete(error) => f.debug_tuple("Delete").field(&ErrorSummary(error)).finish(),
        }
    }
}

impl fmt::Debug for ResumeRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Revoked => f.write_str("Revoked"),
            Self::AlreadyRunning => f.write_str("AlreadyRunning"),
            Self::Unverified(error) => f
                .debug_tuple("Unverified")
                .field(&ErrorSummary(error))
                .finish(),
        }
    }
}

impl fmt::Debug for OwnedProcessFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OwnedProcessFailure")
            .field("primary", &ErrorSummary(&self.primary))
            .field(
                "cleanup",
                &self
                    .cleanup
                    .as_ref()
                    .map(|result| result.as_ref().map_err(ErrorSummary)),
            )
            .finish()
    }
}

impl fmt::Display for OwnedProcessFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "owned-child startup failed: {:?} (cleanup: {:?})",
            ErrorSummary(&self.primary),
            self.cleanup.as_ref().map(|result| {
                result
                    .as_ref()
                    .map(|report| report.outcome())
                    .map_err(ErrorSummary)
            })
        )
    }
}
