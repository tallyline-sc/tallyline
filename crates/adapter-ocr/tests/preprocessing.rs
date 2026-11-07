// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Recognition tensor input contracts.

#![expect(
    non_snake_case,
    reason = "Requirement tags contain a double underscore."
)]

use tallyline_adapter_ocr::api::model::OcrTensorBuffer;
use tallyline_shared_kernel::api::model::ImageRaster;

#[test]
fn r_ocr_1__recognition_input_is_bgr_nchw_with_normalised_zero_padding() {
    let image = ImageRaster::new(1, 1, vec![0xffff_8000]).unwrap();
    let mut buffer = OcrTensorBuffer::default();
    let tensor = buffer.recognition(&image, 320).unwrap();
    assert_eq!(tensor.shape, [1, 3, 48, 320]);
    assert_eq!(tensor.resized_width, 48);
    assert_eq!(tensor.values[0], -1.0);
    assert!((tensor.values[48 * 320] - (128.0 / 255.0 - 0.5) / 0.5).abs() < 0.000001);
    assert_eq!(tensor.values[2 * 48 * 320], 1.0);
    assert_eq!(tensor.values[47], -1.0);
    assert_eq!(tensor.values[48], 0.0);
    assert_eq!(tensor.values[3 * 48 * 320 - 1], 0.0);
}
#[test]
fn r_ocr_1__recognition_resize_uses_pixel_centres_and_reuse_clears_padding() {
    let pixels = [0xff00_0000, 0xffff_ffff].repeat(24);
    let image = ImageRaster::new(2, 24, pixels).unwrap();
    let mut buffer = OcrTensorBuffer::default();
    let tensor = buffer.recognition(&image, 4).unwrap();
    let expected = [
        -1.0,
        (64.0 / 255.0 - 0.5) / 0.5,
        (191.0 / 255.0 - 0.5) / 0.5,
        1.0,
    ];
    for (actual, expected) in tensor.values.iter().take(4).zip(expected) {
        assert!((actual - expected).abs() < 0.000001);
    }
    let black = ImageRaster::new(1, 48, vec![0xff00_0000; 48]).unwrap();
    let tensor = buffer.recognition(&black, 4).unwrap();
    assert_eq!(tensor.values[0], -1.0);
    assert_eq!(tensor.values[1], 0.0);
    assert!(buffer.recognition(&image, 3).is_err());
    assert!(buffer.recognition(&image, 3201).is_err());
}

#[test]
fn r_ocr_1__asymmetric_resize_matches_the_generic_eight_bit_linear_reference() {
    let levels: [u32; 15] = [
        0, 17, 89, 193, 255, 3, 41, 7, 198, 11, 101, 53, 249, 23, 154,
    ];
    let pixels = levels
        .into_iter()
        .map(|code| 0xff00_0000 | code | code << 8 | code << 16)
        .collect();
    let image = ImageRaster::new(3, 5, pixels).unwrap();
    let mut buffer = OcrTensorBuffer::default();
    let tensor = buffer.recognition(&image, 29).unwrap();
    for (x, y, code) in [
        (6, 5, 17),
        (8, 7, 65),
        (9, 7, 68),
        (11, 0, 11),
        (20, 1, 61),
        (22, 2, 76),
    ] {
        let expected = (code as f32 / 255.0 - 0.5) / 0.5;
        assert_eq!(tensor.values[y * 29 + x], expected, "pixel {x},{y}");
    }
}

#[test]
fn r_ocr_1__recognition_tensors_identify_the_frozen_preprocessing_profile() {
    let image = ImageRaster::new(1, 1, vec![0xff00_0000]).unwrap();
    let mut buffer = OcrTensorBuffer::default();
    let tensor = buffer.recognition(&image, 48).unwrap();
    assert_eq!(tensor.preprocessing.identifier(), "ppocr-bgr48-linear8u-v1");
}

#[test]
fn r_ocr_1__detector_input_uses_bgr_imagenet_normalisation_and_stride_rounding() {
    let image = ImageRaster::new(3, 2, vec![0xffff_8000; 6]).unwrap();
    let mut buffer = OcrTensorBuffer::default();
    let tensor = buffer.detection(&image, 2048).unwrap();
    assert_eq!(tensor.shape, [1, 3, 736, 1088]);
    assert!((tensor.values[0] - (-0.485 / 0.229)).abs() < 0.00001);
    assert!((tensor.values[736 * 1088] - ((128.0 / 255.0 - 0.456) / 0.224)).abs() < 0.00001);
    assert!((tensor.values[2 * 736 * 1088] - ((1.0 - 0.406) / 0.225)).abs() < 0.00001);
}

#[test]
fn r_ocr_1__oversized_detector_input_is_refused_without_squeezing_the_image() {
    let image = ImageRaster::new(3000, 736, vec![0; 3000 * 736]).unwrap();
    assert!(OcrTensorBuffer::default().detection(&image, 2048).is_err());
}
