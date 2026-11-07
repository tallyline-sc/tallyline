// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Automatic workspace output locations for repository tools.

use std::ffi::OsStr;
use std::fs;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::process::Command;

const OWNER_FILE: &str = ".tallyline-xtask-owner";

/// An automatic output tree owned by the repository tool.
#[derive(Clone, Copy, Debug)]
pub enum OutputKind {
    /// Isolated vendored sources and their temporary Cargo home.
    Vendor,
    /// Generated notices for shipped targets.
    Notices,
    /// Build artefacts for nested workspace quality checks.
    Check,
    /// Mutation selections and reports.
    Mutation,
}

/// Resolved locations for workspace tool outputs.
pub struct WorkPaths {
    target: PathBuf,
    workspace: PathBuf,
    target_overridden: bool,
}

impl WorkPaths {
    /// Resolves automatic outputs for an invocation of the workspace tools.
    ///
    /// # Errors
    /// Returns an error if the output location cannot be resolved.
    pub fn resolve(
        workspace: &Path,
        invocation: &Path,
        target_override: Option<&OsStr>,
    ) -> io::Result<Self> {
        let workspace = normalise_absolute(workspace)?;
        let invocation = normalise_absolute(invocation)?;
        let target = match target_override {
            Some(value) if value.is_empty() => {
                return Err(invalid_path("CARGO_TARGET_DIR is empty"));
            }
            Some(value) => {
                let path = Path::new(value);
                if !path.is_absolute()
                    && (path.has_root()
                        || matches!(path.components().next(), Some(Component::Prefix(_))))
                {
                    return Err(invalid_path("CARGO_TARGET_DIR is drive-relative"));
                }
                let absolute = invocation.join(path);
                directory_chain(&absolute)?;
                normalise_absolute(&absolute)?
            }
            None => workspace.join("target"),
        };
        directory_chain(&workspace)?;
        let canonical_workspace = workspace.canonicalize()?;
        directory_chain(&target)?;
        if target.parent().is_none()
            || (target.exists() && canonical_workspace.starts_with(target.canonicalize()?))
        {
            return Err(invalid_path("output root contains the workspace"));
        }
        Ok(Self {
            target,
            workspace,
            target_overridden: target_override.is_some(),
        })
    }

    /// Configures a tool subprocess for this workspace.
    #[must_use]
    pub fn command(&self, program: &OsStr) -> Command {
        let mut command = Command::new(program);
        command.current_dir(&self.workspace);
        if self.target_overridden {
            command.env("CARGO_TARGET_DIR", &self.target);
        }
        command
    }

    /// Configures a quality-check subprocess with independent build artefacts.
    ///
    /// # Errors
    /// Refuses foreign, linked or obstructed output and filesystem failures.
    pub fn check_command(&self, program: &OsStr) -> io::Result<Command> {
        let output = self.prepare(OutputKind::Check)?;
        let mut command = self.command(program);
        command
            .env("CARGO_TARGET_DIR", &output)
            .env("CARGO_BUILD_BUILD_DIR", &output);
        Ok(command)
    }

    /// Configures mutation work that executes in copied source roots.
    #[must_use]
    pub fn mutation_command(&self, program: &OsStr) -> Command {
        let mut command = self.command(program);
        command
            .env("CARGO_TARGET_DIR", "target")
            .env("CARGO_BUILD_BUILD_DIR", "target")
            .env("CARGO_PROFILE_DEV_DEBUG", "0")
            .env("CARGO_PROFILE_TEST_DEBUG", "0")
            .env("CARGO_INCREMENTAL", "0");
        command
    }

    /// Creates an owned automatic output directory.
    ///
    /// # Errors
    /// Refuses foreign, linked or obstructed output and filesystem failures.
    pub fn prepare(&self, kind: OutputKind) -> io::Result<PathBuf> {
        self.prepare_path(&self.output(kind))
    }

    /// Replaces this workspace's owned output tree with an empty directory.
    ///
    /// # Errors
    /// Refuses foreign or linked output and filesystem failures.
    pub fn reset(&self, kind: OutputKind) -> io::Result<PathBuf> {
        self.remove(kind)?;
        self.prepare(kind)
    }

    /// Removes only this workspace's owned output tree.
    ///
    /// # Errors
    /// Refuses foreign or linked output and filesystem failures.
    pub fn remove(&self, kind: OutputKind) -> io::Result<()> {
        let path = self.output(kind);
        directory_chain(&path)?;
        if path.exists() {
            self.require_owner(&path)?;
            fs::remove_dir_all(path)?;
        }
        Ok(())
    }

    /// Creates one owned child directory below an automatic output tree.
    ///
    /// # Errors
    /// Refuses path traversal, unowned content, links and filesystem failures.
    pub fn prepare_child(&self, kind: OutputKind, name: &OsStr) -> io::Result<PathBuf> {
        let name_path = Path::new(name);
        if name.to_str().is_none_or(|value| value.contains(':'))
            || name_path.components().count() != 1
            || !matches!(name_path.components().next(), Some(Component::Normal(_)))
        {
            return Err(invalid_path("output child must be a single directory name"));
        }
        let parent = self.prepare(kind)?;
        self.prepare_path(&parent.join(name))
    }

    fn prepare_path(&self, path: &Path) -> io::Result<PathBuf> {
        directory_chain(path)?;
        fs::create_dir_all(
            path.parent()
                .ok_or_else(|| invalid_path("missing output parent"))?,
        )?;
        match fs::create_dir(path) {
            Ok(()) => {
                let mut marker = fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(path.join(OWNER_FILE))?;
                marker.write_all(&self.owner(path)?)?;
                marker.sync_all()?;
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                self.require_owner(path)?
            }
            Err(error) => return Err(error),
        }
        Ok(path.to_path_buf())
    }

    fn owner(&self, path: &Path) -> io::Result<Vec<u8>> {
        let workspace = self.workspace.canonicalize()?;
        let output = path.canonicalize()?;
        Ok(format!(
            "tallyline-xtask-output-v1\n{}\n{}\n",
            crate::hashlist::sha256_hex(workspace.as_os_str().as_encoded_bytes()),
            crate::hashlist::sha256_hex(output.as_os_str().as_encoded_bytes()),
        )
        .into_bytes())
    }

    fn require_owner(&self, path: &Path) -> io::Result<()> {
        let marker = path.join(OWNER_FILE);
        let expected = self.owner(path)?;
        let metadata = fs::symlink_metadata(&marker)?;
        if !metadata.is_file()
            || is_link(&metadata)
            || metadata.len() != expected.len() as u64
            || fs::read(marker)? != expected
        {
            return Err(invalid_path(
                "automatic output is not owned by this workspace",
            ));
        }
        Ok(())
    }

    /// Returns the build output root.
    #[must_use]
    pub fn target_dir(&self) -> &Path {
        &self.target
    }

    /// Returns the location of an automatic output tree.
    #[must_use]
    pub fn output(&self, kind: OutputKind) -> PathBuf {
        self.target.join(match kind {
            OutputKind::Check => "xtask-check",
            OutputKind::Vendor => "xtask-vendor",
            OutputKind::Notices => "notices",
            OutputKind::Mutation => "core-mutation",
        })
    }
}

fn normalise_absolute(path: &Path) -> io::Result<PathBuf> {
    if !path.is_absolute() {
        return Err(invalid_path("output paths require an absolute base"));
    }
    let mut result = PathBuf::new();
    for component in path.components().filter(|part| *part != Component::CurDir) {
        if component == Component::ParentDir {
            if result.file_name().is_none() || !result.pop() {
                return Err(invalid_path("output path escapes its filesystem root"));
            }
        } else {
            result.push(component.as_os_str());
        }
    }
    Ok(result)
}

fn directory_chain(path: &Path) -> io::Result<()> {
    for ancestor in path.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) => {
                if !metadata.is_dir() || is_link(&metadata) {
                    return Err(invalid_path("output path contains a file or link"));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn is_link(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const REPARSE_POINT_ATTRIBUTE: u32 = 0x400;
        metadata.file_attributes() & REPARSE_POINT_ATTRIBUTE != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

fn invalid_path(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
