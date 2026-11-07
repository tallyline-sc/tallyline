// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! JPEG intake: a marker walk for APP2 ICC segments, then `zune-jpeg` in strict mode and the ICC step.

use std::io::Cursor;

use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;
use zune_jpeg::JpegDecoder;

use super::colour::{self, Samples};
use super::model::{Encoding, ImageIntakeSettings, RawRaster, SetAsideReason};

const ICC_SIGNATURE: &[u8] = b"ICC_PROFILE\0";
const MARKER_APP2: u8 = 0xe2;
const MARKER_SOS: u8 = 0xda;
const MARKER_EOI: u8 = 0xd9;
const SOI_LEN: usize = 2;

fn is_standalone(marker: u8) -> bool {
    matches!(marker, 0x01 | 0xd0..=0xd7)
}

fn icc_segments_before_scan(
    bytes: &[u8],
    limits: &ImageIntakeSettings,
) -> Result<usize, SetAsideReason> {
    let mut count = 0;
    let mut profile_bytes = 0usize;
    let mut at = SOI_LEN;
    while let Some(&0xff) = bytes.get(at) {
        let Some(&marker) = bytes.get(at + 1) else {
            break;
        };
        if marker == 0xff {
            at += 1;
            continue;
        }
        if marker == MARKER_SOS || marker == MARKER_EOI {
            break;
        }
        if is_standalone(marker) {
            at += 2;
            continue;
        }
        let Some(len) = bytes
            .get(at + 2..at + 4)
            .and_then(|b| b.try_into().ok())
            .map(u16::from_be_bytes)
            .map(usize::from)
        else {
            break;
        };
        if marker == MARKER_APP2
            && bytes
                .get(at + 4..at + 4 + ICC_SIGNATURE.len())
                .is_some_and(|s| s == ICC_SIGNATURE)
        {
            profile_bytes = profile_bytes.saturating_add(len.saturating_sub(16));
            if profile_bytes > limits.max_profile_bytes {
                return Err(SetAsideReason::ProfileTooLarge);
            }
            count += 1;
        }
        if len < 2 {
            return Err(SetAsideReason::DecodeFailed(
                "invalid JPEG segment length".into(),
            ));
        }
        at += 2 + len;
        if at > super::MAX_HEADER_BYTES {
            return Err(SetAsideReason::HeaderTooLarge);
        }
    }
    Ok(count)
}

/// Reads a JPEG to interleaved 8-bit sRGB.
///
/// # Errors
///
/// The [`SetAsideReason`] when the file is set aside.
pub(crate) fn read(
    bytes: &[u8],
    limits: &ImageIntakeSettings,
) -> Result<RawRaster, SetAsideReason> {
    let icc_segments = icc_segments_before_scan(bytes, limits)?;
    let failed =
        |e: zune_jpeg::errors::DecodeErrors| SetAsideReason::DecodeFailed(format!("{e:?}"));
    let options = DecoderOptions::default()
        .set_strict_mode(true)
        .jpeg_set_out_colorspace(ColorSpace::RGB);
    let mut decoder = JpegDecoder::new_with_options(Cursor::new(bytes), options);
    decoder.decode_headers().map_err(failed)?;
    let info = decoder
        .info()
        .ok_or_else(|| SetAsideReason::DecodeFailed("no header".into()))?;
    let (width, height) = (u32::from(info.width), u32::from(info.height));
    if u64::from(width) * u64::from(height) > limits.max_pixels {
        return Err(SetAsideReason::TooManyPixels);
    }
    let pixels = u64::from(width) * u64::from(height);
    if pixels
        .saturating_mul(24)
        .saturating_add(limits.pixel_budget.min(pixels) * 7)
        .saturating_add(bytes.len() as u64)
        > super::MAX_CODEC_BYTES
    {
        return Err(SetAsideReason::MemoryLimit);
    }
    if decoder.output_buffer_size() != usize::try_from(pixels * 3).ok() {
        return Err(SetAsideReason::DecodeFailed(
            "invalid JPEG output size".into(),
        ));
    }
    if matches!(
        decoder.input_colorspace(),
        Some(ColorSpace::CMYK | ColorSpace::YCCK)
    ) {
        return Err(SetAsideReason::HdrOrUnsupportedEncoding(Encoding::Cmyk));
    }
    let profile = if icc_segments == 0 {
        None
    } else {
        let Some(icc) = decoder.icc_profile() else {
            return Err(SetAsideReason::HdrOrUnsupportedEncoding(
                Encoding::MalformedColourMetadata("APP2 ICC_PROFILE".into()),
            ));
        };
        if icc.len() > limits.max_profile_bytes {
            return Err(SetAsideReason::ProfileTooLarge);
        }
        Some(colour::parse_profile(&icc).map_err(SetAsideReason::HdrOrUnsupportedEncoding)?)
    };
    let pixels = decoder.decode().map_err(failed)?;
    let rgb = match profile {
        None => pixels,
        Some(p) => colour::apply_profile(&p, Samples::Eight(pixels))
            .map_err(SetAsideReason::HdrOrUnsupportedEncoding)?,
    };
    super::box_rows::reduce(
        RawRaster {
            width,
            height,
            rgb,
            downscaled: None,
        },
        limits.pixel_budget,
    )
}
