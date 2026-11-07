// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Decoding of immutable bundled artwork.

use crate::api::model::LogoRaster;
use std::io::Cursor;

/// Decodes an unchanged bundled logo into RGBA pixels.
///
/// # Errors
/// Returns an invalid PNG or unsupported pixel format.
pub fn decode_logo(bytes: &[u8]) -> Result<LogoRaster, png::DecodingError> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info()?;
    let size = reader
        .output_buffer_size()
        .ok_or(png::DecodingError::LimitsExceeded)?;
    let mut buffer = vec![0; size];
    let frame = reader.next_frame(&mut buffer)?;
    let pixels = buffer
        .get(..frame.buffer_size())
        .ok_or(png::DecodingError::LimitsExceeded)?;
    let mut rgba = Vec::new();
    match frame.color_type {
        png::ColorType::Rgba => rgba.extend_from_slice(pixels),
        png::ColorType::Rgb => {
            for pixel in pixels.as_chunks::<3>().0 {
                rgba.extend_from_slice(pixel);
                rgba.push(255);
            }
        }
        png::ColorType::Grayscale | png::ColorType::GrayscaleAlpha | png::ColorType::Indexed => {
            return Err(png::DecodingError::LimitsExceeded);
        }
    }
    Ok(LogoRaster {
        width: frame.width,
        height: frame.height,
        rgba,
    })
}
