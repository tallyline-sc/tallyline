// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Row-wise integer box reduction with complete edge coverage.

use super::model::{Downscaled, RawRaster, SetAsideReason};

pub(crate) struct BoxRows {
    original_width: u32,
    original_height: u32,
    width: u32,
    factor: u32,
    row: u32,
    sums: Vec<[u64; 3]>,
    counts: Vec<u64>,
    output: Vec<u8>,
}

impl BoxRows {
    pub(crate) fn new(width: u32, height: u32, budget: u64) -> Result<Self, SetAsideReason> {
        if width == 0 || height == 0 {
            return Err(SetAsideReason::DecodeFailed("empty dimensions".into()));
        }
        let mut low = 1;
        let mut high = width.max(height);
        while low < high {
            let candidate = low + (high - low) / 2;
            if u64::from(width.div_ceil(candidate)) * u64::from(height.div_ceil(candidate)) > budget
            {
                low = candidate + 1;
            } else {
                high = candidate;
            }
        }
        let factor = low;
        let target_width = width.div_ceil(factor);
        let target_height = height.div_ceil(factor);
        let count = usize::try_from(u64::from(target_width) * u64::from(target_height) * 3)
            .map_err(|_| SetAsideReason::TooManyPixels)?;
        Ok(Self {
            original_width: width,
            original_height: height,
            width: target_width,
            factor,
            row: 0,
            sums: if factor > 1 {
                vec![[0; 3]; target_width as usize]
            } else {
                Vec::new()
            },
            counts: if factor > 1 {
                vec![0; target_width as usize]
            } else {
                Vec::new()
            },
            output: Vec::with_capacity(count),
        })
    }

    pub(crate) fn push(&mut self, rgb: &[u8]) -> Result<(), SetAsideReason> {
        if rgb.len() != self.original_width as usize * 3 || self.row >= self.original_height {
            return Err(SetAsideReason::DecodeFailed("incomplete row".into()));
        }
        if self.factor == 1 {
            self.output.extend_from_slice(rgb);
        } else {
            for (x, pixel) in rgb.as_chunks::<3>().0.iter().enumerate() {
                let index = x / self.factor as usize;
                let sum = self
                    .sums
                    .get_mut(index)
                    .ok_or(SetAsideReason::TooManyPixels)?;
                for (channel, value) in sum.iter_mut().zip(pixel) {
                    *channel += u64::from(*value);
                }
                *self
                    .counts
                    .get_mut(index)
                    .ok_or(SetAsideReason::TooManyPixels)? += 1;
            }
            if (self.row + 1).is_multiple_of(self.factor) || self.row + 1 == self.original_height {
                for (sum, count) in self.sums.iter_mut().zip(&mut self.counts) {
                    for channel in sum.iter_mut() {
                        self.output.push(
                            u8::try_from((*channel + *count / 2) / *count).unwrap_or(u8::MAX),
                        );
                        *channel = 0;
                    }
                    *count = 0;
                }
            }
        }
        self.row += 1;
        Ok(())
    }

    pub(crate) fn finish(self) -> Result<RawRaster, SetAsideReason> {
        if self.row != self.original_height {
            return Err(SetAsideReason::DecodeFailed("incomplete image".into()));
        }
        Ok(RawRaster {
            width: self.width,
            height: self.original_height.div_ceil(self.factor),
            rgb: self.output,
            downscaled: (self.factor > 1).then_some(Downscaled {
                original_width: self.original_width,
                original_height: self.original_height,
                factor: self.factor,
            }),
        })
    }
}

pub(crate) fn reduce(raw: RawRaster, budget: u64) -> Result<RawRaster, SetAsideReason> {
    let mut boxes = BoxRows::new(raw.width, raw.height, budget)?;
    for row in raw.rgb.chunks_exact(raw.width as usize * 3) {
        boxes.push(row)?;
    }
    boxes.finish()
}
