// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Hashing a `cargo vendor` tree and comparing it with the committed hash list (10 S-4, O-120).

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::hashlist::{
    BuildError, Difference, HashList, ListError, build, compare, sha256_hex, tree_hash,
};
use crate::lockfile::{self, LockedPackage, LockfileError};

/// The file `cargo vendor` adds to every package; it is derived data and not part of the hash.
const CHECKSUM_FILE: &str = ".cargo-checksum.json";

/// Why a vendored tree cannot be hashed or compared.
#[derive(Debug)]
pub enum VendorError {
    /// A file or directory cannot be read, or an entry is neither a file nor a directory.
    Io(io::Error),
    /// `Cargo.lock` cannot be read.
    Lockfile(LockfileError),
    /// The committed hash list cannot be read.
    List(ListError),
    /// The lockfile and the vendored tree disagree on which packages exist.
    Build(BuildError),
    /// The vendor directory holds a directory that no locked package accounts for.
    Unlisted(String),
}

impl fmt::Display for VendorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Lockfile(error) => write!(f, "{error}"),
            Self::List(error) => write!(f, "{error}"),
            Self::Build(error) => write!(f, "{error}"),
            Self::Unlisted(directory) => {
                write!(f, "{directory} is vendored but no package of Cargo.lock")
            }
        }
    }
}

impl std::error::Error for VendorError {}

impl From<io::Error> for VendorError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<LockfileError> for VendorError {
    fn from(error: LockfileError) -> Self {
        Self::Lockfile(error)
    }
}

impl From<ListError> for VendorError {
    fn from(error: ListError) -> Self {
        Self::List(error)
    }
}

impl From<BuildError> for VendorError {
    fn from(error: BuildError) -> Self {
        Self::Build(error)
    }
}

/// Vendors the dependencies of the workspace at `root` into a new directory below `work_dir` and
/// returns it.
///
/// `cargo vendor --locked --versioned-dirs` runs with `CARGO_HOME` pointing at an empty directory
/// below `work_dir`, so no cached source or configuration of the machine takes part. The vendor
/// directory exists even when there is nothing to vendor.
///
/// # Errors
///
/// Returns an error when `cargo vendor` cannot start or exits unsuccessfully, for example because
/// `Cargo.lock` is stale or a download fails its checksum; the message carries its output.
pub fn regenerate(root: &Path, cargo: &OsStr, work_dir: &Path) -> io::Result<PathBuf> {
    let vendor_dir = work_dir.join("vendor");
    let cargo_home = work_dir.join("cargo-home");
    fs::create_dir_all(&cargo_home)?;
    let paths = crate::work_paths::WorkPaths::resolve(
        root,
        &std::env::current_dir()?,
        std::env::var_os("CARGO_TARGET_DIR").as_deref(),
    )?;
    let output = paths
        .command(cargo)
        .args(["vendor", "--locked", "--versioned-dirs"])
        .arg(&vendor_dir)
        .current_dir(root)
        .env("CARGO_HOME", &cargo_home)
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "cargo vendor failed: {}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    fs::create_dir_all(&vendor_dir)?;
    Ok(vendor_dir)
}

/// The [`tree_hash`] of the package directory `package_dir`, ignoring its top-level
/// `.cargo-checksum.json`.
///
/// # Errors
///
/// Returns an error when a file cannot be read or an entry is neither a file nor a directory
/// (a symbolic link, for example).
pub fn hash_package_dir(package_dir: &Path) -> io::Result<String> {
    let mut files = Vec::new();
    collect_files(package_dir, "", &mut files)?;
    files.retain(|(path, _)| path != CHECKSUM_FILE);
    Ok(tree_hash(&files))
}

fn collect_files(dir: &Path, prefix: &str, files: &mut Vec<(String, String)>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let relative = format!("{prefix}{name}");
        let kind = entry.file_type()?;
        if kind.is_dir() {
            collect_files(&entry.path(), &format!("{relative}/"), files)?;
        } else if kind.is_file() {
            files.push((relative, sha256_hex(&fs::read(entry.path())?)));
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "{} is neither a file nor a directory",
                    entry.path().display()
                ),
            ));
        }
    }
    Ok(())
}

/// The hash list of the tree in `vendor_dir`, as `Cargo.lock` describes it.
///
/// # Errors
///
/// Returns a [`VendorError`] when the lockfile is invalid, a locked package is not vendored, the
/// vendor directory holds a directory that no locked package explains, or a file cannot be read.
pub fn actual_list(lock_text: &str, vendor_dir: &Path) -> Result<HashList, VendorError> {
    let packages = lockfile::parse(lock_text)?;
    let vendored: Vec<&LockedPackage> = packages
        .iter()
        .filter(|package| package.source.is_some())
        .collect();
    let mut trees = BTreeMap::new();
    for package in &vendored {
        let directory = package.directory_name();
        let path = vendor_dir.join(&directory);
        if path.is_dir() {
            trees.insert(directory, hash_package_dir(&path)?);
        }
    }
    for entry in fs::read_dir(vendor_dir)? {
        let name = entry?.file_name().to_string_lossy().into_owned();
        if !vendored
            .iter()
            .any(|package| package.directory_name() == name)
        {
            return Err(VendorError::Unlisted(name));
        }
    }
    Ok(build(&packages, &trees)?)
}

/// Compares the committed hash list `list_text` with the tree in `vendor_dir`.
///
/// # Errors
///
/// Returns a [`VendorError`] when the list or the lockfile cannot be read, or the tree cannot be
/// hashed (see [`actual_list`]).
pub fn verify(
    lock_text: &str,
    list_text: &str,
    vendor_dir: &Path,
) -> Result<Vec<Difference>, VendorError> {
    let committed = HashList::parse(list_text)?;
    Ok(compare(&committed, &actual_list(lock_text, vendor_dir)?))
}

#[cfg(test)]
#[expect(
    non_snake_case,
    reason = "tagged test names use a double underscore (11 B4)"
)]
mod tests {
    use std::ffi::OsString;

    use super::{VendorError, actual_list, hash_package_dir, regenerate, verify};
    use crate::hashlist::{BuildError, Difference};
    use crate::scratch::Scratch;

    const CHECKSUM_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const CHECKSUM_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn lock(packages: &[(&str, &str, &str)]) -> String {
        let mut text = "version = 4\n".to_owned();
        for (name, version, checksum) in packages {
            text.push_str(&format!(
                "\n[[package]]\nname = \"{name}\"\nversion = \"{version}\"\n\
                 source = \"registry+https://github.com/rust-lang/crates.io-index\"\n\
                 checksum = \"{checksum}\"\n"
            ));
        }
        text.push_str("\n[[package]]\nname = \"xtask\"\nversion = \"0.0.0\"\n");
        text
    }

    fn vendored() -> Scratch {
        let vendor = Scratch::new();
        vendor.write("alpha-1.0.0/Cargo.toml", "[package]\nname = \"alpha\"\n");
        vendor.write("alpha-1.0.0/src/lib.rs", "pub fn alpha() {}\n");
        vendor.write(
            "alpha-1.0.0/.cargo-checksum.json",
            "{\"files\":{},\"package\":\"x\"}",
        );
        vendor.write("beta-2.1.0+meta/Cargo.toml", "[package]\nname = \"beta\"\n");
        vendor
    }

    fn lock_text() -> String {
        lock(&[
            ("alpha", "1.0.0", CHECKSUM_A),
            ("beta", "2.1.0+meta", CHECKSUM_B),
        ])
    }

    fn committed_list(vendor: &Scratch) -> String {
        actual_list(&lock_text(), vendor.path()).unwrap().render()
    }

    #[test]
    fn a_package_hash_ignores_the_top_level_checksum_file() {
        let vendor = vendored();
        let before = hash_package_dir(&vendor.path().join("alpha-1.0.0")).unwrap();
        vendor.write(
            "alpha-1.0.0/.cargo-checksum.json",
            "{\"files\":{\"a\":\"b\"}}",
        );
        assert_eq!(
            hash_package_dir(&vendor.path().join("alpha-1.0.0")).unwrap(),
            before
        );
    }

    #[test]
    fn a_package_hash_counts_a_checksum_file_in_a_subdirectory() {
        let vendor = vendored();
        let before = hash_package_dir(&vendor.path().join("alpha-1.0.0")).unwrap();
        vendor.write("alpha-1.0.0/src/.cargo-checksum.json", "{}");
        assert_ne!(
            hash_package_dir(&vendor.path().join("alpha-1.0.0")).unwrap(),
            before
        );
    }

    #[test]
    fn r_sec_2__the_hash_list_written_from_a_tree_verifies_against_it() {
        let vendor = vendored();
        let list = committed_list(&vendor);
        assert_eq!(
            verify(&lock_text(), &list, vendor.path()).unwrap(),
            Vec::new()
        );
    }

    #[test]
    fn r_sec_2__a_tampered_vendored_file_fails_the_verification() {
        let vendor = vendored();
        let list = committed_list(&vendor);
        vendor.write(
            "alpha-1.0.0/src/lib.rs",
            "pub fn alpha() { steal_the_key(); }\n",
        );
        assert_eq!(
            verify(&lock_text(), &list, vendor.path()).unwrap(),
            [Difference::Tree {
                package: "alpha 1.0.0".to_owned()
            }]
        );
    }

    #[test]
    fn r_sec_2__a_file_added_to_a_vendored_package_fails_the_verification() {
        let vendor = vendored();
        let list = committed_list(&vendor);
        vendor.write("beta-2.1.0+meta/build.rs", "fn main() {}\n");
        assert_eq!(
            verify(&lock_text(), &list, vendor.path()).unwrap(),
            [Difference::Tree {
                package: "beta 2.1.0+meta".to_owned()
            }]
        );
    }

    #[test]
    fn r_sec_2__a_file_removed_from_a_vendored_package_fails_the_verification() {
        let vendor = vendored();
        let list = committed_list(&vendor);
        std::fs::remove_file(vendor.path().join("alpha-1.0.0/src/lib.rs")).unwrap();
        assert_eq!(
            verify(&lock_text(), &list, vendor.path()).unwrap(),
            [Difference::Tree {
                package: "alpha 1.0.0".to_owned()
            }]
        );
    }

    #[test]
    fn r_sec_2__a_lockfile_checksum_that_the_list_does_not_have_fails_the_verification() {
        let vendor = vendored();
        let list = committed_list(&vendor);
        let changed = lock(&[
            ("alpha", "1.0.0", CHECKSUM_B),
            ("beta", "2.1.0+meta", CHECKSUM_B),
        ]);
        assert_eq!(
            verify(&changed, &list, vendor.path()).unwrap(),
            [Difference::Checksum {
                package: "alpha 1.0.0".to_owned()
            }]
        );
    }

    #[test]
    fn r_sec_2__a_package_added_to_the_lockfile_but_not_to_the_list_fails_the_verification() {
        let vendor = vendored();
        let list = committed_list(&vendor);
        vendor.write("gamma-0.1.0/Cargo.toml", "[package]\nname = \"gamma\"\n");
        let grown = lock(&[
            ("alpha", "1.0.0", CHECKSUM_A),
            ("beta", "2.1.0+meta", CHECKSUM_B),
            ("gamma", "0.1.0", CHECKSUM_A),
        ]);
        assert_eq!(
            verify(&grown, &list, vendor.path()).unwrap(),
            [Difference::Unexpected {
                package: "gamma 0.1.0".to_owned()
            }]
        );
    }

    #[test]
    fn r_sec_2__a_locked_package_that_was_not_vendored_cannot_be_hashed() {
        let vendor = vendored();
        std::fs::remove_dir_all(vendor.path().join("beta-2.1.0+meta")).unwrap();
        assert!(matches!(
            actual_list(&lock_text(), vendor.path()),
            Err(VendorError::Build(BuildError::NotVendored(_)))
        ));
    }

    #[test]
    fn r_sec_2__a_vendored_directory_that_the_lockfile_does_not_name_is_refused() {
        let vendor = vendored();
        vendor.write("sneaky-9.9.9/Cargo.toml", "[package]\nname = \"sneaky\"\n");
        assert!(matches!(
            actual_list(&lock_text(), vendor.path()),
            Err(VendorError::Unlisted(name)) if name == "sneaky-9.9.9"
        ));
    }

    #[test]
    fn r_sec_2__a_list_that_cannot_be_read_fails_the_verification() {
        let vendor = vendored();
        assert!(matches!(
            verify(&lock_text(), "not a hash list\n", vendor.path()),
            Err(VendorError::List(_))
        ));
    }

    #[test]
    fn r_sec_2__vendoring_a_workspace_without_dependencies_yields_an_empty_list() {
        let project = Scratch::new();
        project.write(
            "Cargo.toml",
            "[package]
name = \"bare\"
version = \"0.1.0\"
edition = \"2024\"
",
        );
        project.write("src/lib.rs", "");
        let lock = "version = 4

[[package]]
name = \"bare\"
version = \"0.1.0\"
";
        project.write("Cargo.lock", lock);
        let work = Scratch::new();
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
        let vendor_dir = regenerate(project.path(), &cargo, work.path()).unwrap();
        assert_eq!(actual_list(lock, &vendor_dir).unwrap().entries(), []);
    }

    #[test]
    fn r_sec_2__vendoring_against_a_stale_lockfile_fails() {
        let project = Scratch::new();
        project.write(
            "Cargo.toml",
            "[package]
name = \"bare\"
version = \"0.1.0\"
edition = \"2024\"
",
        );
        project.write("src/lib.rs", "");
        project.write(
            "Cargo.lock",
            "version = 4

[[package]]
name = \"bare\"
version = \"0.0.9\"
",
        );
        let work = Scratch::new();
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
        assert!(regenerate(project.path(), &cargo, work.path()).is_err());
    }
}
