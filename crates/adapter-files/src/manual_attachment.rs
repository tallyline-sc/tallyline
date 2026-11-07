// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Bounded private-region preparation and confirmation-gated PNG encoding.

use crate::api::model::{ImageIntake, ImageIntakeSettings, SetAsideReason};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;
use tallyline_reporting::api::model::{
    AttachmentBinding, AttachmentRegion, AttachmentTime, ImageHash, Report,
};
use tallyline_shared_kernel::api::model::{ImageRaster, Timestamp};

const MAX_BASE64_BYTES: usize = 10_000_000;
const MAX_WIDTH: u32 = 1920;
const MAX_HEIGHT: u32 = 1080;

/// A closed image-preparation or encoding failure without paths or image data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AttachmentImageError {
    /// Source bytes or modification time could not be read within the input bound.
    FileUnavailable,
    /// The normal decoder set aside the input.
    UnsupportedImage(SetAsideReason),
    /// The input budget reduced the source, so source-pixel geometry is unsafe.
    ReducedSource,
    /// The selected rectangle exceeds the decoded source.
    InvalidRegion,
    /// The Report has not confirmed this exact image, geometry and observation time.
    Unconfirmed,
    /// The output could not be encoded within its byte bound.
    EncodingLimit,
}

/// A full-size selected-region preview with no retained original screenshot.
#[derive(Clone, Debug)]
pub struct AttachmentPreview {
    pub(crate) binding: AttachmentBinding,
    pub(crate) time: AttachmentTime,
    pub(crate) raster: ImageRaster,
}
impl AttachmentPreview {
    /// Returns the complete source hash and source-pixel rectangle.
    #[must_use]
    pub const fn binding(&self) -> AttachmentBinding {
        self.binding
    }
    /// Returns uncertain imported file time, without substituting the current time.
    #[must_use]
    pub const fn time(&self) -> AttachmentTime {
        self.time
    }
    /// Returns the selected pixels at source scale for full-size review.
    #[must_use]
    pub const fn raster(&self) -> &ImageRaster {
        &self.raster
    }
}

/// A tighten-only limit on the encoded screenshot's Base64 length.
#[derive(Clone, Copy, Debug)]
pub struct AttachmentEncodingSettings {
    max_base64_bytes: usize,
}
impl AttachmentEncodingSettings {
    /// Accepts a nonzero Base64 bound no greater than 10,000,000 bytes.
    ///
    /// # Errors
    /// Refuses a zero or loosened limit.
    pub fn new(max_base64_bytes: usize) -> Result<Self, AttachmentImageError> {
        if max_base64_bytes == 0 || max_base64_bytes > MAX_BASE64_BYTES {
            return Err(AttachmentImageError::EncodingLimit);
        }
        Ok(Self { max_base64_bytes })
    }
}
impl Default for AttachmentEncodingSettings {
    fn default() -> Self {
        Self {
            max_base64_bytes: MAX_BASE64_BYTES,
        }
    }
}

/// A confirmation-gated PNG and its standard padded Base64 payload.
pub struct EncodedAttachment {
    request: tallyline_workflows::api::AttachmentRequest,
    png: Vec<u8>,
    base64: String,
}
impl EncodedAttachment {
    /// Checks request identity against the current Report before exposing this result for use.
    #[must_use]
    pub fn is_current(&self, report: &Report) -> bool {
        self.request.is_current(report) && report.attachment_gate().is_ok()
    }
    /// Borrows only the selected PNG after rechecking the current durable Report state.
    ///
    /// # Errors
    /// Refuses delayed results after any Report edit, changed binding or removed evidence.
    pub fn png_for(&self, current: &Report) -> Result<&[u8], AttachmentImageError> {
        if !self.is_current(current) {
            return Err(AttachmentImageError::Unconfirmed);
        }
        Ok(&self.png)
    }
    /// Borrows Base64 without a data-URL prefix after rechecking the current durable Report.
    ///
    /// # Errors
    /// Refuses delayed results after any Report edit, changed binding or removed evidence.
    pub fn base64_for(&self, current: &Report) -> Result<&str, AttachmentImageError> {
        if !self.is_current(current) {
            return Err(AttachmentImageError::Unconfirmed);
        }
        Ok(&self.base64)
    }
}
impl std::fmt::Debug for EncodedAttachment {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EncodedAttachment")
            .field("png_bytes", &self.png.len())
            .field("base64_bytes", &self.base64.len())
            .finish()
    }
}

/// Reads a bounded PNG/JPEG/BMP source once and retains only the selected source-scale region.
/// Run decoding and hashing on an app-owned bounded CPU worker.
///
/// # Errors
/// Refuses unreadable, unsupported, reduced or out-of-bounds input.
pub fn prepare_manual_attachment_file(
    path: &Path,
    region: AttachmentRegion,
    settings: &ImageIntakeSettings,
) -> Result<AttachmentPreview, AttachmentImageError> {
    let file = std::fs::File::open(path).map_err(|_| AttachmentImageError::FileUnavailable)?;
    let metadata = file
        .metadata()
        .map_err(|_| AttachmentImageError::FileUnavailable)?;
    if metadata.len() > settings.max_file_bytes {
        return Err(AttachmentImageError::FileUnavailable);
    }
    let modified = metadata
        .modified()
        .map_err(|_| AttachmentImageError::FileUnavailable)?
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| AttachmentImageError::FileUnavailable)?;
    let at = Timestamp::from_millis(
        u64::try_from(modified.as_millis()).map_err(|_| AttachmentImageError::FileUnavailable)?,
    );
    let mut bytes = Vec::new();
    file.take(settings.max_file_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| AttachmentImageError::FileUnavailable)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > settings.max_file_bytes {
        return Err(AttachmentImageError::FileUnavailable);
    }
    let hash = ImageHash::new(Sha256::digest(&bytes).into());
    let image = match crate::image_intake::read_image(&bytes, settings) {
        ImageIntake::Decoded(image) => image,
        ImageIntake::SetAside(reason) => {
            return Err(AttachmentImageError::UnsupportedImage(reason));
        }
    };
    if image.downscaled.is_some() {
        return Err(AttachmentImageError::ReducedSource);
    }
    let [x, y, width, height] = region.rectangle();
    if x + width > image.raster.width() || y + height > image.raster.height() {
        return Err(AttachmentImageError::InvalidRegion);
    }
    let stride =
        usize::try_from(image.raster.width()).map_err(|_| AttachmentImageError::InvalidRegion)?;
    let pixels = image
        .raster
        .argb()
        .chunks_exact(stride)
        .skip(y as usize)
        .take(height as usize)
        .flat_map(|row| row.iter().skip(x as usize).take(width as usize).copied())
        .collect();
    let raster =
        ImageRaster::new(width, height, pixels).map_err(|_| AttachmentImageError::InvalidRegion)?;
    Ok(AttachmentPreview {
        binding: AttachmentBinding::new(hash, region),
        time: AttachmentTime::ImportedFile(at),
        raster,
    })
}

/// Encodes only a Report-confirmed region within 1920×1080 and the Base64 byte bound.
/// Run scaling and encoding on an app-owned bounded CPU worker.
///
/// # Errors
/// Refuses unconfirmed or changed evidence and an output above the configured bound.
pub fn encode_manual_attachment(
    report: &Report,
    preview: &AttachmentPreview,
    settings: &AttachmentEncodingSettings,
) -> Result<EncodedAttachment, AttachmentImageError> {
    let attachment = report
        .attachment()
        .ok_or(AttachmentImageError::Unconfirmed)?;
    if report.attachment_gate().is_err() || attachment.binding() != preview.binding {
        return Err(AttachmentImageError::Unconfirmed);
    }
    let raster = preview.raster();
    let rgb: Vec<u8> = raster
        .argb()
        .iter()
        .flat_map(|pixel| {
            let [_, r, g, b] = pixel.to_be_bytes();
            [r, g, b]
        })
        .collect();
    let image = image::RgbImage::from_raw(raster.width(), raster.height(), rgb)
        .ok_or(AttachmentImageError::InvalidRegion)?;
    let numerator = u64::from(MAX_WIDTH).min(u64::from(raster.width()));
    let (width, height) = if u64::from(raster.height()) * numerator
        <= u64::from(MAX_HEIGHT) * u64::from(raster.width())
    {
        (
            u32::try_from(numerator).map_err(|_| AttachmentImageError::InvalidRegion)?,
            u32::try_from(u64::from(raster.height()) * numerator / u64::from(raster.width()))
                .map_err(|_| AttachmentImageError::InvalidRegion)?
                .max(1),
        )
    } else {
        (
            u32::try_from(
                u64::from(raster.width()) * u64::from(MAX_HEIGHT) / u64::from(raster.height()),
            )
            .map_err(|_| AttachmentImageError::InvalidRegion)?
            .max(1),
            MAX_HEIGHT,
        )
    };
    let image = if (width, height) == image.dimensions() {
        image
    } else {
        image::imageops::resize(&image, width, height, image::imageops::FilterType::Triangle)
    };
    let output = crate::png_output::encode_rgb_png(
        width,
        height,
        image.as_raw(),
        settings.max_base64_bytes / 4 * 3,
    )
    .map_err(|_| AttachmentImageError::EncodingLimit)?;
    let base64 = base64(&output);
    if base64.len() > settings.max_base64_bytes {
        return Err(AttachmentImageError::EncodingLimit);
    }
    Ok(EncodedAttachment {
        request: tallyline_workflows::api::AttachmentRequest::new(report)
            .ok_or(AttachmentImageError::Unconfirmed)?,
        png: output,
        base64,
    })
}

fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let character =
        |index: u8| char::from(ALPHABET.get(usize::from(index)).copied().unwrap_or(b'='));
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk.first().copied().unwrap_or(0);
        let second = chunk.get(1).copied();
        let third = chunk.get(2).copied();
        encoded.push(character(first >> 2));
        encoded.push(character((first & 3) << 4 | second.unwrap_or(0) >> 4));
        encoded.push(second.map_or('=', |value| {
            character((value & 15) << 2 | third.unwrap_or(0) >> 6)
        }));
        encoded.push(third.map_or('=', |value| character(value & 63)));
    }
    encoded
}
