// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Immutable image pixels shared by capture and recognition.

/// An immutable row-major raster of eight-bit sRGB ARGB pixels.
#[derive(Clone, PartialEq, Eq)]
pub struct ImageRaster {
    width: u32,
    height: u32,
    argb: Vec<u32>,
}

/// Dimensions do not describe a nonempty complete pixel buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidRaster;

impl ImageRaster {
    /// Creates a raster with exactly one ARGB word per pixel.
    ///
    /// # Errors
    /// Returns `InvalidRaster` for empty dimensions, overflow or an incomplete buffer.
    pub fn new(width: u32, height: u32, argb: Vec<u32>) -> Result<Self, InvalidRaster> {
        let count = u64::from(width) * u64::from(height);
        if width == 0 || height == 0 || usize::try_from(count).ok() != Some(argb.len()) {
            return Err(InvalidRaster);
        }
        Ok(Self {
            width,
            height,
            argb,
        })
    }

    /// Returns the pixel width.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Returns the pixel height.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Returns read-only ARGB pixels in row order.
    #[must_use]
    pub fn argb(&self) -> &[u32] {
        &self.argb
    }
}

impl std::fmt::Debug for ImageRaster {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ImageRaster")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("pixels", &self.argb.len())
            .finish()
    }
}
