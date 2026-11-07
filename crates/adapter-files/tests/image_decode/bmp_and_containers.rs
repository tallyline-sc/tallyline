// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! BMP fixtures and the R-CAP-8 handling of HDR containers and misnamed files.

#![cfg(test)]

use super::common::{HEIGHT, WIDTH, decoded, jpeg, read_default, source};
use jpeg_encoder::SamplingFactor;
use tallyline_adapter_files::api::model::{
    Encoding, HdrContainer, ImageIntake as Intake, SetAsideReason,
};

use super::common::SrgbRaster;

const LCS_SRGB: u32 = 0x7352_4742;
const LCS_WINDOWS_COLOR_SPACE: u32 = 0x5769_6e20;
const LCS_CALIBRATED_RGB: u32 = 0;
const PROFILE_EMBEDDED: u32 = 0x4d42_4544;

fn bmp_v5(src: &SrgbRaster, cs_type: u32, gamma: u32) -> Vec<u8> {
    let row_len = (src.width * 3).div_ceil(4) * 4;
    let pixel_bytes = row_len * src.height;
    let offset = 14 + 124;
    let mut out = Vec::new();
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&(offset + pixel_bytes).to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&offset.to_le_bytes());
    out.extend_from_slice(&124u32.to_le_bytes());
    out.extend_from_slice(&src.width.to_le_bytes());
    out.extend_from_slice(&src.height.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&24u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&pixel_bytes.to_le_bytes());
    out.extend_from_slice(&[0; 16]);
    out.extend_from_slice(&[0; 16]);
    out.extend_from_slice(&cs_type.to_le_bytes());
    out.extend_from_slice(&[0; 36]);
    out.extend_from_slice(&gamma.to_le_bytes());
    out.extend_from_slice(&gamma.to_le_bytes());
    out.extend_from_slice(&gamma.to_le_bytes());
    out.extend_from_slice(&[0; 16]);
    assert_eq!(out.len(), offset as usize);
    for y in (0..src.height).rev() {
        let start = (y * src.width * 3) as usize;
        let row = &src.rgb[start..start + (src.width * 3) as usize];
        for [r, g, b] in row.as_chunks::<3>().0 {
            out.extend_from_slice(&[*b, *g, *r]);
        }
        out.extend(std::iter::repeat_n(0, (row_len - src.width * 3) as usize));
    }
    out
}

#[test]
fn a_24_bit_bmp_from_the_image_encoder_decodes_unchanged() {
    let src = source();
    let mut bytes = Vec::new();
    let mut encoder = image::codecs::bmp::BmpEncoder::new(&mut bytes);
    if let Err(e) = encoder.encode(&src.rgb, WIDTH, HEIGHT, image::ExtendedColorType::Rgb8) {
        panic!("bmp: {e}");
    }
    assert_eq!(decoded(read_default(&bytes)), src);
}

#[test]
fn a_v5_bmp_in_srgb_or_the_windows_colour_space_decodes_unchanged() {
    let src = source();
    for cs in [LCS_SRGB, LCS_WINDOWS_COLOR_SPACE] {
        assert_eq!(decoded(read_default(&bmp_v5(&src, cs, 0))), src, "{cs:#x}");
    }
}

#[test]
fn a_v5_bmp_with_empty_calibration_counts_as_srgb() {
    let src = source();
    assert_eq!(
        decoded(read_default(&bmp_v5(&src, LCS_CALIBRATED_RGB, 0))),
        src
    );
}

#[test]
fn a_v5_bmp_with_calibration_or_an_embedded_profile_is_set_aside() {
    let src = source();
    for (cs, gamma) in [(LCS_CALIBRATED_RGB, 0x0002_3333), (PROFILE_EMBEDDED, 0)] {
        assert_eq!(
            read_default(&bmp_v5(&src, cs, gamma)),
            Intake::SetAside(SetAsideReason::HdrOrUnsupportedEncoding(
                Encoding::BmpColourSpace(cs)
            )),
            "{cs:#x}"
        );
    }
}

#[test]
fn hdr_container_stubs_are_set_aside_with_the_hdr_format_reason() {
    let avif = b"\0\0\0\x1cftypavif\0\0\0\0avifmif1miaf\0\0\0\x08meta".to_vec();
    let cases: [(&[u8], HdrContainer); 4] = [
        (b"II\xbc\x01\x08\0\0\0\0\0\0\0", HdrContainer::JpegXr),
        (&avif, HdrContainer::Avif),
        (b"\0\0\0\x18ftypheic\0\0\0\0mif1heic", HdrContainer::Heif),
        (b"\x76\x2f\x31\x01\x02\0\0\0channels", HdrContainer::OpenExr),
    ];
    for (bytes, container) in cases {
        assert_eq!(
            read_default(bytes),
            Intake::SetAside(SetAsideReason::HdrFormat(container)),
            "{container:?}"
        );
    }
}

#[test]
fn jpeg_data_in_a_file_named_png_is_decoded_as_jpeg() {
    let src = source();
    let bytes = jpeg(&src.rgb, SamplingFactor::R_4_4_4, false, None);
    let raster = decoded(read_default(&bytes));
    assert_eq!((raster.width, raster.height), (WIDTH, HEIGHT));
}

#[test]
fn unknown_content_is_set_aside_as_an_unsupported_format() {
    assert_eq!(
        read_default(b"GIF89a\x01\0\x01\0"),
        Intake::SetAside(SetAsideReason::UnsupportedFormat)
    );
}
