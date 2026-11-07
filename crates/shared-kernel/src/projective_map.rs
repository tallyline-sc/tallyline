// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Validated projective coordinates and replicated-edge raster sampling.

use crate::image_raster::ImageRaster;

const GEOMETRY_EPSILON: f64 = 1e-9;

/// Nonfinite, nonconvex, out-of-raster or singular projective geometry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidGeometry;

/// Maps output edge coordinates to raster edge coordinates, with explicit homogeneous forward and inverse transforms.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProjectiveMap {
    forward: [f64; 8],
    inverse: [f64; 9],
    output: [u32; 2],
}

impl ProjectiveMap {
    /// Validates clockwise top-left, top-right, bottom-right and bottom-left corners.
    ///
    /// # Errors
    /// Refuses invalid geometry or zero raster/output dimensions.
    pub fn new(
        corners: [[f64; 2]; 4],
        source: [u32; 2],
        output: [u32; 2],
    ) -> Result<Self, InvalidGeometry> {
        if source.contains(&0) || output.contains(&0) {
            return Err(InvalidGeometry);
        }
        for [x, y] in corners {
            if !x.is_finite()
                || !y.is_finite()
                || x < 0.
                || y < 0.
                || x > f64::from(source[0])
                || y > f64::from(source[1])
            {
                return Err(InvalidGeometry);
            }
        }
        let [a, b, c, d] = corners;
        for (a, b, c) in [(a, b, c), (b, c, d), (c, d, a), (d, a, b)] {
            if (b[0] - a[0]) * (c[1] - b[1]) - (b[1] - a[1]) * (c[0] - b[0]) <= GEOMETRY_EPSILON {
                return Err(InvalidGeometry);
            }
        }
        let dx1 = b[0] - c[0];
        let dx2 = d[0] - c[0];
        let dx3 = a[0] - b[0] + c[0] - d[0];
        let dy1 = b[1] - c[1];
        let dy2 = d[1] - c[1];
        let dy3 = a[1] - b[1] + c[1] - d[1];
        let denominator = dx1 * dy2 - dx2 * dy1;
        if denominator.abs() <= GEOMETRY_EPSILON {
            return Err(InvalidGeometry);
        }
        let g = (dx3 * dy2 - dx2 * dy3) / denominator;
        let h = (dx1 * dy3 - dx3 * dy1) / denominator;
        let forward = [
            b[0] - a[0] + g * b[0],
            d[0] - a[0] + h * d[0],
            a[0],
            b[1] - a[1] + g * b[1],
            d[1] - a[1] + h * d[1],
            a[1],
            g,
            h,
        ];
        let [a, b, c, d, e, f, g, h] = forward;
        let inverse = [
            e - f * h,
            c * h - b,
            b * f - c * e,
            f * g - d,
            a - c * g,
            c * d - a * f,
            d * h - e * g,
            b * g - a * h,
            a * e - b * d,
        ];
        let map = Self {
            forward,
            inverse,
            output,
        };
        for point in [[0., 0.], [1., 0.], [1., 1.], [0., 1.]] {
            let denominator = g * point[0] + h * point[1] + 1.;
            if !denominator.is_finite() || denominator <= GEOMETRY_EPSILON {
                return Err(InvalidGeometry);
            }
        }
        if forward.iter().chain(inverse.iter()).any(|v| !v.is_finite()) {
            return Err(InvalidGeometry);
        }
        Ok(map)
    }

    /// Returns the homogeneous output-edge to source-edge matrix.
    #[must_use]
    pub fn output_to_source_matrix(&self) -> [[f64; 3]; 3] {
        let [a, b, c, d, e, f, g, h] = self.forward;
        let [w, hout] = self.output.map(f64::from);
        [
            [a / w, b / hout, c],
            [d / w, e / hout, f],
            [g / w, h / hout, 1.],
        ]
    }

    /// Returns the homogeneous source-edge to output-edge matrix.
    #[must_use]
    pub fn source_to_output_matrix(&self) -> [[f64; 3]; 3] {
        let [a, b, c, d, e, f, g, h, i] = self.inverse;
        let [w, hout] = self.output.map(f64::from);
        [
            [a * w, b * w, c * w],
            [d * hout, e * hout, f * hout],
            [g, h, i],
        ]
    }

    /// Maps output edge coordinates to source raster edge coordinates.
    #[must_use]
    pub fn to_source(&self, point: [f64; 2]) -> Option<[f64; 2]> {
        let [a, b, c, d, e, f, g, h] = self.forward;
        let u = point[0] / f64::from(self.output[0]);
        let v = point[1] / f64::from(self.output[1]);
        mapped([a * u + b * v + c, d * u + e * v + f], g * u + h * v + 1.)
    }

    /// Maps source raster edge coordinates to output edge coordinates.
    #[must_use]
    pub fn from_source(&self, [x, y]: [f64; 2]) -> Option<[f64; 2]> {
        let [a, b, c, d, e, f, g, h, i] = self.inverse;
        let [u, v] = mapped([a * x + b * y + c, d * x + e * y + f], g * x + h * y + i)?;
        let point = [u * f64::from(self.output[0]), v * f64::from(self.output[1])];
        point.iter().all(|value| value.is_finite()).then_some(point)
    }
}

fn mapped(numerator: [f64; 2], denominator: f64) -> Option<[f64; 2]> {
    if !denominator.is_finite() || denominator.abs() <= GEOMETRY_EPSILON {
        return None;
    }
    let point = numerator.map(|v| v / denominator);
    point.iter().all(|v| v.is_finite()).then_some(point)
}

/// Samples integer-centred source pixels with bilinear weights and replicated raster edges.
/// The pixel function runs before filtering and may replace prohibited source pixels.
///
/// # Errors
/// Refuses nonfinite sampling coordinates.
pub fn sample_bilinear(
    image: &ImageRaster,
    [x, y]: [f64; 2],
    pixel: impl Fn(u32, u32) -> Option<u32>,
) -> Result<u32, InvalidGeometry> {
    if !x.is_finite() || !y.is_finite() {
        return Err(InvalidGeometry);
    }
    let x = x.clamp(0., f64::from(image.width() - 1));
    let y = y.clamp(0., f64::from(image.height() - 1));
    let x0 = x.floor() as u32;
    let y0 = y.floor() as u32;
    let x1 = (x0 + 1).min(image.width() - 1);
    let y1 = (y0 + 1).min(image.height() - 1);
    let fx = x - f64::from(x0);
    let fy = y - f64::from(y0);
    let coordinates = [
        (x0, y0, (1. - fx) * (1. - fy)),
        (x1, y0, fx * (1. - fy)),
        (x0, y1, (1. - fx) * fy),
        (x1, y1, fx * fy),
    ];
    let mut samples = [(0, 0.); 4];
    for (sample, (x, y, weight)) in samples.iter_mut().zip(coordinates) {
        *sample = (pixel(x, y).ok_or(InvalidGeometry)?, weight);
    }
    let mut output = 0xff00_0000;
    for shift in [0, 8, 16] {
        let sum: f64 = samples
            .iter()
            .map(|(pixel, weight)| f64::from((pixel >> shift) & 255) * weight)
            .sum();
        output |= (sum.round().clamp(0., 255.) as u32) << shift;
    }
    Ok(output)
}
