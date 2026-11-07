// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! BMP intake: the V4/V5 colour-space check, then the BMP codec of `image`.

use std::io::Cursor;

use image::ImageDecoder;
use image::codecs::bmp::BmpDecoder;

use super::model::{Encoding, ImageIntakeSettings, RawRaster, SetAsideReason};

const FILE_HEADER_LEN: usize = 14;
const V4_HEADER_LEN: u32 = 108;
const CS_TYPE_OFFSET: usize = FILE_HEADER_LEN + 56;
const CALIBRATION_RANGE: std::ops::Range<usize> = CS_TYPE_OFFSET + 4..CS_TYPE_OFFSET + 4 + 48;
const LCS_CALIBRATED_RGB: u32 = 0;
const LCS_SRGB: u32 = 0x7352_4742;
const LCS_WINDOWS_COLOR_SPACE: u32 = 0x5769_6e20;

fn le_u32(bytes: &[u8], at: usize) -> Option<u32> {
    bytes
        .get(at..at + 4)
        .and_then(|b| b.try_into().ok())
        .map(u32::from_le_bytes)
}

fn check_colour_space(bytes: &[u8]) -> Result<(), SetAsideReason> {
    let Some(header_len) = le_u32(bytes, FILE_HEADER_LEN) else {
        return Ok(());
    };
    if header_len < V4_HEADER_LEN {
        return Ok(());
    }
    let Some(cs_type) = le_u32(bytes, CS_TYPE_OFFSET) else {
        return Ok(());
    };
    let empty_calibration = bytes
        .get(CALIBRATION_RANGE)
        .is_some_and(|c| c.iter().all(|&b| b == 0));
    match cs_type {
        LCS_SRGB | LCS_WINDOWS_COLOR_SPACE => Ok(()),
        LCS_CALIBRATED_RGB if empty_calibration => Ok(()),
        other => Err(SetAsideReason::HdrOrUnsupportedEncoding(
            Encoding::BmpColourSpace(other),
        )),
    }
}

/// Reads a BMP to interleaved 8-bit sRGB.
///
/// # Errors
///
/// The [`SetAsideReason`] when the file is set aside.
pub(crate) fn read(
    bytes: &[u8],
    limits: &ImageIntakeSettings,
) -> Result<RawRaster, SetAsideReason> {
    let header_len = le_u32(bytes, FILE_HEADER_LEN).unwrap_or(0);
    let offset = le_u32(bytes, 10).unwrap_or(0);
    if header_len as usize > super::MAX_HEADER_BYTES || offset as usize > super::MAX_HEADER_BYTES {
        return Err(SetAsideReason::HeaderTooLarge);
    }
    let declared = if header_len == 12 {
        bytes
            .get(18..22)
            .and_then(|b| b.try_into().ok())
            .map(|[a, b, c, d]: [u8; 4]| {
                u64::from(u16::from_le_bytes([a, b])) * u64::from(u16::from_le_bytes([c, d]))
            })
    } else {
        le_u32(bytes, 18).zip(le_u32(bytes, 22)).map(|(w, h)| {
            u64::from(w) * u64::from(i32::from_le_bytes(h.to_le_bytes()).unsigned_abs())
        })
    };
    if declared.is_some_and(|pixels| pixels > limits.max_pixels) {
        return Err(SetAsideReason::TooManyPixels);
    }
    check_colour_space(bytes)?;
    let failed = |e: image::ImageError| SetAsideReason::DecodeFailed(e.to_string());
    let decoder = BmpDecoder::new(Cursor::new(bytes)).map_err(failed)?;
    let (width, height) = decoder.dimensions();
    if u64::from(width) * u64::from(height) > limits.max_pixels {
        return Err(SetAsideReason::TooManyPixels);
    }
    let pixels = u64::from(width) * u64::from(height);
    if pixels
        .saturating_mul(16)
        .saturating_add(limits.pixel_budget.min(pixels) * 7)
        .saturating_add(bytes.len() as u64)
        > super::MAX_CODEC_BYTES
    {
        return Err(SetAsideReason::MemoryLimit);
    }
    let image = image::DynamicImage::from_decoder(decoder).map_err(failed)?;
    super::box_rows::reduce(
        RawRaster {
            width,
            height,
            rgb: image.into_rgb8().into_raw(),
            downscaled: None,
        },
        limits.pixel_budget,
    )
}
