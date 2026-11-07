// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Public verified Small model and dictionary load contracts.

#![expect(
    non_snake_case,
    reason = "Requirement tags contain a double underscore."
)]

use tallyline_adapter_ocr::api::model::{OcrArtifact, SmallOcrAssets};

#[test]
fn r_sec_7_r_ocr_1__bundled_small_assets_preserve_their_verified_identity_and_alphabet() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/small");
    let assets = SmallOcrAssets::load(&directory).unwrap();
    assert_eq!(assets.decoder().class_count(), 18710);
    let identities = assets.fingerprints();
    assert_eq!(identities.len(), 3);
    assert!(
        identities
            .iter()
            .any(|identity| identity.artifact == OcrArtifact::SmallDetector
                && identity.sha256
                    == "d73e0058b7a8086bbd57f3d10b8bcd4ff95363f67e06e2762b5e814fe9c9410e"
                && identity.bytes == 9880512)
    );
    assert!(
        identities
            .iter()
            .any(|identity| identity.artifact == OcrArtifact::SmallRecognizer
                && identity.sha256
                    == "5435fd747c9e0efe15a96d0b378d5bd157e9492ed8fd80edf08f30d02fa24634"
                && identity.bytes == 21159378)
    );
    assert!(
        identities
            .iter()
            .any(|identity| identity.artifact == OcrArtifact::SmallDictionary
                && identity.sha256
                    == "b5f2bfe2bdd9448429e3e82b51c789775d9b42f2403d082b00662eb77e401c5d"
                && identity.bytes == 74947)
    );
}

#[test]
fn r_sec_7__relative_missing_and_wrong_same_size_assets_are_refused() -> std::io::Result<()> {
    use tallyline_adapter_ocr::api::model::AssetError;
    assert!(matches!(
        SmallOcrAssets::load(std::path::Path::new("assets/small")),
        Err(AssetError::NotAbsolute)
    ));
    let directory = fixture_directory("bad-detector")?;
    assert!(matches!(
        SmallOcrAssets::load(&directory),
        Err(AssetError::Read {
            artifact: OcrArtifact::SmallDetector,
            ..
        })
    ));
    std::fs::File::create(directory.join("PP-OCRv6_small_det.onnx"))?.set_len(9_880_512)?;
    assert!(matches!(
        SmallOcrAssets::load(&directory),
        Err(AssetError::HashMismatch(OcrArtifact::SmallDetector))
    ));
    remove_fixture(&directory)
}

#[test]
fn r_sec_7__a_reordered_dictionary_cannot_reuse_the_small_model_binding() -> std::io::Result<()> {
    use tallyline_adapter_ocr::api::model::AssetError;
    let bundled = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/small");
    let directory = fixture_directory("reordered-dictionary")?;
    for file in [
        "PP-OCRv6_small_det.onnx",
        "PP-OCRv6_small_rec.onnx",
        "ppocrv6_dict.txt",
    ] {
        std::fs::copy(bundled.join(file), directory.join(file))?;
    }
    let text = std::fs::read_to_string(directory.join("ppocrv6_dict.txt"))?;
    let mut labels: Vec<_> = text.lines().collect();
    labels.swap(0, 1);
    let reordered = labels.join("\n") + "\n";
    assert_eq!(reordered.len(), 74947);
    assert_ne!(reordered, text);
    std::fs::write(directory.join("ppocrv6_dict.txt"), reordered)?;
    assert!(matches!(
        SmallOcrAssets::load(&directory),
        Err(AssetError::HashMismatch(OcrArtifact::SmallDictionary))
    ));
    remove_fixture(&directory)
}

#[test]
fn r_ocr_1__the_exact_small_alphabet_preserves_blank_offset_and_appended_space() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/small");
    let assets = SmallOcrAssets::load(&directory).unwrap();
    let mut probabilities = vec![0.0; 2 * 18710];
    *probabilities.get_mut(1).unwrap() = 1.0;
    *probabilities.get_mut(18710 + 18709).unwrap() = 1.0;
    assert_eq!(
        assets
            .decoder()
            .decode(&probabilities, [1, 2, 18710])
            .unwrap()
            .text,
        "! "
    );
}

fn fixture_directory(label: &str) -> std::io::Result<std::path::PathBuf> {
    let base = std::env::temp_dir().canonicalize()?;
    for attempt in 0..100 {
        let path = base.join(format!(
            "tallyline-ocr-assets-{}-{label}-{attempt}",
            std::process::id()
        ));
        match std::fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::other(
        "cannot reserve an owned OCR asset fixture",
    ))
}

fn remove_fixture(path: &std::path::Path) -> std::io::Result<()> {
    let base = std::env::temp_dir().canonicalize()?;
    let actual = path.canonicalize()?;
    let prefix = format!("tallyline-ocr-assets-{}-", std::process::id());
    if !actual.starts_with(base)
        || !actual
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(&prefix))
    {
        return Err(std::io::Error::other(
            "OCR fixture cleanup escaped its owned temporary directory",
        ));
    }
    std::fs::remove_dir_all(actual)
}

#[test]
fn r_sec_7__verified_model_bytes_survive_a_later_path_change_without_an_unchecked_reopen()
-> std::io::Result<()> {
    use std::io::Write;
    use tallyline_adapter_ocr::api::model::AssetError;
    let bundled = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/small");
    let directory = fixture_directory("model-snapshot")?;
    for file in [
        "PP-OCRv6_small_det.onnx",
        "PP-OCRv6_small_rec.onnx",
        "ppocrv6_dict.txt",
    ] {
        std::fs::copy(bundled.join(file), directory.join(file))?;
    }
    let assets = SmallOcrAssets::load(&directory).unwrap();
    let model = assets.open_model(OcrArtifact::SmallRecognizer).unwrap();
    assert_eq!(model.bytes().first(), Some(&8));
    assert_eq!(model.bytes().len(), 21_159_378);
    assert_eq!(model.fingerprint().artifact, OcrArtifact::SmallRecognizer);
    std::fs::OpenOptions::new()
        .write(true)
        .open(directory.join("PP-OCRv6_small_rec.onnx"))?
        .write_all(&[0])?;
    assert_eq!(model.bytes().first(), Some(&8));
    assert!(matches!(
        assets.open_model(OcrArtifact::SmallRecognizer),
        Err(AssetError::HashMismatch(OcrArtifact::SmallRecognizer))
    ));
    assert!(matches!(
        assets.open_model(OcrArtifact::SmallDictionary),
        Err(AssetError::NotModel)
    ));
    remove_fixture(&directory)
}
