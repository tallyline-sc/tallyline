// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Format detection from the file content, never from the extension (R-CAP-8).

use super::model::HdrContainer;

/// The format a file's leading bytes identify.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// PNG signature.
    Png,
    /// JPEG start-of-image marker followed by another marker.
    Jpeg,
    /// Windows bitmap file header.
    Bmp,
    /// A recognised HDR container.
    Hdr(HdrContainer),
    /// Anything else.
    Unknown,
}

const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
const JPEG_SOI: &[u8] = b"\xff\xd8\xff";
const BMP_SIGNATURE: &[u8] = b"BM";
const JPEG_XR_SIGNATURES: [&[u8]; 2] = [b"II\xbc\x01", b"II\xbc\x00"];
const OPENEXR_SIGNATURE: &[u8] = b"\x76\x2f\x31\x01";
const AVIF_BRANDS: [&[u8; 4]; 2] = [b"avif", b"avis"];
const HEIF_BRANDS: [&[u8; 4]; 6] = [b"heic", b"heix", b"heim", b"heis", b"hevc", b"hevx"];
const BMP_HEADER_LEN: usize = 14;

/// Identifies the format of `bytes` by its magic bytes.
#[must_use]
pub fn detect(bytes: &[u8]) -> Format {
    if bytes.starts_with(PNG_SIGNATURE) {
        return Format::Png;
    }
    if bytes.starts_with(JPEG_SOI) {
        return Format::Jpeg;
    }
    if JPEG_XR_SIGNATURES.iter().any(|sig| bytes.starts_with(sig)) {
        return Format::Hdr(HdrContainer::JpegXr);
    }
    if bytes.starts_with(OPENEXR_SIGNATURE) {
        return Format::Hdr(HdrContainer::OpenExr);
    }
    if let Some(container) = iso_bmff_container(bytes) {
        return Format::Hdr(container);
    }
    if bytes.starts_with(BMP_SIGNATURE) && bytes.len() >= BMP_HEADER_LEN {
        return Format::Bmp;
    }
    Format::Unknown
}

fn iso_bmff_container(bytes: &[u8]) -> Option<HdrContainer> {
    let size_field: [u8; 4] = bytes.get(0..4)?.try_into().ok()?;
    if bytes.get(4..8)? != b"ftyp" {
        return None;
    }
    let box_len = usize::try_from(u32::from_be_bytes(size_field)).ok()?;
    let end = box_len.min(bytes.len());
    let major: &[u8; 4] = bytes.get(8..12)?.try_into().ok()?;
    let compatible = bytes.get(16..end).unwrap_or_default();
    let mut brands = std::iter::once(major).chain(compatible.as_chunks::<4>().0);
    if brands.clone().any(|brand| AVIF_BRANDS.contains(&brand)) {
        return Some(HdrContainer::Avif);
    }
    if brands.any(|brand| HEIF_BRANDS.contains(&brand)) {
        return Some(HdrContainer::Heif);
    }
    None
}
