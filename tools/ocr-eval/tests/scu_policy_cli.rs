// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Read-only SCU policy research at the command-line boundary.

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_2_q_manual__explicit_policy_research_counts_complete_quantities_without_promoting_annotations()
 {
    let root = std::env::temp_dir().join(format!("tallyline-scu-policy-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let entry = root.join("fixture");
    std::fs::create_dir(&entry).unwrap();
    let expected = entry.join("expected.json");
    let source = br#"{"schemaVersion":2,"verified":false,"captures":{"one.jpg":{"cards":[{"scu":2},{"scu":86},{"scu":70},{"scu":0},{"scu":86},{"partial":true,"scu":999},{"scu":null}]}}}"#;
    std::fs::write(&expected, source).unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_ocr-eval"))
        .arg("scu-policy")
        .arg("--public")
        .arg(&root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["quantity_rows"], 5);
    assert_eq!(value["distinct_quantities"], 4);
    assert_eq!(value["excluded_partial"], 1);
    assert_eq!(value["missing_quantity"], 1);
    assert_eq!(value["verified_entries"], 0);
    assert_eq!(value["recognition"], "not_run");
    assert_eq!(value["accuracy"], "not_measured");
    let candidates = value["candidates"].as_array().unwrap();
    assert_eq!(candidates.len(), 30);
    let twenty = candidates
        .iter()
        .find(|candidate| candidate["percent"] == 20 && candidate["minimum_scu"] == 16)
        .unwrap();
    assert_eq!(
        twenty["row_weighted"]["delete_one_digit"],
        serde_json::json!({"cases":6,"major":6})
    );
    assert_eq!(
        twenty["row_weighted"]["insert_one_digit"],
        serde_json::json!({"cases":117,"major":106})
    );
    assert_eq!(
        twenty["row_weighted"]["transpose_adjacent"],
        serde_json::json!({"cases":3,"major":3})
    );
    assert_eq!(
        twenty["distinct_quantity"]["insert_one_digit"],
        serde_json::json!({"cases":90,"major":79})
    );
    let twenty_five = candidates
        .iter()
        .find(|candidate| candidate["percent"] == 25 && candidate["minimum_scu"] == 16)
        .unwrap();
    assert_eq!(
        twenty_five["row_weighted"]["transpose_adjacent"],
        serde_json::json!({"cases":3,"major":1})
    );
    assert_eq!(std::fs::read(&expected).unwrap(), source);
    std::fs::remove_file(&expected).unwrap();
    std::fs::remove_dir(&entry).unwrap();
    std::fs::remove_dir(&root).unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_2_q_manual__policy_research_refuses_unsupported_quantities_without_silently_dropping_them()
{
    let root = std::env::temp_dir().join(format!(
        "tallyline-scu-policy-invalid-{}",
        std::process::id()
    ));
    std::fs::create_dir(&root).unwrap();
    let entry = root.join("fixture");
    std::fs::create_dir(&entry).unwrap();
    let expected = entry.join("expected.json");
    for quantity in [
        serde_json::json!(1.5),
        serde_json::json!(-1),
        serde_json::json!("86"),
    ] {
        let source = serde_json::to_vec(&serde_json::json!({"schemaVersion":2,"verified":false,"captures":{"one.jpg":{"cards":[{"scu":quantity}]}}})).unwrap();
        std::fs::write(&expected, &source).unwrap();
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_ocr-eval"))
            .arg("scu-policy")
            .arg("--public")
            .arg(&root)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("non-negative whole-SCU")
        );
        assert_eq!(std::fs::read(&expected).unwrap(), source);
    }
    std::fs::remove_file(&expected).unwrap();
    std::fs::remove_dir(&entry).unwrap();
    std::fs::remove_dir(&root).unwrap();
}
