// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Retained-reader measurement contracts at the offline command-line seam.

use ocr_eval::reader_replay::{self, ReplayError};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::process::{Command, Output};

fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn execute(root: &Path, packet: &Path, hash: &str) -> std::io::Result<Output> {
    Command::new(env!("CARGO_BIN_EXE_ocr-eval"))
        .args(["reader-replay", "--public"])
        .arg(root)
        .arg("--packet")
        .arg(packet)
        .args(["--sha256", hash])
        .output()
}

fn value(value: &str) -> Value {
    json!({"outcome":"value", "value":value, "literal":value, "normalised_literal":value})
}

fn invalid(literal: &str) -> Value {
    json!({"outcome":"invalid", "literal":literal, "normalised_literal":literal})
}

fn missing() -> Value {
    json!({"outcome":"missing", "literal":null, "normalised_literal":null})
}

#[test]
fn retained_replay_counts_invalid_omitted_and_shared_wrong_fields_without_losing_denominators() {
    let root = std::env::temp_dir().join(format!("tallyline-reader-replay-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let entry = root.join("fixture");
    std::fs::create_dir(&entry).unwrap();
    let expected_path = entry.join("expected.json");
    let expected = br#"{"schemaVersion":2,"verified":true,"captures":{"one.png":{"cards":[{"screenOrder":1,"scu":10},{"screenOrder":2,"scu":10},{"screenOrder":3,"scu":10},{"screenOrder":4,"scu":10},{"screenOrder":5,"scu":10},{"screenOrder":6,"scu":10},{"screenOrder":7,"scu":10},{"screenOrder":8,"pricePerScu":"9007199254740993.1"}]}}}"#;
    std::fs::write(&expected_path, expected).unwrap();
    let readings = [
        [value("10"), value("10"), value("10")],
        [value("11"), value("11"), value("11")],
        [value("10"), value("11"), value("10")],
        [invalid("O SCU"), value("10"), missing()],
        [missing(), missing(), value("10")],
        [
            json!({"outcome":"unscorable","reason":"alignment","literal":null,"normalised_literal":null}),
            value("10"),
            value("10"),
        ],
        [value("10"), value("10"), invalid("?")],
        [
            value("9007199254740993.10"),
            value("9007199254740993.1"),
            value("9007199254740993.11"),
        ],
    ];
    let rows = readings
        .into_iter()
        .enumerate()
        .map(|(index, observations)| {
            json!({"entry":"fixture","capture":"one.png","screen_order":index+1,
            "field":if index == 7 {"pricePerScu"} else {"scu"},
            "reference_sha256":digest(expected),"observations":observations})
        })
        .collect::<Vec<_>>();
    let packet = json!({"schema_version":1,"readers":["A","B","C"],"rows":rows});
    let bytes = serde_json::to_vec(&packet).unwrap();
    let packet_path = root.join("retained.json");
    std::fs::write(&packet_path, &bytes).unwrap();
    let output = execute(&root, &packet_path, &digest(&bytes)).unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["fields"], 8);
    assert_eq!(result["cards"], 8);
    assert_eq!(result["captures"], 1);
    assert_eq!(result["inference"], "not_run");
    assert_eq!(
        result["singles"][0]["total"],
        json!({"denominator":8,"correct":4,"wrong":2,"parsed_wrong":1,"invalid":1,"missing":1,"unscorable":1})
    );
    assert_eq!(
        result["singles"][1]["total"],
        json!({"denominator":8,"correct":5,"wrong":2,"parsed_wrong":2,"invalid":0,"missing":1,"unscorable":0})
    );
    assert_eq!(
        result["singles"][2]["total"],
        json!({"denominator":8,"correct":4,"wrong":3,"parsed_wrong":2,"invalid":1,"missing":1,"unscorable":0})
    );
    let pairs = result["pairs"].as_array().unwrap();
    assert_eq!(pairs.len(), 3);
    let pair = &pairs[0]["total"];
    assert_eq!(pair["all_correct"], 3);
    assert_eq!(pair["only_reader_correct"], json!([1, 2]));
    assert_eq!(pair["none_correct"], 2);
    assert_eq!(pair["same_wrong_value"], 1);
    assert_eq!(pair["different_values"], 1);
    assert_eq!(pair["accepted_correct"], 3);
    assert_eq!(pair["accepted_wrong"], 1);
    assert_eq!(pair["abstained"], 4);
    assert_eq!(pair["baseline_wrong_detected"], 1);
    assert_eq!(pair["baseline_correct_flagged"], 1);
    let triples = result["triples"].as_array().unwrap();
    assert_eq!(triples.len(), 1);
    assert_eq!(triples[0]["total"]["accepted_correct"], 1);
    assert_eq!(triples[0]["total"]["accepted_wrong"], 1);
    assert_eq!(triples[0]["total"]["abstained"], 6);
    assert_eq!(std::fs::read(&expected_path).unwrap(), expected);
    assert_eq!(std::fs::read(&packet_path).unwrap(), bytes);
    std::fs::remove_file(packet_path).unwrap();
    std::fs::remove_file(expected_path).unwrap();
    std::fs::remove_dir(entry).unwrap();
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn retained_replay_refuses_changed_hashes_missing_slots_and_false_unscorable_labels() {
    let root =
        std::env::temp_dir().join(format!("tallyline-reader-refusals-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let entry = root.join("fixture");
    std::fs::create_dir(&entry).unwrap();
    let expected_path = entry.join("expected.json");
    let expected =
        br#"{"verified":true,"captures":{"one.png":{"cards":[{"screenOrder":1,"scu":0}]}}}"#;
    std::fs::write(&expected_path, expected).unwrap();
    let packet = json!({"schema_version":1,"readers":["A","B"],"rows":[{"entry":"fixture","capture":"one.png","screen_order":1,"field":"scu","reference_sha256":digest(expected),"observations":[invalid("OSCU"),invalid("OSCU")]}]});
    let packet_path = root.join("retained.json");
    let bytes = serde_json::to_vec(&packet).unwrap();
    std::fs::write(&packet_path, &bytes).unwrap();
    let output = execute(&root, &packet_path, &digest(&bytes)).unwrap();
    assert!(output.status.success());
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["singles"][0]["total"]["wrong"], 1);
    assert_eq!(result["singles"][0]["total"]["unscorable"], 0);
    assert_eq!(result["pairs"][0]["total"]["same_invalid_literal"], 1);
    assert_eq!(result["pairs"][0]["total"]["accepted_wrong"], 0);
    assert_eq!(result["pairs"][0]["total"]["abstained"], 1);
    assert!(matches!(
        reader_replay::analyse(&root, &packet_path, &"0".repeat(64)),
        Err(ReplayError::PacketHashMismatch)
    ));
    let changed_hash = execute(&root, &packet_path, &"0".repeat(64)).unwrap();
    assert!(!changed_hash.status.success());
    assert!(changed_hash.stdout.is_empty());
    assert!(String::from_utf8_lossy(&changed_hash.stderr).contains("packet hash mismatch"));
    let mut missing_slot = packet.clone();
    missing_slot["rows"][0]["observations"]
        .as_array_mut()
        .unwrap()
        .pop();
    let mut false_unscorable = packet.clone();
    false_unscorable["rows"][0]["observations"][0] = json!({"outcome":"unscorable","reason":"malformed","literal":"OSCU","normalised_literal":"OSCU"});
    let mut changed_reference = packet.clone();
    changed_reference["rows"][0]["reference_sha256"] = json!("0".repeat(64));
    let mut duplicate = packet.clone();
    duplicate["rows"]
        .as_array_mut()
        .unwrap()
        .push(packet["rows"][0].clone());
    for (changed, reason) in [
        (missing_slot, "every reader slot"),
        (false_unscorable, "unscorable requires"),
        (changed_reference, "reference hash mismatch"),
        (duplicate, "duplicate retained field"),
    ] {
        let bytes = serde_json::to_vec(&changed).unwrap();
        std::fs::write(&packet_path, &bytes).unwrap();
        let output = execute(&root, &packet_path, &digest(&bytes)).unwrap();
        assert!(!output.status.success(), "{reason}");
        assert!(output.stdout.is_empty());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(reason),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(std::fs::read(&expected_path).unwrap(), expected);
        assert_eq!(std::fs::read(&packet_path).unwrap(), bytes);
    }
    std::fs::remove_file(packet_path).unwrap();
    std::fs::remove_file(expected_path).unwrap();
    std::fs::remove_dir(entry).unwrap();
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn replay_api_distinguishes_schema_malformed_evidence_and_unavailable_inputs() {
    use std::error::Error;

    let root = std::env::temp_dir().join(format!("tallyline-reader-errors-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let packet = root.join("retained.json");
    let error = reader_replay::analyse(&root, &packet, "").unwrap_err();
    assert!(matches!(&error, ReplayError::InputOpen(_)));
    assert!(error.source().is_some());
    assert!(!error.to_string().contains("retained.json"));
    let malformed = b"{";
    std::fs::write(&packet, malformed).unwrap();
    assert!(matches!(
        reader_replay::analyse(&root, &packet, &digest(malformed)),
        Err(ReplayError::MalformedPacket)
    ));
    let unsupported = br#"{"schema_version":2}"#;
    std::fs::write(&packet, unsupported).unwrap();
    assert!(matches!(
        reader_replay::analyse(&root, &packet, &digest(unsupported)),
        Err(ReplayError::UnsupportedSchema)
    ));
    std::fs::remove_file(packet).unwrap();
    std::fs::remove_dir(root).unwrap();
}
