// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! The generated offline legal catalogue of the shipped dependency graph.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::Path;

use super::{Assembly, Origin, Roots};
use serde_json::{Value, json};

#[path = "elections.rs"]
mod elections;

/// Generates the offline legal catalogue from the reviewed shipped graph and vendored files.
///
/// # Errors
/// Returns incomplete assembly, missing metadata, unreadable notices or an unwritable target.
pub fn write_catalogue(target: &Path, assembly: &Assembly, roots: Roots<'_>) -> io::Result<()> {
    let output = target.join("catalogue.json");
    if output.exists() {
        fs::remove_file(&output)?;
    }
    if !assembly.findings.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "cannot catalogue an incomplete notices directory",
        ));
    }
    let elections = elections::Elections::read(roots.repository)?;
    let mut components = Vec::new();
    for row in assembly.index.lines().skip(1) {
        let columns: Vec<_> = row.split('\t').collect();
        let [name, version, declared, detected] = columns.as_slice() else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid notices index row",
            ));
        };
        let package_directory = format!("{name}-{version}");
        let manifest: toml::Table =
            fs::read_to_string(roots.vendor.join(&package_directory).join("Cargo.toml"))?
                .parse()
                .map_err(io::Error::other)?;
        let source = manifest
            .get("package")
            .and_then(|package| package.get("repository"))
            .and_then(toml::Value::as_str)
            .map_or_else(
                || format!("https://crates.io/crates/{name}/{version}"),
                str::to_owned,
            );
        let mut notices = Vec::new();
        let mut copyright = BTreeSet::new();
        for file in &assembly.copies {
            if !file
                .target
                .starts_with(&format!("crates/{package_directory}/"))
                && !file.target.starts_with(&format!("{name}/"))
            {
                continue;
            }
            let path = match &file.origin {
                Origin::Vendor(path) => roots.vendor.join(path),
                Origin::Repository(path) => roots.repository.join(path),
                Origin::Toolchain(path) => roots.toolchain.join(path),
            };
            let text = fs::read_to_string(path)?;
            copyright.extend(copyright_lines(&text));
            notices.push(json!({ "path": file.target, "text": text }));
        }
        if notices.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{name} {version}: no bundled licence text"),
            ));
        }
        if copyright.is_empty() {
            for (path, text) in source_comment_notices(&roots.vendor.join(&package_directory))? {
                let found = copyright_lines(&text);
                if !found.is_empty() {
                    copyright.extend(found);
                    notices.push(json!({ "path": format!("published-source/{package_directory}/{path}"), "text": text }));
                }
            }
        }
        let copyright = copyright_metadata(copyright, name, version, roots)?;
        let elected = elections.select(name, version, declared)?;
        components.push(json!({ "name": name, "version": version, "declared_licence": (*declared != "-").then_some(*declared), "licence": elected.unwrap_or(if *declared == "-" { *detected } else { *declared }), "source": source, "copyright": copyright, "notices": notices }));
    }
    components.extend(resource_components(assembly, roots)?);
    let catalogue: Value = json!({ "schema_version": 1, "components": components });
    fs::create_dir_all(target)?;
    fs::write(
        target.join("catalogue.json"),
        serde_json::to_vec_pretty(&catalogue)?,
    )
}

fn source_comment_notices(root: &Path) -> io::Result<Vec<(String, String)>> {
    let mut pending = vec![root.to_owned()];
    let mut headers = Vec::new();
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let path = entry.path();
            if entry.file_type()?.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                let text = fs::read_to_string(&path)?;
                let relative = path
                    .strip_prefix(root)
                    .map_err(io::Error::other)?
                    .to_string_lossy()
                    .replace(std::path::MAIN_SEPARATOR, "/");
                let mut block = String::new();
                let mut first_line = 1;
                for (index, line) in text
                    .split_inclusive('\n')
                    .chain(std::iter::once(""))
                    .enumerate()
                {
                    let trimmed = line.trim();
                    if !line.is_empty()
                        && (trimmed.is_empty()
                            || trimmed.starts_with("//")
                            || trimmed.starts_with("/*")
                            || trimmed.starts_with('*'))
                    {
                        if block.is_empty() {
                            first_line = index + 1;
                        }
                        block.push_str(line);
                    } else {
                        if !copyright_lines(&block).is_empty() {
                            headers.push((
                                format!("{relative}#L{first_line}"),
                                std::mem::take(&mut block),
                            ));
                        }
                        block.clear();
                    }
                }
            }
        }
    }
    headers.sort();
    Ok(headers)
}
fn copyright_metadata(
    notices: BTreeSet<String>,
    name: &str,
    version: &str,
    roots: Roots<'_>,
) -> io::Result<Value> {
    if !notices.is_empty() {
        return Ok(json!({ "status": "provided", "notices": notices }));
    }
    let proof: toml::Table =
        fs::read_to_string(roots.repository.join("supply-chain/copyright-absence.toml"))?
            .parse()
            .map_err(io::Error::other)?;
    let verified = proof
        .get("absence")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .any(|entry| {
            entry.get("crate").and_then(toml::Value::as_str) == Some(name)
                && entry.get("version").and_then(toml::Value::as_str) == Some(version)
                && ["source", "reason"].iter().all(|key| {
                    entry
                        .get(*key)
                        .and_then(toml::Value::as_str)
                        .is_some_and(|text| !text.trim().is_empty())
                })
        });
    if !verified {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{name} {version}: copyright extraction found no holder and no reviewed absence"
            ),
        ));
    }
    Ok(json!({ "status": "not_stated_by_upstream" }))
}
fn copyright_lines(text: &str) -> BTreeSet<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| {
            let normalised = line.trim_start_matches(|character: char| {
                character == '/' || character == '*' || character.is_whitespace()
            });
            let lower = normalised.to_ascii_lowercase();
            (lower.starts_with("spdx-filecopyrighttext:")
                || (lower.starts_with("copyrights in the `")
                    && lower.ends_with("` project are retained by their contributors."))
                || lower.strip_prefix("copyright ").is_some_and(|notice| {
                    let marked = notice.starts_with("(c)") || notice.starts_with('©');
                    let notice = notice
                        .trim_start_matches("(c)")
                        .trim_start_matches('©')
                        .trim_start()
                        .trim_start_matches('[');
                    let named = normalised.split_once(' ').is_some_and(|(_, holder)| {
                        holder.chars().next().is_some_and(char::is_uppercase)
                            && !["holders", "holder", "notice", "license", "owner", "law"]
                                .iter()
                                .any(|word| notice.starts_with(word))
                    });
                    named
                        || (marked && !notice.is_empty())
                        || notice.as_bytes().first().is_some_and(u8::is_ascii_digit)
                }))
                && !lower.contains("[yyyy]")
                && !lower.contains("[name of copyright owner]")
                && !lower.contains("copyright 2004 the apache software foundation")
        })
        .map(str::to_owned)
        .collect()
}
fn origin_path(origin: &Origin, roots: Roots<'_>) -> std::path::PathBuf {
    match origin {
        Origin::Vendor(path) => roots.vendor.join(path),
        Origin::Repository(path) => roots.repository.join(path),
        Origin::Toolchain(path) => roots.toolchain.join(path),
    }
}

fn resource_components(assembly: &Assembly, roots: Roots<'_>) -> io::Result<Vec<Value>> {
    let crate_names: BTreeSet<_> = assembly
        .index
        .lines()
        .skip(1)
        .filter_map(|line| line.split('\t').next())
        .collect();
    let directories: BTreeSet<_> = assembly
        .copies
        .iter()
        .filter_map(|file| file.target.split('/').next())
        .filter(|directory| *directory != "crates" && !crate_names.contains(directory))
        .collect();
    if directories.is_empty() {
        return Ok(Vec::new());
    }
    let reviewed: toml::Table =
        fs::read_to_string(roots.repository.join("supply-chain/notices.toml"))?
            .parse()
            .map_err(io::Error::other)?;
    let entries = reviewed
        .get("component")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "reviewed components are missing",
            )
        })?;
    let mut resources = Vec::new();
    for directory in directories {
        let component = entries
            .iter()
            .find(|entry| entry.get("directory").and_then(toml::Value::as_str) == Some(directory))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("{directory}: missing reviewed component"),
                )
            })?;
        let name = component
            .get("name")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("{directory}: missing name"),
                )
            })?;
        let metadata = component
            .get("catalogue")
            .and_then(toml::Value::as_table)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("{directory}: missing reviewed catalogue metadata"),
                )
            })?;
        let text = |key: &str| {
            metadata
                .get(key)
                .and_then(toml::Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("{directory}: missing reviewed {key}"),
                    )
                })
        };
        let copyright = metadata
            .get("copyright")
            .and_then(toml::Value::as_array)
            .filter(|values| !values.is_empty())
            .and_then(|values| {
                values
                    .iter()
                    .map(|value| value.as_str().filter(|value| !value.trim().is_empty()))
                    .collect::<Option<Vec<_>>>()
            })
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("{directory}: missing reviewed copyright holders"),
                )
            })?;
        let mut notices = Vec::new();
        for file in assembly
            .copies
            .iter()
            .filter(|file| file.target.starts_with(&format!("{directory}/")))
        {
            notices.push(json!({ "path": file.target, "text": fs::read_to_string(origin_path(&file.origin, roots))? }));
        }
        resources.push(json!({ "name": name, "version": text("version")?, "licence": text("licence")?, "source": text("source")?, "copyright": { "status": "provided", "notices": copyright }, "notices": notices }));
    }
    Ok(resources)
}
#[cfg(test)]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
mod tests {
    use crate::notices::{Assembly, Origin, PlannedFile, Roots};
    use crate::scratch::Scratch;

    #[test]
    fn r_ui_18__offline_catalogue_uses_shipped_vendor_metadata_and_exact_licence_text() {
        let workspace = Scratch::new();
        workspace.write("supply-chain/licence-elections.toml", "");
        workspace.write("vendor/example-1.2.3/Cargo.toml", "[package]\nname = \"example\"\nversion = \"1.2.3\"\nlicense = \"MIT\"\nrepository = \"https://example.invalid/source\"\n");
        workspace.write(
            "vendor/example-1.2.3/LICENSE",
            "Copyright (c) 2025 Example Authors\nCopyright (C) Named Holder Without A Year\nCopyrights in the `example` project are retained by their contributors.\nPermission text\r\ncopyright license to reproduce, prepare Derivative Works of,\ncopyright notice that is included in or attached to the work\n",
        );
        let assembly = Assembly {
            copies: vec![PlannedFile {
                origin: Origin::Vendor("example-1.2.3/LICENSE".to_owned()),
                target: "crates/example-1.2.3/LICENSE".to_owned(),
            }],
            index: "crate\tversion\tdeclared\tdetected\nexample\t1.2.3\tMIT\tMIT\n".to_owned(),
            findings: Vec::new(),
        };
        let target = workspace.path().join("legal");
        super::write_catalogue(
            &target,
            &assembly,
            Roots {
                vendor: &workspace.path().join("vendor"),
                toolchain: &workspace.path().join("sysroot"),
                repository: workspace.path(),
            },
        )
        .unwrap();
        let catalogue: serde_json::Value =
            serde_json::from_slice(&std::fs::read(target.join("catalogue.json")).unwrap()).unwrap();
        let entry = catalogue
            .get("components")
            .and_then(serde_json::Value::as_array)
            .and_then(|entries| entries.first())
            .unwrap();
        assert_eq!(
            entry.get("name").and_then(serde_json::Value::as_str),
            Some("example")
        );
        assert_eq!(
            entry.get("version").and_then(serde_json::Value::as_str),
            Some("1.2.3")
        );
        assert_eq!(
            entry.get("licence").and_then(serde_json::Value::as_str),
            Some("MIT")
        );
        assert_eq!(
            entry.get("source").and_then(serde_json::Value::as_str),
            Some("https://example.invalid/source")
        );
        assert_eq!(
            entry.get("copyright"),
            Some(
                &serde_json::json!({ "status": "provided", "notices": ["Copyright (C) Named Holder Without A Year", "Copyright (c) 2025 Example Authors", "Copyrights in the `example` project are retained by their contributors."] })
            )
        );
        assert_eq!(
            entry
                .get("notices")
                .and_then(serde_json::Value::as_array)
                .and_then(|notices| notices.first())
                .and_then(|notice| notice.get("text"))
                .and_then(serde_json::Value::as_str),
            Some(
                "Copyright (c) 2025 Example Authors\nCopyright (C) Named Holder Without A Year\nCopyrights in the `example` project are retained by their contributors.\nPermission text\r\ncopyright license to reproduce, prepare Derivative Works of,\ncopyright notice that is included in or attached to the work\n"
            )
        );
        assert_eq!(
            catalogue
                .get("components")
                .and_then(serde_json::Value::as_array)
                .map(Vec::len),
            Some(1)
        );
    }
    #[test]
    fn r_ui_18__reviewed_resources_need_real_metadata_and_keep_exact_notices() {
        let workspace = Scratch::new();
        workspace.write("supply-chain/licence-elections.toml", "");
        workspace.write(
            "fonts/OFL.txt",
            "Copyright 2020 The Font Authors\nUnmodified font licence\n",
        );
        workspace.write("supply-chain/notices.toml", "[[component]]\ndirectory = \"font\"\nname = \"Approved Font\"\nshipped = \"always\"\nfiles = [{ repository = \"fonts/OFL.txt\" }]\n[component.catalogue]\nversion = \"2.0\"\nlicence = \"OFL-1.1\"\nsource = \"https://example.invalid/font\"\ncopyright = [\"Copyright 2020 The Font Authors\"]\n");
        let assembly = Assembly {
            copies: vec![PlannedFile {
                origin: Origin::Repository("fonts/OFL.txt".to_owned()),
                target: "font/OFL.txt".to_owned(),
            }],
            index: "crate\tversion\tdeclared\tdetected\n".to_owned(),
            findings: Vec::new(),
        };
        let target = workspace.path().join("legal");
        super::write_catalogue(
            &target,
            &assembly,
            Roots {
                vendor: &workspace.path().join("vendor"),
                toolchain: &workspace.path().join("sysroot"),
                repository: workspace.path(),
            },
        )
        .unwrap();
        let catalogue: serde_json::Value =
            serde_json::from_slice(&std::fs::read(target.join("catalogue.json")).unwrap()).unwrap();
        let entry = catalogue
            .get("components")
            .and_then(serde_json::Value::as_array)
            .and_then(|entries| entries.first())
            .unwrap();
        assert_eq!(
            entry.get("name").and_then(serde_json::Value::as_str),
            Some("Approved Font")
        );
        assert_eq!(
            entry.get("version").and_then(serde_json::Value::as_str),
            Some("2.0")
        );
        assert_eq!(
            entry.get("licence").and_then(serde_json::Value::as_str),
            Some("OFL-1.1")
        );
        assert_eq!(
            entry.get("copyright"),
            Some(
                &serde_json::json!({ "status": "provided", "notices": ["Copyright 2020 The Font Authors"] })
            )
        );
        assert_eq!(
            entry
                .get("notices")
                .and_then(serde_json::Value::as_array)
                .and_then(|notices| notices.first())
                .and_then(|notice| notice.get("text"))
                .and_then(serde_json::Value::as_str),
            Some("Copyright 2020 The Font Authors\nUnmodified font licence\n")
        );
    }
    #[test]
    fn r_ui_18__verified_upstream_absence_is_explicit_and_is_not_invented_attribution() {
        let workspace = Scratch::new();
        workspace.write("supply-chain/licence-elections.toml", "");
        workspace.write("vendor/example-1.2.3/Cargo.toml", "[package]\nname = \"example\"\nversion = \"1.2.3\"\nlicense = \"MIT\"\nrepository = \"https://example.invalid/source\"\nauthors = [\"An author is not proven ownership\"]\n");
        workspace.write(
            "vendor/example-1.2.3/LICENSE",
            "Permission text names no copyright holder.\ncopyright license to reproduce, prepare Derivative Works of,\ncopyright notice that is included in or attached to the work\n",
        );
        workspace.write("supply-chain/copyright-absence.toml", "[[absence]]\ncrate = \"example\"\nversion = \"1.2.3\"\nsource = \"https://example.invalid/source\"\nreason = \"The reviewed published sources and licence name no holder.\"\n");
        let assembly = Assembly {
            copies: vec![PlannedFile {
                origin: Origin::Vendor("example-1.2.3/LICENSE".to_owned()),
                target: "crates/example-1.2.3/LICENSE".to_owned(),
            }],
            index: "crate\tversion\tdeclared\tdetected\nexample\t1.2.3\tMIT\tMIT\n".to_owned(),
            findings: Vec::new(),
        };
        let target = workspace.path().join("legal");
        super::write_catalogue(
            &target,
            &assembly,
            Roots {
                vendor: &workspace.path().join("vendor"),
                toolchain: &workspace.path().join("sysroot"),
                repository: workspace.path(),
            },
        )
        .unwrap();
        let catalogue: serde_json::Value =
            serde_json::from_slice(&std::fs::read(target.join("catalogue.json")).unwrap()).unwrap();
        assert_eq!(
            catalogue
                .get("components")
                .and_then(serde_json::Value::as_array)
                .and_then(|entries| entries.first())
                .and_then(|entry| entry.get("copyright")),
            Some(&serde_json::json!({"status":"not_stated_by_upstream"}))
        );
        workspace.write("vendor/example-1.2.3/src/lib.rs", "// Copyright 2024 Actual Source Authors\n// Licensed under the MIT licence.\n\npub fn example() {}\n");
        super::write_catalogue(
            &target,
            &assembly,
            Roots {
                vendor: &workspace.path().join("vendor"),
                toolchain: &workspace.path().join("sysroot"),
                repository: workspace.path(),
            },
        )
        .unwrap();
        let catalogue: serde_json::Value =
            serde_json::from_slice(&std::fs::read(target.join("catalogue.json")).unwrap()).unwrap();
        let entry = catalogue
            .get("components")
            .and_then(serde_json::Value::as_array)
            .and_then(|entries| entries.first())
            .unwrap();
        assert_eq!(
            entry.get("copyright"),
            Some(
                &serde_json::json!({"status":"provided", "notices":["// Copyright 2024 Actual Source Authors"]})
            )
        );
        assert!(entry.get("notices").and_then(serde_json::Value::as_array).unwrap().iter().any(|notice| notice.get("text").and_then(serde_json::Value::as_str) == Some("// Copyright 2024 Actual Source Authors\n// Licensed under the MIT licence.\n\n")));
        workspace.write("vendor/example-1.2.3/Cargo.toml", "invalid manifest");
        assert!(
            super::write_catalogue(
                &target,
                &assembly,
                Roots {
                    vendor: &workspace.path().join("vendor"),
                    toolchain: &workspace.path().join("sysroot"),
                    repository: workspace.path(),
                }
            )
            .is_err()
        );
        assert!(
            !target.join("catalogue.json").exists(),
            "a failed catalogue must not leave stale success output"
        );
    }
    #[test]
    fn r_ui_18__a_reviewed_dual_licence_election_keeps_the_original_declaration_and_text() {
        let workspace = Scratch::new();
        workspace.write("supply-chain/licence-elections.toml", "");
        let original = "Copyright (c) 2025 Example Authors\nFull original MIT and Apache terms remain here.\r\n";
        workspace.write("vendor/example-1.2.3/Cargo.toml", "[package]\nname='example'\nversion='1.2.3'\nlicense='MIT OR Apache-2.0'\nrepository='https://example.invalid/source'\n");
        workspace.write("vendor/example-1.2.3/LICENSE", original);
        let metadata = "[[election]]\ncrate='example'\nversion='1.2.3'\ndeclared='MIT OR Apache-2.0'\nelected='MIT'\nsource='https://example.invalid/review'\nreason='The reviewed compatible option is MIT.'\n";
        workspace.write("supply-chain/licence-elections.toml", metadata);
        let assembly = Assembly {
            copies: vec![PlannedFile {
                origin: Origin::Vendor("example-1.2.3/LICENSE".to_owned()),
                target: "crates/example-1.2.3/LICENSE".to_owned(),
            }],
            index: "crate\tversion\tdeclared\tdetected\nexample\t1.2.3\tMIT OR Apache-2.0\tMIT\n"
                .to_owned(),
            findings: Vec::new(),
        };
        let roots = Roots {
            vendor: &workspace.path().join("vendor"),
            toolchain: &workspace.path().join("sysroot"),
            repository: workspace.path(),
        };
        let target = workspace.path().join("legal");
        super::write_catalogue(&target, &assembly, roots).unwrap();
        let catalogue: serde_json::Value =
            serde_json::from_slice(&std::fs::read(target.join("catalogue.json")).unwrap()).unwrap();
        let entry = catalogue
            .get("components")
            .and_then(serde_json::Value::as_array)
            .and_then(|entries| entries.first())
            .unwrap();
        assert_eq!(
            entry.get("licence").and_then(serde_json::Value::as_str),
            Some("MIT")
        );
        assert_eq!(
            entry
                .get("declared_licence")
                .and_then(serde_json::Value::as_str),
            Some("MIT OR Apache-2.0")
        );
        assert_eq!(
            entry
                .get("notices")
                .and_then(serde_json::Value::as_array)
                .and_then(|notices| notices.first())
                .and_then(|notice| notice.get("text"))
                .and_then(serde_json::Value::as_str),
            Some(original)
        );
        for invalid in [
            metadata.replace("version='1.2.3'", "version='1.2.4'"),
            metadata.replace("elected='MIT'", "elected='BSD-3-Clause'"),
            metadata.replace("declared='MIT OR Apache-2.0'", "declared='MIT'"),
        ] {
            workspace.write("supply-chain/licence-elections.toml", &invalid);
            assert!(
                super::write_catalogue(&target, &assembly, roots).is_err(),
                "{invalid}"
            );
            assert!(!target.join("catalogue.json").exists());
        }
    }
    #[test]
    fn r_ui_18__elections_preserve_mandatory_licences_and_exception_terms() {
        let workspace = Scratch::new();
        workspace.write(
            "vendor/example-1.2.3/LICENSE",
            "Copyright (c) Example Authors\nOriginal terms\n",
        );
        for (declared, elected, valid) in [
            ("(MIT OR Apache-2.0) AND BSD-3-Clause", "MIT", false),
            (
                "(MIT OR Apache-2.0) AND BSD-3-Clause",
                "MIT AND BSD-3-Clause",
                true,
            ),
            ("ISC AND (Apache-2.0 OR ISC)", "ISC", true),
            ("Apache-2.0 WITH LLVM-exception OR MIT", "MIT", true),
            ("Apache-2.0 WITH LLVM-exception OR MIT", "Apache-2.0", false),
            ("MIT OR Apache-2.0", "MIT OR BSD-3-Clause", false),
            ("MIT OR", "MIT", false),
            ("(MIT OR Apache-2.0", "MIT", false),
        ] {
            workspace.write("vendor/example-1.2.3/Cargo.toml", &format!("[package]\nname='example'\nversion='1.2.3'\nlicense='{declared}'\nrepository='https://example.invalid/source'\n"));
            workspace.write("supply-chain/licence-elections.toml", &format!("[[election]]\ncrate='example'\nversion='1.2.3'\ndeclared='{declared}'\nelected='{elected}'\nsource='https://example.invalid/review'\nreason='Reviewed fixture choice.'\n"));
            let assembly = Assembly {
                copies: vec![PlannedFile {
                    origin: Origin::Vendor("example-1.2.3/LICENSE".to_owned()),
                    target: "crates/example-1.2.3/LICENSE".to_owned(),
                }],
                index: format!(
                    "crate\tversion\tdeclared\tdetected\nexample\t1.2.3\t{declared}\tMIT\n"
                ),
                findings: Vec::new(),
            };
            let result = super::write_catalogue(
                &workspace.path().join("legal"),
                &assembly,
                Roots {
                    vendor: &workspace.path().join("vendor"),
                    toolchain: &workspace.path().join("sysroot"),
                    repository: workspace.path(),
                },
            );
            assert_eq!(result.is_ok(), valid, "{declared} -> {elected}: {result:?}");
        }
    }
}
