// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact input-frame paint bounds for drawn shell contracts.

use egui::{Rect, Shape};
use egui_kittest::Harness;

/// Returns whether the exact input frame fits completely within its paint clip.
pub fn visible<State>(harness: &Harness<'_, State>, rect: Rect) -> bool {
    harness.output().shapes.iter().any(|shape| {
        let Shape::Rect(frame) = &shape.shape else {
            return false;
        };
        frame.rect == rect && shape.clip_rect.contains_rect(frame.rect)
    })
}
