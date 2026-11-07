// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Public packaging adapter contracts against independent archive readers.

#![expect(
    non_snake_case,
    reason = "requirement traceability uses double underscores"
)]

use std::process::Command;

#[test]
fn r_nf_2_q_installers__release_tags_have_one_canonical_msi_compatible_identity() {
    for (tag, expected) in [("v0.1.0", "0.1.0"), ("v255.255.65535", "255.255.65535")] {
        assert_eq!(crate::package::release_version(tag).unwrap(), expected);
    }
    for tag in [
        "0.1.0",
        "v01.2.3",
        "v1.02.3",
        "v1.2.03",
        "v+1.2.3",
        "v1.2.3.4",
        "v1.2.3-rc.1",
        "v1.2.3+build",
        "v256.0.0",
        "v0.256.0",
        "v0.0.65536",
        "v1.2.3\n",
    ] {
        assert!(
            crate::package::release_version(tag).is_err(),
            "accepted {tag:?}"
        );
    }
}

#[test]
fn r_nf_2_q_installers__archives_extract_with_an_independent_reader() {
    let root = std::env::temp_dir().join(format!("tallyline-package-cli-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let input = root.join("input");
    std::fs::create_dir_all(input.join("assets")).unwrap();
    std::fs::write(
        input.join("assets/licence.txt"),
        b"Independent archive fixture\n",
    )
    .unwrap();
    for format in ["zip", "tar.gz"] {
        let destination = root.join(format!("application.{format}"));
        crate::package::run(&[
            "archive".into(),
            format.into(),
            input.display().to_string(),
            destination.display().to_string(),
        ])
        .unwrap();
        let reader = if format == "zip" && !cfg!(windows) {
            "unzip"
        } else {
            "tar"
        };
        let extract_argument = if reader == "unzip" { "-p" } else { "-xOf" };
        let extracted = Command::new(reader)
            .arg(extract_argument)
            .arg(&destination)
            .arg("tallyline/assets/licence.txt")
            .output()
            .unwrap();
        assert!(extracted.status.success());
        assert_eq!(extracted.stdout, b"Independent archive fixture\n");
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn r_sec_8__unapproved_packaging_tools_are_rejected_without_execution() {
    let root = std::env::temp_dir().join(format!("tallyline-package-tools-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    for name in ["wix.exe", "cargo-packager"] {
        std::fs::write(root.join(name), b"unapproved tool fixture").unwrap();
    }
    for target in ["windows", "linux"] {
        let error = crate::package::run(&[
            "tools-verify".into(),
            target.into(),
            root.display().to_string(),
        ])
        .unwrap_err();
        assert!(error.to_string().contains("approved packaging tool"));
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn r_nf_4__token_scanning_catches_bare_and_wide_values_without_disclosing_them() {
    let root = std::env::temp_dir().join(format!("tallyline-package-scan-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let payload = root.join("payload");
    std::fs::create_dir(&payload).unwrap();
    let token = format!(
        "synthetic-opaque-{}-fixture-with+punctuation/and=padding",
        std::process::id()
    );
    let fixture = root.join("fixture");
    std::fs::write(&fixture, &token).unwrap();
    std::fs::write(payload.join("tallyline-app-token"), &token).unwrap();
    let scan = || {
        crate::package::run(&[
            "scan".into(),
            payload.display().to_string(),
            fixture.display().to_string(),
            "tallyline-app-token".into(),
        ])
    };
    assert!(scan().is_ok());
    for representation in [
        token.as_bytes().to_vec(),
        token.encode_utf16().flat_map(u16::to_le_bytes).collect(),
        token.encode_utf16().flat_map(u16::to_be_bytes).collect(),
    ] {
        std::fs::write(payload.join("intermediate.bin"), &representation).unwrap();
        let error = scan().unwrap_err().to_string();
        assert!(!error.contains(&token));
        assert!(error.contains("token boundary violation"));
        std::fs::remove_file(payload.join("intermediate.bin")).unwrap();
    }
    std::fs::remove_file(payload.join("tallyline-app-token")).unwrap();
    assert!(scan().is_err());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn r_nf_2_q_installers__archive_failures_leave_no_partial_or_replaced_output() {
    let root =
        std::env::temp_dir().join(format!("tallyline-package-failure-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let input = root.join("input");
    std::fs::create_dir(&input).unwrap();
    std::fs::write(input.join("a".repeat(101)), b"too-long USTAR basename").unwrap();
    let output = root.join("partial.tar.gz");
    assert!(crate::package::write_archive("tar.gz", &input, &output).is_err());
    assert!(!output.exists());
    std::fs::write(&output, b"existing artefact").unwrap();
    assert!(crate::package::write_archive("zip", &input, &output).is_err());
    assert_eq!(std::fs::read(&output).unwrap(), b"existing artefact");
    let nested = input.join("recursive.zip");
    assert!(crate::package::write_archive("zip", &input, &nested).is_err());
    assert!(!nested.exists());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[cfg(unix)]
fn r_nf_2_q_installers__linux_names_cannot_become_windows_parent_paths() {
    let root = std::env::temp_dir().join(format!("tallyline-package-path-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let input = root.join("input");
    std::fs::create_dir(&input).unwrap();
    std::fs::write(
        input.join("..\\escaped.txt"),
        b"must never escape the extraction root",
    )
    .unwrap();
    for format in ["zip", "tar.gz"] {
        let output = root.join(format!("application.{format}"));
        assert!(
            crate::package::write_archive(format, &input, &output).is_err(),
            "accepted a Windows parent-path entry"
        );
        assert!(!output.exists(), "left a rejected archive output");
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[cfg(unix)]
fn r_nf_2_q_installers__archives_refuse_case_aliases_before_windows_extraction() {
    let root = std::env::temp_dir().join(format!("tallyline-package-case-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let input = root.join("input");
    std::fs::create_dir(&input).unwrap();
    std::fs::write(input.join("NOTICE"), b"first file").unwrap();
    std::fs::write(input.join("notice"), b"second distinct Linux file").unwrap();
    for format in ["zip", "tar.gz"] {
        let output = root.join(format!("application.{format}"));
        assert!(
            crate::package::write_archive(format, &input, &output).is_err(),
            "accepted Windows case aliases"
        );
        assert!(!output.exists());
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[cfg(unix)]
fn r_nf_2_q_installers__archives_refuse_windows_device_and_normalised_names() {
    let root =
        std::env::temp_dir().join(format!("tallyline-package-device-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    for (index, name) in [
        "NUL.txt",
        "COM1",
        "LPT¹.log",
        "report.",
        "report ",
        "drive:note",
        "note?",
        "note\u{1}",
    ]
    .iter()
    .enumerate()
    {
        let input = root.join(format!("input-{index}"));
        std::fs::create_dir(&input).unwrap();
        std::fs::write(input.join(name), b"unsafe Windows name").unwrap();
        for format in ["zip", "tar.gz"] {
            let output = root.join(format!("application-{index}.{format}"));
            assert!(
                crate::package::write_archive(format, &input, &output).is_err(),
                "accepted reserved or normalised name"
            );
            assert!(!output.exists());
        }
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn r_sec_8_r_nf_2_q_installers__prepare_rejects_unapproved_tools_before_creating_a_stage() {
    let directory = crate::scratch::Scratch::new();
    let input = directory.path().join("input");
    let tools = directory.path().join("tools");
    let stage = directory.path().join("stage");
    std::fs::create_dir(&input).unwrap();
    std::fs::create_dir(&tools).unwrap();
    let (target, tool) = if cfg!(windows) {
        ("windows", "wix.exe")
    } else {
        ("linux", "cargo-packager")
    };
    std::fs::write(tools.join(tool), b"unapproved tool bytes").unwrap();
    let error = crate::package::run(&[
        "prepare".into(),
        target.into(),
        "v0.1.0".into(),
        input.display().to_string(),
        tools.display().to_string(),
        stage.display().to_string(),
    ])
    .unwrap_err();
    assert!(error.to_string().contains("approved packaging tool"));
    assert!(!stage.exists());
}

#[test]
fn r_doc_3__legal_inventory_binds_notice_bytes_and_refuses_extra_files() {
    let directory = crate::scratch::Scratch::new();
    directory.write(
        "legal/crates/fixture-1.0.0/LICENSE",
        "Known original licence fixture\n",
    );
    directory.write(
        "legal/crates/index.tsv",
        "crate\tversion\tdeclared\tdetected\nfixture\t1.0.0\tMIT\tMIT\n",
    );
    directory.write("legal/catalogue.json",r#"{"schema_version":1,"components":[{"name":"fixture","version":"1.0.0","declared_licence":"MIT","licence":"MIT","source":"https://example.invalid/fixture","copyright":{"status":"provided","notices":["Synthetic fixture author"]},"notices":[{"path":"crates/fixture-1.0.0/LICENSE","text":"Known original licence fixture\n"}]}]}"#);
    let arguments = [
        "legal-verify".into(),
        directory.path().display().to_string(),
    ];
    crate::package::run(&arguments).unwrap();
    let profile_error = crate::package::run(&[
        "legal-verify".into(),
        "windows".into(),
        directory.path().display().to_string(),
    ])
    .unwrap_err();
    assert!(
        profile_error
            .to_string()
            .contains("approved target legal catalogue")
    );
    directory.write(
        "legal/crates/fixture-1.0.0/LICENSE",
        "Changed attribution text\n",
    );
    assert!(crate::package::run(&arguments).is_err());
    directory.write(
        "legal/crates/fixture-1.0.0/LICENSE",
        "Known original licence fixture\n",
    );
    directory.write("legal/unapproved.exe", "unexpected binary-shaped payload");
    assert!(crate::package::run(&arguments).is_err());
}

#[test]
fn r_sec_8__a_wrong_seal_fails_before_any_tool_inventory_is_trusted() {
    let directory = crate::scratch::Scratch::new();
    directory.write("package-manifest.json", "{}");
    let error = crate::package::verify_stage(directory.path(), &"0".repeat(64)).unwrap_err();
    assert!(error.to_string().contains("seal mismatch"));
}

#[test]
fn r_doc_3__source_inventory_maps_real_unpacked_registry_source() {
    let directory = crate::scratch::Scratch::new();
    directory.write(
        "payload/legal/crates/fixture-1.0.0/LICENSE",
        "Known original licence fixture\n",
    );
    directory.write(
        "payload/legal/crates/index.tsv",
        "crate\tversion\tdeclared\tdetected\nfixture\t1.0.0\tMIT\tMIT\n",
    );
    directory.write("payload/legal/catalogue.json", r#"{"schema_version":1,"components":[{"name":"fixture","version":"1.0.0","declared_licence":"MIT","licence":"MIT","source":"https://example.invalid/fixture","copyright":{"status":"provided","notices":["Synthetic fixture author"]},"notices":[{"path":"crates/fixture-1.0.0/LICENSE","text":"Known original licence fixture\n"}]}]}"#);
    directory.write("source/vendor/fixture-1.0.0/src/lib.rs", "abc");
    let archive = directory.path().join("source.tar.gz");
    crate::package::write_archive("source.tar.gz", &directory.path().join("source"), &archive)
        .unwrap();
    let map = directory.path().join("source-map.json");
    crate::package::run(&[
        "source-map".into(),
        directory.path().join("payload").display().to_string(),
        directory.path().join("source").display().to_string(),
        archive.display().to_string(),
        map.display().to_string(),
    ])
    .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(map).unwrap()).unwrap();
    assert_eq!(
        value["components"][0]["files"][0]["path"],
        "vendor/fixture-1.0.0/src/lib.rs"
    );
    assert_eq!(
        value["components"][0]["files"][0]["sha256"],
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    let unpacked = Command::new("tar")
        .current_dir(directory.path())
        .args(["-xOf"])
        .arg("source.tar.gz")
        .arg("tallyline/vendor/fixture-1.0.0/src/lib.rs")
        .output()
        .unwrap();
    assert!(
        unpacked.status.success(),
        "{}",
        String::from_utf8_lossy(&unpacked.stderr)
    );
    assert_eq!(unpacked.stdout, b"abc");
    let map_bytes = std::fs::read(directory.path().join("source-map.json")).unwrap();
    std::fs::write(
        directory.path().join("payload/legal/source-map.json"),
        &map_bytes,
    )
    .unwrap();
    let verify_arguments = [
        "source-verify".into(),
        directory.path().join("payload").display().to_string(),
        archive.display().to_string(),
    ];
    crate::package::run(&verify_arguments).unwrap();
    let mut changed_map: serde_json::Value = serde_json::from_slice(&map_bytes).unwrap();
    changed_map["archive"]["sha256"] = "0".repeat(64).into();
    std::fs::write(
        directory.path().join("payload/legal/source-map.json"),
        serde_json::to_vec(&changed_map).unwrap(),
    )
    .unwrap();
    assert!(crate::package::run(&verify_arguments).is_err());
    for (field, value) in [
        ("path", "vendor/fixture-1.0.0/src/missing.rs".to_owned()),
        ("sha256", "0".repeat(64)),
    ] {
        let mut incorrect: serde_json::Value = serde_json::from_slice(&map_bytes).unwrap();
        incorrect["components"][0]["files"][0][field] = value.into();
        std::fs::write(
            directory.path().join("payload/legal/source-map.json"),
            serde_json::to_vec(&incorrect).unwrap(),
        )
        .unwrap();
        assert!(
            crate::package::run(&verify_arguments).is_err(),
            "source map file evidence differs from actual archive"
        );
    }
    std::fs::write(
        directory.path().join("payload/legal/source-map.json"),
        map_bytes,
    )
    .unwrap();
    directory.write(
        "source/vendor/fixture-1.0.0/src/lib.rs",
        "changed after archive freeze",
    );
    let mismatched = directory.path().join("mismatched-source-map.json");
    assert!(
        crate::package::run(&[
            "source-map".into(),
            directory.path().join("payload").display().to_string(),
            directory.path().join("source").display().to_string(),
            archive.display().to_string(),
            mismatched.display().to_string(),
        ])
        .is_err(),
        "source archive and unpacked source mismatch was accepted"
    );
    assert!(!mismatched.exists());
}

#[test]
fn r_doc_3_r_nf_2_q_installers__source_archives_have_data_only_file_permissions() {
    let directory = crate::scratch::Scratch::new();
    directory.write("source/source.exe", "abc");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            directory.path().join("source/source.exe"),
            std::fs::Permissions::from_mode(0o777),
        )
        .unwrap();
    }
    crate::package::write_archive(
        "source.tar.gz",
        &directory.path().join("source"),
        &directory.path().join("source.tar.gz"),
    )
    .unwrap();
    let listing = Command::new("tar")
        .current_dir(directory.path())
        .args(["-tvf", "source.tar.gz"])
        .output()
        .unwrap();
    assert!(
        listing.status.success(),
        "{}",
        String::from_utf8_lossy(&listing.stderr)
    );
    let listing = String::from_utf8(listing.stdout).unwrap();
    let source = listing
        .lines()
        .find(|line| line.ends_with("tallyline/source.exe"))
        .unwrap();
    assert!(source.starts_with("-rw-r--r--"), "{source}");
}

#[test]
fn r_doc_3__native_source_scopes_require_the_exact_reviewed_identity() {
    let directory = crate::scratch::Scratch::new();
    directory.write("payload/legal/sqlite/LICENSE", "Synthetic native notice\n");
    directory.write(
        "payload/legal/crates/index.tsv",
        "crate\tversion\tdeclared\tdetected\n",
    );
    let catalogue = r#"{"schema_version":1,"components":[{"name":"SQLite","version":"3.54.1","licence":"blessing","source":"https://www.sqlite.org/2026/sqlite-amalgamation-3540100.zip","copyright":{"status":"provided","notices":["Synthetic fixture"]},"notices":[{"path":"sqlite/LICENSE","text":"Synthetic native notice\n"}]}]}"#;
    directory.write("payload/legal/catalogue.json", catalogue);
    directory.write(
        "source/project/supply-chain/notices/sqlite/NOTICE",
        "Synthetic native notice\n",
    );
    directory.write(
        "source/vendor/libsqlite3-sys-0.39.1/sqlite3/sqlite3.c",
        "abc",
    );
    let archive = directory.path().join("source.tar.gz");
    crate::package::write_archive("source.tar.gz", &directory.path().join("source"), &archive)
        .unwrap();
    let map = directory.path().join("source-map.json");
    let mut arguments = vec![
        "source-map".into(),
        directory.path().join("payload").display().to_string(),
        directory.path().join("source").display().to_string(),
        archive.display().to_string(),
        map.display().to_string(),
    ];
    crate::package::run(&arguments).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(map).unwrap()).unwrap();
    assert_eq!(
        value["components"][0]["patterns"][0],
        "vendor/libsqlite3-sys-0.39.1/sqlite3/*"
    );
    directory.write(
        "payload/legal/catalogue.json",
        &catalogue.replace("3.54.1", "3.55.0"),
    );
    let unreviewed = directory.path().join("unreviewed-source-map.json");
    *arguments.last_mut().unwrap() = unreviewed.display().to_string();
    assert!(crate::package::run(&arguments).is_err());
    assert!(!unreviewed.exists());
}

#[test]
fn r_sec_8__protected_commands_reject_a_bad_seal_before_fixture_access() {
    let directory = crate::scratch::Scratch::new();
    directory.write("package-manifest.json", "{}");
    let missing = directory.path().join("fixture-does-not-exist");
    for command in ["inject", "verify-injected"] {
        let error = crate::package::run(&[
            command.into(),
            directory.path().display().to_string(),
            "0".repeat(64),
            missing.display().to_string(),
        ])
        .unwrap_err();
        assert!(error.to_string().contains("seal mismatch"), "{error}");
    }
    assert!(
        !directory
            .path()
            .join("payload/tallyline-app-token")
            .exists()
    );
}

#[test]
fn r_doc_3_r_sec_8__source_bindings_detect_graph_neutral_lockfile_byte_changes() {
    let directory = crate::scratch::Scratch::new();
    let root = copy_bound_source_inputs(&directory);
    let arguments = [
        "source-bindings".into(),
        directory.path().display().to_string(),
    ];
    crate::package::run(&arguments).unwrap();
    let lock = std::fs::read_to_string(root.join("Cargo.lock")).unwrap();
    directory.write(
        "Cargo.lock",
        &(lock + "\n# Synthetic graph-neutral source-binding change\n"),
    );
    let error = crate::package::run(&arguments).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("stale packaging legal source binding: Cargo.lock")
    );
}

#[test]
fn r_doc_3_r_sec_8__source_bindings_reject_manifest_only_feature_changes() {
    let directory = crate::scratch::Scratch::new();
    let root = copy_bound_source_inputs(&directory);
    let arguments = [
        "source-bindings".into(),
        directory.path().display().to_string(),
    ];
    crate::package::run(&arguments).unwrap();
    let original = std::fs::read_to_string(root.join("crates/app/Cargo.toml")).unwrap();
    let changed = original.replace(", \"rt-multi-thread\"", "");
    assert_ne!(original, changed);
    directory.write("crates/app/Cargo.toml", &changed);
    let error = crate::package::run(&arguments).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("stale packaging legal source binding: crates/app/Cargo.toml"),
        "{error}"
    );
    assert_eq!(
        std::fs::read(root.join("Cargo.lock")).unwrap(),
        std::fs::read(directory.path().join("Cargo.lock")).unwrap()
    );
}

fn copy_bound_source_inputs(directory: &crate::scratch::Scratch) -> std::path::PathBuf {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let profile: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("supply-chain/package-legal-profiles.json")).unwrap(),
    )
    .unwrap();
    let mut names = profile["source_inputs"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    names.extend(["Cargo.toml", "crates/app/Cargo.toml"]);
    for name in names {
        directory.write(name, &std::fs::read_to_string(root.join(name)).unwrap());
    }
    root.to_path_buf()
}

#[test]
fn r_sec_8_r_nf_4__tool_capture_discards_both_streams_before_persisting_a_fixture_leak() {
    let directory = crate::scratch::Scratch::new();
    directory.write("fixture", "capture-contract-harmless-value-918273645");
    let logs = directory.path().join("logs");
    std::fs::create_dir(&logs).unwrap();
    let fixture = std::fs::read(directory.path().join("fixture")).unwrap();
    let text = std::str::from_utf8(&fixture).unwrap();
    let wide = text
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let error = crate::package::retain_tool_output(
        &logs,
        "tool",
        &wide,
        b"innocent stderr",
        &directory.path().join("fixture"),
    )
    .unwrap_err();
    assert!(error.to_string().contains("not retained"));
    for name in ["tool.stdout", "tool.stderr"] {
        let retained = std::fs::read(logs.join(name)).unwrap();
        assert!(
            !retained
                .windows(fixture.len())
                .any(|bytes| bytes == fixture)
        );
        assert!(!retained.windows(wide.len()).any(|bytes| bytes == wide));
        assert!(
            std::str::from_utf8(&retained)
                .unwrap()
                .contains("not retained")
        );
    }
}

#[test]
fn r_doc_3__debian_copyright_preserves_terms_and_reviewed_source_scopes() {
    let directory = crate::scratch::Scratch::new();
    directory.write("payload/LICENSE", "Synthetic project terms\n");
    directory.write(
        "payload/legal/sqlite/LICENSE",
        "Synthetic native terms\n\nFinal preserved line.\n",
    );
    directory.write(
        "payload/legal/crates/index.tsv",
        "crate\tversion\tdeclared\tdetected\n",
    );
    directory.write("payload/legal/catalogue.json", r#"{"schema_version":1,"components":[{"name":"SQLite","version":"3.54.1","licence":"blessing","source":"https://www.sqlite.org/2026/sqlite-amalgamation-3540100.zip","copyright":{"status":"provided","notices":["Synthetic fixture"]},"notices":[{"path":"sqlite/LICENSE","text":"Synthetic native terms\n\nFinal preserved line.\n"}]}]}"#);
    let output = directory.path().join("copyright");
    crate::package::run(&[
        "copyright".into(),
        directory.path().join("payload").display().to_string(),
        output.display().to_string(),
    ])
    .unwrap();
    let inspected = std::process::Command::new(if cfg!(windows) { "python" } else { "python3" }).args(["-c", "import sys,email.parser,json; text=open(sys.argv[1],encoding='utf-8').read(); rows=[dict(email.parser.Parser().parsestr(block).items()) for block in text.strip().split('\\n\\n')]; print(json.dumps(rows))"]).arg(&output).output().unwrap();
    assert!(
        inspected.status.success(),
        "{}",
        String::from_utf8_lossy(&inspected.stderr)
    );
    let rows: serde_json::Value = serde_json::from_slice(&inspected.stdout).unwrap();
    let rows = rows.as_array().unwrap();
    assert_eq!(
        rows[0]["Format"],
        "https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/"
    );
    assert_eq!(
        rows[0]["Source"],
        "https://github.com/tallyline-sc/tallyline"
    );
    let native = rows
        .iter()
        .find(|row| {
            row["Files"]
                .as_str()
                .is_some_and(|paths| paths.contains("vendor/libsqlite3-sys-0.39.1/sqlite3/*"))
        })
        .unwrap();
    assert!(
        native["License"]
            .as_str()
            .unwrap()
            .contains("Synthetic native terms\n .\n Final preserved line.")
    );
    assert!(
        native["Copyright"]
            .as_str()
            .unwrap()
            .contains("Synthetic fixture")
    );
}

#[test]
fn r_nf_2_q_installers__debian_archive_has_safe_layout_modes_and_no_maintainer_scripts() {
    let directory = crate::scratch::Scratch::new();
    directory.write(
        "tree/control/control",
        "Package: tallyline\nVersion: 1.2.3\nArchitecture: amd64\nInstalled-Size: 1\n",
    );
    directory.write("tree/control/md5sums", "900150983cd24fb0d6963f7d28e17f72  usr/bin/tallyline\n900150983cd24fb0d6963f7d28e17f72  usr/lib/tallyline/libonnxruntime.so.1.31.0\n");
    directory.write("tree/data/usr/bin/tallyline", "abc");
    directory.write(
        "tree/data/usr/lib/tallyline/libonnxruntime.so.1.31.0",
        "abc",
    );
    let output = directory.path().join("package.deb");
    crate::package::run(&[
        "repack-deb".into(),
        directory.path().join("tree").display().to_string(),
        output.display().to_string(),
    ])
    .unwrap();
    let script = r#"import sys,io,tarfile,json,hashlib
raw=open(sys.argv[1],'rb').read(); assert raw[:8]==b'!<arch>\n'; offset=8; parts={}
while offset<len(raw):
 h=raw[offset:offset+60]; assert len(h)==60 and h[58:]==b'`\n'; name=h[:16].decode().strip().rstrip('/'); size=int(h[48:58]); assert name not in parts; parts[name]=raw[offset+60:offset+60+size]; offset+=60+size+(size%2)
assert list(parts)==['debian-binary','control.tar.gz','data.tar.gz'] and parts['debian-binary']==b'2.0\n'
with tarfile.open(fileobj=io.BytesIO(parts['data.tar.gz']),mode='r:gz') as tar:
 rows={item.name:item for item in tar}; binary=rows['usr/lib/tallyline/tallyline']; runtime=rows['usr/lib/tallyline/libonnxruntime.so.1.31.0']; link=rows['usr/bin/tallyline']; assert binary.mode==runtime.mode==0o755; assert link.issym() and link.linkname=='../lib/tallyline/tallyline'; assert tar.extractfile(binary).read()==b'abc'
 for item in rows.values(): assert item.uid==item.gid==item.mtime==0
with tarfile.open(fileobj=io.BytesIO(parts['control.tar.gz']),mode='r:gz') as tar:
 assert {item.name for item in tar}=={'control','md5sums'}; sums=tar.extractfile('md5sums').read().decode(); assert 'usr/bin/tallyline' not in sums; assert '900150983cd24fb0d6963f7d28e17f72  usr/lib/tallyline/tallyline' in sums
print('independent Debian archive inspection passed')"#;
    let inspected = std::process::Command::new(if cfg!(windows) { "python" } else { "python3" })
        .args(["-c", script])
        .arg(output)
        .output()
        .unwrap();
    assert!(
        inspected.status.success(),
        "{}",
        String::from_utf8_lossy(&inspected.stderr)
    );
}

#[test]
fn r_sec_8__protected_build_rejects_a_bad_seal_before_fixture_or_output_access() {
    let directory = crate::scratch::Scratch::new();
    directory.write("stage/package-manifest.json", "{}");
    let output = directory.path().join("output");
    let error = crate::package::run(&[
        "build".into(),
        directory.path().join("stage").display().to_string(),
        "0".repeat(64),
        directory
            .path()
            .join("missing-fixture")
            .display()
            .to_string(),
        output.display().to_string(),
    ])
    .unwrap_err();
    assert!(error.to_string().contains("seal mismatch"), "{error}");
    assert!(!output.exists());
}

#[test]
fn r_nf_13__msi_prerequisite_refusal_names_the_runtime_floor_and_microsoft_download_page() {
    let authoring = crate::package::msi_prerequisite_authoring().unwrap();
    let launches = authoring.split("<Launch ").skip(1).collect::<Vec<_>>();
    assert_eq!(launches.len(), 361);
    for launch in launches {
        assert!(launch.contains("Microsoft Visual C++ Redistributable x64 version 14.44 or newer"));
        assert!(
            launch.contains(
                "https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist/"
            )
        );
    }
}

#[test]
fn r_nf_11_r_sec_8__prebuilt_identity_admits_only_the_sealed_executable_and_tag() {
    let directory = crate::scratch::Scratch::new();
    directory.write("app", "Harmless prebuilt fixture; never executed.\n");
    let receipt = serde_json::json!({
        "schema_version": 1,
        "purpose": "release-metadata",
        "application_sha256": "ff7109af899382a34b99be3608b5cc7aee084dac41033e7b59a48a4de2e1e3a7",
        "build_info": {
            "schema_version": 1,
            "product": "tallyline",
            "kind": "release",
            "version": "9.8.7",
            "release_tag": "v9.8.7",
            "source_commit": "0123456789abcdef0123456789abcdef01234567",
            "target": "x86_64-pc-windows-msvc"
        }
    });
    let bytes = serde_json::to_vec_pretty(&receipt).unwrap();
    std::fs::write(directory.path().join("receipt.json"), &bytes).unwrap();
    let seal = crate::hashlist::sha256_hex(&bytes);
    let check = |tag| {
        crate::package::verify_prebuilt_identity(
            &directory.path().join("app"),
            &directory.path().join("receipt.json"),
            &seal,
            crate::package::Target::Windows,
            tag,
        )
    };
    check("v9.8.7").unwrap();
    crate::package::run(&[
        "identity-verify".into(),
        "windows".into(),
        "v9.8.7".into(),
        directory.path().join("app").display().to_string(),
        directory.path().join("receipt.json").display().to_string(),
        seal.clone(),
    ])
    .unwrap();
    assert!(check("v0.1.0").is_err());
    directory.write("app", "Changed prebuilt executable bytes");
    assert!(check("v9.8.7").is_err());
}

#[test]
fn r_nf_11_r_sec_8__source_job_receipt_is_data_only_and_closes_before_invalid_output() {
    let directory = crate::scratch::Scratch::new();
    directory.write("app", "Harmless prebuilt fixture; never executed.\n");
    directory.write("compiled.json", r#"{"schema_version":1,"product":"tallyline","kind":"release","version":"9.8.7","release_tag":"v9.8.7","source_commit":"0123456789abcdef0123456789abcdef01234567","target":"x86_64-pc-windows-msvc"}"#);
    let app = directory.path().join("app");
    let report = directory.path().join("compiled.json");
    let receipt = directory.path().join("receipt.json");
    let seal = crate::package::write_prebuilt_identity(
        &app,
        &report,
        &receipt,
        crate::package::Target::Windows,
        "v9.8.7",
    )
    .unwrap();
    crate::package::verify_prebuilt_identity(
        &app,
        &receipt,
        &seal,
        crate::package::Target::Windows,
        "v9.8.7",
    )
    .unwrap();
    let value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&receipt).unwrap()).unwrap();
    assert_eq!(
        value["application_sha256"],
        "ff7109af899382a34b99be3608b5cc7aee084dac41033e7b59a48a4de2e1e3a7"
    );
    assert_eq!(value["purpose"], "release-metadata");
    crate::package::run(&[
        "identity-receipt".into(),
        "windows".into(),
        "v9.8.7".into(),
        app.display().to_string(),
        report.display().to_string(),
        directory
            .path()
            .join("cli-receipt.json")
            .display()
            .to_string(),
    ])
    .unwrap();
    assert_eq!(
        std::fs::read(directory.path().join("cli-receipt.json")).unwrap(),
        std::fs::read(&receipt).unwrap()
    );
    directory.write("compiled.json", r#"{"schema_version":1,"product":"tallyline","kind":"release","version":"9.8.7","release_tag":"v9.8.7","source_commit":null,"target":"x86_64-pc-windows-msvc"}"#);
    let refused = directory.path().join("refused.json");
    assert!(
        crate::package::write_prebuilt_identity(
            &app,
            &report,
            &refused,
            crate::package::Target::Windows,
            "v9.8.7"
        )
        .is_err()
    );
    assert!(!refused.exists());
}

#[test]
fn r_nf_11_r_sec_8__admitted_preparation_checks_app_identity_before_tools_or_output() {
    let directory = crate::scratch::Scratch::new();
    directory.write(
        "payload/tallyline.exe",
        "Harmless prebuilt fixture; never executed.\n",
    );
    directory.write("compiled.json", r#"{"schema_version":1,"product":"tallyline","kind":"release","version":"9.8.7","release_tag":"v9.8.7","source_commit":"0123456789abcdef0123456789abcdef01234567","target":"x86_64-pc-windows-msvc"}"#);
    let app = directory.path().join("payload/tallyline.exe");
    let receipt = directory.path().join("receipt.json");
    let seal = crate::package::write_prebuilt_identity(
        &app,
        &directory.path().join("compiled.json"),
        &receipt,
        crate::package::Target::Windows,
        "v9.8.7",
    )
    .unwrap();
    directory.write(
        "payload/tallyline.exe",
        "Changed after source-job admission",
    );
    let stage = directory.path().join("stage");
    let error = crate::package::run(&[
        "prepare-admitted".into(),
        "windows".into(),
        "v9.8.7".into(),
        directory.path().join("payload").display().to_string(),
        directory.path().join("absent-tools").display().to_string(),
        stage.display().to_string(),
        receipt.display().to_string(),
        seal,
    ])
    .unwrap_err();
    assert!(
        error.to_string().contains("prebuilt executable differs"),
        "{error}"
    );
    assert!(!stage.exists());
}

#[test]
fn r_nf_11_r_sec_8__resealed_malformed_receipts_do_not_bypass_intrinsic_admission() {
    let directory = crate::scratch::Scratch::new();
    directory.write("app", "Harmless prebuilt fixture; never executed.\n");
    let valid = serde_json::json!({"schema_version":1,"purpose":"release-metadata","application_sha256":"ff7109af899382a34b99be3608b5cc7aee084dac41033e7b59a48a4de2e1e3a7","build_info":{"schema_version":1,"product":"tallyline","kind":"release","version":"9.8.7","release_tag":"v9.8.7","source_commit":"0123456789abcdef0123456789abcdef01234567","target":"x86_64-pc-windows-msvc"}});
    for (path, wrong) in [
        (vec!["purpose"], serde_json::json!("authenticated-release")),
        (
            vec!["application_sha256"],
            serde_json::json!("FF7109AF899382A34B99BE3608B5CC7AEE084DAC41033E7B59A48A4DE2E1E3A7"),
        ),
        (vec!["extra"], serde_json::json!(true)),
        (
            vec!["build_info", "target"],
            serde_json::json!("x86_64-unknown-linux-gnu"),
        ),
        (
            vec!["build_info", "release_tag"],
            serde_json::json!("v0.1.0"),
        ),
        (vec!["build_info", "source_commit"], serde_json::Value::Null),
        (vec!["build_info", "kind"], serde_json::json!("development")),
        (vec!["build_info", "schema_version"], serde_json::json!(2)),
    ] {
        let mut value = valid.clone();
        let mut field = &mut value;
        for part in path {
            field = &mut field[part];
        }
        *field = wrong;
        let bytes = serde_json::to_vec_pretty(&value).unwrap();
        std::fs::write(directory.path().join("receipt.json"), &bytes).unwrap();
        assert!(
            crate::package::verify_prebuilt_identity(
                &directory.path().join("app"),
                &directory.path().join("receipt.json"),
                &crate::hashlist::sha256_hex(&bytes),
                crate::package::Target::Windows,
                "v9.8.7"
            )
            .is_err()
        );
    }
    let mut bytes = serde_json::to_vec_pretty(&valid).unwrap();
    bytes.push(b'\n');
    std::fs::write(directory.path().join("receipt.json"), &bytes).unwrap();
    assert!(
        crate::package::verify_prebuilt_identity(
            &directory.path().join("app"),
            &directory.path().join("receipt.json"),
            &crate::hashlist::sha256_hex(&bytes),
            crate::package::Target::Windows,
            "v9.8.7"
        )
        .is_err()
    );
}

#[test]
fn r_nf_11__development_admission_is_explicit_and_cannot_claim_a_release_tag() {
    let directory = crate::scratch::Scratch::new();
    directory.write("app", "Harmless prebuilt fixture; never executed.\n");
    directory.write("compiled.json", r#"{"schema_version":1,"product":"tallyline","kind":"development","version":"0.0.0","release_tag":null,"source_commit":null,"target":"x86_64-pc-windows-msvc"}"#);
    let app = directory.path().join("app");
    let report = directory.path().join("compiled.json");
    let receipt = directory.path().join("receipt.json");
    let seal = crate::package::write_prebuilt_identity(
        &app,
        &report,
        &receipt,
        crate::package::Target::Windows,
        "v0.0.0",
    )
    .unwrap();
    crate::package::verify_prebuilt_identity(
        &app,
        &receipt,
        &seal,
        crate::package::Target::Windows,
        "v0.0.0",
    )
    .unwrap();
    assert!(
        crate::package::verify_prebuilt_identity(
            &app,
            &receipt,
            &seal,
            crate::package::Target::Windows,
            "v0.1.0"
        )
        .is_err()
    );
    let value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(receipt).unwrap()).unwrap();
    assert_eq!(value["purpose"], "development");
    assert!(value["build_info"]["release_tag"].is_null());
}
