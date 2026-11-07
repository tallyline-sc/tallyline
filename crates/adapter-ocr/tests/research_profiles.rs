// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Public hash-bound research profile contracts without native execution.

#![expect(
    non_snake_case,
    reason = "Requirement tags contain a double underscore."
)]

use tallyline_adapter_ocr::api::model::{
    OcrDetector, OcrRecognizer, OcrResearchProfile, ResearchOcrAssets,
};

#[test]
fn r_sec_7_r_ocr_1__small_research_pair_uses_the_existing_exact_alphabet_and_model_identities() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/small");
    let profile = OcrResearchProfile::new(OcrDetector::Small, OcrRecognizer::Small);
    let assets = ResearchOcrAssets::load(
        profile,
        &directory.join("PP-OCRv6_small_det.onnx"),
        &directory.join("PP-OCRv6_small_rec.onnx"),
        &directory.join("ppocrv6_dict.txt"),
    )
    .unwrap();
    assert_eq!(assets.decoder().class_count(), 18710);
    assert_eq!(
        assets.fingerprints().first().unwrap().sha256,
        "d73e0058b7a8086bbd57f3d10b8bcd4ff95363f67e06e2762b5e814fe9c9410e"
    );
    assert_eq!(
        assets.fingerprints().get(1).unwrap().sha256,
        "5435fd747c9e0efe15a96d0b378d5bd157e9492ed8fd80edf08f30d02fa24634"
    );
    assert_eq!(assets.profile(), profile);
}
#[test]
fn r_ocr_1__tiny_detector_accepts_a_box_that_small_and_medium_refuse() {
    use tallyline_adapter_ocr::api::model::DbPostprocessor;
    let mut map = vec![0.0; 400];
    for y in 4..16 {
        for x in 4..16 {
            *map.get_mut(y * 20 + x).unwrap() = 0.42;
        }
    }
    assert_eq!(
        DbPostprocessor
            .extract_for_detector(OcrDetector::Tiny, &map, [1, 1, 20, 20], [20, 20])
            .unwrap()
            .len(),
        1
    );
    for detector in [OcrDetector::Small, OcrDetector::Medium] {
        assert!(
            DbPostprocessor
                .extract_for_detector(detector, &map, [1, 1, 20, 20], [20, 20])
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn r_ocr_1__research_reader_is_lazy_and_refuses_oversized_input_before_the_one_shot_loader() {
    use std::time::Duration;

    use tallyline_adapter_ocr::api::model::{
        OcrRuntimeSettings, ResearchOcrReader, RuntimeLoadError, SmallReaderError,
        SmallSessionState,
    };
    use tallyline_shared_kernel::api::model::ImageRaster;
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/small");
    let profile = OcrResearchProfile::new(OcrDetector::Small, OcrRecognizer::Small);
    let assets = ResearchOcrAssets::load(
        profile,
        &directory.join("PP-OCRv6_small_det.onnx"),
        &directory.join("PP-OCRv6_small_rec.onnx"),
        &directory.join("ppocrv6_dict.txt"),
    )
    .unwrap();
    let mut reader = ResearchOcrReader::new(
        &assets,
        OcrRuntimeSettings::default(),
        || Duration::ZERO,
        || Err(RuntimeLoadError::Location),
    );
    assert_eq!(reader.profile(), profile);
    assert_eq!(reader.session_state(), SmallSessionState::Empty);
    assert!(!reader.close_if_idle().unwrap());
    let wide = ImageRaster::new(3201, 48, vec![0; 3201 * 48]).unwrap();
    assert!(matches!(
        reader.read_line(&wide),
        Err(SmallReaderError::Input(_))
    ));
    let pixel = ImageRaster::new(1, 1, vec![0xff00_0000]).unwrap();
    assert!(matches!(
        reader.read_line(&pixel),
        Err(SmallReaderError::Runtime(RuntimeLoadError::Location))
    ));
    assert!(matches!(
        reader.read_line(&pixel),
        Err(SmallReaderError::RuntimeInitialiserConsumed)
    ));
    reader.close_sessions();
    assert_eq!(reader.session_state(), SmallSessionState::Empty);
}

#[test]
fn r_sec_7__selected_family_is_checked_before_missing_later_inputs() {
    use tallyline_adapter_ocr::api::model::{AssetError, OcrArtifact};
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/small");
    let missing = directory.join("absent-research-data");
    assert!(matches!(
        ResearchOcrAssets::load(
            OcrResearchProfile::new(OcrDetector::Tiny, OcrRecognizer::Medium),
            &directory.join("PP-OCRv6_small_det.onnx"),
            &missing,
            &missing
        ),
        Err(AssetError::HashMismatch(OcrArtifact::TinyDetector))
    ));
    assert!(matches!(
        ResearchOcrAssets::load(
            OcrResearchProfile::new(OcrDetector::Small, OcrRecognizer::Tiny),
            &directory.join("PP-OCRv6_small_det.onnx"),
            &directory.join("PP-OCRv6_small_rec.onnx"),
            &missing
        ),
        Err(AssetError::HashMismatch(OcrArtifact::TinyRecognizer))
    ));
    assert!(matches!(
        ResearchOcrAssets::load(
            OcrResearchProfile::new(OcrDetector::Medium, OcrRecognizer::Medium),
            &missing,
            &missing,
            &missing
        ),
        Err(AssetError::Read {
            artifact: OcrArtifact::MediumDetector,
            ..
        })
    ));
    assert!(matches!(
        ResearchOcrAssets::load(
            OcrResearchProfile::new(OcrDetector::Small, OcrRecognizer::Small),
            &directory.join("PP-OCRv6_small_det.onnx"),
            &directory.join("PP-OCRv6_small_rec.onnx"),
            std::path::Path::new("relative-dictionary")
        ),
        Err(AssetError::NotAbsolute)
    ));
}

#[test]
fn r_sec_7__research_model_snapshots_survive_changed_files_but_a_fresh_load_refuses_them()
-> std::io::Result<()> {
    use std::io::Write;

    use tallyline_adapter_ocr::api::model::{AssetError, OcrArtifact};
    let bundled = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/small");
    let fixture = fixture_directory("snapshot")?;
    let detector = fixture.join("detector-data");
    let recognizer = fixture.join("recogniser-data");
    let dictionary = fixture.join("dictionary-data");
    for (source, target) in [
        ("PP-OCRv6_small_det.onnx", &detector),
        ("PP-OCRv6_small_rec.onnx", &recognizer),
        ("ppocrv6_dict.txt", &dictionary),
    ] {
        std::fs::copy(bundled.join(source), target)?;
    }
    let profile = OcrResearchProfile::new(OcrDetector::Small, OcrRecognizer::Small);
    let assets = ResearchOcrAssets::load(profile, &detector, &recognizer, &dictionary).unwrap();
    assert_eq!(assets.detector().bytes().first(), Some(&8));
    assert_eq!(assets.recognizer().bytes().first(), Some(&8));
    std::fs::OpenOptions::new()
        .write(true)
        .open(&detector)?
        .write_all(&[0])?;
    std::fs::OpenOptions::new()
        .write(true)
        .open(&recognizer)?
        .write_all(&[0])?;
    assert_eq!(assets.detector().bytes().first(), Some(&8));
    assert_eq!(assets.recognizer().bytes().first(), Some(&8));
    assert!(matches!(
        ResearchOcrAssets::load(profile, &detector, &recognizer, &dictionary),
        Err(AssetError::HashMismatch(OcrArtifact::SmallDetector))
    ));
    std::fs::copy(bundled.join("PP-OCRv6_small_det.onnx"), &detector)?;
    assert!(matches!(
        ResearchOcrAssets::load(profile, &detector, &recognizer, &dictionary),
        Err(AssetError::HashMismatch(OcrArtifact::SmallRecognizer))
    ));
    std::fs::copy(bundled.join("PP-OCRv6_small_rec.onnx"), &recognizer)?;
    let text = std::fs::read_to_string(&dictionary)?;
    let mut lines: Vec<_> = text.lines().collect();
    lines.swap(0, 1);
    let reordered = lines.join("\n") + "\n";
    assert_eq!(reordered.len(), 74947);
    std::fs::write(&dictionary, reordered)?;
    assert!(matches!(
        ResearchOcrAssets::load(profile, &detector, &recognizer, &dictionary),
        Err(AssetError::HashMismatch(OcrArtifact::SmallDictionary))
    ));
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
    for path in [&detector, &recognizer, &dictionary] {
        std::fs::remove_file(path)?;
    }
    std::fs::remove_dir(fixture)
}

#[test]
#[ignore = "Requires the operator's external hash-bound research weights; never downloads data."]
fn r_sec_7_r_ocr_1__external_research_pairs_preserve_each_recogniser_alphabet_and_both_model_identities()
 {
    let external = std::path::PathBuf::from(
        std::env::var_os("TALLYLINE_OCR_RESEARCH_DIR")
            .expect("set the external reviewed-data root"),
    );
    let bundled = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    let mut identifiers = std::collections::HashSet::new();
    for (detector, detector_file, expected_detector_bytes) in [
        (
            OcrDetector::Tiny,
            external.join("PP-OCRv6_tiny_det_onnx/inference.onnx"),
            1_780_590,
        ),
        (
            OcrDetector::Small,
            bundled.join("small/PP-OCRv6_small_det.onnx"),
            9_880_512,
        ),
        (
            OcrDetector::Medium,
            external.join("PP-OCRv6_medium_det_onnx/inference.onnx"),
            62_032_837,
        ),
    ] {
        for (recognizer, recognizer_file, dictionary, classes, expected_recognizer_bytes) in [
            (
                OcrRecognizer::Tiny,
                external.join("PP-OCRv6_tiny_rec_onnx/inference.onnx"),
                bundled.join("research/ppocrv6_tiny_dict.txt"),
                6906,
                4_462_639,
            ),
            (
                OcrRecognizer::Small,
                bundled.join("small/PP-OCRv6_small_rec.onnx"),
                bundled.join("small/ppocrv6_dict.txt"),
                18710,
                21_159_378,
            ),
            (
                OcrRecognizer::Medium,
                external.join("PP-OCRv6_medium_rec_onnx/inference.onnx"),
                bundled.join("small/ppocrv6_dict.txt"),
                18710,
                76_554_979,
            ),
        ] {
            let profile = OcrResearchProfile::new(detector, recognizer);
            let assets =
                ResearchOcrAssets::load(profile, &detector_file, &recognizer_file, &dictionary)
                    .unwrap();
            assert!(identifiers.insert(profile.identifier()));
            assert_eq!(assets.detector().bytes().len(), expected_detector_bytes);
            assert_eq!(assets.recognizer().bytes().len(), expected_recognizer_bytes);
            assert_eq!(assets.decoder().class_count(), classes);
            assert_eq!(assets.profile().detector(), detector);
            assert_eq!(assets.profile().recognizer(), recognizer);
            let mut probabilities = vec![0.0; 2 * classes];
            *probabilities.get_mut(1).unwrap() = 1.0;
            *probabilities.get_mut(2 * classes - 1).unwrap() = 1.0;
            assert_eq!(
                assets
                    .decoder()
                    .decode(&probabilities, [1, 2, classes])
                    .unwrap()
                    .text,
                "! "
            );
            let wrong_dictionary = if recognizer == OcrRecognizer::Tiny {
                bundled.join("small/ppocrv6_dict.txt")
            } else {
                bundled.join("research/ppocrv6_tiny_dict.txt")
            };
            assert!(matches!(
                ResearchOcrAssets::load(
                    profile,
                    &detector_file,
                    &recognizer_file,
                    &wrong_dictionary
                ),
                Err(tallyline_adapter_ocr::api::model::AssetError::HashMismatch(
                    _
                ))
            ));
        }
    }
    assert_eq!(identifiers.len(), 9);
}

fn fixture_directory(label: &str) -> std::io::Result<std::path::PathBuf> {
    let base = std::env::temp_dir().canonicalize()?;
    for attempt in 0..100 {
        let path = base.join(format!(
            "tallyline-ocr-research-{}-{label}-{attempt}",
            std::process::id()
        ));
        match std::fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::other(
        "cannot reserve an owned research-data fixture",
    ))
}
