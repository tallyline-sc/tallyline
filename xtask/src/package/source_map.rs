// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Source-scope evidence bound into the immutable legal payload.

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::Path;

pub(super) fn write(
    payload: &Path,
    source: &Path,
    archive: &Path,
    output: &Path,
) -> io::Result<()> {
    let source = source.canonicalize()?;
    let parent = output
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .canonicalize()?;
    if parent.starts_with(&source)
        || !source.is_dir()
        || !fs::symlink_metadata(archive)?.file_type().is_file()
    {
        return Err(invalid(
            "source evidence needs a regular archive and separate output",
        ));
    }
    let archive_digest = archive_hash(archive)?;
    if super::archive::tar_gzip_digest(&source)? != archive_digest {
        return Err(invalid(
            "source archive differs from the deterministic unpacked source profile",
        ));
    }
    let catalogue = super::manifest::legal_catalogue(payload)?;
    let components = catalogue
        .get("components")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("missing source components"))?;
    let mut mapped = Vec::new();
    for component in components {
        let patterns = source_scopes(component)?;
        let mut files = Vec::new();
        for pattern in &patterns {
            let directory = pattern.strip_suffix("/*").unwrap_or(pattern);
            if !normal(directory) {
                return Err(invalid("unsafe source scope"));
            }
            let mut path = source.clone();
            for component in Path::new(directory).components() {
                path.push(component);
                if fs::symlink_metadata(&path)?.file_type().is_symlink() {
                    return Err(invalid("unpacked source scope contains a symbolic link"));
                }
            }
            collect(&source, &path, &mut files)?;
        }
        files.sort_by(|left, right| {
            left.get("path")
                .and_then(Value::as_str)
                .cmp(&right.get("path").and_then(Value::as_str))
        });
        if files.is_empty() {
            return Err(invalid("source scope does not match any unpacked file"));
        }
        mapped.push(json!({"name":text(component,"name")?,"version":text(component,"version")?,"licence":text(component,"licence")?,"source":text(component,"source")?,"patterns":patterns,"files":files}));
    }
    let filename = archive
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| invalid("source archive name must be UTF-8"))?;
    let evidence = json!({"schema_version":1,"archive":{"file":filename,"sha256":archive_digest},"components":mapped});
    let mut file = File::options().write(true).create_new(true).open(output)?;
    file.write_all(&serde_json::to_vec_pretty(&evidence)?)?;
    file.sync_all()
}

fn collect(root: &Path, path: &Path, files: &mut Vec<Value>) -> io::Result<()> {
    let kind = fs::symlink_metadata(path)?.file_type();
    if kind.is_symlink() {
        return Err(invalid("unpacked source contains a symbolic link"));
    }
    if kind.is_dir() {
        for (child, _) in super::archive::children(path)? {
            collect(root, &child.path(), files)?;
        }
    } else if kind.is_file() {
        let relative = path
            .strip_prefix(root)
            .map_err(io::Error::other)?
            .to_str()
            .ok_or_else(|| invalid("source file path must be UTF-8"))?
            .replace('\\', "/");
        if !normal(&relative) {
            return Err(invalid("unsafe unpacked source path"));
        }
        files.push(json!({"path":relative,"sha256":crate::hashlist::sha256_hex(&fs::read(path)?)}));
    } else {
        return Err(invalid("unpacked source contains a special file"));
    }
    Ok(())
}

fn normal(value: &str) -> bool {
    !value.is_empty()
        && !value.contains(['\\', ':', '\n', '\r', '\0'])
        && value
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

fn text<'a>(value: &'a Value, key: &str) -> io::Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid("invalid source map text"))
}
fn invalid(message: &str) -> io::Error {
    io::Error::other(message)
}

pub(super) fn source_scopes(component: &Value) -> io::Result<Vec<String>> {
    let name = text(component, "name")?;
    let version = text(component, "version")?;
    let profiles: Value = serde_json::from_str(include_str!(
        "../../../supply-chain/package-source-scopes.json"
    ))?;
    for profile in profiles
        .get("components")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("compiled source scopes are malformed"))?
    {
        if ["name", "version", "licence", "source"]
            .iter()
            .all(|key| component.get(*key) == profile.get(*key))
        {
            return profile
                .get("patterns")
                .and_then(Value::as_array)
                .ok_or_else(|| invalid("compiled source patterns are malformed"))?
                .iter()
                .map(|pattern| {
                    pattern
                        .as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| invalid("compiled source pattern is malformed"))
                })
                .collect();
        }
    }
    if component.get("declared_licence").is_none()
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        || !version
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'+' | b'_'))
    {
        return Err(invalid(
            "component needs an explicitly reviewed source scope",
        ));
    }
    Ok(vec![format!("vendor/{name}-{version}/*")])
}

fn archive_hash(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 65536];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(
            buffer
                .get(..count)
                .ok_or_else(|| invalid("archive read exceeds buffer"))?,
        );
    }
    Ok(digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

pub(super) fn validate_metadata(catalogue: &Value, bytes: &[u8]) -> io::Result<Value> {
    let components = catalogue
        .get("components")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("missing source components"))?;
    let map: Value = serde_json::from_slice(bytes)?;
    fields(&map, &["schema_version", "archive", "components"])?;
    if map.get("schema_version").and_then(Value::as_u64) != Some(1) {
        return Err(invalid("unsupported source map schema"));
    }
    let archive = map
        .get("archive")
        .ok_or_else(|| invalid("missing source archive identity"))?;
    fields(archive, &["file", "sha256"])?;
    let filename = text(archive, "file")?;
    if !normal(filename)
        || Path::new(filename).components().count() != 1
        || !hash(text(archive, "sha256")?)
    {
        return Err(invalid("invalid source archive identity"));
    }
    let mapped = map
        .get("components")
        .and_then(Value::as_array)
        .filter(|items| items.len() == components.len())
        .ok_or_else(|| invalid("source map component inventory differs"))?;
    for (component, mapped) in components.iter().zip(mapped) {
        fields(
            mapped,
            &["name", "version", "licence", "source", "patterns", "files"],
        )?;
        for key in ["name", "version", "licence", "source"] {
            if text(component, key)? != text(mapped, key)? {
                return Err(invalid("source map component identity differs"));
            }
        }
        let patterns = source_scopes(component)?;
        if mapped.get("patterns") != Some(&json!(patterns)) {
            return Err(invalid("source map scope differs from the reviewed layout"));
        }
        let files = mapped
            .get("files")
            .and_then(Value::as_array)
            .filter(|items| !items.is_empty())
            .ok_or_else(|| invalid("empty source scope evidence"))?;
        let mut paths = BTreeSet::new();
        for file in files {
            fields(file, &["path", "sha256"])?;
            let path = text(file, "path")?;
            if !normal(path)
                || !paths.insert(path.to_uppercase())
                || !hash(text(file, "sha256")?)
                || !patterns
                    .iter()
                    .any(|pattern| matches_pattern(pattern, path))
            {
                return Err(invalid(
                    "source map contains invalid or out-of-scope file evidence",
                ));
            }
        }
    }
    Ok(map)
}

fn hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
fn fields(value: &Value, expected: &[&str]) -> io::Result<()> {
    if value.as_object().is_some_and(|object| {
        object.len() == expected.len() && expected.iter().all(|key| object.contains_key(*key))
    }) {
        Ok(())
    } else {
        Err(invalid("unknown or missing source map field"))
    }
}

fn matches_pattern(pattern: &str, path: &str) -> bool {
    pattern.strip_suffix("/*").map_or_else(
        || pattern == path,
        |directory| path.starts_with(&format!("{directory}/")),
    )
}

pub(super) fn validate(
    payload: &Path,
    archive: &Path,
) -> io::Result<super::manifest::SourceIdentity> {
    let catalogue = super::manifest::legal_catalogue(payload)?;
    let map = validate_metadata(
        &catalogue,
        &fs::read(payload.join("legal/source-map.json"))?,
    )?;
    if !fs::symlink_metadata(archive)?.file_type().is_file()
        || archive.file_name().and_then(|name| name.to_str())
            != map
                .get("archive")
                .and_then(|value| value.get("file"))
                .and_then(Value::as_str)
        || Some(archive_hash(archive)?.as_str())
            != map
                .get("archive")
                .and_then(|value| value.get("sha256"))
                .and_then(Value::as_str)
    {
        return Err(invalid(
            "source archive identity differs from the source map",
        ));
    }
    let mut expected = BTreeMap::new();
    for component in map
        .get("components")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("missing source components"))?
    {
        for file in component
            .get("files")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("missing source files"))?
        {
            let path = text(file, "path")?.to_owned();
            let hash = text(file, "sha256")?.to_owned();
            if expected
                .insert(path, hash.clone())
                .is_some_and(|previous| previous != hash)
            {
                return Err(invalid("conflicting source file hashes"));
            }
        }
    }
    super::source_archive::verify(archive, &expected)?;
    archive_identity(
        map.get("archive")
            .ok_or_else(|| invalid("source archive identity missing"))?,
    )
}

pub(super) fn archive_identity(value: &Value) -> io::Result<super::manifest::SourceIdentity> {
    fields(value, &["file", "sha256"])?;
    let file = text(value, "file")?;
    let sha256 = text(value, "sha256")?;
    if !super::archive::portable_name(file) || !hash(sha256) {
        return Err(invalid("source archive identity is malformed"));
    }
    Ok(super::manifest::SourceIdentity {
        file: file.into(),
        sha256: sha256.into(),
    })
}

pub(super) fn sealed_identity(payload: &Path) -> io::Result<super::manifest::SourceIdentity> {
    let catalogue = super::manifest::legal_catalogue(payload)?;
    let map = validate_metadata(
        &catalogue,
        &fs::read(payload.join("legal/source-map.json"))?,
    )?;
    archive_identity(
        map.get("archive")
            .ok_or_else(|| invalid("source archive identity missing"))?,
    )
}
