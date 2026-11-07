// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Suspended owned-child lifetime and measured bounded release.

use super::handles::Handle;
use super::model::{
    CleanupFailure, CleanupReport, OwnedProcessCommand, OwnedProcessFailure,
    OwnedProcessGeneration, OwnedProcessLimits, OwnedProcessObservation, OwnedProcessState,
    ResumeRefusal,
};
use super::pipes::Capture;
use super::{child, job, model, pipes};
use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const CLEANUP_BUDGET: Duration = Duration::from_secs(5);
const POLL_INTERVAL: Duration = Duration::from_millis(1);

/// One exclusively owned Windows job and retained suspended or running root.
pub struct OwnedProcess {
    job: Handle,
    child: child::Child,
    stdin: Handle,
    child_pipe_ends: Vec<Handle>,
    stdout: Capture,
    stderr: Capture,
    private: PathBuf,
    limits: OwnedProcessLimits,
    creation: Option<u64>,
    generation: OwnedProcessGeneration,
    state: OwnedProcessState,
    cleanup_attempted: Option<()>,
}
impl OwnedProcess {
    /// Creates the exact command suspended with job and stdio lists applied at creation.
    ///
    /// # Errors
    /// Returns the startup failure with explicit cleanup observations after child creation.
    pub fn create_suspended(
        command: OwnedProcessCommand,
        limits: OwnedProcessLimits,
    ) -> Result<Self, OwnedProcessFailure> {
        let generation = OwnedProcessGeneration::new();
        let job = job::create(limits)?;
        let (stdin_read, stdin) = pipes::pair()?;
        let (stdout_read, stdout_write) = pipes::pair()?;
        let (stderr_read, stderr_write) = pipes::pair()?;
        stdin.clear_inheritance()?;
        stdout_read.clear_inheritance()?;
        stderr_read.clear_inheritance()?;
        let child = child::spawn(&command, &job, &stdin_read, &stdout_write, &stderr_write)?;
        let mut owned = Self {
            job,
            child,
            stdin,
            child_pipe_ends: vec![stdin_read, stdout_write, stderr_write],
            stdout: Capture::new(stdout_read, command.capture),
            stderr: Capture::new(stderr_read, command.capture),
            private: command.private,
            limits,
            creation: None,
            generation,
            state: OwnedProcessState::Suspended,
            cleanup_attempted: None,
        };
        if let Err(primary) = owned.finish_startup() {
            owned.cancel();
            let cleanup = Some(owned.cleanup_failed_startup().map(Box::new));
            return Err(OwnedProcessFailure { primary, cleanup });
        }
        Ok(owned)
    }

    fn finish_startup(&mut self) -> io::Result<()> {
        self.child.root.clear_inheritance()?;
        self.child.thread.clear_inheritance()?;
        for end in &mut self.child_pipe_ends {
            end.close()?;
        }
        self.creation = Some(child::creation_time(&self.child.root)?);
        self.observe()?;
        Ok(())
    }

    /// Returns the adapter-local lifetime token, separate from metadata domain identities.
    #[must_use]
    pub const fn generation(&self) -> &OwnedProcessGeneration {
        &self.generation
    }
    /// Returns the synchronous admission state.
    #[must_use]
    pub const fn state(&self) -> OwnedProcessState {
        self.state
    }

    /// Rechecks live retained-root identity, membership, priority and current job limits.
    ///
    /// # Errors
    /// Refuses revoked, exited, changed, nonmember or unqueryable identities.
    pub fn observe(&self) -> io::Result<OwnedProcessObservation> {
        if self.state == OwnedProcessState::Revoked {
            return Err(io::Error::other("owned root admission is revoked"));
        }
        let live_member = !child::exited(&self.child.root)?
            && Some(child::creation_time(&self.child.root)?) == self.creation
            && job::includes(&self.job, &self.child.root)?
            && job::members(&self.job)?.contains(&self.child.pid);
        if !live_member {
            return Err(io::Error::other("owned root is not a current live member"));
        }
        child::verify_priority(&self.child.root)?;
        job::verify(&self.job, self.limits)?;
        let (active, _) = job::counts(&self.job)?;
        if child::exited(&self.child.root)? {
            return Err(io::Error::other("owned root exited during observation"));
        }
        Ok(OwnedProcessObservation {
            state: self.state,
            generation: self.generation.clone(),
            limits: self.limits,
            active,
            live_member,
        })
    }

    /// Resumes the checked initial thread exactly once.
    ///
    /// # Errors
    /// Refuses revocation, repeated resume or unavailable identity/suspend-count evidence.
    pub fn resume(&mut self) -> Result<(), ResumeRefusal> {
        match self.state {
            OwnedProcessState::Revoked => return Err(ResumeRefusal::Revoked),
            OwnedProcessState::Running => return Err(ResumeRefusal::AlreadyRunning),
            OwnedProcessState::Suspended => {}
        }
        if let Err(error) = self
            .observe()
            .and_then(|_| child::resume(&self.child.thread))
        {
            self.cancel();
            return Err(ResumeRefusal::Unverified(error));
        }
        self.state = OwnedProcessState::Running;
        Ok(())
    }

    /// Permanently revokes admission before any blocking cleanup effects.
    pub const fn cancel(&mut self) {
        self.state = OwnedProcessState::Revoked;
    }

    /// Terminates the owned tree and records release under one total monotonic budget.
    ///
    /// # Errors
    /// Refuses zero or over-five-second budgets and repeated cleanup attempts.
    /// Operational failures remain in the returned incomplete report.
    pub fn cleanup(&mut self, budget: Duration) -> io::Result<CleanupReport> {
        if budget.is_zero() || budget > CLEANUP_BUDGET || self.cleanup_attempted.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid or repeated owned cleanup",
            ));
        }
        let deadline = Self::cleanup_deadline(budget)?;
        Ok(self.cleanup_until(deadline, None))
    }

    fn cleanup_failed_startup(&mut self) -> io::Result<CleanupReport> {
        let deadline = Self::cleanup_deadline(CLEANUP_BUDGET)?;
        let reconciliation = child::terminate_failed_root(&self.child.root).err();
        Ok(self.cleanup_until(deadline, reconciliation))
    }

    fn cleanup_deadline(budget: Duration) -> io::Result<Instant> {
        Instant::now()
            .checked_add(budget)
            .ok_or_else(|| io::Error::other("cleanup deadline overflow"))
    }

    fn cleanup_until(
        &mut self,
        deadline: Instant,
        reconciliation: Option<io::Error>,
    ) -> CleanupReport {
        self.cancel();
        self.cleanup_attempted = Some(());
        let mut report = CleanupReport {
            root: None,
            counts: None,
            stdout_eof: None,
            stderr_eof: None,
            handles: None,
            deleted: None,
            stdout: Vec::new(),
            stderr: Vec::new(),
            failures: Vec::new(),
        };
        if let Some(error) = reconciliation {
            report.failures.push(CleanupFailure::Process(error));
        }
        if let Err(error) = job::terminate(&self.job) {
            report.failures.push(CleanupFailure::Process(error));
        }
        for end in &mut self.child_pipe_ends {
            if let Err(error) = end.close() {
                report.failures.push(CleanupFailure::Close(error));
            }
        }
        loop {
            if Instant::now() >= deadline {
                report.failures.push(CleanupFailure::Deadline);
                break;
            }
            match child::exited(&self.child.root) {
                Ok(true) => report.root = Some(()),
                Ok(false) => {}
                Err(error) => {
                    report.failures.push(CleanupFailure::Process(error));
                    break;
                }
            }
            match job::counts(&self.job) {
                Ok(counts) => report.counts = Some(counts),
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
                Err(error) => {
                    report.failures.push(CleanupFailure::Process(error));
                    break;
                }
            }
            if let Err(error) = self.stdout.poll() {
                report.failures.push(CleanupFailure::Pipe(error));
                break;
            }
            if let Err(error) = self.stderr.poll() {
                report.failures.push(CleanupFailure::Pipe(error));
                break;
            }
            if report.root.is_some()
                && report.counts == Some((0, 0))
                && self.stdout.eof.is_some()
                && self.stderr.eof.is_some()
            {
                break;
            }
            std::thread::sleep(
                POLL_INTERVAL.min(deadline.saturating_duration_since(Instant::now())),
            );
        }
        report.stdout_eof = self.stdout.eof;
        report.stderr_eof = self.stderr.eof;
        report.stdout = std::mem::take(&mut self.stdout.bytes);
        report.stderr = std::mem::take(&mut self.stderr.bytes);
        if self.stdout.exceeded.is_some() || self.stderr.exceeded.is_some() {
            report.failures.push(CleanupFailure::OutputLimit);
        }
        for handle in [
            &mut self.stdin,
            &mut self.stdout.reader,
            &mut self.stderr.reader,
            &mut self.child.thread,
            &mut self.child.root,
            &mut self.job,
        ] {
            if let Err(error) = handle.close() {
                report.failures.push(CleanupFailure::Close(error));
            }
        }
        if self.stdin.is_closed()
            && self.stdout.reader.is_closed()
            && self.stderr.reader.is_closed()
            && self.child.thread.is_closed()
            && self.child.root.is_closed()
            && self.job.is_closed()
            && self.child_pipe_ends.iter().all(Handle::is_closed)
        {
            report.handles = Some(());
        }
        if Instant::now() < deadline
            && report.root.is_some()
            && report.counts == Some((0, 0))
            && report.stdout_eof.is_some()
            && report.stderr_eof.is_some()
            && report.handles.is_some()
        {
            match std::fs::remove_dir_all(&self.private) {
                Ok(()) => match self.private.try_exists() {
                    Ok(false) => report.deleted = Some(()),
                    Ok(true) => report
                        .failures
                        .push(CleanupFailure::Delete(io::Error::other(
                            "owned private directory remains",
                        ))),
                    Err(error) => report.failures.push(CleanupFailure::Delete(error)),
                },
                Err(error) => report.failures.push(CleanupFailure::Delete(error)),
            }
        }
        if Instant::now() >= deadline
            && !report
                .failures
                .iter()
                .any(|error| matches!(error, CleanupFailure::Deadline))
        {
            report.failures.push(CleanupFailure::Deadline);
        }
        report
    }
}
impl Drop for OwnedProcess {
    fn drop(&mut self) {
        if self.cleanup_attempted.is_none() {
            match self.cleanup(CLEANUP_BUDGET) {
                Ok(report) if report.outcome() == model::CleanupOutcome::ObservedReleased => {}
                Ok(report) => tracing::error!(
                    failure_count = report.failures.len(),
                    "owned process fallback cleanup incomplete"
                ),
                Err(error) => tracing::error!(?error, "owned process fallback cleanup failed"),
            }
        }
    }
}
