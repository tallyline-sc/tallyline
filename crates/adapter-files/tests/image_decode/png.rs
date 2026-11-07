// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! PNG decode fixtures of 07 §4: bit depth, `cICP`, `iCCP`, `sRGB`, `gAMA`, `cHRM` and the hard limits.

#![cfg(test)]

use super::common;

use super::common::{
    DISPLAY_P3_D50, PngSpec, SRGB_D50, Trc, be_samples, bt709_chromaticities, decoded,
    exact_srgb_of_p3, icc_matrix_trc, map_samples_8, map_samples_16, max_abs_diff,
    p3_chromaticities, read_default, set_aside, source, srgb_eotf, to_display_p3_8,
};
use tallyline_adapter_files::api::model::{
    Encoding, ImageIntake as Intake, ImageIntakeSettings as IntakeLimits, SetAsideReason,
};
use tallyline_adapter_files::api::read_image as read;

fn sixteen_bit_of_source() -> Vec<u8> {
    be_samples(&map_samples_16(&source(), |v| v))
}

#[test]
fn an_untagged_8_bit_png_decodes_unchanged() {
    let src = source();
    let png = PngSpec::rgb8(&src.rgb).encode();
    assert_eq!(decoded(read_default(&png)), src);
}

#[test]
fn eight_and_sixteen_bit_pngs_of_one_image_differ_by_at_most_one() {
    let src = source();
    let eight = decoded(read_default(&PngSpec::rgb8(&src.rgb).encode()));
    let sixteen_data = sixteen_bit_of_source();
    let sixteen = decoded(read_default(&PngSpec::rgb16(&sixteen_data).encode()));
    assert!(max_abs_diff(&eight, &sixteen) <= 1);
}

#[test]
fn sixteen_bit_samples_are_scaled_not_truncated_to_the_high_byte() {
    let data = be_samples(&[511, 0x7f80, 0xffff].repeat((common::WIDTH * common::HEIGHT) as usize));
    let raster = decoded(read_default(&PngSpec::rgb16(&data).encode()));
    assert_eq!(&raster.rgb[0..3], &[2, 127, 255]);
}

#[test]
fn a_16_bit_png_with_a_linear_icc_profile_decodes_to_the_srgb_source_within_two() {
    let src = source();
    let data = be_samples(&map_samples_16(&src, srgb_eotf));
    let profile = icc_matrix_trc(SRGB_D50, &Trc::Gamma(1.0));
    let png = PngSpec::rgb16(&data).icc(profile).encode();
    let raster = decoded(read_default(&png));
    assert!(
        max_abs_diff(&raster, &src) <= 2,
        "max diff {}",
        max_abs_diff(&raster, &src)
    );
}

#[test]
fn an_8_bit_png_with_a_display_p3_profile_decodes_to_the_exact_conversion_within_two() {
    let data = to_display_p3_8(&source());
    let profile = icc_matrix_trc(DISPLAY_P3_D50, &Trc::Srgb);
    let png = PngSpec::rgb8(&data).icc(profile).encode();
    let raster = decoded(read_default(&png));
    let exact = exact_srgb_of_p3(&data);
    assert!(
        max_abs_diff(&raster, &exact) <= 2,
        "max diff {}",
        max_abs_diff(&raster, &exact)
    );
}

#[test]
fn an_icc_profile_with_a_pq_cicp_tag_is_set_aside() {
    let profile = match moxcms::ColorProfile::new_bt2020_pq().encode() {
        Ok(p) => p,
        Err(e) => panic!("encode: {e:?}"),
    };
    let data = sixteen_bit_of_source();
    let png = PngSpec::rgb16(&data).icc(profile).encode();
    assert_eq!(
        set_aside(&png),
        SetAsideReason::HdrOrUnsupportedEncoding(Encoding::IccHdr)
    );
}

#[test]
fn a_cicp_pq_png_is_never_read_as_srgb() {
    let data = sixteen_bit_of_source();
    let png = PngSpec::rgb16(&data)
        .chunk(b"cICP", &[9, 16, 0, 1])
        .encode();
    assert_eq!(
        set_aside(&png),
        SetAsideReason::HdrOrUnsupportedEncoding(Encoding::CicpHdr {
            primaries: 9,
            transfer: 16
        })
    );
}

#[test]
fn a_cicp_hlg_png_is_never_read_as_srgb() {
    let data = sixteen_bit_of_source();
    let png = PngSpec::rgb16(&data)
        .chunk(b"cICP", &[9, 18, 0, 1])
        .encode();
    assert_eq!(
        set_aside(&png),
        SetAsideReason::HdrOrUnsupportedEncoding(Encoding::CicpHdr {
            primaries: 9,
            transfer: 18
        })
    );
}

#[test]
fn a_cicp_srgb_png_decodes_unchanged() {
    let src = source();
    let png = PngSpec::rgb8(&src.rgb)
        .chunk(b"cICP", &[1, 13, 0, 1])
        .encode();
    assert_eq!(decoded(read_default(&png)), src);
}

#[test]
fn any_other_cicp_is_set_aside() {
    let src = source();
    for (cicp, full_range) in [
        ([12, 13, 0, 0], false),
        ([12, 13, 0, 1], true),
        ([1, 8, 0, 1], true),
    ] {
        let png = PngSpec::rgb8(&src.rgb).chunk(b"cICP", &cicp).encode();
        assert_eq!(
            set_aside(&png),
            SetAsideReason::HdrOrUnsupportedEncoding(Encoding::CicpOther {
                primaries: cicp[0],
                transfer: cicp[1],
                full_range,
            }),
            "cICP {cicp:?}"
        );
    }
}

#[test]
fn a_narrow_range_srgb_cicp_png_is_expanded_to_full_range() {
    let src = source();
    let eight = map_samples_8(&src, |v| (16.0 + v * 219.0) / 255.0);
    let png = PngSpec::rgb8(&eight)
        .chunk(b"cICP", &[1, 13, 0, 0])
        .encode();
    let raster = decoded(read_default(&png));
    assert!(
        max_abs_diff(&raster, &src) <= 1,
        "8 bit: {}",
        max_abs_diff(&raster, &src)
    );
    let sixteen = be_samples(&map_samples_16(&src, |v| {
        (16.0 + v * 219.0) * 256.0 / 65_535.0
    }));
    let png = PngSpec::rgb16(&sixteen)
        .chunk(b"cICP", &[1, 13, 0, 0])
        .encode();
    let raster = decoded(read_default(&png));
    assert!(
        max_abs_diff(&raster, &src) <= 1,
        "16 bit: {}",
        max_abs_diff(&raster, &src)
    );
}

#[test]
fn narrow_range_samples_outside_16_to_235_are_clipped() {
    let data = [0u8, 16, 255].repeat((common::WIDTH * common::HEIGHT) as usize);
    let png = PngSpec::rgb8(&data).chunk(b"cICP", &[1, 13, 0, 0]).encode();
    let raster = decoded(read_default(&png));
    assert_eq!(&raster.rgb[0..3], &[0, 0, 255]);
}

#[test]
fn a_cicp_with_non_zero_matrix_coefficients_is_not_read_as_srgb() {
    let src = source();
    let png = PngSpec::rgb8(&src.rgb)
        .chunk(b"cICP", &[1, 13, 1, 1])
        .encode();
    assert!(matches!(
        read_default(&png),
        Intake::SetAside(SetAsideReason::HdrOrUnsupportedEncoding(
            Encoding::MalformedColourMetadata(_)
        ))
    ));
}

#[test]
fn cicp_takes_precedence_over_iccp() {
    let src = source();
    let profile = icc_matrix_trc(DISPLAY_P3_D50, &Trc::Srgb);
    let png = PngSpec::rgb8(&src.rgb)
        .icc(profile)
        .chunk(b"cICP", &[1, 13, 0, 1])
        .encode();
    assert_eq!(decoded(read_default(&png)), src);
}

#[test]
fn srgb_takes_precedence_over_gama_and_chrm() {
    let src = source();
    let png = PngSpec::rgb8(&src.rgb)
        .chunk(b"sRGB", &[0])
        .chunk(b"gAMA", &100_000u32.to_be_bytes())
        .chunk(b"cHRM", &p3_chromaticities())
        .encode();
    assert_eq!(decoded(read_default(&png)), src);
}

#[test]
fn iccp_takes_precedence_over_srgb() {
    let src = source();
    let data = to_display_p3_8(&src);
    let profile = icc_matrix_trc(DISPLAY_P3_D50, &Trc::Srgb);
    let png = PngSpec::rgb8(&data)
        .icc(profile)
        .chunk(b"sRGB", &[0])
        .encode();
    let raster = decoded(read_default(&png));
    assert!(max_abs_diff(&raster, &exact_srgb_of_p3(&data)) <= 2);
    assert!(max_abs_diff(&raster, &src) > 2);
}

#[test]
fn a_linear_gama_is_applied() {
    let src = source();
    let data = be_samples(&map_samples_16(&src, srgb_eotf));
    let png = PngSpec::rgb16(&data)
        .chunk(b"gAMA", &100_000u32.to_be_bytes())
        .encode();
    let raster = decoded(read_default(&png));
    assert!(
        max_abs_diff(&raster, &src) <= 2,
        "max diff {}",
        max_abs_diff(&raster, &src)
    );
}

#[test]
fn a_gama_of_one_over_two_point_two_is_applied_as_a_pure_power_curve() {
    let src = source();
    let data = map_samples_8(&src, |v| srgb_eotf(v).powf(0.454_55));
    let png = PngSpec::rgb8(&data)
        .chunk(b"gAMA", &45_455u32.to_be_bytes())
        .encode();
    let raster = decoded(read_default(&png));
    assert!(
        max_abs_diff(&raster, &src) <= 2,
        "max diff {}",
        max_abs_diff(&raster, &src)
    );
}

#[test]
fn bt709_chrm_with_gama_decodes() {
    let src = source();
    let data = map_samples_8(&src, |v| srgb_eotf(v).powf(0.454_55));
    let png = PngSpec::rgb8(&data)
        .chunk(b"gAMA", &45_455u32.to_be_bytes())
        .chunk(b"cHRM", &bt709_chromaticities())
        .encode();
    let raster = decoded(read_default(&png));
    assert!(max_abs_diff(&raster, &src) <= 2);
}

#[test]
fn chrm_primaries_other_than_bt709_are_set_aside() {
    let src = source();
    let png = PngSpec::rgb8(&src.rgb)
        .chunk(b"gAMA", &45_455u32.to_be_bytes())
        .chunk(b"cHRM", &p3_chromaticities())
        .encode();
    assert_eq!(
        set_aside(&png),
        SetAsideReason::HdrOrUnsupportedEncoding(Encoding::ChromaticitiesNotBt709)
    );
}

#[test]
fn colour_chunks_the_decoder_drops_as_malformed_are_set_aside() {
    let src = source();
    let cases: [(&[u8; 4], Vec<u8>); 8] = [
        (b"cICP", vec![9, 16, 0]),
        (b"iCCP", b"p\0\0not a zlib stream".to_vec()),
        (b"gAMA", 0u32.to_be_bytes().to_vec()),
        (b"cHRM", vec![0; 31]),
        (b"cICP", vec![1, 13, 0, 2]),
        (b"sRGB", vec![4]),
        (b"mDCV", vec![0; 23]),
        (b"cLLI", vec![0; 7]),
    ];
    for (kind, body) in cases {
        let png = PngSpec::rgb8(&src.rgb).chunk(kind, &body).encode();
        let name = String::from_utf8_lossy(kind).into_owned();
        let reason = set_aside(&png);
        assert_eq!(
            reason,
            SetAsideReason::HdrOrUnsupportedEncoding(Encoding::MalformedColourMetadata(
                name.clone()
            )),
            "{name}"
        );
    }
}

#[test]
fn hdr_metadata_without_cicp_is_set_aside() {
    let src = source();
    let png = PngSpec::rgb8(&src.rgb)
        .chunk(b"cLLI", &[0, 0, 0x27, 0x10, 0, 0, 0x03, 0xe8])
        .encode();
    assert_eq!(
        set_aside(&png),
        SetAsideReason::HdrOrUnsupportedEncoding(Encoding::HdrMetadataWithoutCicp)
    );
}

#[test]
fn a_greyscale_png_decodes_to_equal_channels() {
    let grey: Vec<u8> = (0..common::WIDTH * common::HEIGHT)
        .map(|i| (i % 256) as u8)
        .collect();
    let png = PngSpec {
        color: png::ColorType::Grayscale,
        ..PngSpec::rgb8(&grey)
    }
    .encode();
    let raster = decoded(read_default(&png));
    assert_eq!(&raster.rgb[3..6], &[1, 1, 1]);
}

#[test]
fn a_truncated_png_is_set_aside() {
    let src = source();
    let png = PngSpec::rgb8(&src.rgb).encode();
    let cut = &png[..png.len() / 2];
    assert!(matches!(set_aside(cut), SetAsideReason::DecodeFailed(_)));
}

#[test]
fn a_header_declaring_too_many_pixels_is_set_aside_without_decoding() {
    let src = source();
    let mut png = PngSpec::rgb8(&src.rgb).encode();
    png.truncate(33);
    png[16..20].copy_from_slice(&20_000u32.to_be_bytes());
    png[20..24].copy_from_slice(&20_000u32.to_be_bytes());
    assert_eq!(set_aside(&png), SetAsideReason::TooManyPixels);
}

#[test]
fn an_oversized_iccp_chunk_is_set_aside_without_decoding() {
    let src = source();
    let mut body = b"p\0\0".to_vec();
    body.extend(std::iter::repeat_n(0xa5, 4096));
    let mut png = PngSpec::rgb8(&src.rgb).chunk(b"iCCP", &body).encode();
    png.truncate(png.len() - 40);
    let limits = IntakeLimits::new(200 * 1024 * 1024, 100_000_000, 1024, 20_000_000).unwrap();
    assert_eq!(
        read(&png, &limits),
        Intake::SetAside(SetAsideReason::ProfileTooLarge)
    );
}

#[test]
fn an_iccp_that_inflates_beyond_the_limit_is_set_aside() {
    let src = source();
    let png = PngSpec::rgb8(&src.rgb)
        .icc(vec![0; 4 * 1024 * 1024])
        .encode();
    assert!(png.len() < 64 * 1024);
    assert_eq!(set_aside(&png), SetAsideReason::ProfileTooLarge);
}

#[test]
fn a_file_above_the_size_limit_is_set_aside() {
    let src = source();
    let png = PngSpec::rgb8(&src.rgb).encode();
    let limits = IntakeLimits::new(100, 100_000_000, 1024 * 1024, 20_000_000).unwrap();
    assert_eq!(
        read(&png, &limits),
        Intake::SetAside(SetAsideReason::FileTooLarge)
    );
}

#[test]
fn pq_and_hlg_icc_curves_without_cicp_are_never_treated_as_sdr() {
    for (transfer, gamma) in [(16, 1.0_f64), (18, 1.0), (18, 1.2)] {
        let samples: Vec<u16> = (0..4096)
            .map(|i| {
                let signal = f64::from(i) / 4095.0;
                let linear = if transfer == 16 {
                    let p = signal.powf(32.0 / 2523.0);
                    ((p - 3424.0 / 4096.0).max(0.0) / (2413.0 / 128.0 - 2392.0 / 128.0 * p))
                        .powf(16384.0 / 2610.0)
                } else if signal <= 0.5 {
                    signal * signal / 3.0
                } else {
                    (((signal - 0.559_910_73) / 0.178_832_77).exp() + 0.284_668_92) / 12.0
                };
                (linear.powf(gamma) * 65535.0).round() as u16
            })
            .collect();
        let profile = icc_matrix_trc(SRGB_D50, &Trc::Samples(samples));
        assert!(
            moxcms::ColorProfile::new_from_slice(&profile)
                .unwrap()
                .cicp
                .is_none()
        );
        let image = PngSpec::rgb8(&source().rgb).icc(profile.clone()).encode();
        assert_eq!(
            set_aside(&image),
            SetAsideReason::HdrOrUnsupportedEncoding(Encoding::IccHdr),
            "transfer {transfer}, gamma {gamma}"
        );
        let jpeg = common::jpeg(
            &source().rgb,
            jpeg_encoder::SamplingFactor::R_4_4_4,
            false,
            Some(&profile),
        );
        assert_eq!(
            set_aside(&jpeg),
            SetAsideReason::HdrOrUnsupportedEncoding(Encoding::IccHdr),
            "transfer {transfer}, gamma {gamma}"
        );
    }
}
