// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! The actual approved font files used by desktop and harness rendering.

use egui::ColorImage;
use std::io::Cursor;
use tallyline_ui::api::model::FontFiles;

pub fn font_files() -> Result<FontFiles, String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/fonts");
    let read = |name: &str| std::fs::read(root.join(name)).map_err(|error| error.to_string());
    Ok(FontFiles {
        material_symbols: read("material-symbols/MaterialSymbolsRounded-Tallyline.ttf")?
            .try_into()
            .map_err(|error: &str| error.to_owned())?,
        plex_regular: read("ibm-plex/IBMPlexSans-Regular.ttf")?,
        plex_semibold: read("ibm-plex/IBMPlexSans-SemiBold.ttf")?,
        plex_bold: read("ibm-plex/IBMPlexSans-Bold.ttf")?,
        mono_regular: read("jetbrains-mono/JetBrainsMonoNL-Regular.ttf")?,
        mono_bold: read("jetbrains-mono/JetBrainsMonoNL-Bold.ttf")?,
    })
}

/// Decodes an unchanged bundled logo into RGBA pixels.
///
/// # Errors
/// Returns an invalid PNG or unsupported pixel format.
pub fn decode_logo(bytes: &[u8]) -> Result<ColorImage, png::DecodingError> {
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
    Ok(ColorImage::from_rgba_unmultiplied(
        [frame.width as usize, frame.height as usize],
        &rgba,
    ))
}
