// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Conversion of decoded samples to 8-bit sRGB: ICC profiles through `moxcms`, a PNG `gAMA` by formula.

use moxcms::{
    ColorProfile, DataColorSpace, Layout, RenderingIntent, TransferCharacteristics,
    TransformOptions,
};

use super::model::Encoding;

/// Interleaved RGB samples at the decoder's bit depth.
pub(crate) enum Samples {
    /// Eight bits per sample.
    Eight(Vec<u8>),
    /// Sixteen bits per sample, full range.
    Sixteen(Vec<u16>),
}

/// Scales a full-range 16-bit sample to 8 bits with rounding.
pub(crate) fn scale_16_to_8(v: u16) -> u8 {
    let scaled = (u32::from(v) * 255 + 32_767) / 65_535;
    u8::try_from(scaled).unwrap_or(u8::MAX)
}

/// Returns the samples as 8-bit values without a colour conversion.
pub(crate) fn as_srgb(samples: Samples) -> Vec<u8> {
    match samples {
        Samples::Eight(v) => v,
        Samples::Sixteen(v) => v.into_iter().map(scale_16_to_8).collect(),
    }
}

const NARROW_BLACK_8: f64 = 16.0;
const NARROW_SPAN_8: f64 = 219.0;
const SIXTEEN_BIT_SHIFT: f64 = 256.0;

/// Expands narrow-range samples (black 16, white 235 at 8 bits, ITU-T H.273) to full range and returns 8-bit values.
pub(crate) fn expand_narrow_range(samples: Samples) -> Vec<u8> {
    let expand = |v: f64, scale: f64| to_u8((v / scale - NARROW_BLACK_8) / NARROW_SPAN_8);
    match samples {
        Samples::Eight(v) => v.into_iter().map(|s| expand(f64::from(s), 1.0)).collect(),
        Samples::Sixteen(v) => v
            .into_iter()
            .map(|s| expand(f64::from(s), SIXTEEN_BIT_SHIFT))
            .collect(),
    }
}

fn srgb_oetf(linear: f64) -> f64 {
    let v = linear.clamp(0.0, 1.0);
    if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

fn to_u8(unit: f64) -> u8 {
    (unit * 255.0).round().clamp(0.0, 255.0) as u8
}

/// Applies a PNG `gAMA` file gamma: the stored sample is `linear ^ file_gamma`.
pub(crate) fn apply_file_gamma(file_gamma: f64, samples: Samples) -> Vec<u8> {
    let decode = |unit: f64| to_u8(srgb_oetf(unit.powf(1.0 / file_gamma)));
    match samples {
        Samples::Eight(v) => {
            let table: Vec<u8> = (0..=u8::MAX)
                .map(|s| decode(f64::from(s) / 255.0))
                .collect();
            v.into_iter()
                .map(|s| table.get(usize::from(s)).copied().unwrap_or(0))
                .collect()
        }
        Samples::Sixteen(v) => v
            .into_iter()
            .map(|s| decode(f64::from(s) / 65_535.0))
            .collect(),
    }
}

/// Parses an ICC profile and refuses those that are not RGB or that declare a PQ or HLG transfer.
///
/// # Errors
///
/// The [`Encoding`] that makes the profile unusable.
pub(crate) fn parse_profile(bytes: &[u8]) -> Result<ColorProfile, Encoding> {
    let profile = ColorProfile::new_from_slice_with_options(
        bytes,
        moxcms::ParsingOptions {
            max_profile_size: bytes.len().saturating_add(1),
            max_allowed_clut_size: bytes.len(),
            max_allowed_trc_size: bytes.len() / 2,
        },
    )
    .map_err(|e| Encoding::IccUnusable(format!("{e:?}")))?;
    if let Some(cicp) = &profile.cicp
        && matches!(
            cicp.transfer_characteristics,
            TransferCharacteristics::Smpte2084 | TransferCharacteristics::Hlg
        )
    {
        return Err(Encoding::IccHdr);
    }
    if super::hdr_curve::is_hdr(&profile) {
        return Err(Encoding::IccHdr);
    }
    if profile.color_space != DataColorSpace::Rgb {
        return Err(Encoding::IccNotRgb);
    }
    Ok(profile)
}

pub(crate) enum ProfileConversion {
    Eight(std::sync::Arc<moxcms::Transform8BitExecutor>),
    Sixteen(std::sync::Arc<moxcms::Transform16BitExecutor>),
}

impl ProfileConversion {
    pub(crate) fn new(profile: &ColorProfile, depth: png::BitDepth) -> Result<Self, Encoding> {
        let srgb = ColorProfile::new_srgb();
        let options = TransformOptions {
            rendering_intent: RenderingIntent::RelativeColorimetric,
            ..TransformOptions::default()
        };
        if depth == png::BitDepth::Sixteen {
            profile
                .create_transform_16bit(Layout::Rgb, &srgb, Layout::Rgb, options)
                .map(Self::Sixteen)
                .map_err(unusable)
        } else {
            profile
                .create_transform_8bit(Layout::Rgb, &srgb, Layout::Rgb, options)
                .map(Self::Eight)
                .map_err(unusable)
        }
    }

    pub(crate) fn apply(&self, samples: Samples) -> Result<Vec<u8>, Encoding> {
        match (self, samples) {
            (Self::Eight(transform), Samples::Eight(src)) => {
                let mut dst = vec![0; src.len()];
                transform.transform(&src, &mut dst).map_err(unusable)?;
                Ok(dst)
            }
            (Self::Sixteen(transform), Samples::Sixteen(src)) => {
                let mut dst = vec![0; src.len()];
                transform.transform(&src, &mut dst).map_err(unusable)?;
                Ok(dst.into_iter().map(scale_16_to_8).collect())
            }
            (Self::Eight(_), Samples::Sixteen(_)) | (Self::Sixteen(_), Samples::Eight(_)) => Err(
                Encoding::IccUnusable("sample depth differs from ICC transform".into()),
            ),
        }
    }
}

fn unusable(error: moxcms::CmsError) -> Encoding {
    Encoding::IccUnusable(format!("{error:?}"))
}

pub(crate) fn apply_profile(profile: &ColorProfile, samples: Samples) -> Result<Vec<u8>, Encoding> {
    let depth = match &samples {
        Samples::Eight(_) => png::BitDepth::Eight,
        Samples::Sixteen(_) => png::BitDepth::Sixteen,
    };
    ProfileConversion::new(profile, depth)?.apply(samples)
}
