// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Installed and portable state locations.

use std::fs::{self, DirBuilder, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Separate locations for configuration, durable data, logs and regenerable files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StorageLayout {
    config: PathBuf,
    data: PathBuf,
    logs: PathBuf,
    cache: PathBuf,
}

/// A failed directory probe or an overly broad Unix permission.
#[derive(Debug)]
pub struct DirectoryIssue {
    /// The directory that needs attention.
    pub path: PathBuf,
    /// The underlying filesystem or permissions failure.
    pub error: io::Error,
    /// Whether the probe failed or only directory permissions need attention.
    pub kind: DirectoryIssueKind,
}

/// The effect of a directory self-test finding on startup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirectoryIssueKind {
    /// Read/write probing succeeded but Unix permissions permit other users.
    PermissionsWarning,
    /// Creating, opening, writing, syncing or removing the probe failed.
    IoFailure,
}

impl StorageLayout {
    /// Resolves installed Windows state under the supplied profile directories.
    #[must_use]
    pub fn installed_windows(roaming: &Path, local: &Path) -> Self {
        Self {
            config: roaming.join("Tallyline"),
            data: local.join("Tallyline/data"),
            logs: local.join("Tallyline/logs"),
            cache: local.join("Tallyline/cache"),
        }
    }

    /// Resolves installed Linux state under the supplied XDG directories.
    #[must_use]
    pub fn installed_linux(config: &Path, data: &Path, state: &Path, cache: &Path) -> Self {
        Self {
            config: config.join("tallyline"),
            data: data.join("tallyline"),
            logs: state.join("tallyline/logs"),
            cache: cache.join("tallyline"),
        }
    }
    /// Resolves the portable layout below the executable's data directory.
    #[must_use]
    pub fn portable(executable_directory: &Path) -> Self {
        let root = executable_directory.join("data");
        Self {
            config: root.join("config"),
            data: root.join("data"),
            logs: root.join("logs"),
            cache: root.join("cache"),
        }
    }

    /// Returns the configuration directory.
    #[must_use]
    pub fn config(&self) -> &Path {
        &self.config
    }

    /// Returns the durable-data directory.
    #[must_use]
    pub fn data(&self) -> &Path {
        &self.data
    }

    /// Returns the log directory.
    #[must_use]
    pub fn logs(&self) -> &Path {
        &self.logs
    }

    /// Returns the regenerable-cache directory.
    #[must_use]
    pub fn cache(&self) -> &Path {
        &self.cache
    }

    /// Creates and probes every directory without falling back to another location.
    #[must_use]
    pub fn self_test(&self) -> Vec<DirectoryIssue> {
        [&self.config, &self.data, &self.logs, &self.cache]
            .into_iter()
            .filter_map(|path| match probe_directory(path) {
                Ok(false) => None,
                Ok(true) => Some(DirectoryIssue {
                    path: path.clone(),
                    error: io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "directory is accessible to another user",
                    ),
                    kind: DirectoryIssueKind::PermissionsWarning,
                }),
                Err(error) => Some(DirectoryIssue {
                    path: path.clone(),
                    error,
                    kind: DirectoryIssueKind::IoFailure,
                }),
            })
            .collect()
    }
}

fn probe_directory(path: &Path) -> io::Result<bool> {
    let mut builder = DirBuilder::new();
    builder.recursive(true);
    let accessible_to_others;
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        builder.mode(0o700);
        builder.create(path)?;
        accessible_to_others = fs::metadata(path)?.permissions().mode() & 0o077 != 0;
    }
    #[cfg(not(unix))]
    {
        builder.create(path)?;
        accessible_to_others = false;
    }
    let probe = path.join(".tallyline-write-test");
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&probe)?;
    let result = file
        .write_all(b"Tallyline storage self-test")
        .and_then(|()| file.sync_all());
    drop(file);
    fs::remove_file(probe)?;
    result.map(|()| accessible_to_others)
}
