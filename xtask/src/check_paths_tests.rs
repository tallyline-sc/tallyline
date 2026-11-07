// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Real Cargo contracts for build-tool executable isolation.

use crate::scratch::Scratch;
use crate::work_paths::{OutputKind, WorkPaths};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};

struct RunningTool(Child);

impl Drop for RunningTool {
    fn drop(&mut self) {
        if self.0.try_wait().expect("inspect fixture tool").is_none() {
            self.0.kill().expect("stop fixture tool");
        }
        self.0.wait().expect("reap fixture tool");
    }
}

fn successful(command: &mut Command) {
    let output = command.output().expect("run Cargo fixture");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn write_workspace(fixture: &Scratch) {
    fixture.write(
        "workspace/Cargo.toml",
        "[workspace]\nmembers = [\"tool\", \"shared\", \"consumer\"]\nresolver = \"3\"\n",
    );
    fixture.write(
        "workspace/tool/Cargo.toml",
        "[package]\nname = \"lock-probe\"\nversion = \"0.0.0\"\nedition = \"2024\"\n[dependencies]\nshared = { path = \"../shared\" }\n",
    );
    fixture.write(
        "workspace/tool/src/main.rs",
        r#"use std::io;
fn main() {
    println!("{}", shared::value());
    if std::env::args().any(|arg| arg == "hold") {
        let mut line = String::new();
        io::stdin().read_line(&mut line).unwrap();
    }
}
"#,
    );
    fixture.write(
        "workspace/shared/Cargo.toml",
        "[package]\nname = \"shared\"\nversion = \"0.0.0\"\nedition = \"2024\"\n[features]\nwide = []\n",
    );
    fixture.write(
        "workspace/shared/src/lib.rs",
        "pub fn value() -> &'static str { if cfg!(feature = \"wide\") { \"wide\" } else { \"narrow\" } }\n",
    );
    fixture.write(
        "workspace/consumer/Cargo.toml",
        "[package]\nname = \"consumer\"\nversion = \"0.0.0\"\nedition = \"2024\"\n[dependencies]\nshared = { path = \"../shared\", features = [\"wide\"] }\n",
    );
    fixture.write("workspace/consumer/src/lib.rs", "pub use shared::value;\n");
    fixture.write(
        "workspace/Cargo.lock",
        "version = 4\n\n[[package]]\nname = \"consumer\"\nversion = \"0.0.0\"\ndependencies = [\"shared\"]\n\n[[package]]\nname = \"lock-probe\"\nversion = \"0.0.0\"\ndependencies = [\"shared\"]\n\n[[package]]\nname = \"shared\"\nversion = \"0.0.0\"\n",
    );
}

fn tool_value(executable: &Path) -> String {
    let output = Command::new(executable)
        .output()
        .expect("read tool feature");
    assert!(output.status.success());
    String::from_utf8(output.stdout).expect("fixture output")
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use the prescribed separator."
)]
fn r_sec_1__workspace_feature_unification_preserves_the_running_tool() {
    let fixture = Scratch::new();
    write_workspace(&fixture);
    let workspace = fixture.path().join("workspace");
    let target = fixture.path().join("external-build");
    let paths = WorkPaths::resolve(&workspace, &workspace, Some(target.as_os_str())).unwrap();
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    successful(
        paths
            .command(&cargo)
            .env("CARGO_BUILD_BUILD_DIR", &target)
            .args(["build", "--package", "lock-probe", "--locked", "--offline"]),
    );
    let executable = target
        .join("debug")
        .join(format!("lock-probe{}", std::env::consts::EXE_SUFFIX));
    let original = fs::read(&executable).unwrap();
    let mut running = RunningTool(
        Command::new(&executable)
            .arg("hold")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("start tool with narrow features"),
    );
    let mut ready = String::new();
    BufReader::new(running.0.stdout.take().unwrap())
        .read_line(&mut ready)
        .unwrap();
    assert_eq!(ready, "narrow\n");
    successful(paths.check_command(&cargo).unwrap().args([
        "build",
        "--workspace",
        "--locked",
        "--offline",
    ]));
    assert!(
        fs::read(&executable).unwrap() == original,
        "running tool executable changed"
    );
    assert_eq!(tool_value(&executable), "narrow\n");
    let metadata = paths
        .check_command(&cargo)
        .unwrap()
        .args(["metadata", "--locked", "--offline", "--format-version", "1"])
        .output()
        .unwrap();
    assert!(metadata.status.success());
    let metadata: serde_json::Value = serde_json::from_slice(&metadata.stdout).unwrap();
    let nested_target = Path::new(metadata["target_directory"].as_str().unwrap());
    let nested_build = Path::new(metadata["build_directory"].as_str().unwrap());
    assert_eq!(nested_target, nested_build);
    assert!(nested_target.starts_with(&target));
    assert_ne!(nested_target, target);
    let nested_executable = nested_target
        .join("debug")
        .join(format!("lock-probe{}", std::env::consts::EXE_SUFFIX));
    assert_eq!(tool_value(&nested_executable), "wide\n");
    assert!(!workspace.join("target").exists());
    running
        .0
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"done\n")
        .unwrap();
    assert!(running.0.wait().unwrap().success());
}

#[test]
fn check_commands_preserve_unowned_output() {
    let fixture = Scratch::new();
    let workspace = fixture.path().join("workspace");
    fs::create_dir(&workspace).unwrap();
    let paths = WorkPaths::resolve(
        &workspace,
        &workspace,
        Some(fixture.path().join("external-build").as_os_str()),
    )
    .unwrap();
    let output = paths.output(OutputKind::Check);
    fs::create_dir_all(&output).unwrap();
    fs::write(output.join("foreign"), "preserved").unwrap();
    assert!(paths.check_command("cargo".as_ref()).is_err());
    assert_eq!(
        fs::read_to_string(output.join("foreign")).unwrap(),
        "preserved"
    );
}
