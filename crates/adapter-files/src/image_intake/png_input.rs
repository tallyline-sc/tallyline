// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! PNG intake: a chunk walk before `IDAT`, then `png` decoding and the colour step in R-CAP-8 precedence.

use std::io::Cursor;

use png::{BitDepth, ColorType, Transformations};

use super::colour::{self, Samples};
use super::model::{Encoding, ImageIntakeSettings, RawRaster, SetAsideReason};
use super::png_header::{check_parsed, walk};

const BT709_CHROMATICITIES: [(f32, f32); 4] =
    [(0.3127, 0.3290), (0.64, 0.33), (0.30, 0.60), (0.15, 0.06)];
const CHROMATICITY_TOLERANCE: f32 = 0.001;
const CICP_BT709_PRIMARIES: u8 = 1;
const CICP_SRGB_TRANSFER: u8 = 13;
const CICP_PQ_TRANSFER: u8 = 16;
const CICP_HLG_TRANSFER: u8 = 18;
const GAMA_SCALE: f64 = 100_000.0;

enum Plan {
    AsSrgb,
    NarrowRangeSrgb,
    Profile(Box<colour::ProfileConversion>),
    FileGamma(f64),
}

fn chromaticities_are_bt709(c: &png::SourceChromaticities) -> bool {
    let actual = [c.white, c.red, c.green, c.blue];
    actual
        .iter()
        .zip(BT709_CHROMATICITIES)
        .all(|((x, y), (ex, ey))| {
            (x.into_value() - ex).abs() <= CHROMATICITY_TOLERANCE
                && (y.into_value() - ey).abs() <= CHROMATICITY_TOLERANCE
        })
}

fn unsupported(encoding: Encoding) -> SetAsideReason {
    SetAsideReason::HdrOrUnsupportedEncoding(encoding)
}

fn plan(info: &png::Info<'_>, limits: &ImageIntakeSettings) -> Result<Plan, SetAsideReason> {
    if let Some(cicp) = info.coding_independent_code_points {
        let (primaries, transfer) = (cicp.color_primaries, cicp.transfer_function);
        if primaries == CICP_BT709_PRIMARIES && transfer == CICP_SRGB_TRANSFER {
            return Ok(if cicp.is_video_full_range_image {
                Plan::AsSrgb
            } else {
                Plan::NarrowRangeSrgb
            });
        }
        if transfer == CICP_PQ_TRANSFER || transfer == CICP_HLG_TRANSFER {
            return Err(unsupported(Encoding::CicpHdr {
                primaries,
                transfer,
            }));
        }
        return Err(unsupported(Encoding::CicpOther {
            primaries,
            transfer,
            full_range: cicp.is_video_full_range_image,
        }));
    }
    if info.mastering_display_color_volume.is_some() || info.content_light_level.is_some() {
        return Err(unsupported(Encoding::HdrMetadataWithoutCicp));
    }
    if let Some(icc) = &info.icc_profile {
        if icc.len() > limits.max_profile_bytes {
            return Err(SetAsideReason::ProfileTooLarge);
        }
        if matches!(
            info.color_type,
            ColorType::Grayscale | ColorType::GrayscaleAlpha
        ) {
            return Err(unsupported(Encoding::GreyWithIcc));
        }
        return colour::parse_profile(icc)
            .and_then(|p| colour::ProfileConversion::new(&p, info.bit_depth))
            .map(|p| Plan::Profile(Box::new(p)))
            .map_err(unsupported);
    }
    if info.srgb.is_some() {
        return Ok(Plan::AsSrgb);
    }
    if let Some(chrm) = &info.chrm_chunk
        && !chromaticities_are_bt709(chrm)
    {
        return Err(unsupported(Encoding::ChromaticitiesNotBt709));
    }
    if let Some(gamma) = info.gama_chunk {
        return Ok(Plan::FileGamma(f64::from(gamma.into_scaled()) / GAMA_SCALE));
    }
    Ok(Plan::AsSrgb)
}

fn to_rgb(buf: &[u8], color: ColorType, depth: BitDepth) -> Result<Samples, SetAsideReason> {
    let channels = match color {
        ColorType::Grayscale => 1,
        ColorType::GrayscaleAlpha => 2,
        ColorType::Rgb => 3,
        ColorType::Rgba => 4,
        ColorType::Indexed => {
            return Err(SetAsideReason::DecodeFailed("palette not expanded".into()));
        }
    };
    let pick = |px: &[u16]| -> [u16; 3] {
        match px {
            [g] | [g, _] => [*g, *g, *g],
            [r, g, b] | [r, g, b, _] => [*r, *g, *b],
            _ => [0, 0, 0],
        }
    };
    match depth {
        BitDepth::Eight => {
            let wide: Vec<u16> = buf.iter().map(|&b| u16::from(b)).collect();
            let rgb = wide.chunks_exact(channels).flat_map(pick);
            Ok(Samples::Eight(
                rgb.map(|v| u8::try_from(v).unwrap_or(u8::MAX)).collect(),
            ))
        }
        BitDepth::Sixteen => {
            let wide: Vec<u16> = buf
                .as_chunks::<2>()
                .0
                .iter()
                .map(|b| u16::from_be_bytes(*b))
                .collect();
            Ok(Samples::Sixteen(
                wide.chunks_exact(channels).flat_map(pick).collect(),
            ))
        }
        BitDepth::One | BitDepth::Two | BitDepth::Four => Err(SetAsideReason::DecodeFailed(
            "low bit depth not expanded".into(),
        )),
    }
}

/// Reads a PNG to interleaved 8-bit sRGB.
///
/// # Errors
///
/// The [`SetAsideReason`] when the file is set aside.
pub(crate) fn read(
    bytes: &[u8],
    limits: &ImageIntakeSettings,
) -> Result<RawRaster, SetAsideReason> {
    let walk = walk(bytes, limits)?;
    if u64::from(walk.width) * u64::from(walk.height) > limits.max_pixels {
        return Err(SetAsideReason::TooManyPixels);
    }
    if walk
        .colour_chunks
        .iter()
        .any(|(kind, len)| kind == b"iCCP" && *len > limits.max_profile_bytes)
    {
        return Err(SetAsideReason::ProfileTooLarge);
    }
    let pixels = u64::from(walk.width) * u64::from(walk.height);
    let retained = pixels.min(limits.pixel_budget).saturating_mul(7);
    let interlaced = bytes.get(28) == Some(&1);
    let workspace = if interlaced {
        pixels.saturating_mul(24)
    } else {
        u64::from(walk.width).saturating_mul(32)
    };
    if workspace
        .saturating_add(retained)
        .saturating_add(bytes.len() as u64)
        > super::MAX_CODEC_BYTES
    {
        return Err(SetAsideReason::MemoryLimit);
    }
    let decoder_limits = png::Limits {
        bytes: usize::try_from(limits.max_pixels.saturating_mul(8))
            .unwrap_or(usize::MAX)
            .saturating_add(limits.max_profile_bytes),
    };
    let mut decoder = png::Decoder::new_with_limits(Cursor::new(bytes), decoder_limits);
    decoder.set_transformations(Transformations::EXPAND);
    decoder.set_ignore_text_chunk(true);
    let failed = |e: png::DecodingError| SetAsideReason::DecodeFailed(e.to_string());
    let mut reader = decoder.read_info().map_err(failed)?;
    check_parsed(&walk, reader.info())?;
    let plan = plan(reader.info(), limits)?;
    if !reader.info().interlaced {
        let (color, depth) = reader.output_color_type();
        let mut boxes =
            super::box_rows::BoxRows::new(walk.width, walk.height, limits.pixel_budget)?;
        while let Some(row) = reader.next_row().map_err(failed)? {
            let samples = to_rgb(row.data(), color, depth)?;
            let rgb = match &plan {
                Plan::AsSrgb => colour::as_srgb(samples),
                Plan::NarrowRangeSrgb => colour::expand_narrow_range(samples),
                Plan::Profile(profile) => profile.apply(samples).map_err(unsupported)?,
                Plan::FileGamma(gamma) => colour::apply_file_gamma(*gamma, samples),
            };
            boxes.push(&rgb)?;
        }
        reader.finish().map_err(failed)?;
        return boxes.finish();
    }
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| SetAsideReason::DecodeFailed("output size overflows".into()))?;
    let mut buf = vec![0u8; size];
    let frame = reader.next_frame(&mut buf).map_err(failed)?;
    reader.finish().map_err(failed)?;
    let used = buf.get(..frame.buffer_size()).unwrap_or_default();
    let samples = to_rgb(used, frame.color_type, frame.bit_depth)?;
    let rgb = match plan {
        Plan::AsSrgb => colour::as_srgb(samples),
        Plan::NarrowRangeSrgb => colour::expand_narrow_range(samples),
        Plan::Profile(profile) => profile.apply(samples).map_err(unsupported)?,
        Plan::FileGamma(gamma) => colour::apply_file_gamma(gamma, samples),
    };
    super::box_rows::reduce(
        RawRaster {
            width: frame.width,
            height: frame.height,
            rgb,
            downscaled: None,
        },
        limits.pixel_budget,
    )
}
