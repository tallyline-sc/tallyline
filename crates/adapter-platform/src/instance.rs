// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Lifetime ownership of cooperating-writer locks and local activation.

use crate::{protocol, socket::LocalSocket};
use socket2::SockAddr;
use std::fs::{self, DirBuilder, File, OpenOptions, TryLockError};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

/// Resolved lock and endpoint paths independent of activation arguments.
pub struct InstancePaths {
    state_lock: Option<PathBuf>,
    data_lock: PathBuf,
    user_lock: PathBuf,
    endpoint: PathBuf,
}

impl InstancePaths {
    /// Resolves Windows election paths independently of installed or portable data.
    #[must_use]
    pub fn windows(data: &Path, local: &Path) -> Self {
        let root = local.join("Tallyline");
        Self::new(
            data.join("instance.lock"),
            root.join("instance.lock"),
            root.join("run/a"),
        )
    }

    /// Resolves Linux runtime paths, falling back to the private state directory.
    #[cfg(unix)]
    #[must_use]
    pub fn linux(data: &Path, state: &Path, runtime: Option<&Path>) -> Self {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let valid_runtime = runtime.filter(|path| {
            path.is_absolute()
                && fs::symlink_metadata(path).is_ok_and(|metadata| {
                    metadata.is_dir()
                        && metadata.permissions().mode() & 0o777 == 0o700
                        && metadata.uid() == rustix::process::geteuid().as_raw()
                })
        });
        match valid_runtime {
            Some(root) => Self {
                state_lock: Some(state.join("tallyline/instance.lock")),
                data_lock: data.join("instance.lock"),
                user_lock: root.join("tallyline.lock"),
                endpoint: root.join("tl-a"),
            },
            None => {
                let root = state.join("tallyline");
                Self::new(
                    data.join("instance.lock"),
                    root.join("instance.lock"),
                    root.join("run/a"),
                )
            }
        }
    }
    /// Uses explicit resolved paths for the data lock, fixed user lock and socket.
    #[must_use]
    pub fn new(data_lock: PathBuf, user_lock: PathBuf, endpoint: PathBuf) -> Self {
        Self {
            state_lock: None,
            data_lock,
            user_lock,
            endpoint,
        }
    }
}

/// The only outcomes of attempting to become the per-user writer.
pub enum Election {
    /// Both exclusive locks remain owned until this guard is dropped.
    Primary(PrimaryInstance),
    /// Another writer holds a lock; this process must exit.
    Secondary(ActivationOutcome),
}

/// Whether the existing writer acknowledged the exact activation request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivationOutcome {
    /// The writer accepted the activation token.
    Activated,
    /// Activation failed or exceeded the bounded startup deadline.
    Unavailable,
}

/// Owns both locks until the application's writable state has closed.
pub struct PrimaryInstance {
    lease: WriterLease,
    listener: ActivationListener,
}

struct Locks {
    _state: Option<File>,
    _user: File,
    _data: File,
}

/// Keeps both writer locks held independently of the listener worker.
pub struct WriterLease {
    _locks: Arc<Locks>,
}

/// Services bounded local requests while retaining the same writer locks.
pub struct ActivationListener {
    listener: Option<LocalSocket>,
    endpoint: PathBuf,
    _locks: Arc<Locks>,
}

impl PrimaryInstance {
    /// Separates application ownership from the app-created listener worker.
    #[must_use]
    pub fn into_parts(self) -> (WriterLease, ActivationListener) {
        (self.lease, self.listener)
    }

    /// Polls one client without releasing either writer lock.
    ///
    /// # Errors
    /// Returns an unexpected listener failure.
    pub fn poll_activation(&mut self) -> io::Result<bool> {
        self.listener.poll_activation()
    }
}

impl ActivationListener {
    /// Polls one client, accepting only the bounded exact activation frame.
    ///
    /// # Errors
    /// Returns an unexpected listener error; malformed clients are rejected.
    pub fn poll_activation(&mut self) -> io::Result<bool> {
        let listener = self
            .listener
            .as_ref()
            .ok_or_else(|| io::Error::other("listener is closed"))?;
        match listener.accept() {
            Ok(client) => match protocol::accept(&client) {
                Ok(accepted) => Ok(accepted),
                Err(error) => {
                    tracing::debug!(kind = ?error.kind(), "Activation client rejected");
                    Ok(false)
                }
            },
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(false),
            Err(error) => Err(error),
        }
    }
}

impl Drop for ActivationListener {
    fn drop(&mut self) {
        drop(self.listener.take());
        if let Err(error) = fs::remove_file(&self.endpoint)
            && error.kind() != io::ErrorKind::NotFound
        {
            tracing::warn!(%error, "Instance endpoint cleanup failed");
        }
    }
}

/// Acquires both locks or activates the existing writer without opening data.
///
/// # Errors
/// Returns an I/O failure if primary directories, locks or listening fail.
pub fn elect(paths: InstancePaths) -> io::Result<Election> {
    let state = match &paths.state_lock {
        Some(path) => match lock(path)? {
            Some(file) => Some(file),
            None => return Ok(secondary(&paths.endpoint)),
        },
        None => None,
    };
    let Some(user) = lock(&paths.user_lock)? else {
        return Ok(secondary(&paths.endpoint));
    };
    let Some(data) = lock(&paths.data_lock)? else {
        return Ok(secondary(&paths.endpoint));
    };
    let address = SockAddr::unix(&paths.endpoint)?;
    let parent = paths.endpoint.parent().ok_or(io::ErrorKind::InvalidInput)?;
    directory(parent)?;
    match fs::remove_file(&paths.endpoint) {
        Ok(()) => (),
        Err(error) if error.kind() == io::ErrorKind::NotFound => (),
        Err(error) => return Err(error),
    }
    let listener = LocalSocket::new()?;
    listener.nonblocking()?;
    listener.bind(&address)?;
    listener.listen()?;
    let locks = Arc::new(Locks {
        _state: state,
        _user: user,
        _data: data,
    });
    Ok(Election::Primary(PrimaryInstance {
        lease: WriterLease {
            _locks: Arc::clone(&locks),
        },
        listener: ActivationListener {
            listener: Some(listener),
            endpoint: paths.endpoint,
            _locks: locks,
        },
    }))
}

fn secondary(endpoint: &Path) -> Election {
    let deadline = Instant::now() + protocol::LIMITS.startup_timeout;
    let result = (|| {
        let address = SockAddr::unix(endpoint)?;
        loop {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or_else(|| io::Error::from(io::ErrorKind::TimedOut))?;
            let socket = LocalSocket::new()?;
            match socket.connect(&address, remaining) {
                Ok(()) => return protocol::request(&socket, deadline),
                Err(error) => {
                    tracing::debug!(kind = ?error.kind(), "Instance connection not ready");
                    protocol::wait(deadline)?;
                }
            }
        }
    })();
    let outcome = match result {
        Ok(()) => ActivationOutcome::Activated,
        Err(error) => {
            tracing::debug!(kind = ?error.kind(), "Instance activation unavailable");
            ActivationOutcome::Unavailable
        }
    };
    Election::Secondary(outcome)
}

fn directory(path: &Path) -> io::Result<()> {
    let mut builder = DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
        builder.mode(0o700);
        builder.create(path)?;
        let metadata = fs::symlink_metadata(path)?;
        if !metadata.is_dir()
            || metadata.permissions().mode() & 0o077 != 0
            || metadata.uid() != rustix::process::geteuid().as_raw()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "instance directory is not private to this user",
            ));
        }
    }
    #[cfg(windows)]
    builder.create(path)?;
    Ok(())
}

fn lock(path: &Path) -> io::Result<Option<File>> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "lock parent missing"))?;
    directory(parent)?;
    if fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "instance lock is a symbolic link",
        ));
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(TryLockError::WouldBlock) => Ok(None),
        Err(TryLockError::Error(error)) => Err(error),
    }
}
