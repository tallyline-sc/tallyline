// SPDX-FileCopyrightText: 2000-2008 Intel Corporation
// SPDX-FileCopyrightText: 2017 Intel Corporation
// SPDX-FileCopyrightText: 2009 Willow Garage Inc.
// SPDX-FileCopyrightText: 2014-2015 Itseez Inc.
// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later AND BSD-3-Clause

//! Portable generic eight-bit linear interpolation.

use tallyline_shared_kernel::api::model::ImageRaster;

const COEFFICIENT_SCALE: f32 = 2048.0;
const HORIZONTAL_PRECISION_DROP: u32 = 4;
const VERTICAL_PRODUCT_SHIFT: u32 = 16;
const OUTPUT_PRECISION_DROP: u32 = 2;
const OUTPUT_ROUNDING: i32 = 2;

enum Axis {
    Horizontal,
    Vertical,
}

struct Sample {
    first: usize,
    second: usize,
    first_weight: i32,
    second_weight: i32,
}

pub(super) fn pixel(
    image: &ImageRaster,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    shift: u32,
) -> Option<u8> {
    let source_width = usize::try_from(image.width()).ok()?;
    let source_height = usize::try_from(image.height()).ok()?;
    let horizontal = sample(x, source_width, width, Axis::Horizontal);
    let vertical = sample(y, source_height, height, Axis::Vertical);
    let code = |row: usize, column: usize| -> Option<i32> {
        let argb = image.argb().get(row * source_width + column)?;
        i32::try_from((argb >> shift) & 255).ok()
    };
    let row = |y| -> Option<i32> {
        Some(
            code(y, horizontal.first)? * horizontal.first_weight
                + code(y, horizontal.second)? * horizontal.second_weight,
        )
    };
    let upper = row(vertical.first)? >> HORIZONTAL_PRECISION_DROP;
    let lower = row(vertical.second)? >> HORIZONTAL_PRECISION_DROP;
    let upper = (vertical.first_weight * upper) >> VERTICAL_PRODUCT_SHIFT;
    let lower = (vertical.second_weight * lower) >> VERTICAL_PRODUCT_SHIFT;
    u8::try_from((upper + lower + OUTPUT_ROUNDING) >> OUTPUT_PRECISION_DROP).ok()
}

fn sample(destination: usize, source_size: usize, target_size: usize, axis: Axis) -> Sample {
    let scale = source_size as f64 / target_size as f64;
    let position = ((destination as f64 + 0.5) * scale - 0.5) as f32;
    let base = position.floor() as i64;
    let last = source_size as i64 - 1;
    let phase = match axis {
        Axis::Horizontal if base < 0 || base >= last => 0.0,
        Axis::Horizontal | Axis::Vertical => position - base as f32,
    };
    Sample {
        first: base.clamp(0, last) as usize,
        second: (base + 1).clamp(0, last) as usize,
        first_weight: ((1.0 - phase) * COEFFICIENT_SCALE).round_ties_even() as i32,
        second_weight: (phase * COEFFICIENT_SCALE).round_ties_even() as i32,
    }
}
