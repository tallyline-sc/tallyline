// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Convex text-region rectangle geometry.

use super::DbError;

#[derive(Clone, Copy, PartialEq)]
pub(super) struct Point {
    pub x: f64,
    pub y: f64,
}

pub(super) struct Rectangle {
    u: Point,
    v: Point,
    min_u: f64,
    max_u: f64,
    min_v: f64,
    max_v: f64,
}

impl Rectangle {
    pub(super) fn minimum(points: Vec<Point>) -> Result<Option<Self>, DbError> {
        let hull = hull(points);
        if hull.len() < 3 {
            return Ok(None);
        }
        if hull.len() > 1024 {
            return Err(DbError::Complexity);
        }
        let mut best: Option<Self> = None;
        for (first, second) in hull.iter().zip(hull.iter().cycle().skip(1)) {
            let dx = second.x - first.x;
            let dy = second.y - first.y;
            let length = dx.hypot(dy);
            let u = Point {
                x: dx / length,
                y: dy / length,
            };
            let v = Point { x: -u.y, y: u.x };
            let mut rectangle = Self {
                u,
                v,
                min_u: f64::INFINITY,
                max_u: f64::NEG_INFINITY,
                min_v: f64::INFINITY,
                max_v: f64::NEG_INFINITY,
            };
            for point in &hull {
                let pu = dot(*point, u);
                let pv = dot(*point, v);
                rectangle.min_u = rectangle.min_u.min(pu);
                rectangle.max_u = rectangle.max_u.max(pu);
                rectangle.min_v = rectangle.min_v.min(pv);
                rectangle.max_v = rectangle.max_v.max(pv);
            }
            if best
                .as_ref()
                .is_none_or(|current| rectangle.area() < current.area())
            {
                best = Some(rectangle);
            }
        }
        Ok(best)
    }

    fn area(&self) -> f64 {
        (self.max_u - self.min_u) * (self.max_v - self.min_v)
    }

    pub(super) fn short_side(&self) -> f64 {
        (self.max_u - self.min_u).min(self.max_v - self.min_v)
    }

    pub(super) fn expand(mut self, ratio: f64) -> Self {
        let perimeter = 2.0 * (self.max_u - self.min_u + self.max_v - self.min_v);
        let distance = self.area() * ratio / perimeter;
        self.min_u -= distance;
        self.max_u += distance;
        self.min_v -= distance;
        self.max_v += distance;
        self
    }

    pub(super) fn corners(&self) -> [Point; 4] {
        let point = |u: f64, v: f64| Point {
            x: ((self.u.x * u + self.v.x * v) * 1e9).round() / 1e9,
            y: ((self.u.y * u + self.v.y * v) * 1e9).round() / 1e9,
        };
        let mut points = [
            point(self.min_u, self.min_v),
            point(self.max_u, self.min_v),
            point(self.max_u, self.max_v),
            point(self.min_u, self.max_v),
        ];
        points.sort_by(|a, b| a.x.total_cmp(&b.x).then_with(|| a.y.total_cmp(&b.y)));
        let [a, b, c, d] = points;
        let (tl, bl) = if a.y <= b.y { (a, b) } else { (b, a) };
        let (tr, br) = if c.y <= d.y { (c, d) } else { (d, c) };
        [tl, tr, br, bl]
    }

    pub(super) fn score(&self, probabilities: &[f32], width: usize, height: usize) -> f64 {
        let corners = self.corners();
        let min_x = corners
            .iter()
            .map(|p| p.x.floor().max(0.0) as usize)
            .min()
            .unwrap_or(0);
        let max_x = corners
            .iter()
            .map(|p| p.x.ceil().max(0.0) as usize)
            .max()
            .unwrap_or(0)
            .min(width - 1);
        let min_y = corners
            .iter()
            .map(|p| p.y.floor().max(0.0) as usize)
            .min()
            .unwrap_or(0);
        let max_y = corners
            .iter()
            .map(|p| p.y.ceil().max(0.0) as usize)
            .max()
            .unwrap_or(0)
            .min(height - 1);
        let mut sum = 0.0;
        let mut count = 0;
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let point = Point {
                    x: x as f64,
                    y: y as f64,
                };
                let u = dot(point, self.u);
                let v = dot(point, self.v);
                if u + 1e-9 >= self.min_u
                    && u <= self.max_u + 1e-9
                    && v + 1e-9 >= self.min_v
                    && v <= self.max_v + 1e-9
                    && let Some(value) = probabilities.get(y * width + x)
                {
                    sum += f64::from(*value);
                    count += 1;
                }
            }
        }
        if count == 0 {
            0.0
        } else {
            sum / f64::from(count)
        }
    }
}

fn dot(a: Point, b: Point) -> f64 {
    a.x * b.x + a.y * b.y
}

fn cross(origin: Point, a: Point, b: Point) -> f64 {
    (a.x - origin.x) * (b.y - origin.y) - (a.y - origin.y) * (b.x - origin.x)
}

fn hull(mut points: Vec<Point>) -> Vec<Point> {
    points.sort_by(|a, b| a.x.total_cmp(&b.x).then_with(|| a.y.total_cmp(&b.y)));
    points.dedup();
    if points.len() < 3 {
        return points;
    }
    let half = |ordered: &[Point]| {
        let mut chain = Vec::<Point>::new();
        for &point in ordered {
            while let [.., a, b] = chain.as_slice() {
                if cross(*a, *b, point) > 0.0 {
                    break;
                }
                chain.pop();
            }
            chain.push(point);
        }
        chain.pop();
        chain
    };
    let mut lower = half(&points);
    points.reverse();
    lower.extend(half(&points));
    lower
}
