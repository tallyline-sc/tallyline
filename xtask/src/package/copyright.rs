// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! DEP-5 metadata derived from the delivered legal catalogue.

use serde_json::Value;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;

pub(super) fn write(payload: &Path, output: &Path) -> io::Result<()> {
    let catalogue = super::manifest::legal_catalogue(payload)?;
    let licence_type = fs::symlink_metadata(payload.join("LICENSE"))?.file_type();
    if licence_type.is_symlink() || !licence_type.is_file() {
        return Err(io::Error::other(
            "DEP-5 project licence must be a regular file",
        ));
    }
    let components = catalogue
        .get("components")
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty())
        .ok_or_else(|| io::Error::other("DEP-5 requires a non-empty component catalogue"))?;
    let mut result = String::from(
        "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/\nUpstream-Name: Tallyline\nSource: https://github.com/tallyline-sc/tallyline\nComment: This local spike is not a Debian archive submission. Component grants\n and original notices remain in force for their respective code and assets.\n Files scopes describe the attached source capsule, with versioned vendor/\n directories, project/ asset paths and toolchain/rust-std-<version>/.\n Unlisted project documentation and source-only assets retain their own SPDX\n headers and project/REUSE.toml annotations; this file grants no new terms.\n The executable includes the components identified in legal/catalogue.json;\n their copyright notices and complete terms are retained below.\n\nFiles: project/crates/*.rs project/xtask/*.rs project/tools/*.rs\n project/Cargo.toml project/Cargo.lock project/rust-toolchain.toml\n project/crates/*/Cargo.toml project/tools/*/Cargo.toml project/xtask/Cargo.toml\n project/.cargo/*\nCopyright: 2026 Lucas Greuloch\n",
    );
    result.push_str("License: GPL-3.0-or-later\n");
    continuation(&mut result, &fs::read_to_string(payload.join("LICENSE"))?);
    let mut scoped = components
        .iter()
        .map(|component| Ok((super::source_map::source_scopes(component)?, component)))
        .collect::<io::Result<Vec<_>>>()?;
    scoped.sort_by_key(|(patterns, _)| patterns.iter().map(String::len).min().unwrap_or_default());
    for (scopes, component) in scoped {
        let name = text(component, "name")?;
        let version = text(component, "version")?;
        let source = text(component, "source")?;
        let applied = text(component, "licence")?;
        let licence = format!(
            "Tallyline-component-{}",
            crate::hashlist::sha256_hex(format!("{name}\n{version}\n{source}").as_bytes())
        );
        let notices = component
            .get("notices")
            .and_then(Value::as_array)
            .filter(|items| !items.is_empty())
            .ok_or_else(|| io::Error::other("DEP-5 component has no preserved notice"))?;
        result.push_str("\nFiles:\n");
        for scope in scopes {
            result.push_str(&format!(" {scope}\n"));
        }
        result.push_str("Copyright:\n");
        continuation(&mut result, &copyright(component)?);
        result.push_str(&format!(
            "Comment: {name} {version}\n Source: {source}\n Applied component terms: {applied}\n"
        ));
        if let Some(declared) = component.get("declared_licence").and_then(Value::as_str) {
            result.push_str(&format!(
                " Declared SPDX: {declared}; applied SPDX: {}\n",
                text(component, "licence")?
            ));
        }
        result.push_str(&format!("License: {licence}\n"));
        for notice in notices {
            continuation(&mut result, text(notice, "text")?);
        }
    }
    let mut file = File::options().write(true).create_new(true).open(output)?;
    file.write_all(result.as_bytes())?;
    file.sync_all()
}

fn text<'a>(value: &'a Value, field: &str) -> io::Result<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty() && (field == "text" || !text.contains(['\n', '\r'])))
        .ok_or_else(|| io::Error::other("DEP-5 component metadata is incomplete"))
}

fn copyright(component: &Value) -> io::Result<String> {
    let value = component
        .get("copyright")
        .ok_or_else(|| io::Error::other("DEP-5 component has no copyright status"))?;
    if let Some(text) = value.as_str() {
        return Ok(text.to_owned());
    }
    if let Some(notices) = value.get("notices").and_then(Value::as_array) {
        let notices = notices
            .iter()
            .map(|notice| {
                notice
                    .as_str()
                    .ok_or_else(|| io::Error::other("invalid copyright notice"))
            })
            .collect::<io::Result<Vec<_>>>()?;
        if !notices.is_empty() {
            return Ok(notices.join("\n"));
        }
    }
    if value.get("status").and_then(Value::as_str) == Some("not_stated_by_upstream") {
        return Ok("Not stated by upstream in the preserved licence/notice files.".into());
    }
    value
        .get("reason")
        .and_then(Value::as_str)
        .map(|reason| format!("Not supplied upstream: {reason}"))
        .ok_or_else(|| io::Error::other("unexplained missing copyright notice"))
}

fn continuation(output: &mut String, value: &str) {
    for line in value.lines() {
        output.push(' ');
        output.push_str(if line.is_empty() { "." } else { line });
        output.push('\n');
    }
}
