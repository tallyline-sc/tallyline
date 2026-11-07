// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Contracts for external automatic tool outputs.

use crate::scratch::Scratch;
use crate::work_paths::{OutputKind, WorkPaths};
use std::fs;

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use the prescribed separator."
)]
fn r_sec_1__external_target_keeps_scratch_notices_and_mutation_outside_the_workspace() {
    let fixture = Scratch::new();
    let workspace = fixture.path().join("workspace");
    let external = fixture.path().join("external-build");
    fs::create_dir(&workspace).unwrap();
    let paths = WorkPaths::resolve(&workspace, &workspace, Some(external.as_os_str())).unwrap();
    for kind in [
        OutputKind::Check,
        OutputKind::Vendor,
        OutputKind::Notices,
        OutputKind::Mutation,
    ] {
        let output = paths.prepare(kind).unwrap();
        assert!(output.starts_with(&external), "{}", output.display());
        fs::create_dir_all(&output).unwrap();
        fs::write(output.join("receipt"), "external output").unwrap();
    }
    assert!(external.join("xtask-vendor/receipt").is_file());
    assert!(external.join("notices/receipt").is_file());
    assert!(external.join("core-mutation/receipt").is_file());
    assert!(!workspace.join("target").exists());
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use the prescribed separator."
)]
fn r_sec_1__relative_target_uses_the_invocation_directory_and_default_stays_in_the_workspace() {
    let fixture = Scratch::new();
    let workspace = fixture.path().join("workspace");
    let invocation = fixture.path().join("invocation");
    fs::create_dir(&workspace).unwrap();
    fs::create_dir(&invocation).unwrap();
    let paths =
        WorkPaths::resolve(&workspace, &invocation, Some("relative-build".as_ref())).unwrap();
    assert_eq!(paths.target_dir(), invocation.join("relative-build"));
    let defaults = WorkPaths::resolve(&workspace, &invocation, None).unwrap();
    assert_eq!(defaults.target_dir(), workspace.join("target"));
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use the prescribed separator."
)]
fn r_sec_1__unsafe_and_obstructed_targets_are_refused_without_writing() {
    let fixture = Scratch::new();
    let workspace = fixture.path().join("workspace");
    fs::create_dir(&workspace).unwrap();
    let file = fixture.path().join("file");
    fs::write(&file, "preserved").unwrap();
    for path in [
        std::path::PathBuf::new(),
        workspace.clone(),
        fixture.path().to_path_buf(),
        file.clone(),
        file.join("build"),
        workspace.join("build/.."),
    ] {
        assert!(
            WorkPaths::resolve(&workspace, &workspace, Some(path.as_os_str())).is_err(),
            "{}",
            path.display()
        );
    }
    assert_eq!(fs::read_to_string(file).unwrap(), "preserved");
    assert!(!workspace.join("target").exists());
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use the prescribed separator."
)]
fn r_sec_1__external_output_replacement_preserves_unowned_directories() {
    let fixture = Scratch::new();
    let workspace = fixture.path().join("workspace");
    let external = fixture.path().join("external-build");
    fs::create_dir(&workspace).unwrap();
    fs::create_dir_all(external.join("notices")).unwrap();
    fs::write(external.join("notices/foreign.txt"), "preserved").unwrap();
    let paths = WorkPaths::resolve(&workspace, &workspace, Some(external.as_os_str())).unwrap();
    assert!(paths.reset(OutputKind::Notices).is_err());
    assert_eq!(
        fs::read_to_string(external.join("notices/foreign.txt")).unwrap(),
        "preserved"
    );
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use the prescribed separator."
)]
fn r_sec_1__nested_cargo_preserves_the_original_relative_output_root() {
    let fixture = Scratch::new();
    fixture.write(
        "workspace/Cargo.toml",
        "[workspace]\n[package]\nname = \"output-probe\"\nversion = \"0.0.0\"\n[lib]\npath = \"empty.rs\"\n",
    );
    fixture.write("workspace/empty.rs", "");
    fixture.write(
        "workspace/Cargo.lock",
        "version = 4\n\n[[package]]\nname = \"output-probe\"\nversion = \"0.0.0\"\n",
    );
    let workspace = fixture.path().join("workspace");
    let invocation = fixture.path().join("invocation");
    fs::create_dir(&invocation).unwrap();
    let paths =
        WorkPaths::resolve(&workspace, &invocation, Some("relative-build".as_ref())).unwrap();
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = paths
        .command(&cargo)
        .args(["metadata", "--locked", "--no-deps", "--format-version", "1"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        std::path::Path::new(metadata["target_directory"].as_str().unwrap()),
        invocation.join("relative-build")
    );
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use the prescribed separator."
)]
fn r_sec_1__mutation_copies_have_independent_target_and_intermediate_directories() {
    let fixture = Scratch::new();
    let workspace = fixture.path().join("workspace");
    fs::create_dir(&workspace).unwrap();
    let external = fixture.path().join("external-build");
    let paths = WorkPaths::resolve(&workspace, &workspace, Some(external.as_os_str())).unwrap();
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    for name in ["copy-a", "copy-b"] {
        fixture.write(
            &format!("{name}/Cargo.toml"),
            "[workspace]\n[package]\nname = \"mutation-copy\"\nversion = \"0.0.0\"\n[lib]\npath = \"empty.rs\"\n",
        );
        fixture.write(&format!("{name}/empty.rs"), "");
        fixture.write(
            &format!("{name}/Cargo.lock"),
            "version = 4\n\n[[package]]\nname = \"mutation-copy\"\nversion = \"0.0.0\"\n",
        );
        let copied_root = fixture.path().join(name);
        let output = paths
            .mutation_command(&cargo)
            .current_dir(&copied_root)
            .args(["metadata", "--locked", "--no-deps", "--format-version", "1"])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let metadata: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        for field in ["target_directory", "build_directory"] {
            assert_eq!(
                std::path::Path::new(metadata[field].as_str().unwrap()),
                copied_root.join("target")
            );
        }
    }
    assert!(!external.exists());
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use the prescribed separator."
)]
fn r_sec_1__owned_output_is_reusable_but_another_workspace_cannot_replace_it() {
    let fixture = Scratch::new();
    let first = fixture.path().join("first");
    let second = fixture.path().join("second");
    let external = fixture.path().join("external-build");
    fs::create_dir(&first).unwrap();
    fs::create_dir(&second).unwrap();
    let owner = WorkPaths::resolve(&first, &first, Some(external.as_os_str())).unwrap();
    let output = owner.prepare(OutputKind::Notices).unwrap();
    fs::write(output.join("stale"), "owned").unwrap();
    owner.reset(OutputKind::Notices).unwrap();
    assert!(!output.join("stale").exists());
    fs::write(output.join("current"), "preserved").unwrap();
    let foreign = WorkPaths::resolve(&second, &second, Some(external.as_os_str())).unwrap();
    assert!(foreign.reset(OutputKind::Notices).is_err());
    assert!(foreign.remove(OutputKind::Notices).is_err());
    assert_eq!(
        fs::read_to_string(output.join("current")).unwrap(),
        "preserved"
    );
    owner.remove(OutputKind::Notices).unwrap();
    assert!(!output.exists());
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use the prescribed separator."
)]
fn r_sec_1__mutation_shards_refuse_traversal_and_write_under_the_external_output() {
    let fixture = Scratch::new();
    let workspace = fixture.path().join("workspace");
    let external = fixture.path().join("external-build");
    fs::create_dir(&workspace).unwrap();
    let paths = WorkPaths::resolve(&workspace, &workspace, Some(external.as_os_str())).unwrap();
    for name in ["", "..", "../escape", "nested/escape", "file:stream"] {
        assert!(
            paths
                .prepare_child(OutputKind::Mutation, name.as_ref())
                .is_err()
        );
    }
    let shard = paths
        .prepare_child(OutputKind::Mutation, "shard-0-of-4".as_ref())
        .unwrap();
    fs::write(shard.join("selected-mutants.json"), "[]").unwrap();
    assert_eq!(
        fs::read_to_string(external.join("core-mutation/shard-0-of-4/selected-mutants.json"))
            .unwrap(),
        "[]"
    );
    assert!(!workspace.join("target").exists());
    assert!(!fixture.path().join("escape").exists());
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use the prescribed separator."
)]
fn r_sec_1__notice_files_are_assembled_in_the_resolved_external_tree() {
    use crate::notices::{Assembly, Origin, PlannedFile, Roots, write_directory};
    let fixture = Scratch::new();
    fixture.write("workspace/LICENSE", "reviewed licence bytes");
    let workspace = fixture.path().join("workspace");
    let external = fixture.path().join("external-build");
    let paths = WorkPaths::resolve(&workspace, &workspace, Some(external.as_os_str())).unwrap();
    let legal = paths
        .reset(OutputKind::Notices)
        .unwrap()
        .join("windows/legal");
    let assembly = Assembly {
        copies: vec![PlannedFile {
            origin: Origin::Repository("LICENSE".to_owned()),
            target: "component/LICENSE".to_owned(),
        }],
        index: "crate\tlicence\n".to_owned(),
        findings: Vec::new(),
    };
    write_directory(
        &legal,
        &assembly,
        Roots {
            vendor: &workspace,
            toolchain: &workspace,
            repository: &workspace,
        },
    )
    .unwrap();
    assert_eq!(
        fs::read_to_string(external.join("notices/windows/legal/component/LICENSE")).unwrap(),
        "reviewed licence bytes"
    );
    assert_eq!(
        fs::read_to_string(legal.join("crates/index.tsv")).unwrap(),
        "crate\tlicence\n"
    );
    assert!(!workspace.join("target").exists());
}

#[cfg(windows)]
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use the prescribed separator."
)]
fn r_sec_1__windows_drive_relative_and_root_relative_targets_are_refused() {
    let fixture = Scratch::new();
    for target in ["D:relative-build", "\\relative-build", "D:\\"] {
        assert!(WorkPaths::resolve(fixture.path(), fixture.path(), Some(target.as_ref())).is_err());
    }
}

#[cfg(unix)]
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use the prescribed separator."
)]
fn r_sec_1__linked_output_roots_and_ancestors_are_refused() {
    let fixture = Scratch::new();
    let workspace = fixture.path().join("workspace");
    let foreign = fixture.path().join("foreign");
    fs::create_dir(&workspace).unwrap();
    fs::create_dir(&foreign).unwrap();
    let link = fixture.path().join("link");
    std::os::unix::fs::symlink(&foreign, &link).unwrap();
    for target in [link.clone(), link.join("child"), link.join("../build")] {
        assert!(WorkPaths::resolve(&workspace, &workspace, Some(target.as_os_str())).is_err());
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use the prescribed separator."
)]
fn r_sec_1__a_missing_workspace_cannot_claim_output_directories() {
    let fixture = Scratch::new();
    let missing = fixture.path().join("missing-workspace");
    let external = fixture.path().join("external-build");
    assert!(WorkPaths::resolve(&missing, fixture.path(), Some(external.as_os_str())).is_err());
    assert!(!missing.exists());
    assert!(!external.exists());
}
