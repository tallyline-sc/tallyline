// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! The notices directory of R-DOC-3: the licence and notice files of every shipped component.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

use serde_json::Value;

use crate::osv::PackageId;

/// The reviewed list of 10 S-34, relative to the workspace root.
pub const REVIEWED_LIST_PATH: &str = "supply-chain/notices.toml";

/// The notices directory inside every artefact.
pub const NOTICES_DIRECTORY: &str = "legal";

/// The subdirectory of the notices directory that holds the files of the third-party crates.
pub const CRATES_DIRECTORY: &str = "crates";

/// The index of the crates directory: one line per crate with its version, the SPDX expression its
/// manifest declares (`-` when it declares none) and the licences `cargo-deny` detected.
pub const CRATE_INDEX: &str = "index.tsv";

const INDEX_HEADER: &str = "crate\tversion\tdeclared\tdetected\n";

const UNDECLARED: &str = "-";

const NOTICE_PREFIXES: [&str; 8] = [
    "licence",
    "license",
    "copying",
    "copyright",
    "notice",
    "unlicense",
    "thirdpartynotice",
    "patents",
];

const RUST_SOURCE_SUFFIX: &str = ".rs";

const LICENCE_DIRECTORIES: [&str; 4] = ["licence", "licences", "license", "licenses"];

/// When a component of the reviewed list ships.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Shipped {
    /// In every artefact.
    Always,
    /// Whenever the named crate is in the shipped dependency graph.
    WithCrate(String),
}

/// Where a file of a component of the reviewed list comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComponentFile {
    /// A path relative to the sysroot of the pinned toolchain.
    Toolchain(String),
    /// A path relative to the workspace root.
    Repository(String),
}

impl ComponentFile {
    fn path(&self) -> &str {
        match self {
            Self::Toolchain(path) | Self::Repository(path) => path,
        }
    }

    fn file_name(&self) -> &str {
        let path = self.path();
        path.rsplit('/').next().unwrap_or(path)
    }

    fn origin(&self) -> Origin {
        match self {
            Self::Toolchain(path) => Origin::Toolchain(path.clone()),
            Self::Repository(path) => Origin::Repository(path.clone()),
        }
    }
}

/// A component of the reviewed list: something the binary carries that `cargo-deny` cannot see.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Component {
    /// The subdirectory of the notices directory that receives the files.
    pub directory: String,
    /// The name of the component.
    pub name: String,
    /// When the component ships.
    pub shipped: Shipped,
    /// The licence and notice files, copied under their file names.
    pub files: Vec<ComponentFile>,
}

/// Parses the reviewed list: an array of `[[component]]` tables with `directory`, `name`,
/// `shipped` (`"always"` or `{ with-crate = "<crate>" }`) and `files` (tables with one `toolchain`
/// or `repository` path each).
///
/// # Errors
///
/// Returns a description when the text is not TOML, has a key other than `component`, or a
/// component has an unknown or missing key, a directory that is not one lower-case segment, is
/// `crates` or repeats another, an unknown `shipped` value, no files, a path that is empty,
/// absolute or contains `..`, or two files with the same name.
pub fn parse_reviewed_list(text: &str) -> Result<Vec<Component>, String> {
    let table: toml::Table = text
        .parse()
        .map_err(|error| format!("{REVIEWED_LIST_PATH} is not valid TOML: {error}"))?;
    if let Some(key) = table.keys().find(|key| key.as_str() != "component") {
        return Err(format!("{REVIEWED_LIST_PATH} has the unknown key `{key}`"));
    }
    let Some(entries) = table.get("component") else {
        return Ok(Vec::new());
    };
    let Some(entries) = entries.as_array() else {
        return Err(format!(
            "{REVIEWED_LIST_PATH}: `component` must be an array of tables"
        ));
    };
    let mut directories = BTreeSet::new();
    let mut components = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        let component = parse_component(index + 1, entry)?;
        if !directories.insert(component.directory.clone()) {
            return Err(format!(
                "{REVIEWED_LIST_PATH}: the directory `{}` is used twice",
                component.directory
            ));
        }
        components.push(component);
    }
    Ok(components)
}

fn parse_component(number: usize, entry: &toml::Value) -> Result<Component, String> {
    let fail = |problem: &str| format!("{REVIEWED_LIST_PATH}: component {number}: {problem}");
    let Some(entry) = entry.as_table() else {
        return Err(fail("is not a table"));
    };
    if let Some(key) = entry.keys().find(|key| {
        !matches!(
            key.as_str(),
            "directory" | "name" | "shipped" | "files" | "catalogue"
        )
    }) {
        return Err(fail(&format!("has the unknown key `{key}`")));
    }
    let text = |key: &str| -> Result<String, String> {
        entry
            .get(key)
            .and_then(toml::Value::as_str)
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| fail(&format!("needs a non-empty `{key}` string")))
    };
    let directory = text("directory")?;
    if directory == CRATES_DIRECTORY
        || !directory
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(fail(&format!(
            "the directory must be one lower-case segment other than `{CRATES_DIRECTORY}`"
        )));
    }
    let name = text("name")?;
    let shipped = parse_shipped(entry.get("shipped"))
        .ok_or_else(|| fail("`shipped` must be \"always\" or { with-crate = \"<crate>\" }"))?;
    let files = parse_files(entry.get("files")).map_err(|problem| fail(&problem))?;
    Ok(Component {
        directory,
        name,
        shipped,
        files,
    })
}

fn parse_shipped(value: Option<&toml::Value>) -> Option<Shipped> {
    let value = value?;
    if value.as_str() == Some("always") {
        return Some(Shipped::Always);
    }
    let table = value.as_table()?;
    if table.len() != 1 {
        return None;
    }
    table
        .get("with-crate")
        .and_then(toml::Value::as_str)
        .filter(|name| !name.trim().is_empty())
        .map(|name| Shipped::WithCrate(name.trim().to_owned()))
}

fn parse_files(value: Option<&toml::Value>) -> Result<Vec<ComponentFile>, String> {
    let entries = value
        .and_then(toml::Value::as_array)
        .filter(|entries| !entries.is_empty())
        .ok_or_else(|| "needs a non-empty `files` array".to_owned())?;
    let mut names = BTreeSet::new();
    let mut files = Vec::new();
    for entry in entries {
        let file = parse_file(entry)?;
        if !names.insert(file.file_name().to_owned()) {
            return Err(format!("has two files named `{}`", file.file_name()));
        }
        files.push(file);
    }
    Ok(files)
}

fn parse_file(entry: &toml::Value) -> Result<ComponentFile, String> {
    let problem = || "every file is a table with one `toolchain` or `repository` path".to_owned();
    let table = entry
        .as_table()
        .filter(|table| table.len() == 1)
        .ok_or_else(problem)?;
    let (key, path) = table.iter().next().ok_or_else(problem)?;
    let path = path.as_str().ok_or_else(problem)?.to_owned();
    if !is_relative_path(&path) {
        return Err(format!(
            "the path `{path}` must be relative with `/` separators and without `..`"
        ));
    }
    match key.as_str() {
        "toolchain" => Ok(ComponentFile::Toolchain(path)),
        "repository" => Ok(ComponentFile::Repository(path)),
        _ => Err(problem()),
    }
}

fn is_relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains('\\')
        && !path.contains(':')
        && path
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "..")
}

/// Whether a file of a vendored crate is a licence or notice file.
///
/// The name, lower-cased and without `-` and `_`, starts with `licence`, `license`, `copying`,
/// `copyright`, `notice`, `unlicense`, `thirdpartynotice` or `patents`, and is not a Rust source
/// file.
#[must_use]
pub fn is_notice_file(file_name: &str) -> bool {
    let normalised: String = file_name
        .chars()
        .filter(|c| !matches!(c, '-' | '_'))
        .flat_map(char::to_lowercase)
        .collect();
    !normalised.ends_with(RUST_SOURCE_SUFFIX)
        && NOTICE_PREFIXES
            .iter()
            .any(|prefix| normalised.starts_with(prefix))
}

/// Reads the licences per package from `cargo deny list --format json --layout crate`.
///
/// # Errors
///
/// Returns a description when the text is not a JSON object whose keys are `name version source`
/// and whose values hold a `licenses` array of strings.
pub fn parse_deny_report(report: &str) -> Result<BTreeMap<PackageId, Vec<String>>, String> {
    let document: Value = serde_json::from_str(report)
        .map_err(|error| format!("the cargo-deny report is not valid JSON: {error}"))?;
    let entries = document
        .as_object()
        .ok_or_else(|| "the cargo-deny report is not a JSON object".to_owned())?;
    let mut licences = BTreeMap::new();
    for (key, value) in entries {
        let mut words = key.split(' ');
        let (Some(name), Some(version), Some(_source)) = (words.next(), words.next(), words.next())
        else {
            return Err(format!(
                "the cargo-deny report names `{key}`, not `name version source`"
            ));
        };
        let names = value
            .get("licenses")
            .and_then(Value::as_array)
            .and_then(|names| {
                names
                    .iter()
                    .map(|name| name.as_str().map(str::to_owned))
                    .collect::<Option<Vec<_>>>()
            })
            .ok_or_else(|| {
                format!("the cargo-deny report has no `licenses` strings for `{key}`")
            })?;
        licences.insert(
            PackageId {
                name: name.to_owned(),
                version: version.to_owned(),
            },
            names,
        );
    }
    Ok(licences)
}

/// Where a file of the notices directory is copied from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Origin {
    /// A path relative to the vendor directory.
    Vendor(String),
    /// A path relative to the toolchain sysroot.
    Toolchain(String),
    /// A path relative to the workspace root.
    Repository(String),
}

/// One file of the notices directory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlannedFile {
    /// Where the file comes from.
    pub origin: Origin,
    /// Its path relative to the notices directory, with `/` separators.
    pub target: String,
}

/// A reason the notices directory would be incomplete.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Finding {
    /// A shipped crate has no directory in the vendored sources.
    NotVendored(PackageId),
    /// A shipped crate's vendored sources hold no licence or notice file.
    NoNoticeFiles(PackageId),
    /// A shipped crate is missing from the `cargo-deny` report or has no licence there.
    NoLicence(PackageId),
    /// A component of the reviewed list ships with a crate that `Cargo.lock` does not hold.
    UnknownCrate {
        /// The component's name.
        component: String,
        /// The crate it names.
        name: String,
    },
    /// A file of a shipped component of the reviewed list does not exist.
    MissingFile {
        /// The component's name.
        component: String,
        /// The missing file.
        file: ComponentFile,
    },
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotVendored(package) => write!(f, "{package}: not in the vendored sources"),
            Self::NoNoticeFiles(package) => {
                write!(
                    f,
                    "{package}: no licence or notice file in the vendored sources"
                )
            }
            Self::NoLicence(package) => write!(f, "{package}: no licence in the cargo-deny report"),
            Self::UnknownCrate { component, name } => {
                write!(
                    f,
                    "{component}: ships with `{name}`, which Cargo.lock does not hold"
                )
            }
            Self::MissingFile { component, file } => match file {
                ComponentFile::Toolchain(path) => {
                    write!(
                        f,
                        "{component}: {path} is missing from the toolchain sysroot"
                    )
                }
                ComponentFile::Repository(path) => {
                    write!(f, "{component}: {path} is missing from the repository")
                }
            },
        }
    }
}

/// What the notices directory holds, or why it cannot be complete.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Assembly {
    /// The files to copy.
    pub copies: Vec<PlannedFile>,
    /// The text of [`CRATE_INDEX`].
    pub index: String,
    /// The findings; the directory is complete only when there are none.
    pub findings: Vec<Finding>,
}

/// What the vendored sources say about one package.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VendoredCrate {
    /// The SPDX expression of the `license` field of its manifest, absent when it has none.
    pub declared: Option<String>,
    /// Its licence and notice files, relative to its directory.
    pub files: Vec<String>,
}

/// The inputs of [`assemble`].
#[derive(Clone, Copy, Debug)]
pub struct Inputs<'a> {
    /// The third-party crates of the shipped dependency graph.
    pub shipped: &'a BTreeSet<PackageId>,
    /// The names of all packages in `Cargo.lock`.
    pub locked: &'a BTreeSet<String>,
    /// The vendored packages.
    pub vendored: &'a BTreeMap<PackageId, VendoredCrate>,
    /// The licences per package from the `cargo-deny` report.
    pub licences: &'a BTreeMap<PackageId, Vec<String>>,
    /// The reviewed list.
    pub components: &'a [Component],
}

/// Plans the notices directory from `inputs`; `exists` tells whether a component file exists.
///
/// Every shipped crate contributes its vendored licence and notice files under
/// `crates/<name>-<version>/` and a line of the index with its declared SPDX expression and the
/// licences of the report; every shipped component of the reviewed list contributes its files
/// under its directory. A shipped crate that is not vendored, has no such file in either its
/// vendor tree or reviewed-list entry, has no licence in the report, a component that names a
/// crate `Cargo.lock` does not hold, and a missing component file, are findings.
#[must_use]
pub fn assemble(inputs: Inputs<'_>, exists: impl Fn(&ComponentFile) -> bool) -> Assembly {
    let mut assembly = Assembly {
        index: INDEX_HEADER.to_owned(),
        ..Assembly::default()
    };
    for package in inputs.shipped {
        let directory = vendor_directory(package);
        let vendored = inputs.vendored.get(package);
        match vendored {
            None => assembly
                .findings
                .push(Finding::NotVendored(package.clone())),
            Some(vendored)
                if vendored.files.is_empty()
                    && !inputs.components.iter().any(|component| {
                        matches!(
                            &component.shipped,
                            Shipped::WithCrate(name) if name == &package.name
                        ) && component.files.iter().all(&exists)
                    }) =>
            {
                assembly
                    .findings
                    .push(Finding::NoNoticeFiles(package.clone()));
            }
            Some(vendored) => {
                assembly
                    .copies
                    .extend(vendored.files.iter().map(|file| PlannedFile {
                        origin: Origin::Vendor(format!("{directory}/{file}")),
                        target: format!("{CRATES_DIRECTORY}/{directory}/{file}"),
                    }))
            }
        }
        match inputs
            .licences
            .get(package)
            .filter(|licences| !licences.is_empty())
        {
            None => assembly.findings.push(Finding::NoLicence(package.clone())),
            Some(licences) => assembly.index.push_str(&format!(
                "{}\t{}\t{}\t{}\n",
                package.name,
                package.version,
                vendored
                    .and_then(|vendored| vendored.declared.as_deref())
                    .unwrap_or(UNDECLARED),
                licences.join(", ")
            )),
        }
    }
    for component in inputs.components {
        let ships = match &component.shipped {
            Shipped::Always => true,
            Shipped::WithCrate(name) => {
                if !inputs.locked.contains(name) {
                    assembly.findings.push(Finding::UnknownCrate {
                        component: component.name.clone(),
                        name: name.clone(),
                    });
                }
                inputs.shipped.iter().any(|package| &package.name == name)
            }
        };
        if !ships {
            continue;
        }
        for file in &component.files {
            if exists(file) {
                assembly.copies.push(PlannedFile {
                    origin: file.origin(),
                    target: format!("{}/{}", component.directory, file.file_name()),
                });
            } else {
                assembly.findings.push(Finding::MissingFile {
                    component: component.name.clone(),
                    file: file.clone(),
                });
            }
        }
    }
    assembly
}

fn vendor_directory(package: &PackageId) -> String {
    format!("{}-{}", package.name, package.version)
}

fn is_licence_directory(name: &str) -> bool {
    LICENCE_DIRECTORIES.contains(&name.to_ascii_lowercase().as_str())
}

/// The licence and notice files below `package_dir`, relative to it with `/` separators, sorted.
///
/// A file counts when its name passes [`is_notice_file`] or it lies in a directory named
/// `LICENSES`, `LICENCES`, `LICENSE` or `LICENCE` (in any case).
///
/// # Errors
///
/// Returns an error when a directory cannot be read, a name is not UTF-8, or an entry is neither
/// a file nor a directory.
pub fn notice_files(package_dir: &Path) -> io::Result<Vec<String>> {
    let mut found = Vec::new();
    walk(package_dir, "", false, &mut found)?;
    found.sort();
    Ok(found)
}

fn walk(
    directory: &Path,
    prefix: &str,
    inside_notice_directory: bool,
    found: &mut Vec<String>,
) -> io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|name| io::Error::other(format!("{} is not UTF-8", name.to_string_lossy())))?;
        let relative = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        let kind = entry.file_type()?;
        if kind.is_dir() {
            walk(
                &entry.path(),
                &relative,
                inside_notice_directory || is_licence_directory(&name),
                found,
            )?;
        } else if kind.is_file() {
            if inside_notice_directory || is_notice_file(&name) {
                found.push(relative);
            }
        } else {
            return Err(io::Error::other(format!(
                "{} is neither a file nor a directory",
                entry.path().display()
            )));
        }
    }
    Ok(())
}

/// The declared licence and the licence and notice files of every package of `packages` that has
/// a directory `<name>-<version>` in `vendor_dir`; a package without one is left out.
///
/// # Errors
///
/// Returns an error when a package directory cannot be read (see [`notice_files`]) or its
/// `Cargo.toml` cannot be read as TOML.
pub fn vendored_crates(
    vendor_dir: &Path,
    packages: &BTreeSet<PackageId>,
) -> io::Result<BTreeMap<PackageId, VendoredCrate>> {
    let mut vendored = BTreeMap::new();
    for package in packages {
        let package_dir = vendor_dir.join(vendor_directory(package));
        if package_dir.is_dir() {
            vendored.insert(
                package.clone(),
                VendoredCrate {
                    declared: declared_licence(&package_dir)?,
                    files: notice_files(&package_dir)?,
                },
            );
        }
    }
    Ok(vendored)
}

fn declared_licence(package_dir: &Path) -> io::Result<Option<String>> {
    let path = package_dir.join("Cargo.toml");
    let manifest: toml::Table = fs::read_to_string(&path)?
        .parse()
        .map_err(|error| io::Error::other(format!("{}: {error}", path.display())))?;
    Ok(manifest
        .get("package")
        .and_then(|package| package.get("license"))
        .and_then(toml::Value::as_str)
        .map(str::to_owned))
}

/// The directories the origins of an [`Assembly`] are relative to.
#[derive(Clone, Copy, Debug)]
pub struct Roots<'a> {
    /// The vendor directory.
    pub vendor: &'a Path,
    /// The sysroot of the pinned toolchain.
    pub toolchain: &'a Path,
    /// The workspace root.
    pub repository: &'a Path,
}

/// Replaces `target` with the notices directory of `assembly`: every copy, unmodified, and the
/// crate index.
///
/// # Errors
///
/// Returns an error when the old directory cannot be removed or a file cannot be copied or
/// written.
pub fn write_directory(target: &Path, assembly: &Assembly, roots: Roots<'_>) -> io::Result<()> {
    if target.exists() {
        fs::remove_dir_all(target)?;
    }
    for copy in &assembly.copies {
        let source = match &copy.origin {
            Origin::Vendor(path) => roots.vendor.join(path),
            Origin::Toolchain(path) => roots.toolchain.join(path),
            Origin::Repository(path) => roots.repository.join(path),
        };
        let destination = target.join(&copy.target);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(&source, &destination).map_err(|error| {
            io::Error::new(error.kind(), format!("{}: {error}", source.display()))
        })?;
    }
    let crates = target.join(CRATES_DIRECTORY);
    fs::create_dir_all(&crates)?;
    fs::write(crates.join(CRATE_INDEX), &assembly.index)
}

#[cfg(test)]
#[expect(
    non_snake_case,
    reason = "tagged test names use a double underscore (11 B4)"
)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;

    use super::{
        Assembly, Component, ComponentFile, Finding, Inputs, Origin, PlannedFile, Roots, Shipped,
        VendoredCrate, assemble, is_notice_file, notice_files, parse_deny_report,
        parse_reviewed_list, vendored_crates, write_directory,
    };
    use crate::osv::PackageId;
    use crate::scratch::Scratch;

    fn package(name: &str, version: &str) -> PackageId {
        PackageId {
            name: name.to_owned(),
            version: version.to_owned(),
        }
    }

    fn vendored(declared: &str, files: &[&str]) -> VendoredCrate {
        VendoredCrate {
            declared: Some(declared.to_owned()),
            files: files.iter().map(|&file| file.to_owned()).collect(),
        }
    }

    fn names(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|&name| name.to_owned()).collect()
    }

    fn rust_std() -> Component {
        Component {
            directory: "rust-std".to_owned(),
            name: "The Rust standard library".to_owned(),
            shipped: Shipped::Always,
            files: vec![ComponentFile::Toolchain(
                "share/doc/rust/COPYRIGHT-library.html".to_owned(),
            )],
        }
    }

    fn onnxruntime() -> Component {
        Component {
            directory: "onnxruntime".to_owned(),
            name: "ONNX Runtime".to_owned(),
            shipped: Shipped::WithCrate("ort".to_owned()),
            files: vec![ComponentFile::Repository(
                "supply-chain/notices/onnxruntime/ThirdPartyNotices.txt".to_owned(),
            )],
        }
    }

    #[test]
    fn the_reviewed_list_reads_components_with_their_condition_and_files() {
        let text = r#"
[[component]]
directory = "rust-std"
name = "The Rust standard library"
shipped = "always"
files = [{ toolchain = "share/doc/rust/COPYRIGHT-library.html" }]

[[component]]
directory = "onnxruntime"
name = "ONNX Runtime"
shipped = { with-crate = "ort" }
files = [{ repository = "supply-chain/notices/onnxruntime/ThirdPartyNotices.txt" }]
"#;
        assert_eq!(
            parse_reviewed_list(text),
            Ok(vec![rust_std(), onnxruntime()])
        );
    }

    #[test]
    fn r_doc_3__the_committed_reviewed_list_parses_and_ships_the_standard_library_always() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(super::REVIEWED_LIST_PATH);
        let components = parse_reviewed_list(&fs::read_to_string(path).unwrap()).unwrap();
        assert!(
            components
                .iter()
                .any(|component| component.directory == "rust-std"
                    && component.shipped == Shipped::Always)
        );
    }

    #[test]
    fn an_empty_reviewed_list_has_no_components() {
        assert_eq!(parse_reviewed_list(""), Ok(Vec::new()));
    }

    #[test]
    fn the_reviewed_list_rejects_malformed_components() {
        let component = |directory: &str, shipped: &str, files: &str| {
            format!(
                "[[component]]\ndirectory = \"{directory}\"\nname = \"x\"\nshipped = {shipped}\nfiles = {files}\n"
            )
        };
        let good_files = "[{ repository = \"a/LICENSE\" }]";
        let rejected = [
            "component = 1".to_owned(),
            "other = 1".to_owned(),
            component("crates", "\"always\"", good_files),
            component("", "\"always\"", good_files),
            component("Rust Std", "\"always\"", good_files),
            component("a/b", "\"always\"", good_files),
            component("x", "\"sometimes\"", good_files),
            component("x", "{ with-crate = \"\" }", good_files),
            component("x", "\"always\"", "[]"),
            component("x", "\"always\"", "[{ elsewhere = \"a/LICENSE\" }]"),
            component("x", "\"always\"", "[{ repository = \"../LICENSE\" }]"),
            component("x", "\"always\"", "[{ repository = \"/etc/LICENSE\" }]"),
            component("x", "\"always\"", "[{ repository = \"a\\\\LICENSE\" }]"),
            component(
                "x",
                "\"always\"",
                "[{ repository = \"a/LICENSE\" }, { toolchain = \"b/LICENSE\" }]",
            ),
            format!(
                "{}{}",
                component("x", "\"always\"", good_files),
                component("x", "\"always\"", good_files)
            ),
            "[[component]]\ndirectory = \"x\"\nshipped = \"always\"\nfiles = [{ repository = \"a/LICENSE\" }]\n"
                .to_owned(),
            format!("{}extra = 1\n", component("x", "\"always\"", good_files)),
        ];
        for text in rejected {
            assert!(parse_reviewed_list(&text).is_err(), "accepted:\n{text}");
        }
    }

    #[test]
    fn r_doc_3__licence_and_notice_files_are_recognised_by_name() {
        for name in [
            "LICENSE",
            "LICENSE-MIT",
            "LICENSE-APACHE",
            "license-apache-2.0",
            "LICENSE.md",
            "LICENSE.txt",
            "LICENCE",
            "COPYING",
            "COPYRIGHT",
            "NOTICE",
            "UNLICENSE",
            "ThirdPartyNotices.txt",
            "LICENSE-THIRD-PARTY",
        ] {
            assert!(is_notice_file(name), "{name}");
        }
        for name in [
            "README.md",
            "Cargo.toml",
            "license.rs",
            "lib.rs",
            "CHANGELOG.md",
        ] {
            assert!(!is_notice_file(name), "{name}");
        }
    }

    #[test]
    fn the_deny_report_gives_the_licences_per_package() {
        let report = r#"{
            "memchr 2.8.3 registry+https://github.com/rust-lang/crates.io-index": {"licenses": ["Unlicense", "MIT"]},
            "tallyline-adapter-uex 0.1.0 path+file:///D:/repo/crates/adapter-uex": {"licenses": ["GPL-3.0-or-later"]}
        }"#;
        let licences = parse_deny_report(report).unwrap();
        assert_eq!(
            licences.get(&package("memchr", "2.8.3")),
            Some(&vec!["Unlicense".to_owned(), "MIT".to_owned()])
        );
        assert_eq!(licences.len(), 2);
    }

    #[test]
    fn the_deny_report_is_rejected_when_it_is_not_the_crate_layout() {
        for report in [
            "not json",
            "[]",
            r#"{"memchr": {"licenses": ["MIT"]}}"#,
            r#"{"memchr 2.8.3 registry+x": {}}"#,
            r#"{"memchr 2.8.3 registry+x": {"licenses": [1]}}"#,
        ] {
            assert!(parse_deny_report(report).is_err(), "accepted {report}");
        }
    }

    #[test]
    fn r_doc_3__every_shipped_crate_contributes_its_files_and_an_index_line() {
        let shipped = BTreeSet::from([package("memchr", "2.8.3"), package("tokio", "1.53.2")]);
        let locked = names(&["memchr", "tokio", "cc"]);
        let vendored = BTreeMap::from([
            (
                package("memchr", "2.8.3"),
                vendored("Unlicense OR MIT", &["COPYING", "LICENSE-MIT"]),
            ),
            (
                package("tokio", "1.53.2"),
                VendoredCrate {
                    declared: None,
                    files: vec!["LICENSE".to_owned()],
                },
            ),
            (package("cc", "1.6.0"), vendored("MIT", &["LICENSE-MIT"])),
        ]);
        let licences = BTreeMap::from([
            (
                package("memchr", "2.8.3"),
                vec!["Unlicense".to_owned(), "MIT".to_owned()],
            ),
            (package("tokio", "1.53.2"), vec!["MIT".to_owned()]),
        ]);
        let assembly = assemble(
            Inputs {
                shipped: &shipped,
                locked: &locked,
                vendored: &vendored,
                licences: &licences,
                components: &[],
            },
            |_| true,
        );
        assert_eq!(
            assembly,
            Assembly {
                copies: vec![
                    PlannedFile {
                        origin: Origin::Vendor("memchr-2.8.3/COPYING".to_owned()),
                        target: "crates/memchr-2.8.3/COPYING".to_owned(),
                    },
                    PlannedFile {
                        origin: Origin::Vendor("memchr-2.8.3/LICENSE-MIT".to_owned()),
                        target: "crates/memchr-2.8.3/LICENSE-MIT".to_owned(),
                    },
                    PlannedFile {
                        origin: Origin::Vendor("tokio-1.53.2/LICENSE".to_owned()),
                        target: "crates/tokio-1.53.2/LICENSE".to_owned(),
                    },
                ],
                index: "crate\tversion\tdeclared\tdetected\n\
                        memchr\t2.8.3\tUnlicense OR MIT\tUnlicense, MIT\n\
                        tokio\t1.53.2\t-\tMIT\n"
                    .to_owned(),
                findings: Vec::new(),
            }
        );
    }

    #[test]
    fn r_doc_3__reviewed_component_supplies_a_crates_missing_vendored_licence_text() {
        let shipped = BTreeSet::from([package("egui", "0.36.2")]);
        let locked = names(&["egui"]);
        let vendored = BTreeMap::from([(
            package("egui", "0.36.2"),
            vendored("MIT OR Apache-2.0", &[]),
        )]);
        let licences = BTreeMap::from([(
            package("egui", "0.36.2"),
            vec!["MIT".to_owned(), "Apache-2.0".to_owned()],
        )]);
        let components = vec![Component {
            directory: "egui".to_owned(),
            name: "egui".to_owned(),
            shipped: Shipped::WithCrate("egui".to_owned()),
            files: vec![
                ComponentFile::Repository("supply-chain/notices/egui/LICENSE-MIT".to_owned()),
                ComponentFile::Repository("supply-chain/notices/egui/LICENSE-APACHE".to_owned()),
            ],
        }];
        let assembly = assemble(
            Inputs {
                shipped: &shipped,
                locked: &locked,
                vendored: &vendored,
                licences: &licences,
                components: &components,
            },
            |_| true,
        );

        assert!(assembly.findings.is_empty());
        assert_eq!(assembly.copies.len(), 2);
        assert!(
            assembly
                .copies
                .iter()
                .all(|file| file.target.starts_with("egui/"))
        );
    }

    #[test]
    fn r_doc_3__a_shipped_crate_without_vendored_sources_files_or_licence_is_a_finding() {
        let shipped = BTreeSet::from([
            package("absent", "1.0.0"),
            package("bare", "1.0.0"),
            package("unknown", "1.0.0"),
        ]);
        let locked = names(&["absent", "bare", "unknown"]);
        let vendored = BTreeMap::from([
            (package("bare", "1.0.0"), vendored("MIT", &[])),
            (package("unknown", "1.0.0"), vendored("MIT", &["LICENSE"])),
        ]);
        let licences = BTreeMap::from([
            (package("absent", "1.0.0"), vec!["MIT".to_owned()]),
            (package("bare", "1.0.0"), vec!["MIT".to_owned()]),
            (package("unknown", "1.0.0"), Vec::new()),
        ]);
        let assembly = assemble(
            Inputs {
                shipped: &shipped,
                locked: &locked,
                vendored: &vendored,
                licences: &licences,
                components: &[],
            },
            |_| true,
        );
        assert_eq!(
            assembly.findings,
            [
                Finding::NotVendored(package("absent", "1.0.0")),
                Finding::NoNoticeFiles(package("bare", "1.0.0")),
                Finding::NoLicence(package("unknown", "1.0.0")),
            ]
        );
    }

    #[test]
    fn r_doc_3__a_component_ships_always_or_with_its_crate() {
        let empty_vendored = BTreeMap::new();
        let empty_licences = BTreeMap::new();
        let locked = names(&["ort"]);
        let components = [rust_std(), onnxruntime()];
        let without_ort = BTreeSet::new();
        let assembly = assemble(
            Inputs {
                shipped: &without_ort,
                locked: &locked,
                vendored: &empty_vendored,
                licences: &empty_licences,
                components: &components,
            },
            |_| true,
        );
        assert_eq!(
            assembly.copies,
            [PlannedFile {
                origin: Origin::Toolchain("share/doc/rust/COPYRIGHT-library.html".to_owned()),
                target: "rust-std/COPYRIGHT-library.html".to_owned(),
            }]
        );
        assert!(assembly.findings.is_empty());

        let ort = package("ort", "2.0.0-rc.14");
        let with_ort = BTreeSet::from([ort.clone()]);
        let vendored_ort = BTreeMap::from([(ort.clone(), vendored("MIT", &["LICENSE-MIT"]))]);
        let licences = BTreeMap::from([(ort, vec!["MIT".to_owned()])]);
        let assembly = assemble(
            Inputs {
                shipped: &with_ort,
                locked: &locked,
                vendored: &vendored_ort,
                licences: &licences,
                components: &components,
            },
            |_| true,
        );
        assert!(assembly.copies.contains(&PlannedFile {
            origin: Origin::Repository(
                "supply-chain/notices/onnxruntime/ThirdPartyNotices.txt".to_owned()
            ),
            target: "onnxruntime/ThirdPartyNotices.txt".to_owned(),
        }));
    }

    #[test]
    fn r_doc_3__a_component_naming_a_crate_outside_the_lockfile_is_a_finding() {
        let empty_vendored = BTreeMap::new();
        let empty_licences = BTreeMap::new();
        let shipped = BTreeSet::new();
        let locked = names(&["tokio"]);
        let assembly = assemble(
            Inputs {
                shipped: &shipped,
                locked: &locked,
                vendored: &empty_vendored,
                licences: &empty_licences,
                components: &[onnxruntime()],
            },
            |_| true,
        );
        assert_eq!(
            assembly.findings,
            [Finding::UnknownCrate {
                component: "ONNX Runtime".to_owned(),
                name: "ort".to_owned(),
            }]
        );
    }

    #[test]
    fn r_doc_3__a_shipped_component_whose_file_is_missing_is_a_finding() {
        let ort = package("ort", "2.0.0-rc.14");
        let shipped = BTreeSet::from([ort.clone()]);
        let locked = names(&["ort"]);
        let vendored_ort = BTreeMap::from([(ort.clone(), vendored("MIT", &["LICENSE-MIT"]))]);
        let licences = BTreeMap::from([(ort, vec!["MIT".to_owned()])]);
        let components = [rust_std(), onnxruntime()];
        let only_toolchain = |file: &ComponentFile| matches!(file, ComponentFile::Toolchain(_));
        let assembly = assemble(
            Inputs {
                shipped: &shipped,
                locked: &locked,
                vendored: &vendored_ort,
                licences: &licences,
                components: &components,
            },
            only_toolchain,
        );
        assert_eq!(
            assembly.findings,
            [Finding::MissingFile {
                component: "ONNX Runtime".to_owned(),
                file: ComponentFile::Repository(
                    "supply-chain/notices/onnxruntime/ThirdPartyNotices.txt".to_owned()
                ),
            }]
        );
        let not_shipped = BTreeSet::new();
        let empty_vendored = BTreeMap::new();
        let empty_licences = BTreeMap::new();
        assert!(
            assemble(
                Inputs {
                    shipped: &not_shipped,
                    locked: &locked,
                    vendored: &empty_vendored,
                    licences: &empty_licences,
                    components: &components,
                },
                only_toolchain,
            )
            .findings
            .is_empty()
        );
    }

    #[test]
    fn r_doc_3__notice_files_are_found_in_nested_directories_of_a_vendored_crate() {
        let crate_dir = Scratch::new();
        crate_dir.write("LICENSE", "MIT");
        crate_dir.write("README.md", "readme");
        crate_dir.write("src/lib.rs", "");
        crate_dir.write("src/spin/LICENSE", "MIT");
        crate_dir.write("aws-lc/third_party/fiat/LICENSE", "MIT");
        crate_dir.write("LICENSES/MIT.txt", "MIT");
        crate_dir.write("src/license_data/table.rs", "");
        crate_dir.write("src/notice_board/mod.rs", "");
        assert_eq!(
            notice_files(crate_dir.path()).unwrap(),
            [
                "LICENSE",
                "LICENSES/MIT.txt",
                "aws-lc/third_party/fiat/LICENSE",
                "src/spin/LICENSE"
            ]
        );
    }

    #[test]
    fn r_doc_3__the_vendored_manifest_gives_the_declared_licence() {
        let vendor = Scratch::new();
        vendor.write(
            "tokio-1.53.2/Cargo.toml",
            "[package]\nname = \"tokio\"\nlicense = \"MIT\"\n",
        );
        vendor.write("tokio-1.53.2/LICENSE", "MIT");
        vendor.write(
            "ring-0.17.14/Cargo.toml",
            "[package]\nname = \"ring\"\nlicense-file = \"LICENSE\"\n",
        );
        vendor.write("ring-0.17.14/LICENSE", "ISC");
        let packages = BTreeSet::from([
            package("tokio", "1.53.2"),
            package("ring", "0.17.14"),
            package("absent", "1.0.0"),
        ]);
        assert_eq!(
            vendored_crates(vendor.path(), &packages).unwrap(),
            BTreeMap::from([
                (package("tokio", "1.53.2"), vendored("MIT", &["LICENSE"])),
                (
                    package("ring", "0.17.14"),
                    VendoredCrate {
                        declared: None,
                        files: vec!["LICENSE".to_owned()],
                    }
                ),
            ])
        );
    }

    #[test]
    fn r_doc_3__the_notices_directory_holds_unmodified_copies_and_the_index() {
        let workspace = Scratch::new();
        workspace.write(
            "vendor/tokio-1.53.2/Cargo.toml",
            "[package]\nname = \"tokio\"\nlicense = \"MIT\"\n",
        );
        workspace.write("vendor/tokio-1.53.2/LICENSE", "tokio licence\r\n");
        workspace.write("vendor/tokio-1.53.2/src/lib.rs", "");
        workspace.write("vendor/serde-1.0.0/LICENSE-MIT", "serde licence");
        workspace.write("sysroot/share/doc/rust/COPYRIGHT-library.html", "<html>");
        workspace.write("legal/stale.txt", "left over");
        let vendor = workspace.path().join("vendor");
        let shipped = BTreeSet::from([package("tokio", "1.53.2")]);
        let locked = names(&["tokio", "serde"]);
        let vendored = vendored_crates(&vendor, &shipped).unwrap();
        let licences = BTreeMap::from([(package("tokio", "1.53.2"), vec!["MIT".to_owned()])]);
        let components = [rust_std()];
        let assembly = assemble(
            Inputs {
                shipped: &shipped,
                locked: &locked,
                vendored: &vendored,
                licences: &licences,
                components: &components,
            },
            |_| true,
        );
        let target = workspace.path().join("legal");
        write_directory(
            &target,
            &assembly,
            Roots {
                vendor: &vendor,
                toolchain: &workspace.path().join("sysroot"),
                repository: workspace.path(),
            },
        )
        .unwrap();
        assert_eq!(
            fs::read(target.join("crates/tokio-1.53.2/LICENSE")).unwrap(),
            b"tokio licence\r\n"
        );
        assert_eq!(
            fs::read_to_string(target.join("rust-std/COPYRIGHT-library.html")).unwrap(),
            "<html>"
        );
        assert_eq!(
            fs::read_to_string(target.join("crates/index.tsv")).unwrap(),
            "crate\tversion\tdeclared\tdetected\ntokio\t1.53.2\tMIT\tMIT\n"
        );
        assert!(!target.join("stale.txt").exists());
        assert!(!target.join("crates/serde-1.0.0").exists());
    }
}

mod catalogue;

pub use catalogue::write_catalogue;
