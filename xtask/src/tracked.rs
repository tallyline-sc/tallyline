// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! The files of the repository that the licence and language checks read.

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::Path;
use std::process::Command;

use crate::guard::SourceFile;

/// The arguments after `git` that list the files Git tracks or would track, NUL-separated.
pub const LIST_ARGUMENTS: [&str; 5] = [
    "ls-files",
    "-z",
    "--cached",
    "--others",
    "--exclude-standard",
];

const SNIFF_BYTES: u64 = 8192;

const GIT_ENVIRONMENT: [&str; 17] = [
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_CONFIG",
    "GIT_CONFIG_PARAMETERS",
    "GIT_CONFIG_COUNT",
    "GIT_OBJECT_DIRECTORY",
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_IMPLICIT_WORK_TREE",
    "GIT_INTERNAL_SUPER_PREFIX",
    "GIT_GRAFT_FILE",
    "GIT_INDEX_FILE",
    "GIT_NO_REPLACE_OBJECTS",
    "GIT_REPLACE_REF_BASE",
    "GIT_PREFIX",
    "GIT_SHALLOW_FILE",
    "GIT_COMMON_DIR",
    "GIT_CEILING_DIRECTORIES",
];

/// Removes from `command` the repository-local `GIT_*` variables that a Git hook or
/// `git rebase --exec` exports, so a child process finds its repository from its own directory.
pub fn without_git_environment(command: &mut Command) -> &mut Command {
    for name in GIT_ENVIRONMENT {
        command.env_remove(name);
    }
    command
}

/// A `git` command that runs in `root` without the `GIT_*` variables a Git hook exports, so it
/// acts on the repository at `root` and never on the one that started the hook.
#[must_use]
pub fn git_command(root: &Path) -> Command {
    let mut command = Command::new("git");
    without_git_environment(&mut command).current_dir(root);
    command
}

/// Splits the NUL-separated output of `git ls-files -z` into sorted, distinct paths.
#[must_use]
pub fn parse_listing(output: &[u8]) -> Vec<String> {
    output
        .split(|&byte| byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| String::from_utf8_lossy(path).into_owned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// Lists the files Git tracks under `root`, plus new files that are not ignored.
///
/// # Errors
///
/// Returns an error when `git` cannot be run or exits unsuccessfully, for example outside a
/// repository.
pub fn paths(root: &Path) -> io::Result<Vec<String>> {
    let output = git_command(root).args(LIST_ARGUMENTS).output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "git {} failed: {}",
            LIST_ARGUMENTS.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(parse_listing(&output.stdout))
}

/// Reads the listed files that are valid UTF-8 text.
///
/// A listed path that no longer exists (deleted in the working tree), a directory (a submodule),
/// a file with a NUL byte in its first 8 KiB and a file that is not valid UTF-8 (an image, a
/// model) are skipped; the result keeps the order of `paths`.
///
/// # Errors
///
/// Returns an error when an existing file cannot be read.
pub fn read_text(root: &Path, paths: &[String]) -> io::Result<Vec<SourceFile>> {
    let mut files = Vec::new();
    for path in paths {
        let located = |error: io::Error| io::Error::new(error.kind(), format!("{path}: {error}"));
        let full = root.join(path);
        match fs::metadata(&full) {
            Ok(metadata) if metadata.is_dir() => continue,
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(located(error)),
        }
        let mut file = File::open(&full).map_err(located)?;
        let mut bytes = Vec::new();
        (&mut file)
            .take(SNIFF_BYTES)
            .read_to_end(&mut bytes)
            .map_err(located)?;
        if bytes.contains(&0) {
            continue;
        }
        file.read_to_end(&mut bytes).map_err(located)?;
        if let Ok(text) = String::from_utf8(bytes) {
            files.push(SourceFile {
                path: path.clone(),
                text,
            });
        }
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::process::Command;

    use super::{git_command, parse_listing, paths, read_text};
    use crate::scratch::Scratch;

    fn removed_variables(command: &Command) -> Vec<String> {
        command
            .get_envs()
            .filter(|(_, value)| value.is_none())
            .map(|(name, _)| name.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn a_git_command_drops_every_variable_git_calls_repository_local() {
        let output = Command::new("git")
            .args(["rev-parse", "--local-env-vars"])
            .output()
            .unwrap();
        assert!(output.status.success());
        let removed = removed_variables(&git_command(Path::new(".")));
        let local = String::from_utf8(output.stdout).unwrap();
        let inherited: Vec<&str> = local
            .lines()
            .filter(|name| !removed.iter().any(|removed| removed == name))
            .collect();
        assert!(inherited.is_empty(), "{inherited:?}");
    }

    #[test]
    fn the_listing_is_split_at_nul_sorted_and_deduplicated() {
        assert_eq!(
            parse_listing(b"b/c.rs\0a.md\0b/c.rs\0\0"),
            ["a.md", "b/c.rs"]
        );
    }

    #[test]
    fn an_empty_listing_has_no_paths() {
        assert_eq!(parse_listing(b""), Vec::<String>::new());
    }

    #[test]
    fn text_files_are_read_and_binary_or_missing_ones_are_skipped() {
        let scratch = Scratch::new();
        scratch.write("a.txt", "alpha\n");
        scratch.write("dir/b.txt", "beta\n");
        std::fs::write(scratch.path().join("image.png"), [0xff_u8, 0xd8, 0x00]).unwrap();
        let listed: Vec<String> = ["a.txt", "dir/b.txt", "gone.txt", "image.png"]
            .map(str::to_owned)
            .to_vec();
        let files = read_text(scratch.path(), &listed).unwrap();
        let read: Vec<(&str, &str)> = files
            .iter()
            .map(|file| (file.path.as_str(), file.text.as_str()))
            .collect();
        assert_eq!(read, [("a.txt", "alpha\n"), ("dir/b.txt", "beta\n")]);
    }

    #[test]
    fn git_lists_tracked_and_new_files_but_not_ignored_ones() {
        let scratch = Scratch::new();
        assert!(scratch.git(&["init", "--quiet"]).0);
        scratch.write(
            ".gitignore",
            "ignored.txt
",
        );
        scratch.write(
            "tracked.txt",
            "x
",
        );
        assert!(scratch.git(&["add", ".gitignore", "tracked.txt"]).0);
        scratch.write(
            "new.txt", "y
",
        );
        scratch.write(
            "ignored.txt",
            "z
",
        );
        assert_eq!(
            paths(scratch.path()).unwrap(),
            [".gitignore", "new.txt", "tracked.txt"]
        );
    }

    #[test]
    fn files_with_a_nul_byte_and_directories_are_skipped() {
        let scratch = Scratch::new();
        scratch.write(
            "text.txt", "alpha
",
        );
        std::fs::write(scratch.path().join("binary.bin"), b"ascii text\0more").unwrap();
        std::fs::create_dir(scratch.path().join("module")).unwrap();
        let listed: Vec<String> = ["binary.bin", "module", "text.txt"]
            .map(str::to_owned)
            .to_vec();
        let files = read_text(scratch.path(), &listed).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "text.txt");
    }

    #[test]
    fn listing_outside_a_repository_is_an_error() {
        let scratch = Scratch::new();
        let error = paths(scratch.path()).unwrap_err();
        assert!(error.to_string().contains("git ls-files"), "{error}");
    }
}
