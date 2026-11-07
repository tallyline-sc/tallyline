// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Fixed Debian packaging through a sealed preinstalled tool.

use super::manifest::{Role, Verified};
use serde_json::json;
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub(super) fn build(
    stage: &Path,
    description: &Verified,
    fixture: &Path,
    output: &Path,
) -> io::Result<()> {
    let work = output.join("inspection");
    fs::create_dir(&work)?;
    let copyright = work.join("copyright");
    super::write_debian_copyright(&stage.join("payload"), &copyright)?;
    let desktop = work.join("tallyline.desktop");
    fs::write(&desktop, super::debian_desktop_entry())?;
    let packager = work.join("packager");
    fs::create_dir(&packager)?;
    let resources = [
        "assets",
        "libonnxruntime.so.1.31.0",
        "legal",
        "LICENSE",
        "NOTICE",
        "README.md",
        "tallyline-app-token",
    ]
    .into_iter()
    .map(|name| json!({"src":stage.join("payload").join(name),"target":name}))
    .collect::<Vec<_>>();
    let configuration = json!({
        "name":"tallyline", "productName":"Tallyline", "version":description.version,
        "identifier":"org.tallyline.client", "formats":["deb"], "outDir":packager,
        "binariesDir":stage.join("payload"), "binaries":[{"path":"tallyline","main":true}],
        "targetTriple":"x86_64-unknown-linux-gnu", "description":"Star Citizen commodity terminal data client",
        "homepage":"https://github.com/tallyline-sc/tallyline", "authors":["Tallyline contributors <lucas.greuloch@gmail.com>"],
        "licenseFile":stage.join("payload/LICENSE"), "resources":resources,
        "beforePackagingCommand":null,"beforeEachPackageCommand":null,
        "linux":{"generateDesktopEntry":false},
        "deb":{"packageName":"tallyline", "section":"utils", "priority":"optional",
            "depends":["libc6 (>= 2.36)","libgcc-s1","libstdc++6","libx11-6","libxcursor1","libxrandr2","libxi6","libxkbcommon0","libwayland-client0","libegl1","libgl1","libvulkan1"],
            "files":{copyright.to_string_lossy().to_string():"usr/share/doc/tallyline/copyright",desktop.to_string_lossy().to_string():"usr/share/applications/tallyline.desktop",stage.join("payload/assets/branding/tallyline-icon-256.png").to_string_lossy().to_string():"usr/share/icons/hicolor/256x256/apps/tallyline.png"}}
    });
    let config = work.join("packager.json");
    fs::write(&config, serde_json::to_vec_pretty(&configuration)?)?;
    invoke(stage, &work, &config, fixture)?;
    let tree = packager
        .join(".cargo-packager/deb")
        .join(format!("tallyline_{}_amd64", description.version));
    verify_payload(stage, description, &tree.join("data"), fixture)?;
    super::repack_debian(&tree, &output.join("tallyline-linux.deb"))?;
    super::scan_command(
        &tree.join("data"),
        fixture,
        Some(Path::new("usr/lib/tallyline/tallyline-app-token")),
    )?;
    super::scan_command(&tree.join("control"), fixture, None)?;
    Ok(())
}

fn invoke(stage: &Path, work: &Path, config: &Path, fixture: &Path) -> io::Result<()> {
    for name in ["home", "cache", "config", "temporary", "logs"] {
        fs::create_dir(work.join(name))?;
    }
    let result = std::process::Command::new(stage.join("tools/cargo-packager"))
        .arg("--config")
        .arg(config)
        .current_dir(work)
        .env_clear()
        .env("PATH", stage.join("tools"))
        .env("HOME", work.join("home"))
        .env("XDG_CACHE_HOME", work.join("cache"))
        .env("XDG_CONFIG_HOME", work.join("config"))
        .env("TMPDIR", work.join("temporary"))
        .env("LANG", "C.UTF-8")
        .env("LC_ALL", "C.UTF-8")
        .output()?;
    super::capture::write(
        &work.join("logs"),
        "packager",
        &result.stdout,
        &result.stderr,
        fixture,
    )?;
    super::scan_command(&work.join("logs"), fixture, None)?;
    if !result.status.success() {
        return Err(io::Error::other(
            "sealed Debian tool failed; inspect captured output",
        ));
    }
    Ok(())
}

fn verify_payload(
    stage: &Path,
    description: &Verified,
    data: &Path,
    fixture: &Path,
) -> io::Result<()> {
    let mut expected = BTreeSet::new();
    for file in description
        .files
        .iter()
        .filter(|file| file.role == Role::Payload)
    {
        let relative = file
            .path
            .strip_prefix("payload/")
            .ok_or_else(|| io::Error::other("invalid Debian payload path"))?;
        let path = if relative == "tallyline" {
            PathBuf::from("usr/bin/tallyline")
        } else {
            Path::new("usr/lib/tallyline").join(relative)
        };
        if fs::read(data.join(&path))? != fs::read(stage.join(&file.path))? {
            return Err(io::Error::other(
                "Debian tool changed a sealed payload file",
            ));
        }
        expected.insert(path);
    }
    expected.extend([
        PathBuf::from("usr/lib/tallyline/tallyline-app-token"),
        PathBuf::from("usr/share/doc/tallyline/copyright"),
        PathBuf::from("usr/share/applications/tallyline.desktop"),
        PathBuf::from("usr/share/icons/hicolor/256x256/apps/tallyline.png"),
    ]);
    if fs::read(data.join("usr/share/icons/hicolor/256x256/apps/tallyline.png"))?
        != fs::read(stage.join("payload/assets/branding/tallyline-icon-256.png"))?
    {
        return Err(io::Error::other(
            "Debian desktop icon differs from the sealed artwork",
        ));
    }
    let mut actual = BTreeSet::new();
    collect(data, data, &mut actual)?;
    if actual != expected {
        return Err(io::Error::other(
            "Debian payload contains missing or unapproved files",
        ));
    }
    super::scan_command(
        data,
        fixture,
        Some(Path::new("usr/lib/tallyline/tallyline-app-token")),
    )
}

fn collect(root: &Path, path: &Path, files: &mut BTreeSet<PathBuf>) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.is_dir() {
        for child in fs::read_dir(path)? {
            collect(root, &child?.path(), files)?;
        }
    } else if metadata.is_file() {
        files.insert(
            path.strip_prefix(root)
                .map_err(io::Error::other)?
                .to_owned(),
        );
    } else {
        return Err(io::Error::other(
            "Debian payload contains a link or special file",
        ));
    }
    Ok(())
}
