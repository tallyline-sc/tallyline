// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Bounded area filtering over projective source footprints.

use crate::image_raster::ImageRaster;
use crate::projective_map::ProjectiveMap;

/// An area sample has invalid geometry or exceeds its remaining source-pixel budget.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AreaSampleError {
    /// The footprint is not finite, convex and inside the source raster.
    Geometry,
    /// The footprint's source-cell traversal exceeds the remaining work budget.
    WorkLimit,
}

/// Integrates piecewise constant source pixels over a clockwise raster-edge footprint.
/// The replacement pixel function runs before filtering; the work budget counts candidate cells.
///
/// # Errors
/// Refuses invalid footprints or insufficient work budget before traversing source pixels.
pub fn sample_area(
    image: &ImageRaster,
    footprint: [[f64; 2]; 4],
    remaining: &mut u64,
    pixel: impl Fn(u32, u32) -> Option<u32>,
) -> Result<u32, AreaSampleError> {
    ProjectiveMap::new(footprint, [image.width(), image.height()], [1, 1])
        .map_err(|_| AreaSampleError::Geometry)?;
    let bounds = footprint.into_iter().fold(
        [
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ],
        |[x0, y0, x1, y1], [x, y]| [x0.min(x), y0.min(y), x1.max(x), y1.max(y)],
    );
    let [x0, y0, x1, y1] = [
        bounds[0].floor() as u32,
        bounds[1].floor() as u32,
        (bounds[2].ceil() as u32).min(image.width()),
        (bounds[3].ceil() as u32).min(image.height()),
    ];
    let count = u64::from(x1 - x0) * u64::from(y1 - y0);
    *remaining = remaining
        .checked_sub(count)
        .ok_or(AreaSampleError::WorkLimit)?;
    let mut sum = [0.; 3];
    let mut total = 0.;
    for y in y0..y1 {
        for x in x0..x1 {
            let weight = cell_area(footprint, f64::from(x), f64::from(y))?;
            if weight <= 0. {
                continue;
            }
            let value = pixel(x, y).ok_or(AreaSampleError::Geometry)?;
            for (channel, shift) in sum.iter_mut().zip([0, 8, 16]) {
                *channel += f64::from((value >> shift) & 255) * weight;
            }
            total += weight;
        }
    }
    if total <= 0. || !total.is_finite() {
        return Err(AreaSampleError::Geometry);
    }
    let mut output = 0xff00_0000;
    for (sum, shift) in sum.into_iter().zip([0, 8, 16]) {
        output |= ((sum / total).round().clamp(0., 255.) as u32) << shift;
    }
    Ok(output)
}

fn cell_area(quad: [[f64; 2]; 4], x: f64, y: f64) -> Result<f64, AreaSampleError> {
    let mut polygon = [[0.; 2]; 12];
    polygon[..4].copy_from_slice(&quad);
    let mut length = 4;
    for (normal, bound) in [
        ([1., 0.], x),
        ([-1., 0.], -x - 1.),
        ([0., 1.], y),
        ([0., -1.], -y - 1.),
    ] {
        if length == 0 {
            return Ok(0.);
        }
        let points = polygon.get(..length).ok_or(AreaSampleError::Geometry)?;
        let mut clipped = [[0.; 2]; 12];
        let mut count = 0;
        let mut previous = *points.last().ok_or(AreaSampleError::Geometry)?;
        let project = |point: [f64; 2]| point[0] * normal[0] + point[1] * normal[1] - bound;
        for current in points {
            let start = project(previous);
            let end = project(*current);
            if (start >= 0.) != (end >= 0.) {
                let fraction = start / (start - end);
                append(
                    &mut clipped,
                    &mut count,
                    [
                        previous[0] + fraction * (current[0] - previous[0]),
                        previous[1] + fraction * (current[1] - previous[1]),
                    ],
                )?;
            }
            if end >= 0. {
                append(&mut clipped, &mut count, *current)?;
            }
            previous = *current;
        }
        polygon = clipped;
        length = count;
    }
    if length == 0 {
        return Ok(0.);
    }
    let points = polygon.get(..length).ok_or(AreaSampleError::Geometry)?;
    let mut previous = *points.last().ok_or(AreaSampleError::Geometry)?;
    let mut area = 0.;
    for current in points {
        area += previous[0] * current[1] - previous[1] * current[0];
        previous = *current;
    }
    Ok(area.abs() * 0.5)
}

fn append(
    polygon: &mut [[f64; 2]; 12],
    length: &mut usize,
    point: [f64; 2],
) -> Result<(), AreaSampleError> {
    *polygon.get_mut(*length).ok_or(AreaSampleError::Geometry)? = point;
    *length += 1;
    Ok(())
}
