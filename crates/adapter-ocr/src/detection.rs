// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Bounded portable DB probability-map geometry.

mod geometry;

use geometry::{Point, Rectangle};

const MAX_PIXELS: usize = 4_194_304;
const MAX_CONTOURS: usize = 1000;
const BINARISATION: f32 = 0.2;
const UNCLIP_RATIO: f64 = 1.4;

/// One expanded text quadrilateral in the original raster's coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct DetectionBox {
    /// Clockwise top-left, top-right, bottom-right and bottom-left corners.
    pub corners: [[f64; 2]; 4],
    /// Mean native probability inside the unexpanded minimum-area rectangle.
    pub probability: f32,
}

/// Invalid or excessive probability-map input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DbError {
    /// Batch/channel, dimensions, element count or source dimensions are invalid.
    Shape,
    /// A native probability is nonfinite or outside zero to one.
    Probability,
    /// The bounded map or contour complexity budget is exceeded.
    Complexity,
    /// A requested original-raster quadrilateral cannot be rectified safely.
    Geometry,
}

/// Fixed Small DB thresholds with explicitly portable geometry arithmetic.
#[derive(Default)]
pub struct DbPostprocessor;

impl DbPostprocessor {
    /// Extracts text boxes without loading a native runtime or retaining the input map.
    ///
    /// # Errors
    /// Rejects invalid probabilities/shapes, maps above four megapixels, more than
    /// 1,000 contours or convex hulls above 1,024 vertices rather than returning partial text.
    pub fn extract(
        &self,
        probabilities: &[f32],
        shape: [usize; 4],
        source: [u32; 2],
    ) -> Result<Vec<DetectionBox>, DbError> {
        self.extract_for_detector(
            crate::research::OcrDetector::Small,
            probabilities,
            shape,
            source,
        )
    }

    /// Extracts boxes with the selected detector's reviewed threshold and portable arithmetic.
    ///
    /// # Errors
    /// Applies the same probability, geometry and complexity refusal as `extract`.
    pub fn extract_for_detector(
        &self,
        detector: crate::research::OcrDetector,
        probabilities: &[f32],
        shape: [usize; 4],
        source: [u32; 2],
    ) -> Result<Vec<DetectionBox>, DbError> {
        let [batch, channels, height, width] = shape;
        if batch != 1
            || channels != 1
            || width == 0
            || height == 0
            || width.checked_mul(height) != Some(probabilities.len())
            || source.contains(&0)
        {
            return Err(DbError::Shape);
        }
        if probabilities.len() > MAX_PIXELS {
            return Err(DbError::Complexity);
        }
        if probabilities
            .iter()
            .any(|p| !p.is_finite() || !(0.0..=1.0).contains(p))
        {
            return Err(DbError::Probability);
        }
        let mask: Vec<bool> = probabilities.iter().map(|p| *p > BINARISATION).collect();
        let contours = contours(&mask, width, height);
        if contours.len() > MAX_CONTOURS {
            return Err(DbError::Complexity);
        }
        let mut boxes = Vec::new();
        for contour in contours {
            let Some(rectangle) = Rectangle::minimum(contour)? else {
                continue;
            };
            if rectangle.short_side() < 3.0 {
                continue;
            }
            let probability = rectangle.score(probabilities, width, height);
            if probability < detector.box_threshold() {
                continue;
            }
            let expanded = rectangle.expand(UNCLIP_RATIO);
            if expanded.short_side() < 5.0 {
                continue;
            }
            let corners = expanded.corners().map(|point| {
                [
                    (point.x * f64::from(source[0]) / width as f64)
                        .round_ties_even()
                        .clamp(0.0, f64::from(source[0])),
                    (point.y * f64::from(source[1]) / height as f64)
                        .round_ties_even()
                        .clamp(0.0, f64::from(source[1])),
                ]
            });
            boxes.push(DetectionBox {
                corners,
                probability: probability as f32,
            });
        }
        boxes.sort_by(|left, right| {
            left.corners[0][1]
                .total_cmp(&right.corners[0][1])
                .then_with(|| left.corners[0][0].total_cmp(&right.corners[0][0]))
        });
        Ok(boxes)
    }
}

fn contours(mask: &[bool], width: usize, height: usize) -> Vec<Vec<Point>> {
    let mut result = Vec::new();
    for foreground in [true, false] {
        let mut visited = vec![false; mask.len()];
        for (start, value) in mask.iter().copied().enumerate() {
            if value != foreground || visited.get(start).copied().unwrap_or(true) {
                continue;
            }
            let mut pending = vec![start];
            if let Some(value) = visited.get_mut(start) {
                *value = true;
            }
            let mut points = Vec::new();
            let mut touches_edge = false;
            while let Some(index) = pending.pop() {
                let x = index % width;
                let y = index / width;
                let mut boundary = false;
                for dy in -1_isize..=1 {
                    for dx in -1_isize..=1 {
                        if (dx == 0 && dy == 0) || (!foreground && dx != 0 && dy != 0) {
                            continue;
                        }
                        let Some(nx) = x.checked_add_signed(dx).filter(|value| *value < width)
                        else {
                            boundary = true;
                            touches_edge = true;
                            continue;
                        };
                        let Some(ny) = y.checked_add_signed(dy).filter(|value| *value < height)
                        else {
                            boundary = true;
                            touches_edge = true;
                            continue;
                        };
                        let neighbour = ny * width + nx;
                        if mask.get(neighbour).copied() != Some(foreground) {
                            boundary = true;
                            if !foreground {
                                points.push(Point {
                                    x: nx as f64,
                                    y: ny as f64,
                                });
                            }
                            continue;
                        }
                        if let Some(seen) = visited.get_mut(neighbour)
                            && !*seen
                        {
                            *seen = true;
                            pending.push(neighbour);
                        }
                    }
                }
                if foreground && boundary {
                    points.push(Point {
                        x: x as f64,
                        y: y as f64,
                    });
                }
            }
            if foreground || !touches_edge {
                result.push(points);
                if result.len() > MAX_CONTOURS {
                    return result;
                }
            }
        }
    }
    result
}
mod crop;
