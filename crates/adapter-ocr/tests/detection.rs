// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Public detector geometry contracts.

#![expect(
    non_snake_case,
    reason = "Requirement tags contain a double underscore."
)]
use tallyline_adapter_ocr::api::model::DbPostprocessor;

#[test]
fn r_ocr_1__rectangle_probability_map_expands_and_maps_to_source_coordinates() {
    let mut probabilities = vec![0.0; 32 * 32];
    for y in 10..=16 {
        for x in 8..=20 {
            probabilities[y * 32 + x] = 0.9;
        }
    }
    let boxes = DbPostprocessor
        .extract(&probabilities, [1, 1, 32, 32], [64, 64])
        .unwrap();
    assert_eq!(boxes.len(), 1);
    assert_eq!(
        boxes[0].corners,
        [[10.0, 14.0], [46.0, 14.0], [46.0, 38.0], [10.0, 38.0]]
    );
    assert!((boxes[0].probability - 0.9).abs() < 0.00001);
}

#[test]
fn r_ocr_1__rotated_text_region_keeps_its_minimum_area_quadrilateral() {
    let mut probabilities = vec![0.0; 32 * 32];
    for y in 0_usize..32 {
        for x in 0_usize..32 {
            if x.abs_diff(16) + y.abs_diff(16) <= 6 {
                probabilities[y * 32 + x] = 1.0;
            }
        }
    }
    let boxes = DbPostprocessor
        .extract(&probabilities, [1, 1, 32, 32], [32, 32])
        .unwrap();
    assert_eq!(boxes.len(), 1);
    assert_eq!(
        boxes[0].corners,
        [[16.0, 6.0], [26.0, 16.0], [16.0, 26.0], [6.0, 16.0]]
    );
    assert_eq!(boxes[0].probability, 1.0);
}

#[test]
fn r_ocr_1__enclosed_background_hole_is_a_separate_db_contour() {
    let mut probabilities = vec![0.0; 32 * 32];
    for y in 5..=25 {
        for x in 5..=25 {
            if !(12..=16).contains(&x) || !(12..=16).contains(&y) {
                probabilities[y * 32 + x] = 1.0;
            }
        }
    }
    let boxes = DbPostprocessor
        .extract(&probabilities, [1, 1, 32, 32], [32, 32])
        .unwrap();
    assert_eq!(boxes.len(), 2);
}

#[test]
fn r_ocr_1__malformed_detector_shapes_probabilities_and_subthreshold_regions_are_refused_or_empty()
{
    use tallyline_adapter_ocr::api::model::DbError;
    let processor = DbPostprocessor;
    assert_eq!(
        processor.extract(&[0.5], [2, 1, 1, 1], [1, 1]),
        Err(DbError::Shape)
    );
    assert_eq!(
        processor.extract(&[0.5], [1, 1, 1, 1], [0, 1]),
        Err(DbError::Shape)
    );
    for value in [f32::NAN, f32::INFINITY, -0.1, 1.01] {
        assert_eq!(
            processor.extract(&[value], [1, 1, 1, 1], [1, 1]),
            Err(DbError::Probability)
        );
    }
    for value in [0.2, 0.44] {
        assert!(
            processor
                .extract(&vec![value; 64], [1, 1, 8, 8], [8, 8])
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn r_ocr_1__original_raster_crop_uses_the_detected_quadrilateral_without_an_intermediate_panel() {
    use tallyline_adapter_ocr::api::model::DetectionBox;
    use tallyline_shared_kernel::api::model::ImageRaster;
    let image = ImageRaster::new(4, 4, (0..16).map(|value| 0xff00_0000 | value).collect()).unwrap();
    let region = DetectionBox {
        corners: [[1.0, 1.0], [3.0, 1.0], [3.0, 3.0], [1.0, 3.0]],
        probability: 0.9,
    };
    let crop = region.rectify(&image).unwrap();
    assert_eq!([crop.width(), crop.height()], [2, 2]);
    assert_eq!(
        crop.argb(),
        [0xff00_0005, 0xff00_0006, 0xff00_0009, 0xff00_000a]
    );
}

#[test]
fn r_ocr_1__projective_crop_maps_destination_samples_to_the_original_trapezoid() {
    use tallyline_adapter_ocr::api::model::DetectionBox;
    use tallyline_shared_kernel::api::model::ImageRaster;
    let image = ImageRaster::new(
        7,
        7,
        (0..7)
            .flat_map(|y| (0..7).map(move |x| 0xff00_0000 | (10 * y + x)))
            .collect(),
    )
    .unwrap();
    let region = DetectionBox {
        corners: [[1.0, 1.0], [5.0, 1.0], [4.0, 5.0], [2.0, 5.0]],
        probability: 0.9,
    };
    let crop = region.rectify(&image).unwrap();
    assert_eq!([crop.width(), crop.height()], [4, 4]);
    assert_eq!(
        &crop.argb()[0..4],
        [0xff00_000b, 0xff00_000c, 0xff00_000d, 0xff00_000e]
    );
    assert_eq!(
        &crop.argb()[8..12],
        [0xff00_0026, 0xff00_0027, 0xff00_0028, 0xff00_0028]
    );
}

#[test]
fn r_ocr_1__tall_original_raster_crop_rotates_anticlockwise_and_invalid_regions_are_refused() {
    use tallyline_adapter_ocr::api::model::{DbError, DetectionBox};
    use tallyline_shared_kernel::api::model::ImageRaster;
    let image = ImageRaster::new(
        6,
        6,
        (0..6)
            .flat_map(|y| (0..6).map(move |x| 0xff00_0000 | (10 * y + x)))
            .collect(),
    )
    .unwrap();
    let mut region = DetectionBox {
        corners: [[1.0, 1.0], [3.0, 1.0], [3.0, 5.0], [1.0, 5.0]],
        probability: 0.9,
    };
    let crop = region.rectify(&image).unwrap();
    assert_eq!([crop.width(), crop.height()], [4, 2]);
    assert_eq!(
        crop.argb(),
        [
            0xff00_000c,
            0xff00_0016,
            0xff00_0020,
            0xff00_002a,
            0xff00_000b,
            0xff00_0015,
            0xff00_001f,
            0xff00_0029
        ]
    );
    for corners in [
        [[0.0, 0.0]; 4],
        [[0.0, 0.0], [2.0, 2.0], [2.0, 0.0], [0.0, 2.0]],
        [[f64::NAN, 1.0], [3.0, 1.0], [3.0, 5.0], [1.0, 5.0]],
        [[-1.0, 1.0], [3.0, 1.0], [3.0, 5.0], [1.0, 5.0]],
    ] {
        region.corners = corners;
        assert_eq!(region.rectify(&image), Err(DbError::Geometry));
    }
}
