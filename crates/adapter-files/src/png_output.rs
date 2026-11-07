// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Bounded sRGB PNG encoding for admitted rasters.

use std::io::Write;
use tallyline_shared_kernel::api::model::ImageRaster;

/// PNG encoding exceeded its allocation or byte bound, or the codec refused the raster.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PngOutputError;

/// Encodes an admitted raster without scaling, within four megapixels and 32 MiB of PNG bytes.
/// Privacy admission belongs to the caller; encoding does not infer a safe image extent.
///
/// # Errors
/// Refuses zero or raised byte bounds, excessive dimensions, allocation failure and codec errors.
pub fn encode_raster_png(raster: &ImageRaster, maximum: usize) -> Result<Vec<u8>, PngOutputError> {
    if raster.argb().len() > 4_194_304 {
        return Err(PngOutputError);
    }
    let mut rgb = Vec::new();
    rgb.try_reserve_exact(raster.argb().len() * 3)
        .map_err(|_| PngOutputError)?;
    for pixel in raster.argb() {
        let [_, r, g, b] = pixel.to_be_bytes();
        rgb.extend_from_slice(&[r, g, b]);
    }
    encode_rgb_png(raster.width(), raster.height(), &rgb, maximum)
}

pub(crate) fn encode_rgb_png(
    width: u32,
    height: u32,
    rgb: &[u8],
    maximum: usize,
) -> Result<Vec<u8>, PngOutputError> {
    if maximum == 0 || maximum > 32 * 1024 * 1024 {
        return Err(PngOutputError);
    }
    let mut output = BoundedOutput {
        bytes: Vec::new(),
        maximum,
    };
    {
        let mut encoder = png::Encoder::new(&mut output, width, height);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
        let mut writer = encoder.write_header().map_err(|_| PngOutputError)?;
        writer.write_image_data(rgb).map_err(|_| PngOutputError)?;
        writer.finish().map_err(|_| PngOutputError)?;
    }
    Ok(output.bytes)
}

struct BoundedOutput {
    bytes: Vec<u8>,
    maximum: usize,
}
impl Write for BoundedOutput {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > self.maximum.saturating_sub(self.bytes.len()) {
            return Err(std::io::Error::other("PNG exceeds its byte bound"));
        }
        self.bytes
            .try_reserve(bytes.len())
            .map_err(std::io::Error::other)?;
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
