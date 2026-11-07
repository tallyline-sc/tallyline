// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Strict source-independent verification of sealed packaging inputs.

use super::Target;
use super::manifest::{Entry, MANIFEST, Role, Verified};
use serde_json::Value;
use std::fs;
use std::io;
use std::path::Path;
pub(super) fn read(stage: &Path, seal: &str) -> io::Result<Verified> {
    inventory(stage, seal, false)
}

fn inventory(stage: &Path, seal: &str, injected: bool) -> io::Result<Verified> {
    use std::collections::{BTreeMap, BTreeSet};
    if !fs::symlink_metadata(stage)?.file_type().is_dir()
        || !fs::symlink_metadata(stage.join(MANIFEST))?
            .file_type()
            .is_file()
    {
        return Err(io::Error::other(
            "packaging stage or manifest is not a regular input",
        ));
    }
    let bytes = fs::read(stage.join(MANIFEST))?;
    if !hash_is_valid(seal) || crate::hashlist::sha256_hex(&bytes) != seal {
        return Err(io::Error::other("packaging manifest seal mismatch"));
    }
    let value: Value = serde_json::from_slice(&bytes)?;
    let schema = value.get("schema_version").and_then(Value::as_u64);
    let fields: &[&str] = match schema {
        Some(2) => &[
            "schema_version",
            "target",
            "version",
            "files",
            "source_archive",
        ],
        Some(3) => &[
            "schema_version",
            "target",
            "version",
            "files",
            "source_archive",
            "build_identity",
        ],
        _ => return Err(io::Error::other("unsupported packaging manifest schema")),
    };
    exact_fields(&value, fields)?;
    let build_identity = value
        .get("build_identity")
        .filter(|identity| schema == Some(3) && !identity.is_null())
        .cloned();
    let target = match value.get("target").and_then(Value::as_str) {
        Some("x86_64-pc-windows-msvc") => Target::Windows,
        Some("x86_64-unknown-linux-gnu") => Target::Linux,
        _ => return Err(io::Error::other("unsupported packaging target")),
    };
    let version = text(&value, "version")?.to_owned();
    let mut files = Vec::new();
    let mut names = BTreeSet::new();
    for item in value
        .get("files")
        .and_then(Value::as_array)
        .ok_or_else(|| io::Error::other("invalid sealed file inventory"))?
    {
        exact_fields(item, &["path", "sha256", "mode", "role"])?;
        let path = text(item, "path")?.to_owned();
        if path.is_empty()
            || path.contains(['\\', ':'])
            || Path::new(&path)
                .components()
                .any(|part| !matches!(part, std::path::Component::Normal(_)))
            || !names.insert(path.to_lowercase())
        {
            return Err(io::Error::other("unsafe or duplicate sealed path"));
        }
        let role = match text(item, "role")? {
            "payload" if path.starts_with("payload/") => Role::Payload,
            "tool" if path.starts_with("tools/") => Role::Tool,
            "helper" if path == target.helper() => Role::Helper,
            _ => return Err(io::Error::other("invalid sealed file role")),
        };
        let mode = item["mode"]
            .as_u64()
            .and_then(|mode| u32::try_from(mode).ok())
            .filter(|mode| matches!(mode, 0o644 | 0o755))
            .ok_or_else(|| io::Error::other("invalid sealed file mode"))?;
        let sha256 = text(item, "sha256")?.to_owned();
        if !hash_is_valid(&sha256) {
            return Err(io::Error::other("invalid sealed file hash"));
        }
        files.push(Entry {
            path,
            sha256,
            mode,
            role,
        });
    }
    let source_value = value
        .get("source_archive")
        .ok_or_else(|| io::Error::other("source archive reference missing"))?;
    let source_archive = if source_value.is_null() {
        None
    } else {
        Some(super::source_map::archive_identity(source_value)?)
    };
    let description = Verified {
        target,
        version,
        files,
        source_archive,
        build_identity,
    };
    super::release_version(&format!("v{}", description.version))?;
    super::tools::verify(description.target.name(), &stage.join("tools"))?;
    let mut expected = BTreeMap::new();
    let mut directories = BTreeSet::new();
    for file in &description.files {
        validate_role(file, description.target)?;
        let path = Path::new(&file.path);
        expected.insert(path.to_owned(), file.sha256.clone());
        for ancestor in path.ancestors().skip(1) {
            directories.insert(ancestor.to_owned());
        }
    }
    for required in [
        format!("payload/{}", description.target.executable()),
        format!("payload/{}", description.target.runtime()),
        "payload/LICENSE".into(),
        "payload/NOTICE".into(),
        "payload/README.md".into(),
        "payload/legal/catalogue.json".into(),
        format!("tools/{}", description.target.tool()),
        description.target.helper().into(),
    ] {
        if !expected.contains_key(Path::new(&required)) {
            return Err(io::Error::other(
                "sealed packaging input is missing a required file",
            ));
        }
    }
    if injected {
        expected.insert("payload/tallyline-app-token".into(), String::new());
    }
    expected.insert(MANIFEST.into(), seal.into());
    check_tree(stage, stage, &expected, &directories)?;
    for file in &description.files {
        let path = stage.join(&file.path);
        if crate::hashlist::sha256_hex(&fs::read(&path)?) != file.sha256 {
            return Err(io::Error::other("sealed packaging file has changed"));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if fs::metadata(&path)?.permissions().mode() & 0o777 != file.mode {
                return Err(io::Error::other("sealed packaging permission has changed"));
            }
        }
    }
    super::payload::verify(&stage.join("payload"), description.target)?;
    if let Some(identity) = &description.source_archive {
        if identity.file != super::manifest::SOURCE_ARCHIVE
            || *identity != super::source_map::sealed_identity(&stage.join("payload"))?
        {
            return Err(io::Error::other(
                "sealed source archive reference differs from its legal source map",
            ));
        }
    } else if stage.join("payload/legal/source-map.json").try_exists()? {
        return Err(io::Error::other(
            "sealed source map has no verified source archive reference",
        ));
    }
    if let Some(identity) = &description.build_identity {
        exact_fields(identity, &["receipt_sha256", "receipt"])?;
        let seal = text(identity, "receipt_sha256")?;
        let receipt = &identity["receipt"];
        if !hash_is_valid(seal)
            || crate::hashlist::sha256_hex(&serde_json::to_vec_pretty(receipt)?) != seal
        {
            return Err(io::Error::other(
                "sealed build identity receipt differs from admission",
            ));
        }
        super::build_identity::validate(
            receipt,
            &stage.join("payload").join(description.target.executable()),
            description.target,
            &format!("v{}", description.version),
        )?;
    }
    let helper = description
        .files
        .iter()
        .find(|file| file.role == Role::Helper)
        .ok_or_else(|| io::Error::other("sealed helper is missing"))?;
    if crate::hashlist::sha256_hex(&fs::read(std::env::current_exe()?)?) != helper.sha256 {
        return Err(io::Error::other(
            "running packaging helper does not match the seal",
        ));
    }
    Ok(description)
}

fn validate_role(file: &Entry, target: Target) -> io::Result<()> {
    if file.role == Role::Payload {
        if super::payload::forbidden_runtime(Path::new(&file.path)) {
            return Err(io::Error::other(
                "sealed application contains a prohibited Microsoft runtime file",
            ));
        }
        let relative = file
            .path
            .strip_prefix("payload/")
            .ok_or_else(|| io::Error::other("invalid payload path"))?;
        let root = relative.split('/').next().unwrap_or_default();
        if ![
            target.executable(),
            target.runtime(),
            "assets",
            "legal",
            "LICENSE",
            "NOTICE",
            "README.md",
        ]
        .contains(&root)
        {
            return Err(io::Error::other("unapproved payload root"));
        }
    }
    if file.role == Role::Tool
        && Path::new(&file.path).components().any(|part| {
            let name = part.as_os_str().to_string_lossy().to_lowercase();
            [
                "sdk",
                "src",
                ".git",
                ".nuget",
                "cargo",
                "cargo.exe",
                "rustc",
                "rustc.exe",
                "cl.exe",
                "msbuild.exe",
                "nuget.exe",
                "mergemod.dll",
            ]
            .contains(&name.as_str())
        })
    {
        return Err(io::Error::other(
            "unapproved compiler, source or proprietary tool component",
        ));
    }
    if (file.role == Role::Helper
        || file.path == format!("payload/{}", target.executable())
        || file.path == format!("tools/{}", target.tool()))
        && file.mode != 0o755
    {
        return Err(io::Error::other(
            "sealed executable is missing execute permission",
        ));
    }
    Ok(())
}

fn check_tree(
    root: &Path,
    path: &Path,
    expected: &std::collections::BTreeMap<std::path::PathBuf, String>,
    directories: &std::collections::BTreeSet<std::path::PathBuf>,
) -> io::Result<()> {
    let relative = path.strip_prefix(root).map_err(io::Error::other)?;
    let kind = fs::symlink_metadata(path)?.file_type();
    if kind.is_symlink() {
        return Err(io::Error::other(
            "sealed packaging tree contains a symbolic link",
        ));
    }
    if kind.is_dir() && (relative.as_os_str().is_empty() || directories.contains(relative)) {
        for child in fs::read_dir(path)? {
            check_tree(root, &child?.path(), expected, directories)?;
        }
    } else if !kind.is_file() || !expected.contains_key(relative) {
        return Err(io::Error::other(
            "sealed packaging tree contains an unlisted input",
        ));
    }
    Ok(())
}

fn exact_fields(value: &Value, fields: &[&str]) -> io::Result<()> {
    if !value.as_object().is_some_and(|object| {
        object.len() == fields.len() && fields.iter().all(|field| object.contains_key(*field))
    }) {
        return Err(io::Error::other(
            "unknown or missing packaging manifest field",
        ));
    }
    Ok(())
}

fn text<'a>(value: &'a Value, field: &str) -> io::Result<&'a str> {
    value[field]
        .as_str()
        .ok_or_else(|| io::Error::other("invalid packaging manifest field type"))
}

fn hash_is_valid(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(super) fn injected(stage: &Path, seal: &str, fixture: &Path) -> io::Result<Verified> {
    let description = inventory(stage, seal, true)?;
    super::require_source(&description)?;
    let path = stage.join("payload/tallyline-app-token");
    let metadata = fs::symlink_metadata(&path)?;
    if !metadata.file_type().is_file() || metadata.len() == 0 || metadata.len() > 1024 {
        return Err(io::Error::other(
            "injected data file is not a bounded regular token file",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o777 != 0o644 {
            return Err(io::Error::other(
                "injected data file has incorrect permissions",
            ));
        }
    }
    let token = super::read_fixture(fixture)?;
    if fs::read(path)? != token {
        return Err(io::Error::other(
            "injected data file differs from the fixture",
        ));
    }
    super::scan_command(
        stage,
        fixture,
        Some(Path::new("payload/tallyline-app-token")),
    )?;
    Ok(description)
}
