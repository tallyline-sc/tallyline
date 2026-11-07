// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Public lazy Small reader contracts without native initialisation.

#![expect(
    non_snake_case,
    reason = "Requirement tags contain a double underscore."
)]
use std::path::Path;
use std::time::Duration;
use tallyline_adapter_ocr::api::model::{
    OcrRuntimeSettings, SmallOcrAssets, SmallOcrReader, SmallSessionState,
};

#[test]
fn r_ocr_1__new_reader_and_idle_cleanup_do_not_create_native_sessions() {
    let assets =
        SmallOcrAssets::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/small")).unwrap();
    let mut reader = SmallOcrReader::new(
        &assets,
        OcrRuntimeSettings::default(),
        || Duration::from_secs(300),
        || Err(tallyline_adapter_ocr::api::model::RuntimeLoadError::Location),
    );
    assert_eq!(reader.session_state(), SmallSessionState::Empty);
    assert!(!reader.close_if_idle().unwrap());
    assert_eq!(reader.session_state(), SmallSessionState::Empty);
}

#[test]
fn r_ocr_1__raw_line_input_is_bounded_before_any_native_session_is_requested() {
    use tallyline_shared_kernel::api::model::ImageRaster;
    let assets =
        SmallOcrAssets::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/small")).unwrap();
    let mut reader = SmallOcrReader::new(
        &assets,
        OcrRuntimeSettings::default(),
        || Duration::ZERO,
        || Err(tallyline_adapter_ocr::api::model::RuntimeLoadError::Location),
    );
    let image = ImageRaster::new(3201, 48, vec![0; 3201 * 48]).unwrap();
    assert!(matches!(
        reader.read_line(&image),
        Err(tallyline_adapter_ocr::api::model::SmallReaderError::Input(
            _
        ))
    ));
    assert_eq!(reader.session_state(), SmallSessionState::Empty);
}

#[test]
fn r_ocr_1__failed_runtime_initialisation_is_not_retried_on_a_later_line() {
    use tallyline_adapter_ocr::api::model::{RuntimeLoadError, SmallReaderError};
    use tallyline_shared_kernel::api::model::ImageRaster;
    let assets =
        SmallOcrAssets::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/small")).unwrap();
    let mut reader = SmallOcrReader::new(
        &assets,
        OcrRuntimeSettings::default(),
        || Duration::ZERO,
        || Err(RuntimeLoadError::Location),
    );
    let image = ImageRaster::new(1, 1, vec![0xff00_0000]).unwrap();
    assert!(matches!(
        reader.read_line(&image),
        Err(SmallReaderError::Runtime(RuntimeLoadError::Location))
    ));
    assert!(matches!(
        reader.read_line(&image),
        Err(SmallReaderError::RuntimeInitialiserConsumed)
    ));
}

#[test]
fn r_ocr_1__raw_image_input_is_bounded_before_requesting_the_app_runtime() {
    use tallyline_adapter_ocr::api::model::{RuntimeLoadError, SmallReaderError};
    use tallyline_shared_kernel::api::model::ImageRaster;
    let assets =
        SmallOcrAssets::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/small")).unwrap();
    let mut reader = SmallOcrReader::new(
        &assets,
        OcrRuntimeSettings::default(),
        || Duration::ZERO,
        || Err(RuntimeLoadError::Location),
    );
    let image = ImageRaster::new(3000, 736, vec![0; 3000 * 736]).unwrap();
    assert!(matches!(
        reader.read_image(&image),
        Err(SmallReaderError::Input(_))
    ));
    assert_eq!(reader.session_state(), SmallSessionState::Empty);
}

#[test]
fn r_ocr_1__raw_reader_identifies_its_portable_geometry_and_crop_profile() {
    let assets =
        SmallOcrAssets::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/small")).unwrap();
    let reader = SmallOcrReader::new(
        &assets,
        OcrRuntimeSettings::default(),
        || Duration::ZERO,
        || Err(tallyline_adapter_ocr::api::model::RuntimeLoadError::Location),
    );
    assert_eq!(
        reader.profile().identifier(),
        "ppocr-small-portable-db-bilinear-v1"
    );
}
