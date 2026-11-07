// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

use serde_json::json;

use crate::corpus::inspect;
use crate::scratch::Scratch;

#[test]
fn missing_private_corpus_is_reported_as_skipped() {
    let scratch = Scratch::new();
    let report = inspect(scratch.path(), None).unwrap();
    assert_eq!(
        report["skipped"][0],
        "private corpus: TALLYLINE_CORPUS_DIR is unset"
    );
    assert_eq!(
        report["classes"]["fileResolution=1920x1080"]["status"],
        "missing"
    );
    assert_eq!(
        report["recognition"],
        "skipped: recognition is not implemented"
    );
}

fn entry() -> serde_json::Value {
    json!({"schemaVersion":2,"id":"sample","verified":false,"transcription":"manual",
        "source":{},"location":{},"theme":"blue","gameVersion":null,
        "measured":{"priceCapHeightPx":null,"displayHeightFraction":null,"textHeightBand":null},
        "captures":{"one.png":{"side":"BUY","capturedAt":null,
            "conditions":{"sourceFidelity":"original","fileResolution":"1920x1080"},
            "sectionHeaders":[],"cards":[]}}})
}

#[test]
fn lfs_pointer_fails_and_missing_images_are_skipped() {
    let scratch = Scratch::new();
    scratch.write("sample/expected.json", &entry().to_string());
    let report = inspect(scratch.path(), None).unwrap();
    assert!(
        report["skipped"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value.as_str().unwrap().contains("one.png"))
    );
    scratch.write(
        "sample/one.png",
        "version https://git-lfs.github.com/spec/v1\noid sha256:abc\nsize 1\n",
    );
    assert!(
        inspect(scratch.path(), None)
            .unwrap_err()
            .contains("LFS pointer")
    );
}

#[test]
fn r_qa_3__class_readiness_uses_verified_real_available_captures_from_two_terminals() {
    let scratch = Scratch::new();
    for index in 0..4 {
        let mut value = entry();
        value["id"] = json!(format!("entry-{index}"));
        value["verified"] = json!(true);
        value["expectedTerminalId"] = json!(index % 2 + 1);
        value["captures"]["one.png"]["cards"] = json!((0..5).map(|i| json!({"screenOrder":i,"section":"IN STOCK","name":"Known","status":"High","scu":20,"pricePerScu":50,"containerSizes":null})).collect::<Vec<_>>());
        scratch.write(&format!("entry-{index}/expected.json"), &value.to_string());
        scratch.write(&format!("entry-{index}/one.png"), "available image");
    }
    let report = inspect(scratch.path(), None).unwrap();
    assert_eq!(
        report["classes"]["fileResolution=1920x1080"]["status"],
        "gated"
    );
    assert_eq!(
        report["classes"]["fileResolution=1920x1080"]["numericFields"],
        40
    );
}

#[test]
fn annotation_creates_separate_unverified_proposal_and_refuses_overwrite() {
    let scratch = Scratch::new();
    scratch.write("expected.json", &entry().to_string());
    scratch.write(
        "anchors.json",
        r#"{"priceTop":100,"priceBottom":110,"displayHeight":1000}"#,
    );
    let expected = scratch.path().join("expected.json");
    let anchors = scratch.path().join("anchors.json");
    let proposal = scratch.path().join("proposal.json");
    crate::corpus::annotate(&expected, &anchors, &proposal).unwrap();
    let value: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&proposal).unwrap()).unwrap();
    assert_eq!(value["verified"], false);
    assert_eq!(value["measured"]["priceCapHeightPx"].as_f64(), Some(10.0));
    assert_eq!(value["measured"]["textHeightBand"], "10-14");
    assert_eq!(
        std::fs::read_to_string(&expected).unwrap(),
        entry().to_string()
    );
    assert!(crate::corpus::annotate(&expected, &anchors, &expected).is_err());
    assert!(crate::corpus::annotate(&expected, &anchors, &proposal).is_err());
}

#[test]
fn private_entries_without_checksums_are_skipped_and_mismatches_fail() {
    let public = Scratch::new();
    let private = Scratch::new();
    private.write("sample/expected.json", &entry().to_string());
    let report = inspect(public.path(), Some(private.path())).unwrap();
    assert!(report["entries"].as_array().unwrap().is_empty());
    assert!(
        report["skipped"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value.as_str().unwrap().contains("SHA256SUMS"))
    );
    private.write(
        "sample/SHA256SUMS",
        &format!("{}  expected.json\n", "0".repeat(64)),
    );
    assert!(
        inspect(public.path(), Some(private.path()))
            .unwrap_err()
            .contains("SHA-256 mismatch")
    );
}

#[test]
fn frozen_reference_metadata_is_reported_and_public_price_dumps_are_rejected() {
    let scratch = Scratch::new();
    let mut value = entry();
    value["expectedTerminalId"] = json!(7);
    scratch.write("sample/expected.json", &value.to_string());
    scratch.write("sample/reference/metadata.json", r#"{"fetchedAt":"2026-10-14T12:00:00Z","priorProvenance":"post-capture","recordings":{"/commodities_prices":"prices.json"}}"#);
    scratch.write(
        "sample/reference/prices.json",
        r#"{"data":[{"id_terminal":999,"id_commodity":3}]}"#,
    );
    assert!(
        inspect(scratch.path(), None)
            .unwrap_err()
            .contains("outside the entry")
    );
    scratch.write("sample/reference/prices.json", r#"{"data":[]}"#);
    let report = inspect(scratch.path(), None).unwrap();
    assert_eq!(
        report["entries"][0]["reference"]["priorProvenance"],
        "post-capture"
    );
    assert_eq!(
        report["entries"][0]["reference"]["priorDependentMetrics"],
        "skipped: recognition is not implemented"
    );
}

#[test]
fn private_snapshot_uses_public_annotations_without_double_counting_captures() {
    use sha2::{Digest, Sha256};
    let public = Scratch::new();
    let private = Scratch::new();
    public.write("sample/expected.json", &entry().to_string());
    public.write("sample/one.png", "available image");
    let metadata =
        r#"{"fetchedAt":"2026-10-14T12:00:00Z","priorProvenance":"post-capture","recordings":{}}"#;
    private.write("sample/reference/metadata.json", metadata);
    let digest = Sha256::digest(metadata)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    private.write(
        "sample/SHA256SUMS",
        &format!("{digest}  reference/metadata.json\n"),
    );
    let report = inspect(public.path(), Some(private.path())).unwrap();
    assert_eq!(report["entries"].as_array().unwrap().len(), 1);
    assert_eq!(
        report["entries"][0]["reference"]["priorProvenance"],
        "post-capture"
    );
    assert_eq!(report["classes"]["fileResolution=1920x1080"]["captures"], 1);
}

#[test]
fn malformed_v2_conditions_and_measurements_are_rejected() {
    let mut value = entry();
    value["captures"]["one.png"]["capturedAt"] = json!(42);
    assert!(crate::corpus::validate_json(&value.to_string()).is_err());
    value["captures"]["one.png"]["capturedAt"] = json!(null);
    value["captures"]["one.png"]["conditions"]["windowsHdr"] = json!(42);
    assert!(crate::corpus::validate_json(&value.to_string()).is_err());
    value["captures"]["one.png"]["conditions"]["windowsHdr"] = json!(null);
    value["measured"]["priceCapHeightPx"] = json!(-1);
    assert!(crate::corpus::validate_json(&value.to_string()).is_err());
}

#[test]
fn synthetic_and_reencoded_captures_never_supply_real_resolution_coverage() {
    let scratch = Scratch::new();
    let mut value = entry();
    value["captures"]["one.png"]["synthetic"] = json!("synthetic:scale-box(8)");
    scratch.write("sample/expected.json", &value.to_string());
    scratch.write("sample/one.png", "available image");
    let report = inspect(scratch.path(), None).unwrap();
    assert_eq!(
        report["classes"]["fileResolution=1920x1080"]["status"],
        "synthetic only"
    );
    value["captures"]["one.png"]
        .as_object_mut()
        .unwrap()
        .remove("synthetic");
    value["captures"]["one.png"]["conditions"]["sourceFidelity"] = json!("reencoded-unknown");
    scratch.write("sample/expected.json", &value.to_string());
    let report = inspect(scratch.path(), None).unwrap();
    assert_eq!(
        report["classes"]["fileResolution=1920x1080"]["status"],
        "missing"
    );
    assert_eq!(report["classes"]["theme=blue"]["status"], "insufficient");
}

#[test]
fn unavailable_pinned_images_are_skipped_instead_of_reported_as_passed() {
    use sha2::{Digest, Sha256};
    let public = Scratch::new();
    let private = Scratch::new();
    let expected = entry().to_string();
    private.write("sample/expected.json", &expected);
    let digest = Sha256::digest(&expected)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    private.write(
        "sample/SHA256SUMS",
        &format!("{digest}  expected.json\n{}  one.png\n", "0".repeat(64)),
    );
    let report = inspect(public.path(), Some(private.path())).unwrap();
    assert_eq!(
        report["classes"]["fileResolution=1920x1080"]["status"],
        "missing"
    );
    assert!(
        report["skipped"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value.as_str().unwrap().contains("one.png"))
    );
}

#[test]
fn reference_global_metadata_preserves_recorded_object_envelopes() {
    let scratch = Scratch::new();
    scratch.write("sample/expected.json", &entry().to_string());
    scratch.write("sample/reference/metadata.json", r#"{"fetchedAt":"2026-10-14T12:00:00Z","priorProvenance":"post-capture","recordings":{"/game_versions":"versions.json"}}"#);
    scratch.write(
        "sample/reference/versions.json",
        r#"{"data":{"live":"4.10.2","ptu":null}}"#,
    );
    assert!(inspect(scratch.path(), None).is_ok());
}

#[test]
fn absent_reference_recordings_are_reported_as_skipped() {
    let scratch = Scratch::new();
    scratch.write("sample/expected.json", &entry().to_string());
    scratch.write("sample/reference/metadata.json", r#"{"fetchedAt":"2026-10-14T12:00:00Z","priorProvenance":"post-capture","recordings":{"/commodities_prices":"missing.json"}}"#);
    let report = inspect(scratch.path(), None).unwrap();
    assert_eq!(
        report["entries"][0]["reference"]["status"],
        "skipped: frozen reference recording unavailable"
    );
}
