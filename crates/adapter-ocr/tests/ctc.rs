// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Public CTC decoding contracts.

#![expect(
    non_snake_case,
    reason = "Requirement tags contain a double underscore."
)]

use tallyline_adapter_ocr::api::model::CtcDecoder;

#[test]
fn r_ocr_1__ctc_preserves_repeated_digits_separated_by_blank_and_the_best_frame_probability() {
    let decoder = CtcDecoder::new(vec!["1".to_owned(), "2".to_owned(), " ".to_owned()]).unwrap();
    let reading = decoder
        .decode(
            &[
                0.05, 0.80, 0.10, 0.05, 0.01, 0.95, 0.02, 0.02, 0.90, 0.05, 0.03, 0.02, 0.02, 0.85,
                0.10, 0.03, 0.01, 0.03, 0.04, 0.92, 0.01, 0.03, 0.94, 0.02,
            ],
            [1, 6, 4],
        )
        .unwrap();
    assert_eq!(reading.text, "11 2");
    assert_eq!(reading.characters.len(), 4);
    assert_eq!(reading.characters[0].probability, 0.95);
    assert_eq!(reading.characters[0].frames, 0..2);
    assert_eq!(reading.characters[1].frames, 3..4);
}
#[test]
fn r_ocr_1__equal_labels_in_distinct_model_classes_are_preserved() {
    let decoder = CtcDecoder::new(vec!["1".to_owned(), "1".to_owned()]).unwrap();
    assert_eq!(
        decoder
            .decode(&[0.0, 0.9, 0.1, 0.0, 0.1, 0.9], [1, 2, 3])
            .unwrap()
            .text,
        "11"
    );
}

#[test]
fn r_ocr_1__invalid_model_shapes_and_probabilities_are_refused() {
    use tallyline_adapter_ocr::api::model::CtcError;
    let decoder = CtcDecoder::new(vec!["1".to_owned()]).unwrap();
    for shape in [
        [2, 1, 2],
        [1, 0, 2],
        [1, 1, 3],
        [1, 2, 2],
        [1, usize::MAX, 2],
    ] {
        assert_eq!(
            decoder.decode(&[0.1, 0.9], shape),
            Err(CtcError::InvalidShape)
        );
    }
    for probability in [f32::NAN, f32::INFINITY, -0.1, 1.1] {
        assert_eq!(
            decoder.decode(&[probability, 0.9], [1, 1, 2]),
            Err(CtcError::InvalidProbability)
        );
    }
}
