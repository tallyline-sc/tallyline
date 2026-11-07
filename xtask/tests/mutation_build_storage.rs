// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Real public mutation commands retain workspace tests with bounded build settings.

#![cfg(test)]

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = env::temp_dir().join(format!("mutation-cli-{}-{unique}", std::process::id()));
        fs::create_dir(&root).unwrap();
        Self(root)
    }

    fn write(&self, relative: &str, contents: &str) {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    fn cargo(&self) -> Command {
        let mut command = Command::new(env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
        command
            .current_dir(self.0.join("workspace"))
            .env("CARGO_TARGET_DIR", self.0.join("build"))
            .env("CARGO_BUILD_BUILD_DIR", self.0.join("build"))
            .env("CARGO_TERM_COLOR", "never")
            .env("CARGO_NET_OFFLINE", "true");
        command
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if std::thread::panicking() {
            use std::io::Write;
            writeln!(
                std::io::stderr(),
                "retained failed mutation CLI fixture: {}",
                self.0.display()
            )
            .unwrap();
            return;
        }
        fs::remove_dir_all(&self.0).expect("remove owned mutation CLI fixture");
    }
}

fn successful(command: &mut Command) -> Output {
    let output = command.output().expect("run real Cargo command");
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn write_workspace(fixture: &Fixture) {
    fixture.write(
        "workspace/Cargo.toml",
        "[workspace]\nmembers = [\"cli\", \"core\", \"probe\"]\nresolver = \"3\"\n",
    );
    fixture.write(
        "workspace/.cargo/config.toml",
        "[alias]\nxtask = \"run --package mutation-cli --bin mutation-cli --locked --quiet --\"\n",
    );
    let xtask = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dependency = serde_json::to_string(xtask.to_str().unwrap()).unwrap();
    fixture.write("workspace/cli/Cargo.toml", &format!(
        "[package]\nname = \"mutation-cli\"\nversion = \"0.0.0\"\nedition = \"2024\"\n[dependencies]\nxtask = {{ path = {dependency} }}\nserde_json = {{ version = \"=1.0.154\", default-features = false, features = [\"std\", \"arbitrary_precision\"] }}\n[[bin]]\nname = \"rustc-recorder\"\npath = \"src/recorder.rs\"\n"));
    fixture.write(
        "workspace/cli/src/main.rs",
        &fs::read_to_string(xtask.join("src/main.rs")).unwrap(),
    );
    fixture.write("workspace/cli/src/recorder.rs", RECORDER);
    for (directory, package, test) in [
        (
            "core",
            "tallyline-shared-kernel",
            "core_settings_are_preserved",
        ),
        ("probe", "adapter-probe", "workspace_settings_are_preserved"),
    ] {
        fixture.write(
            &format!("workspace/{directory}/Cargo.toml"),
            &format!("[package]\nname = \"{package}\"\nversion = \"0.0.0\"\nedition = \"2024\"\n"),
        );
        let selected = if directory == "core" {
            "pub fn selected() -> bool { true }\npub fn chained(ready: bool, value: Option<()>) -> bool {\n    if ready\n        && let Some(()) = value\n    { true } else { false }\n}\n"
        } else {
            ""
        };
        let check = if directory == "core" {
            "assert!(super::selected());\nassert!(super::chained(true, Some(())));\nassert!(!super::chained(false, Some(())));\nassert!(!super::chained(true, None));\nassert!(!super::chained(false, None));"
        } else {
            ""
        };
        fixture.write(&format!("workspace/{directory}/src/lib.rs"), &format!(
            "{selected}\n#[cfg(test)]\nmod tests {{\n#[test]\nfn {test}() {{\n{check}\nassert!(cfg!(debug_assertions));\nlet root = std::env::var_os(\"MUTATION_PROBE_TESTS\").unwrap();\nstd::fs::write(std::path::Path::new(&root).join(\"{test}\"), \"passed\").unwrap();\n}}\n#[test]\n#[should_panic(expected = \"attempt to add with overflow\")]\nfn overflow_checks_are_preserved() {{\nlet _ = std::hint::black_box(u8::MAX) + 1;\n}}\n}}\n"));
    }
    fixture.write(
        "workspace/Cargo.lock",
        &fs::read_to_string(xtask.parent().unwrap().join("Cargo.lock")).unwrap(),
    );
    successful(
        fixture
            .cargo()
            .args(["metadata", "--offline", "--format-version", "1"]),
    );
    successful(fixture.cargo().args([
        "build",
        "--package",
        "mutation-cli",
        "--bins",
        "--locked",
        "--offline",
    ]));
}

const RECORDER: &str = r#"use std::env;
use std::fs;
use std::process::{Command, ExitCode};
fn main() -> ExitCode {
    let mut args = env::args_os().skip(1);
    let compiler = args.next().unwrap();
    let arguments: Vec<_> = args.collect();
    if let Some(directory) = env::var_os("MUTATION_PROBE_COMPILERS") {
        let record = arguments.iter().map(|arg| arg.to_string_lossy()).collect::<Vec<_>>().join("\n");
        fs::write(std::path::Path::new(&directory).join(format!("{}.args", std::process::id())), record).unwrap();
    }
    let status = Command::new(compiler).args(arguments).status().unwrap();
    ExitCode::from(u8::try_from(status.code().unwrap_or(1)).unwrap_or(1))
}
"#;

fn observed_command(fixture: &Fixture, label: &str) -> Command {
    let compilers = fixture.0.join(format!("{label}-compilers"));
    let tests = fixture.0.join(format!("{label}-tests"));
    fs::create_dir(&compilers).unwrap();
    fs::create_dir(&tests).unwrap();
    let recorder = fixture
        .0
        .join("build/debug")
        .join(format!("rustc-recorder{}", env::consts::EXE_SUFFIX));
    let mut command = fixture.cargo();
    command
        .env("RUSTC_WRAPPER", recorder)
        .env("MUTATION_PROBE_COMPILERS", compilers)
        .env("MUTATION_PROBE_TESTS", tests)
        .env("CARGO_PROFILE_DEV_DEBUG", "2")
        .env("CARGO_PROFILE_TEST_DEBUG", "2")
        .env("CARGO_INCREMENTAL", "1");
    command
}

fn compiler_records(fixture: &Fixture, label: &str) -> Vec<String> {
    fs::read_dir(fixture.0.join(format!("{label}-compilers")))
        .unwrap()
        .map(|entry| fs::read_to_string(entry.unwrap().path()).unwrap())
        .filter(|args| {
            args.lines()
                .any(|arg| matches!(arg, "tallyline_shared_kernel" | "adapter_probe"))
        })
        .collect()
}

fn assert_tests_ran(fixture: &Fixture, label: &str) {
    for test in [
        "core_settings_are_preserved",
        "workspace_settings_are_preserved",
    ] {
        assert_eq!(
            fs::read_to_string(fixture.0.join(format!("{label}-tests/{test}"))).unwrap(),
            "passed"
        );
    }
}

#[test]
fn public_mutation_bounds_build_storage_and_preserves_workspace_test_semantics() {
    let fixture = Fixture::new();
    write_workspace(&fixture);
    successful(observed_command(&fixture, "ordinary").args([
        "test",
        "--package",
        "tallyline-shared-kernel",
        "--package",
        "adapter-probe",
        "--locked",
        "--offline",
    ]));
    assert_tests_ran(&fixture, "ordinary");
    let ordinary = compiler_records(&fixture, "ordinary");
    assert!(!ordinary.is_empty());
    assert!(
        ordinary
            .iter()
            .all(|args| args.contains("debuginfo=2") && args.contains("incremental=")),
        "{ordinary:#?}"
    );

    let result = successful(observed_command(&fixture, "mutation").args(["xtask", "mutation"]));
    assert!(String::from_utf8_lossy(&result.stderr).contains("core mutation score: 100.00%"));
    assert_tests_ran(&fixture, "mutation");
    let mutation = compiler_records(&fixture, "mutation");
    assert!(
        mutation.iter().any(|args| args.contains("adapter_probe")),
        "{mutation:#?}"
    );
    assert!(
        mutation
            .iter()
            .all(|args| !args.contains("debuginfo=2") && !args.contains("incremental=")),
        "{mutation:#?}"
    );
    campaign::assert_constructed_partitions(&fixture);
}

#[path = "mutation_build_storage/campaign.rs"]
mod campaign;

#[path = "mutation_build_storage/uncoded.rs"]
mod uncoded;
