// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Original-raster projective crops with bounded portable bilinear sampling.

use super::{DbError, DetectionBox};
use tallyline_shared_kernel::api;
use tallyline_shared_kernel::api::model::{ImageRaster, ProjectiveMap};

impl DetectionBox {
    /// Rectifies this convex quadrilateral at source scale using replicated-edge bilinear sampling.
    /// Tall crops are rotated anticlockwise; this portable profile does not claim OpenCV cubic parity.
    ///
    /// # Errors
    /// Rejects nonfinite, out-of-raster, degenerate or nonconvex geometry and crops above
    /// 4,096 pixels per side or four megapixels before allocating output pixels.
    pub fn rectify(&self, image: &ImageRaster) -> Result<ImageRaster, DbError> {
        let transform = ProjectiveMap::new(self.corners, [image.width(), image.height()], [1, 1])
            .map_err(|_| DbError::Geometry)?;
        let [a, b, c, d] = self.corners;
        let distance = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).hypot(a[1] - b[1]);
        let width = distance(a, b).max(distance(c, d)).floor() as usize;
        let height = distance(a, d).max(distance(b, c)).floor() as usize;
        if width == 0 || height == 0 {
            return Err(DbError::Geometry);
        }
        if width > 4096
            || height > 4096
            || width
                .checked_mul(height)
                .is_none_or(|count| count > 4_194_304)
        {
            return Err(DbError::Complexity);
        }
        let rotated = height as f64 / width as f64 >= 1.5;
        let (out_width, out_height) = if rotated {
            (height, width)
        } else {
            (width, height)
        };
        let mut pixels = Vec::with_capacity(width * height);
        for y in 0..out_height {
            for x in 0..out_width {
                let (px, py) = if rotated { (width - 1 - y, x) } else { (x, y) };
                let source = transform
                    .to_source([px as f64 / width as f64, py as f64 / height as f64])
                    .ok_or(DbError::Geometry)?;
                pixels.push(
                    api::sample_bilinear(image, source, |x, y| {
                        image
                            .argb()
                            .get(y as usize * image.width() as usize + x as usize)
                            .copied()
                    })
                    .map_err(|_| DbError::Geometry)?,
                );
            }
        }
        ImageRaster::new(out_width as u32, out_height as u32, pixels).map_err(|_| DbError::Geometry)
    }
}
