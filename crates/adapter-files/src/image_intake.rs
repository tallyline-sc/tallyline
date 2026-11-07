// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Bounded image decoding and colour conversion.

mod bmp_input;
mod box_rows;
mod colour;
mod format;
mod hdr_curve;
mod jpeg_input;
pub(crate) mod model;
mod png_input;

pub(crate) const MAX_HEADER_BYTES: usize = 4 * 1024 * 1024;
pub(crate) const MAX_CODEC_BYTES: u64 = 1024 * 1024 * 1024;

use format::{Format, detect};
use model::{ImageIntake, ImageIntakeSettings, SetAsideReason};

/// Reads image bytes into sRGB or sets them aside with a reason.
#[must_use]
pub fn read_image(bytes: &[u8], settings: &ImageIntakeSettings) -> ImageIntake {
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > settings.max_file_bytes {
        return ImageIntake::SetAside(SetAsideReason::FileTooLarge);
    }
    let (format, result) = match detect(bytes) {
        Format::Hdr(container) => {
            return ImageIntake::SetAside(SetAsideReason::HdrFormat(container));
        }
        Format::Png => (model::ImageFormat::Png, png_input::read(bytes, settings)),
        Format::Bmp => (model::ImageFormat::Bmp, bmp_input::read(bytes, settings)),
        Format::Jpeg => (model::ImageFormat::Jpeg, jpeg_input::read(bytes, settings)),
        Format::Unknown => return ImageIntake::SetAside(SetAsideReason::UnsupportedFormat),
    };
    match result {
        Ok(raw) => {
            let pixels = raw
                .rgb
                .as_chunks::<3>()
                .0
                .iter()
                .map(|&[r, g, b]| u32::from_be_bytes([255, r, g, b]))
                .collect();
            match tallyline_shared_kernel::api::model::ImageRaster::new(
                raw.width, raw.height, pixels,
            ) {
                Ok(raster) => ImageIntake::Decoded(model::DecodedImage {
                    raster,
                    format,
                    downscaled: raw.downscaled,
                }),
                Err(_) => ImageIntake::SetAside(SetAsideReason::DecodeFailed(
                    "invalid raster dimensions".into(),
                )),
            }
        }
        Err(reason) => ImageIntake::SetAside(reason),
    }
}

/// Reads a file through the same bounded image intake as byte-based sources.
#[must_use]
pub fn read_image_file(path: &std::path::Path, settings: &ImageIntakeSettings) -> ImageIntake {
    use std::io::Read;
    let read = || -> Result<Vec<u8>, SetAsideReason> {
        let file =
            std::fs::File::open(path).map_err(|e| SetAsideReason::FileUnreadable(e.to_string()))?;
        let metadata = file
            .metadata()
            .map_err(|e| SetAsideReason::FileUnreadable(e.to_string()))?;
        if metadata.len() > settings.max_file_bytes {
            return Err(SetAsideReason::FileTooLarge);
        }
        let mut bytes = Vec::new();
        file.take(settings.max_file_bytes + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| SetAsideReason::FileUnreadable(e.to_string()))?;
        if bytes.len() as u64 > settings.max_file_bytes {
            return Err(SetAsideReason::FileTooLarge);
        }
        Ok(bytes)
    };
    match read() {
        Ok(bytes) => read_image(&bytes, settings),
        Err(reason) => ImageIntake::SetAside(reason),
    }
}
mod png_header;
