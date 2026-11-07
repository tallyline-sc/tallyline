// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Bounded reusable recognition tensor storage.

use tallyline_shared_kernel::api::model::ImageRaster;

mod linear;

pub(crate) const HEIGHT: usize = 48;
pub(crate) const MAX_WIDTH: usize = 3200;
const DETECTOR_MIN_SIDE: f64 = 736.0;
const DETECTOR_STRIDE: f64 = 32.0;
const DETECTOR_MAX_SIDE: usize = 4096;
const DETECTOR_MAX_PIXELS: usize = 4_194_304;
const CHANNEL_SHIFTS: [u32; 3] = [0, 8, 16];

/// Reusable per-worker storage for model input pixels.
#[derive(Default)]
pub struct OcrTensorBuffer {
    values: Vec<f32>,
}

/// A borrowed batch-one tensor in BGR channel-first order.
pub struct OcrInputTensor<'a> {
    /// The exact portable preprocessing algorithm used for these values.
    pub preprocessing: RecognitionPreprocessing,
    /// Batch, channel, height and padded width dimensions.
    pub shape: [usize; 4],
    /// Resized image width, excluding normalised-space padding.
    pub resized_width: usize,
    /// Contiguous NCHW values, valid until the worker reuses its buffer.
    pub values: &'a [f32],
}

/// An image cannot fit the bounded tensor without changing its aspect ratio.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TensorInputError;

impl OcrTensorBuffer {
    /// Builds height-48 recognition input with pixel-centre linear resize and zero padding.
    ///
    /// # Errors
    /// Returns `TensorInputError` when the padded width exceeds 3,200 or would shrink the line.
    pub fn recognition(
        &mut self,
        image: &ImageRaster,
        padded_width: usize,
    ) -> Result<OcrInputTensor<'_>, TensorInputError> {
        let width = (u64::from(image.width()) * HEIGHT as u64).div_ceil(u64::from(image.height()));
        let resized_width = usize::try_from(width).map_err(|_| TensorInputError)?;
        if padded_width == 0 || padded_width > MAX_WIDTH || resized_width > padded_width {
            return Err(TensorInputError);
        }
        self.values.resize(3 * HEIGHT * padded_width, 0.0);
        self.values.fill(0.0);
        for (channel, shift) in CHANNEL_SHIFTS.into_iter().enumerate() {
            for y in 0..HEIGHT {
                for x in 0..resized_width {
                    let code = linear::pixel(image, x, y, resized_width, HEIGHT, shift)
                        .ok_or(TensorInputError)?;
                    let index = channel * HEIGHT * padded_width + y * padded_width + x;
                    *self.values.get_mut(index).ok_or(TensorInputError)? =
                        (f32::from(code) / 255.0 - 0.5) / 0.5;
                }
            }
        }
        Ok(OcrInputTensor {
            preprocessing: RecognitionPreprocessing::Linear8uV1,
            shape: [1, 3, HEIGHT, padded_width],
            resized_width,
            values: &self.values,
        })
    }
}

/// A frozen recognition input algorithm for reproducible reader evaluation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecognitionPreprocessing {
    /// BGR, height 48, generic eight-bit fixed-point linear resize and normalised-zero padding.
    Linear8uV1,
}

impl RecognitionPreprocessing {
    /// Returns the stable profile identifier recorded with raw reader results.
    #[must_use]
    pub const fn identifier(self) -> &'static str {
        match self {
            Self::Linear8uV1 => "ppocr-bgr48-linear8u-v1",
        }
    }
}

/// A borrowed detector tensor with the source dimensions retained for box mapping.
pub struct DetectionInputTensor<'a> {
    /// Batch, BGR channels and stride-rounded detector height/width.
    pub shape: [usize; 4],
    /// The original raster width and height.
    pub source: [u32; 2],
    /// Reusable channel-first normalised detector values.
    pub values: &'a [f32],
}

impl OcrTensorBuffer {
    /// Builds a BGR detector tensor with shorter side at least 736 before stride rounding.
    ///
    /// # Errors
    /// Refuses a dimension above the caller's tile limit, limits above 4,096, or more
    /// than four megapixels; it never shrinks an image merely to fit the detector limit.
    pub fn detection(
        &mut self,
        image: &ImageRaster,
        tile_limit: usize,
    ) -> Result<DetectionInputTensor<'_>, TensorInputError> {
        let shorter = image.width().min(image.height());
        let ratio = (DETECTOR_MIN_SIDE / f64::from(shorter)).max(1.0);
        let dimension = |source: u32| {
            ((f64::from(source) * ratio).floor() / DETECTOR_STRIDE)
                .round_ties_even()
                .max(1.0) as usize
                * DETECTOR_STRIDE as usize
        };
        let width = dimension(image.width());
        let height = dimension(image.height());
        if tile_limit == 0
            || tile_limit > DETECTOR_MAX_SIDE
            || width > tile_limit
            || height > tile_limit
            || width
                .checked_mul(height)
                .is_none_or(|count| count > DETECTOR_MAX_PIXELS)
        {
            return Err(TensorInputError);
        }
        self.values.resize(3 * width * height, 0.0);
        let means = [0.485_f32, 0.456, 0.406];
        let deviations = [0.229_f32, 0.224, 0.225];
        for ((shift, mean), deviation) in CHANNEL_SHIFTS.into_iter().zip(means).zip(deviations) {
            let channel = match shift {
                0 => 0,
                8 => 1,
                _ => 2,
            };
            for y in 0..height {
                for x in 0..width {
                    let code =
                        linear::pixel(image, x, y, width, height, shift).ok_or(TensorInputError)?;
                    let index = channel * height * width + y * width + x;
                    *self.values.get_mut(index).ok_or(TensorInputError)? =
                        (f32::from(code) / 255.0 - mean) / deviation;
                }
            }
        }
        Ok(DetectionInputTensor {
            shape: [1, 3, height, width],
            source: [image.width(), image.height()],
            values: &self.values,
        })
    }
}
