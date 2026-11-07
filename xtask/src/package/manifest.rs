// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Secret-free staging and seals for immutable packaging inputs.

use super::Target;
use serde_json::{Value, json};
use std::fs;
use std::io;
use std::path::Path;

pub(super) const MANIFEST: &str = "package-manifest.json";
pub(super) const SOURCE_ARCHIVE: &str = "source-capsule.tar.gz";

pub(super) fn prepare(
    input: &Path,
    tools: &Path,
    stage: &Path,
    target: Target,
    version: &str,
    build_identity: Option<Value>,
) -> io::Result<String> {
    super::release_version(&format!("v{version}"))?;
    let input = input.canonicalize()?;
    let tools = tools.canonicalize()?;
    let parent = stage
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .canonicalize()?;
    if parent.starts_with(&input) || parent.starts_with(&tools) || stage.try_exists()? {
        return Err(io::Error::other(
            "package staging must be a fresh directory outside its inputs",
        ));
    }
    if input.join("tallyline-app-token").try_exists()? {
        return Err(io::Error::other(
            "secret-free preparation refuses an existing token data file",
        ));
    }
    super::payload::verify(&input, target)?;
    if !tools.join(target.tool()).is_file()
        || (target == Target::Windows && !tools.join("runtime").is_dir())
    {
        return Err(io::Error::other(
            "preinstalled packaging tool or private runtime is missing",
        ));
    }
    let source_archive = if input.join("legal/source-map.json").try_exists()? {
        let identity = super::source_map::validate(&input, &input.join(SOURCE_ARCHIVE))?;
        if identity.file != SOURCE_ARCHIVE {
            return Err(io::Error::other(
                "source archive filename differs from the fixed handoff",
            ));
        }
        Some(identity)
    } else {
        if input.join(SOURCE_ARCHIVE).try_exists()? {
            return Err(io::Error::other(
                "source archive requires its complete source map",
            ));
        }
        None
    };
    fs::create_dir(stage)?;
    let mut files = Vec::new();
    for name in [
        target.executable(),
        target.runtime(),
        "assets",
        "legal",
        "LICENSE",
        "NOTICE",
        "README.md",
    ] {
        copy(
            &input.join(name),
            stage,
            &format!("payload/{name}"),
            "payload",
            target,
            &mut files,
        )?;
    }
    copy(&tools, stage, "tools", "tool", target, &mut files)?;
    copy(
        &std::env::current_exe()?,
        stage,
        target.helper(),
        "helper",
        target,
        &mut files,
    )?;
    files.sort_by(|left, right| left["path"].as_str().cmp(&right["path"].as_str()));
    if let Some(identity) = &build_identity {
        super::build_identity::validate(
            &identity["receipt"],
            &stage.join("payload").join(target.executable()),
            target,
            &format!("v{version}"),
        )?;
    }
    let manifest = json!({"schema_version":3,"target":target.triple(),"version":version,"files":files,"source_archive":source_archive.map(|source| json!({"file":source.file,"sha256":source.sha256})),"build_identity":build_identity});
    let bytes = serde_json::to_vec_pretty(&manifest)?;
    fs::write(stage.join(MANIFEST), &bytes)?;
    Ok(crate::hashlist::sha256_hex(&bytes))
}

pub(super) fn legal_catalogue(input: &Path) -> io::Result<Value> {
    let legal = fs::symlink_metadata(input.join("legal"))?.file_type();
    if legal.is_symlink() || !legal.is_dir() {
        return Err(io::Error::other(
            "package legal directory must not be linked",
        ));
    }
    let catalogue_type = fs::symlink_metadata(input.join("legal/catalogue.json"))?.file_type();
    if catalogue_type.is_symlink() || !catalogue_type.is_file() {
        return Err(io::Error::other(
            "package legal catalogue must be a regular file",
        ));
    }
    if fs::metadata(input.join("legal/catalogue.json"))?.len() > super::legal::MAX_NOTICE_BYTES {
        return Err(io::Error::other("legal catalogue exceeds its size bound"));
    }
    let original = fs::read(input.join("legal/catalogue.json"))?;
    let catalogue: Value = serde_json::from_slice(&original)?;
    let components = catalogue
        .get("components")
        .and_then(Value::as_array)
        .filter(|components| !components.is_empty())
        .ok_or_else(|| io::Error::other("package legal catalogue is empty or malformed"))?;
    if catalogue.get("schema_version").and_then(Value::as_u64) != Some(1) {
        return Err(io::Error::other(
            "unsupported package legal catalogue schema",
        ));
    }
    for component in components {
        if ["name", "version", "licence", "source"]
            .iter()
            .any(|field| {
                !component[field]
                    .as_str()
                    .is_some_and(|value| !value.trim().is_empty())
            })
            || component["copyright"].is_null()
            || !component["notices"].as_array().is_some_and(|notices| {
                !notices.is_empty()
                    && notices.iter().all(|notice| {
                        notice["text"]
                            .as_str()
                            .is_some_and(|text| !text.trim().is_empty())
                    })
            })
        {
            return Err(io::Error::other(
                "package legal catalogue component is incomplete",
            ));
        }
    }
    super::legal::verify(&input.join("legal"), &catalogue, &original)?;
    Ok(catalogue)
}

fn copy(
    source: &Path,
    stage: &Path,
    relative: &str,
    role: &str,
    target: Target,
    files: &mut Vec<Value>,
) -> io::Result<()> {
    let kind = fs::symlink_metadata(source)?.file_type();
    if kind.is_symlink() {
        return Err(io::Error::other(
            "package preparation rejects symbolic links",
        ));
    }
    let output = stage.join(relative);
    if kind.is_dir() {
        for (child, filename) in super::archive::children(source)? {
            copy(
                &child.path(),
                stage,
                &format!("{relative}/{filename}"),
                role,
                target,
                files,
            )?;
        }
    } else if kind.is_file() {
        if role == "payload" && super::payload::forbidden_runtime(source) {
            return Err(io::Error::other(
                "application payload must not bundle Microsoft runtime files",
            ));
        }
        let bytes = fs::read(source)?;
        let mode = if role == "helper"
            || relative == format!("payload/{}", target.executable())
            || relative == format!("tools/{}", target.tool())
            || source
                .extension()
                .is_some_and(|extension| extension == "exe")
        {
            0o755
        } else {
            0o644
        };
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&output, &bytes)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&output, fs::Permissions::from_mode(mode))?;
        }
        files.push(json!({"path":relative,"sha256":crate::hashlist::sha256_hex(&bytes),"mode":mode,"role":role}));
    } else {
        return Err(io::Error::other("package input is not a regular file"));
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Role {
    Payload,
    Tool,
    Helper,
}

pub(super) struct Entry {
    pub path: String,
    pub sha256: String,
    pub mode: u32,
    pub role: Role,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SourceIdentity {
    pub file: String,
    pub sha256: String,
}

pub(super) struct Verified {
    pub target: Target,
    pub version: String,
    pub files: Vec<Entry>,
    pub source_archive: Option<SourceIdentity>,
    pub build_identity: Option<Value>,
}
