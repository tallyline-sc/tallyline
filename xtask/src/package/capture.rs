// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Capture without persisting a tool's accidental fixture disclosure.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;

const MAX_CAPTURE_BYTES: usize = 8 * 1024 * 1024;

pub(super) fn write(
    directory: &Path,
    name: &str,
    stdout: &[u8],
    stderr: &[u8],
    fixture: &Path,
) -> io::Result<()> {
    if !fs::symlink_metadata(directory)?.file_type().is_dir()
        || !super::archive::portable_name(name)
        || stdout.len() > MAX_CAPTURE_BYTES
        || stderr.len() > MAX_CAPTURE_BYTES
    {
        return Err(io::Error::other(
            "invalid or excessive packaging tool capture",
        ));
    }
    let leaked = super::scan::contains_fixture(stdout, fixture)?
        || super::scan::contains_fixture(stderr, fixture)?;
    let discarded = b"Captured content contained fixture bytes and was not retained.\n";
    for (extension, bytes) in [("stdout", stdout), ("stderr", stderr)] {
        let mut file = File::options()
            .write(true)
            .create_new(true)
            .open(directory.join(format!("{name}.{extension}")))?;
        file.write_all(if leaked { discarded } else { bytes })?;
        file.sync_all()?;
    }
    if leaked {
        return Err(io::Error::other(
            "packaging tool disclosed fixture bytes; captured content was not retained",
        ));
    }
    Ok(())
}
