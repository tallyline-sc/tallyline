// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Full-width upward-closed source-cell exclusion before sampling.

pub(crate) struct PrivacyMask {
    bottom_left: [f64; 2],
    dx: Enclosure,
    dy: Enclosure,
}

impl PrivacyMask {
    pub(crate) fn try_new(corners: [[f64; 2]; 4], source: [u32; 2]) -> Option<Self> {
        let [top_left, top_right, bottom_right, bottom_left] = corners;
        let dx = Enclosure::difference(bottom_right[0], bottom_left[0])?;
        if dx.lower <= 0.
            || Enclosure::difference(top_right[0], top_left[0])?.lower <= 0.
            || Enclosure::difference(bottom_left[1], top_left[1])?.lower <= 0.
            || Enclosure::difference(bottom_right[1], top_right[1])?.lower <= 0.
        {
            return None;
        }
        let mask = Self {
            bottom_left,
            dx,
            dy: Enclosure::difference(bottom_right[1], bottom_left[1])?,
        };
        if mask.evaluate(top_left)?.upper >= 0. || mask.evaluate(top_right)?.upper >= 0. {
            return None;
        }
        let [width, height] = source.map(f64::from);
        for point in [[0., 0.], [width, 0.], [width, height], [0., height]] {
            mask.evaluate(point)?;
        }
        Some(mask)
    }

    pub(crate) fn covers(&self, quad: [[f64; 2]; 4]) -> bool {
        quad.into_iter().all(|point| !self.provably_below(point))
    }

    pub(crate) fn excludes(&self, x: u32, y: u32) -> bool {
        let x = f64::from(x);
        let y = f64::from(y);
        !self.provably_below([x, y]) || !self.provably_below([x + 1., y])
    }

    fn provably_below(&self, point: [f64; 2]) -> bool {
        self.evaluate(point).is_some_and(|value| value.lower > 0.)
    }

    fn evaluate(&self, [x, y]: [f64; 2]) -> Option<Enclosure> {
        self.dx
            .multiply(Enclosure::difference(y, self.bottom_left[1])?)?
            .subtract(
                self.dy
                    .multiply(Enclosure::difference(x, self.bottom_left[0])?)?,
            )
    }
}

/// Outward-rounded bounds enclosing real arithmetic on the supplied binary64 operands.
#[derive(Clone, Copy)]
struct Enclosure {
    lower: f64,
    upper: f64,
}

impl Enclosure {
    fn difference(left: f64, right: f64) -> Option<Self> {
        Self::rounded(left - right, left - right)
    }

    fn subtract(self, right: Self) -> Option<Self> {
        Self::rounded(self.lower - right.upper, self.upper - right.lower)
    }

    fn multiply(self, right: Self) -> Option<Self> {
        let products = [
            self.lower * right.lower,
            self.lower * right.upper,
            self.upper * right.lower,
            self.upper * right.upper,
        ];
        if products.iter().any(|value| !value.is_finite()) {
            return None;
        }
        Self::rounded(
            products.into_iter().fold(f64::INFINITY, f64::min),
            products.into_iter().fold(f64::NEG_INFINITY, f64::max),
        )
    }

    fn rounded(lower: f64, upper: f64) -> Option<Self> {
        let lower = lower.next_down();
        let upper = upper.next_up();
        (lower.is_finite() && upper.is_finite()).then_some(Self { lower, upper })
    }
}
