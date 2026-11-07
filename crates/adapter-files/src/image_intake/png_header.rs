// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Allocation and colour-presence guards for PNG metadata.

use super::model::{Encoding, ImageIntakeSettings, SetAsideReason};
const SIGNATURE_LEN: usize = 8;
const CHUNK_OVERHEAD: usize = 12;
const COLOUR_CHUNKS: [&[u8; 4]; 7] = [
    b"cICP", b"iCCP", b"sRGB", b"gAMA", b"cHRM", b"mDCV", b"cLLI",
];
pub(crate) struct ChunkWalk {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) colour_chunks: Vec<([u8; 4], usize)>,
}

pub(crate) fn walk(
    bytes: &[u8],
    limits: &ImageIntakeSettings,
) -> Result<ChunkWalk, SetAsideReason> {
    let mut walk = ChunkWalk {
        width: 0,
        height: 0,
        colour_chunks: Vec::new(),
    };
    let mut at = SIGNATURE_LEN;
    let mut image_data = false;
    while let Some(head) = bytes.get(at..at + 8) {
        let (len_bytes, kind) = head.split_at(4);
        let len = len_bytes
            .try_into()
            .map(u32::from_be_bytes)
            .ok()
            .and_then(|l| usize::try_from(l).ok())
            .unwrap_or(usize::MAX);
        if kind == b"IEND" {
            break;
        }
        if kind == b"IDAT" {
            image_data = true;
        }
        if kind == b"IHDR"
            && let Some(dims) = bytes.get(at + 8..at + 16)
        {
            let (w, h) = dims.split_at(4);
            walk.width = w.try_into().map(u32::from_be_bytes).unwrap_or(0);
            walk.height = h.try_into().map(u32::from_be_bytes).unwrap_or(0);
            if u64::from(walk.width) * u64::from(walk.height) > limits.max_pixels {
                return Err(SetAsideReason::TooManyPixels);
            }
        }
        if let Some(colour) = COLOUR_CHUNKS.iter().find(|c| c.as_slice() == kind) {
            if image_data {
                return Err(unsupported(Encoding::MalformedColourMetadata(
                    String::from_utf8_lossy(kind).into_owned(),
                )));
            }
            check_fixed_chunk(kind, bytes, at, len)?;
            walk.colour_chunks.push((**colour, len));
        }
        if kind == b"iCCP" {
            if len > limits.max_profile_bytes {
                return Err(SetAsideReason::ProfileTooLarge);
            }
            let body = bytes
                .get(at + 8..at + 8 + len)
                .ok_or_else(|| unsupported(Encoding::MalformedColourMetadata("iCCP".into())))?;
            let name_end = body
                .iter()
                .position(|&b| b == 0)
                .filter(|&n| (1..=79).contains(&n))
                .ok_or_else(|| unsupported(Encoding::MalformedColourMetadata("iCCP".into())))?;
            if body.get(name_end + 1) != Some(&0) {
                return Err(unsupported(Encoding::MalformedColourMetadata(
                    "iCCP".into(),
                )));
            }
            let compressed = body
                .get(name_end + 2..)
                .ok_or_else(|| unsupported(Encoding::MalformedColourMetadata("iCCP".into())))?;
            match fdeflate::decompress_to_vec_bounded(compressed, limits.max_profile_bytes) {
                Ok(_) => (),
                Err(fdeflate::BoundedDecompressionError::OutputTooLarge { .. }) => {
                    return Err(SetAsideReason::ProfileTooLarge);
                }
                Err(fdeflate::BoundedDecompressionError::DecompressionError { .. }) => {
                    return Err(unsupported(Encoding::MalformedColourMetadata(
                        "iCCP".into(),
                    )));
                }
            }
        }
        let Some(next) = at
            .checked_add(CHUNK_OVERHEAD)
            .and_then(|n| n.checked_add(len))
        else {
            break;
        };
        if !image_data && next > super::MAX_HEADER_BYTES {
            return Err(SetAsideReason::HeaderTooLarge);
        }
        at = next;
    }
    Ok(walk)
}

pub(crate) fn check_parsed(walk: &ChunkWalk, info: &png::Info<'_>) -> Result<(), SetAsideReason> {
    for kind in COLOUR_CHUNKS {
        let count = walk.colour_chunks.iter().filter(|(k, _)| k == kind).count();
        let parsed = match kind {
            b"cICP" => info.coding_independent_code_points.is_some(),
            b"iCCP" => info.icc_profile.is_some(),
            b"sRGB" => info.srgb.is_some(),
            b"gAMA" => info.gama_chunk.is_some(),
            b"cHRM" => info.chrm_chunk.is_some(),
            b"mDCV" => info.mastering_display_color_volume.is_some(),
            b"cLLI" => info.content_light_level.is_some(),
            _ => false,
        };
        if count > 1 || (count == 1 && !parsed) {
            return Err(unsupported(Encoding::MalformedColourMetadata(
                String::from_utf8_lossy(kind).into_owned(),
            )));
        }
    }
    Ok(())
}

fn unsupported(encoding: Encoding) -> SetAsideReason {
    SetAsideReason::HdrOrUnsupportedEncoding(encoding)
}

fn check_fixed_chunk(
    kind: &[u8],
    bytes: &[u8],
    at: usize,
    len: usize,
) -> Result<(), SetAsideReason> {
    let expected = match kind {
        b"cICP" | b"gAMA" => 4,
        b"sRGB" => 1,
        b"cHRM" => 32,
        b"mDCV" => 24,
        b"cLLI" => 8,
        _ => return Ok(()),
    };
    let malformed = || {
        unsupported(Encoding::MalformedColourMetadata(
            String::from_utf8_lossy(kind).into_owned(),
        ))
    };
    if len != expected {
        return Err(malformed());
    }
    let body = bytes.get(at + 8..at + 8 + len).ok_or_else(malformed)?;
    match kind {
        b"cICP" if !matches!(body, [_, _, 0, 0 | 1]) => Err(malformed()),
        b"sRGB" if !matches!(body, [0..=3]) => Err(malformed()),
        b"gAMA" if body == [0, 0, 0, 0] => Err(malformed()),
        _ => Ok(()),
    }
}
