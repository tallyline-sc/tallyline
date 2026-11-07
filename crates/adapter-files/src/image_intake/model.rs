// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Image intake outcomes and bounded release settings.

use tallyline_shared_kernel::api::model::ImageRaster;

/// A supported SDR container identified from its bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageFormat {
    /// Portable Network Graphics.
    Png,
    /// JPEG image.
    Jpeg,
    /// Windows bitmap.
    Bmp,
}

/// The result of bounded image intake.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImageIntake {
    /// A supported image converted to sRGB.
    Decoded(DecodedImage),
    /// An input set aside with a concrete reason.
    SetAside(SetAsideReason),
}

/// The decoded raster and any reduction finding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedImage {
    /// Immutable eight-bit sRGB pixels.
    pub raster: ImageRaster,
    /// Format identified by content.
    pub format: ImageFormat,
    /// Original dimensions when the pixel budget required integer box reduction.
    pub downscaled: Option<Downscaled>,
}

/// Original dimensions and the integer box factor used during intake.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Downscaled {
    /// Original pixel width.
    pub original_width: u32,
    /// Original pixel height.
    pub original_height: u32,
    /// Integer box side length in source pixels.
    pub factor: u32,
}
/// HDR container formats that are recognised but never decoded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HdrContainer {
    /// JPEG XR (`.jxr`).
    JpegXr,
    /// AVIF (`.avif`).
    Avif,
    /// HEIF other than AVIF (`.heic`, `.heif`).
    Heif,
    /// OpenEXR (`.exr`).
    OpenExr,
}

/// Why a file is set aside.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SetAsideReason {
    /// The source file could not be read.
    FileUnreadable(String),
    /// Image metadata exceeds the bounded header size.
    HeaderTooLarge,
    /// The conservative codec allocation estimate exceeds the release memory bound.
    MemoryLimit,
    /// A recognised HDR container without a supported SDR decode.
    HdrFormat(HdrContainer),
    /// "HDR or unsupported colour encoding – use an SDR copy".
    HdrOrUnsupportedEncoding(Encoding),
    /// The content is none of the supported or recognised formats.
    UnsupportedFormat,
    /// The file is larger than the file-size limit.
    FileTooLarge,
    /// The header declares more pixels than the hard limit.
    TooManyPixels,
    /// An embedded colour profile is larger than the profile limit.
    ProfileTooLarge,
    /// The decoder failed, for example on a truncated file.
    DecodeFailed(String),
}

/// The colour encoding that made a file unreadable as sRGB.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Encoding {
    /// A PNG `cICP` with a PQ (16) or HLG (18) transfer.
    CicpHdr {
        /// Colour primaries code point.
        primaries: u8,
        /// Transfer characteristics code point.
        transfer: u8,
    },
    /// A PNG `cICP` outside the supported BT.709/sRGB encodings.
    CicpOther {
        /// Colour primaries code point.
        primaries: u8,
        /// Transfer characteristics code point.
        transfer: u8,
        /// Whether the image is full range.
        full_range: bool,
    },
    /// An ICC profile that declares or follows a standard PQ or HLG transfer.
    IccHdr,
    /// An ICC profile the colour library cannot read or convert.
    IccUnusable(String),
    /// An ICC profile of a colour space other than RGB.
    IccNotRgb,
    /// A PNG `cHRM` whose primaries are not BT.709.
    ChromaticitiesNotBt709,
    /// A colour chunk or marker is present that the decoder dropped as malformed.
    MalformedColourMetadata(String),
    /// PNG `mDCV` or `cLLI` HDR metadata without a `cICP`.
    HdrMetadataWithoutCicp,
    /// A JPEG in CMYK or YCCK.
    Cmyk,
    /// A BMP colour space other than sRGB or the Windows default.
    BmpColourSpace(u32),
    /// A greyscale image with an embedded ICC profile.
    GreyWithIcc,
}

/// Validated release settings for untrusted image intake.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImageIntakeSettings {
    pub(crate) max_file_bytes: u64,
    pub(crate) max_pixels: u64,
    pub(crate) max_profile_bytes: usize,
    pub(crate) pixel_budget: u64,
}

/// An intake setting is zero or exceeds its hard release maximum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidImageIntakeSettings;

impl ImageIntakeSettings {
    /// Creates bounded settings no larger than 200 MiB, 100 MP and a 1 MiB ICC profile.
    ///
    /// # Errors
    /// Returns an error for zero values, raised hard limits or a budget above the pixel limit.
    pub fn new(
        max_file_bytes: u64,
        max_pixels: u64,
        max_profile_bytes: usize,
        pixel_budget: u64,
    ) -> Result<Self, InvalidImageIntakeSettings> {
        let defaults = Self::default();
        if max_file_bytes == 0
            || max_file_bytes > defaults.max_file_bytes
            || max_pixels == 0
            || max_pixels > defaults.max_pixels
            || max_profile_bytes == 0
            || max_profile_bytes > defaults.max_profile_bytes
            || pixel_budget == 0
            || pixel_budget > max_pixels
        {
            return Err(InvalidImageIntakeSettings);
        }
        Ok(Self {
            max_file_bytes,
            max_pixels,
            max_profile_bytes,
            pixel_budget,
        })
    }
}

impl Default for ImageIntakeSettings {
    fn default() -> Self {
        Self {
            max_file_bytes: 200 * 1024 * 1024,
            max_pixels: 100_000_000,
            max_profile_bytes: 1024 * 1024,
            pixel_budget: 20_000_000,
        }
    }
}

pub(crate) struct RawRaster {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) rgb: Vec<u8>,
    pub(crate) downscaled: Option<Downscaled>,
}

impl SetAsideReason {
    /// Returns capture advice suitable for an import result.
    #[must_use]
    pub fn message(&self) -> &'static str {
        match self {
            Self::HdrOrUnsupportedEncoding(_) => {
                "HDR or unsupported colour encoding – use an SDR copy"
            }
            Self::HdrFormat(HdrContainer::JpegXr) => {
                "HDR screenshot format (JPEG XR) – not supported; use the SDR PNG copy if available"
            }
            Self::HdrFormat(HdrContainer::Avif) => {
                "HDR screenshot format (AVIF) – not supported; use an SDR PNG or JPEG capture"
            }
            Self::HdrFormat(HdrContainer::Heif) => {
                "HDR screenshot format (HEIF) – not supported; use an SDR PNG or JPEG capture"
            }
            Self::HdrFormat(HdrContainer::OpenExr) => {
                "HDR screenshot format (EXR) – not supported; use an SDR PNG or JPEG capture"
            }
            Self::UnsupportedFormat => "Image format not supported – use PNG, JPEG or BMP",
            Self::FileTooLarge => "Image exceeds the file-size limit – use a smaller SDR copy",
            Self::TooManyPixels => {
                "Image dimensions exceed the hard pixel limit – use a smaller SDR copy"
            }
            Self::ProfileTooLarge => {
                "Image colour profile exceeds the profile limit – use an SDR sRGB copy"
            }
            Self::HeaderTooLarge => "Image metadata exceeds the header limit – use an SDR copy",
            Self::MemoryLimit => "Image exceeds the decoder memory bound – use a smaller SDR copy",
            Self::DecodeFailed(_) => {
                "Image could not be decoded – it may still be being written or may be damaged"
            }
            Self::FileUnreadable(_) => {
                "Image file could not be read – check that it is available and readable"
            }
        }
    }

    /// Returns whether a stable-file gate may retry this failure.
    #[must_use]
    pub const fn retryable(&self) -> bool {
        matches!(self, Self::DecodeFailed(_) | Self::FileUnreadable(_))
    }
}
