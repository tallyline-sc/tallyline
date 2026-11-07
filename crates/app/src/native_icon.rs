// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! The fixed native window identity.

use egui::IconData;
use std::io;

/// Decodes the approved fixed-colour Line icon for native window chrome.
///
/// # Errors
/// Returns an invalid bundled PNG or unsupported pixel format.
pub fn native_window_icon() -> io::Result<IconData> {
    let raster = tallyline_adapter_files::api::decode_logo(include_bytes!(
        "../../../branding/tallyline-icon-256.png"
    ))
    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    Ok(IconData {
        rgba: raster.rgba,
        width: raster.width,
        height: raster.height,
    })
}
