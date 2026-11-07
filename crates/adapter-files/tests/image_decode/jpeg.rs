// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! JPEG decode fixtures of 07 §4: subsampling, progressive, the APP2 ICC profile and failures.

#![cfg(test)]

use super::common::{
    DISPLAY_P3_D50, SRGB_D50, Trc, decoded, icc_matrix_trc, jpeg, mean_abs_diff, read_default,
    set_aside, source, to_display_p3_8,
};
use jpeg_encoder::SamplingFactor;
use tallyline_adapter_files::api::model::{
    Encoding, ImageIntake as Intake, ImageIntakeSettings as IntakeLimits, SetAsideReason,
};
use tallyline_adapter_files::api::read_image as read;

const MAX_MEAN_ABS_ERROR: f64 = 3.0;

#[test]
fn baseline_and_progressive_jpegs_decode_to_the_source_within_tolerance() {
    let src = source();
    for (sampling, progressive) in [
        (SamplingFactor::R_4_4_4, false),
        (SamplingFactor::R_4_2_0, false),
        (SamplingFactor::R_4_2_0, true),
        (SamplingFactor::R_4_4_4, true),
    ] {
        let bytes = jpeg(&src.rgb, sampling, progressive, None);
        let raster = decoded(read_default(&bytes));
        let mae = mean_abs_diff(&raster, &src);
        assert!(
            mae <= MAX_MEAN_ABS_ERROR,
            "{sampling:?} progressive={progressive}: {mae}"
        );
    }
}

#[test]
fn an_app2_display_p3_profile_is_applied() {
    let src = source();
    let profile = icc_matrix_trc(DISPLAY_P3_D50, &Trc::Srgb);
    let bytes = jpeg(
        &to_display_p3_8(&src),
        SamplingFactor::R_4_4_4,
        false,
        Some(&profile),
    );
    let raster = decoded(read_default(&bytes));
    let mae = mean_abs_diff(&raster, &src);
    assert!(mae <= MAX_MEAN_ABS_ERROR, "{mae}");
}

#[test]
fn the_same_p3_pixels_without_their_profile_differ_visibly() {
    let src = source();
    let bytes = jpeg(&to_display_p3_8(&src), SamplingFactor::R_4_4_4, false, None);
    let raster = decoded(read_default(&bytes));
    assert!(mean_abs_diff(&raster, &src) > MAX_MEAN_ABS_ERROR);
}

#[test]
fn an_app2_srgb_profile_decodes_to_the_source() {
    let src = source();
    let profile = icc_matrix_trc(SRGB_D50, &Trc::Srgb);
    let bytes = jpeg(&src.rgb, SamplingFactor::R_4_2_0, true, Some(&profile));
    let raster = decoded(read_default(&bytes));
    assert!(mean_abs_diff(&raster, &src) <= MAX_MEAN_ABS_ERROR);
}

#[test]
fn an_app2_profile_with_a_pq_cicp_tag_is_set_aside() {
    let src = source();
    let profile = match moxcms::ColorProfile::new_bt2020_pq().encode() {
        Ok(p) => p,
        Err(e) => panic!("encode: {e:?}"),
    };
    let bytes = jpeg(&src.rgb, SamplingFactor::R_4_4_4, false, Some(&profile));
    assert_eq!(
        set_aside(&bytes),
        SetAsideReason::HdrOrUnsupportedEncoding(Encoding::IccHdr)
    );
}

#[test]
fn a_corrupt_app2_profile_sequence_is_set_aside() {
    let src = source();
    let profile = icc_matrix_trc(DISPLAY_P3_D50, &Trc::Srgb);
    let mut bytes = jpeg(&src.rgb, SamplingFactor::R_4_4_4, false, Some(&profile));
    let at = bytes
        .windows(12)
        .position(|w| w == b"ICC_PROFILE\0")
        .unwrap_or_else(|| panic!("no ICC marker"));
    bytes[at + 12] = 0;
    assert_eq!(
        set_aside(&bytes),
        SetAsideReason::HdrOrUnsupportedEncoding(Encoding::MalformedColourMetadata(
            "APP2 ICC_PROFILE".into()
        ))
    );
}

#[test]
fn a_truncated_jpeg_is_set_aside() {
    let src = source();
    for progressive in [false, true] {
        let bytes = jpeg(&src.rgb, SamplingFactor::R_4_2_0, progressive, None);
        let cut = &bytes[..bytes.len() * 2 / 3];
        assert!(
            matches!(set_aside(cut), SetAsideReason::DecodeFailed(_)),
            "progressive={progressive}"
        );
    }
}

#[test]
fn a_jpeg_above_the_pixel_limit_is_set_aside() {
    let src = source();
    let bytes = jpeg(&src.rgb, SamplingFactor::R_4_2_0, false, None);
    let limits = IntakeLimits::new(200 * 1024 * 1024, 100, 1024 * 1024, 100).unwrap();
    assert_eq!(
        read(&bytes, &limits),
        Intake::SetAside(SetAsideReason::TooManyPixels)
    );
}
