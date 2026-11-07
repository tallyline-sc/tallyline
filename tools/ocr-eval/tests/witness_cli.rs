// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Explicit witness evaluation at the command-line boundary.

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ocr_20__explicit_witness_command_prints_only_safe_unavailable_output() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_ocr-eval"))
        .args([
            "witness",
            "--channel",
            "SYNTHETIC_PRIVATE_NONEXISTENT_CHANNEL",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        value,
        serde_json::json!({"state":"not_available","reason":"read_failure"})
    );
    assert!(!text.contains("SYNTHETIC_"));
    assert!(output.stderr.is_empty());
}
