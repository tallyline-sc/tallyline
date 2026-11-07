// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Protected assembly from a verified prebuilt handoff only.

use super::{Target, manifest};
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

pub(super) fn build(stage: &Path, seal: &str, fixture: &Path, output: &Path) -> io::Result<()> {
    let description = super::verify::read(stage, seal)?;
    super::require_source(&description)?;
    let stage = normal_path(&stage.canonicalize()?)?;
    let parent = output
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .canonicalize()?;
    if parent.starts_with(&stage) || output.try_exists()? {
        return Err(io::Error::other(
            "package output must be fresh and outside staging",
        ));
    }
    if !matches!(description.target, Target::Windows if cfg!(windows))
        && !matches!(description.target, Target::Linux if cfg!(target_os = "linux"))
    {
        return Err(io::Error::other(
            "package backend requires its native operating system",
        ));
    }
    fs::create_dir(output)?;
    let output = normal_path(&output.canonicalize()?)?;
    let working = output.join("staging");
    clone_sealed(&stage, &working, &description)?;
    super::verify::read(&working, seal)?;
    super::inject_fixture(&working, seal, fixture)?;
    let result = match description.target {
        Target::Windows => super::msi::build(&working, &description, fixture, &output),
        Target::Linux => super::debian::build(&working, &description, fixture, &output),
    };
    super::verify_injected_stage(&working, seal, fixture)?;
    result?;
    let (format, name) = match description.target {
        Target::Windows => ("zip", "tallyline-windows.zip"),
        Target::Linux => ("tar.gz", "tallyline-linux.tar.gz"),
    };
    super::write_archive(format, &working.join("payload"), &output.join(name))?;
    super::scan_command(
        &working,
        fixture,
        Some(Path::new("payload/tallyline-app-token")),
    )
}

fn clone_sealed(source: &Path, output: &Path, description: &manifest::Verified) -> io::Result<()> {
    fs::create_dir(output)?;
    for entry in &description.files {
        let destination = output.join(&entry.path);
        fs::create_dir_all(
            destination
                .parent()
                .ok_or_else(|| io::Error::other("missing sealed parent"))?,
        )?;
        let mut file = File::options()
            .write(true)
            .create_new(true)
            .open(&destination)?;
        io::copy(&mut File::open(source.join(&entry.path))?, &mut file)?;
        file.sync_all()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&destination, fs::Permissions::from_mode(entry.mode))?;
        }
    }
    let mut manifest_file = File::options()
        .write(true)
        .create_new(true)
        .open(output.join(manifest::MANIFEST))?;
    io::copy(
        &mut File::open(source.join(manifest::MANIFEST))?,
        &mut manifest_file,
    )?;
    manifest_file.sync_all()
}

fn normal_path(path: &Path) -> io::Result<PathBuf> {
    let value = path
        .to_str()
        .ok_or_else(|| io::Error::other("package tool path must be UTF-8"))?;
    Ok(Path::new(value.strip_prefix(r"\\?\").unwrap_or(value)).to_owned())
}
