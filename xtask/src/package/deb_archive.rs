// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Deterministic final Debian layout and archive records.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;

const LINK_TARGET: &str = "../lib/tallyline/tallyline";

pub(super) fn repack(tree: &Path, output: &Path) -> io::Result<()> {
    if output.try_exists()? {
        return Err(io::Error::other("Debian output already exists"));
    }
    let tree = tree.canonicalize()?;
    if output
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .canonicalize()?
        .starts_with(&tree)
    {
        return Err(io::Error::other(
            "Debian output must be outside its input tree",
        ));
    }
    let control = tree.join("control");
    normalise(&control)?;
    for entry in fs::read_dir(&control)? {
        let entry = entry?;
        if !entry.file_type()?.is_file()
            || !["control", "md5sums"].contains(&entry.file_name().to_string_lossy().as_ref())
        {
            return Err(io::Error::other(
                "unexpected Debian control file or maintainer script",
            ));
        }
    }
    let data = tree.join("data");
    normalise(&data)?;
    let binary = data.join("usr/bin/tallyline");
    let destination = data.join("usr/lib/tallyline/tallyline");
    if !binary.is_file() || destination.try_exists()? {
        return Err(io::Error::other("unexpected Debian executable layout"));
    }
    let sums = fs::read_to_string(control.join("md5sums"))?;
    let mut relocated = 0;
    let sums = sums
        .lines()
        .map(|line| {
            if let Some(hash) = line.strip_suffix("  usr/bin/tallyline") {
                relocated += 1;
                format!("{hash}  usr/lib/tallyline/tallyline\n")
            } else {
                format!("{line}\n")
            }
        })
        .collect::<String>();
    if relocated != 1 {
        return Err(io::Error::other(
            "Debian executable checksum is missing or duplicated",
        ));
    }
    fs::create_dir_all(
        destination
            .parent()
            .ok_or_else(|| io::Error::other("invalid Debian destination"))?,
    )?;
    fs::rename(binary, &destination)?;
    fs::write(control.join("md5sums"), sums)?;
    let link_size = u64::try_from(LINK_TARGET.len())
        .map_err(io::Error::other)?
        .div_ceil(1024);
    let size = installed_size(&data)?
        .checked_sub(1)
        .and_then(|size| size.checked_add(link_size))
        .ok_or_else(|| io::Error::other("Debian installed size exceeds limits"))?;
    let original = fs::read_to_string(control.join("control"))?;
    let mut sizes = 0;
    let updated = original
        .lines()
        .map(|line| {
            if line.starts_with("Installed-Size:") {
                sizes += 1;
                format!("Installed-Size: {size}\n")
            } else {
                format!("{line}\n")
            }
        })
        .collect::<String>();
    if sizes != 1 {
        return Err(io::Error::other(
            "Debian installed-size field is missing or duplicated",
        ));
    }
    fs::write(control.join("control"), updated)?;
    normalise(&data)?;
    normalise(&control)?;
    let control_archive = tree.join("final-control.tar.gz");
    let data_archive = tree.join("final-data.tar.gz");
    super::archive::tree_gzip(&control, create(&control_archive)?, None, file_mode)?;
    super::archive::tree_gzip(
        &data,
        create(&data_archive)?,
        Some(("usr/bin/tallyline", LINK_TARGET)),
        file_mode,
    )?;
    let mut archive = create(output)?;
    let result = (|| {
        archive.write_all(b"!<arch>\n")?;
        member(&mut archive, "debian-binary", b"2.0\n")?;
        member(&mut archive, "control.tar.gz", &fs::read(control_archive)?)?;
        member(&mut archive, "data.tar.gz", &fs::read(data_archive)?)?;
        archive.sync_all()
    })();
    drop(archive);
    if result.is_err() {
        fs::remove_file(output)?;
    }
    result
}

fn create(path: &Path) -> io::Result<File> {
    File::options().write(true).create_new(true).open(path)
}

fn member(output: &mut File, name: &str, bytes: &[u8]) -> io::Result<()> {
    let name = format!("{name}/");
    let header = format!(
        "{name:<16}{:<12}{:<6}{:<6}{:<8}{:<10}`\n",
        0,
        0,
        0,
        "100644",
        bytes.len()
    );
    if header.len() != 60 {
        return Err(io::Error::other("Debian ar member exceeds format limits"));
    }
    output.write_all(header.as_bytes())?;
    output.write_all(bytes)?;
    if bytes.len() % 2 == 1 {
        output.write_all(b"\n")?;
    }
    Ok(())
}

fn normalise(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || (!metadata.is_dir() && !metadata.is_file()) {
        return Err(io::Error::other(
            "Debian input must contain only regular files and directories",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(file_mode(path, &metadata)))?;
    }
    if metadata.is_dir() {
        for child in fs::read_dir(path)? {
            normalise(&child?.path())?;
        }
    }
    Ok(())
}

fn file_mode(path: &Path, metadata: &fs::Metadata) -> u32 {
    if metadata.is_dir()
        || path.ends_with("usr/bin/tallyline")
        || path.ends_with("usr/lib/tallyline/tallyline")
        || path.ends_with("usr/lib/tallyline/libonnxruntime.so.1.31.0")
    {
        0o755
    } else {
        0o644
    }
}

fn installed_size(path: &Path) -> io::Result<u64> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.is_file() {
        return Ok(metadata.len().div_ceil(1024));
    }
    if !metadata.is_dir() {
        return Err(io::Error::other("unexpected Debian size input"));
    }
    fs::read_dir(path)?.try_fold(1_u64, |sum, child| {
        sum.checked_add(installed_size(&child?.path())?)
            .ok_or_else(|| io::Error::other("Debian installed size exceeds limits"))
    })
}
