// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Fixed MSI authoring and verified extraction through the sealed WiX tool.

use super::desktop::escape;
use super::manifest::{Role, Verified};
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::Path;

pub(super) fn build(
    stage: &Path,
    description: &Verified,
    fixture: &Path,
    output: &Path,
) -> io::Result<()> {
    let work = output.join("inspection");
    fs::create_dir(&work)?;
    let files = payload_paths(description)?;
    let authoring = msi_package_authoring(&stage.join("payload"), &description.version, &files)?;
    let source = work.join("package.wxs");
    fs::write(&source, &authoring)?;
    if authoring.contains("$(")
        || authoring.contains("!(")
        || fs::read_to_string(&source)? != authoring
        || [
            "<Module",
            "<Merge",
            "<MergeRef",
            "<Bundle",
            "<CustomAction",
            "<UI",
            "<Extension",
        ]
        .iter()
        .any(|tag| authoring.contains(tag))
    {
        return Err(io::Error::other(
            "MSI authoring differs from the fixed Package profile",
        ));
    }
    let package = output.join("tallyline-windows.msi");
    command(
        stage,
        &work,
        fixture,
        "build",
        &[
            "build".into(),
            source.into_os_string(),
            "-arch".into(),
            "x64".into(),
            "-pdbtype".into(),
            "none".into(),
            "-o".into(),
            package.as_os_str().to_owned(),
        ],
    )?;
    inspect(stage, description, fixture, &package, &work)
}

pub(super) fn inspect(
    stage: &Path,
    description: &Verified,
    fixture: &Path,
    package: &Path,
    work: &Path,
) -> io::Result<()> {
    let files = payload_paths(description)?;
    command(
        stage,
        work,
        fixture,
        "validate",
        &[
            "msi".into(),
            "validate".into(),
            package.as_os_str().to_owned(),
        ],
    )?;
    let extracted = work.join("extracted");
    command(
        stage,
        work,
        fixture,
        "decompile",
        &[
            "msi".into(),
            "decompile".into(),
            package.as_os_str().to_owned(),
            "-x".into(),
            extracted.clone().into_os_string(),
            "-o".into(),
            work.join("decompiled.wxs").into_os_string(),
        ],
    )?;
    let expected = files
        .iter()
        .map(|file| Path::new("File").join(id("F", file)))
        .collect::<BTreeSet<_>>();
    let mut actual = BTreeSet::new();
    extracted_files(&extracted, &extracted, &mut actual)?;
    if actual != expected {
        return Err(io::Error::other(
            "MSI contains missing or unapproved embedded files",
        ));
    }
    for file in &files {
        let unpacked = extracted.join("File").join(id("F", file));
        let expected = if file == "tallyline-app-token" {
            fixture.to_owned()
        } else {
            stage.join("payload").join(file)
        };
        if fs::read(unpacked)? != fs::read(expected)? {
            return Err(io::Error::other(
                "MSI extracted payload differs from the sealed input",
            ));
        }
    }
    super::scan_command(
        &extracted,
        fixture,
        Some(&Path::new("File").join(id("F", "tallyline-app-token"))),
    )?;
    for name in [
        "build.stdout",
        "build.stderr",
        "validate.stdout",
        "validate.stderr",
        "decompile.stdout",
        "decompile.stderr",
        "package.wxs",
        "decompiled.wxs",
    ] {
        let isolated = work.join("scan");
        if !isolated.exists() {
            fs::create_dir(&isolated)?;
        }
        if work.join(name).is_file() {
            fs::copy(work.join(name), isolated.join(name))?;
        }
        super::scan_command(&isolated, fixture, None)?;
    }
    Ok(())
}

fn payload_paths(description: &Verified) -> io::Result<Vec<String>> {
    let mut files = description
        .files
        .iter()
        .filter(|file| file.role == Role::Payload)
        .map(|file| {
            file.path
                .strip_prefix("payload/")
                .map(str::to_owned)
                .ok_or_else(|| io::Error::other("invalid sealed MSI payload path"))
        })
        .collect::<io::Result<Vec<_>>>()?;
    files.push("tallyline-app-token".into());
    files.sort();
    Ok(files)
}

fn directory(
    payload: &Path,
    parent: &str,
    files: &[String],
    authoring: &mut String,
    components: &mut Vec<String>,
) -> io::Result<()> {
    let mut children = BTreeSet::new();
    for path in files {
        let Some(relative) = path.strip_prefix(parent) else {
            continue;
        };
        if let Some((child, _)) = relative.split_once('/') {
            children.insert(child);
            continue;
        }
        let component = id("C", path);
        let file = id("F", path);
        let source = payload.join(path);
        let source = source
            .to_str()
            .ok_or_else(|| io::Error::other("MSI source path is not UTF-8"))?;
        authoring.push_str(&format!("<Component Id=\"{component}\" Guid=\"*\" Bitness=\"always64\"><File Id=\"{file}\" Name=\"{}\" Source=\"{}\" KeyPath=\"yes\">", escape(relative), escape(source)));
        if path == "tallyline.exe" {
            authoring.push_str(super::desktop::msi_shortcut_authoring());
        }
        authoring.push_str("</File></Component>");
        components.push(component);
    }
    for child in children {
        let relative = format!("{parent}{child}/");
        authoring.push_str(&format!(
            "<Directory Id=\"{}\" Name=\"{}\">",
            id("D", &relative),
            escape(child)
        ));
        directory(payload, &relative, files, authoring, components)?;
        authoring.push_str("</Directory>");
    }
    Ok(())
}

fn id(prefix: &str, path: &str) -> String {
    format!(
        "{prefix}{}",
        super::super::hashlist::sha256_hex(path.as_bytes())
    )
}

fn command(
    stage: &Path,
    work: &Path,
    fixture: &Path,
    name: &str,
    arguments: &[std::ffi::OsString],
) -> io::Result<()> {
    invoke(&stage.join("tools"), work, Some(fixture), name, arguments)?;
    let logs = work.join("scanned-logs");
    fs::create_dir_all(&logs)?;
    for extension in ["stdout", "stderr"] {
        fs::copy(
            work.join(format!("{name}.{extension}")),
            logs.join(format!("{name}.{extension}")),
        )?;
    }
    super::scan_command(&logs, fixture, None)?;

    Ok(())
}

fn extracted_files(
    root: &Path,
    path: &Path,
    files: &mut BTreeSet<std::path::PathBuf>,
) -> io::Result<()> {
    let kind = fs::symlink_metadata(path)?.file_type();
    if kind.is_dir() {
        for child in fs::read_dir(path)? {
            extracted_files(root, &child?.path(), files)?;
        }
    } else if kind.is_file() {
        files.insert(
            path.strip_prefix(root)
                .map_err(io::Error::other)?
                .to_owned(),
        );
    } else {
        return Err(io::Error::other(
            "MSI extraction contains a link or special file",
        ));
    }
    Ok(())
}

fn invoke(
    tools: &Path,
    work: &Path,
    fixture: Option<&Path>,
    name: &str,
    arguments: &[std::ffi::OsString],
) -> io::Result<()> {
    super::tools::verify("windows", tools)?;
    let windows = Path::new("C:\\Windows");
    let profile = work.join("profile");
    let temporary = work.join("temporary");
    fs::create_dir_all(&profile)?;
    fs::create_dir_all(&temporary)?;
    let result = std::process::Command::new(tools.join("wix.exe"))
        .args(arguments)
        .current_dir(work)
        .env_clear()
        .env("SystemRoot", windows)
        .env("WINDIR", windows)
        .env("PATH", windows.join("System32"))
        .env("USERPROFILE", &profile)
        .env("APPDATA", &profile)
        .env("LOCALAPPDATA", &profile)
        .env("TEMP", &temporary)
        .env("TMP", &temporary)
        .env("DOTNET_ROOT", tools.join("runtime"))
        .env("DOTNET_ROOT_X64", tools.join("runtime"))
        .env("DOTNET_MULTILEVEL_LOOKUP", "0")
        .env("DOTNET_ROLL_FORWARD", "Major")
        .output()?;
    if let Some(fixture) = fixture {
        super::capture::write(work, name, &result.stdout, &result.stderr, fixture)?;
    } else {
        if result.stdout.len() > 8 * 1024 * 1024 || result.stderr.len() > 8 * 1024 * 1024 {
            return Err(io::Error::other("excessive secret-free MSI probe output"));
        }
        for (extension, bytes) in [("stdout", &result.stdout), ("stderr", &result.stderr)] {
            use std::io::Write;
            let mut file = fs::File::options()
                .create_new(true)
                .write(true)
                .open(work.join(format!("{name}.{extension}")))?;
            file.write_all(bytes)?;
            file.sync_all()?;
        }
    }
    if !result.status.success() {
        return Err(io::Error::other(format!(
            "sealed MSI tool failed during {name}; inspect captured output"
        )));
    }
    Ok(())
}

pub(super) fn probe(tools: &Path, output: &Path) -> io::Result<()> {
    if !cfg!(windows) {
        return Err(io::Error::other("MSI tool probe requires Windows"));
    }
    let tools = tools.canonicalize()?;
    super::tools::verify("windows", &tools)?;
    if output.try_exists()?
        || output
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .canonicalize()?
            .starts_with(&tools)
    {
        return Err(io::Error::other(
            "MSI probe output must be fresh and outside tools",
        ));
    }
    fs::create_dir(output)?;
    let canonical = output.canonicalize()?;
    let canonical = canonical
        .to_str()
        .ok_or_else(|| io::Error::other("MSI probe path must be UTF-8"))?;
    let output = Path::new(canonical.strip_prefix(r"\\?\").unwrap_or(canonical));
    let source = output.join("probe.wxs");
    let marker = output.join("probe.txt");
    fs::write(&marker, b"Secret-free prerequisite table probe.\n")?;
    let marker = marker
        .to_str()
        .ok_or_else(|| io::Error::other("MSI probe path must be UTF-8"))?;
    let marker = marker.strip_prefix(r"\\?\").unwrap_or(marker);
    let authoring = format!(
        "<Wix xmlns=\"http://wixtoolset.org/schemas/v4/wxs\"><Package Name=\"Tallyline prerequisite probe\" Manufacturer=\"Tallyline Project\" Version=\"1.0.0\" Language=\"1033\" Scope=\"perMachine\" UpgradeCode=\"6A4B6AF5-C25E-4804-9E88-3464C3A1816F\"><MediaTemplate EmbedCab=\"yes\"/>{}<StandardDirectory Id=\"ProgramFiles64Folder\"><Directory Id=\"INSTALLFOLDER\" Name=\"Tallyline prerequisite probe\"><Component Id=\"Probe\" Guid=\"*\" Bitness=\"always64\"><File Id=\"ProbeData\" Name=\"probe.txt\" Source=\"{}\" KeyPath=\"yes\"/></Component></Directory></StandardDirectory><Feature Id=\"ProbeFeature\" Title=\"Probe\" Level=\"1\"><ComponentRef Id=\"Probe\"/></Feature></Package></Wix>",
        super::msi_prerequisite_authoring()?,
        escape(marker)
    );
    if authoring.contains("$(") || authoring.contains("!(") {
        return Err(io::Error::other("unsafe MSI probe authoring path"));
    }
    fs::write(&source, authoring)?;
    let package = output.join("probe.msi");
    invoke(
        &tools,
        output,
        None,
        "probe-build",
        &[
            "build".into(),
            source.into_os_string(),
            "-arch".into(),
            "x64".into(),
            "-pdbtype".into(),
            "none".into(),
            "-o".into(),
            package.as_os_str().to_owned(),
        ],
    )?;
    invoke(
        &tools,
        output,
        None,
        "probe-validate",
        &[
            "msi".into(),
            "validate".into(),
            package.as_os_str().to_owned(),
        ],
    )?;
    invoke(
        &tools,
        output,
        None,
        "probe-decompile",
        &[
            "msi".into(),
            "decompile".into(),
            package.into_os_string(),
            "-o".into(),
            output.join("decompiled.wxs").into_os_string(),
        ],
    )
}

/// Authors the complete Windows package without invoking installer tools.
///
/// The caller supplies a verified payload and its exact relative file inventory.
///
/// # Errors
/// Returns an invalid release version or a non-UTF-8 source path.
pub fn msi_package_authoring(
    payload: &Path,
    version: &str,
    files: &[String],
) -> io::Result<String> {
    super::release_version(&format!("v{version}"))?;
    let mut authoring = format!(
        "<Wix xmlns=\"http://wixtoolset.org/schemas/v4/wxs\"><Package Name=\"Tallyline\" Manufacturer=\"Tallyline Project\" Version=\"{}\" Language=\"1033\" Scope=\"perMachine\" UpgradeCode=\"B8E64389-A01F-4C23-9F28-E77312875B38\"><MajorUpgrade DowngradeErrorMessage=\"A newer Tallyline version is already installed.\"/><MediaTemplate EmbedCab=\"yes\"/>{}{}<StandardDirectory Id=\"ProgramFiles64Folder\"><Directory Id=\"INSTALLFOLDER\" Name=\"Tallyline\">",
        version,
        super::msi_prerequisite_authoring()?,
        super::desktop::msi_icon_authoring(payload)?
    );
    let mut components = Vec::new();
    directory(payload, "", files, &mut authoring, &mut components)?;
    authoring.push_str("</Directory></StandardDirectory><StandardDirectory Id=\"ProgramMenuFolder\"/><Feature Id=\"Application\" Title=\"Tallyline\" Level=\"1\">");
    for component in components {
        authoring.push_str(&format!("<ComponentRef Id=\"{component}\"/>"));
    }
    authoring.push_str("</Feature></Package></Wix>");
    Ok(authoring)
}
