// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Filesystem adapter results.

pub use crate::config::{LoadedSettings, SettingsIssue};
pub use crate::layout::{DirectoryIssue, DirectoryIssueKind};

/// The RGBA pixels of immutable bundled legal artwork.
pub struct LogoRaster {
    /// Pixel width.
    pub width: u32,
    /// Pixel height.
    pub height: u32,
    /// Unpremultiplied red, green, blue and alpha bytes in row order.
    pub rgba: Vec<u8>,
}
pub use crate::game_config::LoadedGamePreferences;

/// Exact configuration input for a coordinated migration backup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigBackupSource {
    /// Validated supported schema of these bytes.
    pub schema: u32,
    /// Original bytes, retained without reserialisation.
    pub bytes: Vec<u8>,
}
pub use crate::image_intake::model::{
    DecodedImage, Downscaled, Encoding, HdrContainer, ImageFormat, ImageIntake,
    ImageIntakeSettings, InvalidImageIntakeSettings, SetAsideReason,
};
pub use crate::manual_attachment::{
    AttachmentEncodingSettings, AttachmentImageError, AttachmentPreview, EncodedAttachment,
};

pub use crate::png_output::PngOutputError;
pub use crate::private_artifact::PrivateArtifactError;
