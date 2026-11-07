// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exclusive private crop-dump creation and owned-output cleanup.

use std::fs;
use std::path::{Component, Path};
use tallyline_adapter_files::api;

pub(super) fn write(destination: &Path, artifacts: [(&str, &[u8]); 3]) -> Result<(), String> {
    if !destination.is_absolute()
        || destination
            .components()
            .any(|v| matches!(v, Component::ParentDir | Component::CurDir))
    {
        return Err("output needs an absolute new directory".into());
    }
    let parent = destination.parent().ok_or("output parent is missing")?;
    for ancestor in parent.ancestors() {
        let metadata =
            fs::symlink_metadata(ancestor).map_err(|_| "output parent is unavailable")?;
        if !metadata.is_dir() || linked(&metadata) {
            return Err("output parent contains a link or non-directory".into());
        }
    }
    #[cfg(unix)]
    let builder = {
        use std::os::unix::fs::DirBuilderExt;
        let mut builder = fs::DirBuilder::new();
        builder.mode(0o700);
        builder
    };
    #[cfg(not(unix))]
    let builder = fs::DirBuilder::new();
    builder
        .create(destination)
        .map_err(|_| "output directory exists or cannot be created")?;
    let result = artifacts.iter().try_for_each(|(name, bytes)| {
        api::write_private_artifact(&destination.join(name), bytes)
            .map_err(|_| "private output write failed".to_owned())
    });
    if let Err(error) = result {
        for (name, _) in artifacts {
            match fs::remove_file(destination.join(name)) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => {
                    return Err("private output failed and file cleanup was incomplete".into());
                }
            }
        }
        fs::remove_dir(destination)
            .map_err(|_| "private output failed and directory cleanup was incomplete")?;
        return Err(error);
    }
    Ok(())
}

fn linked(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}
