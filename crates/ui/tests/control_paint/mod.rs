// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Effective paint bounds for drawn shell control contracts.

use egui::{Rect, Shape};
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::{Harness, Node};

/// Returns the effective clip and complete layout bounds of painted text.
pub fn text_bounds<State>(harness: &Harness<'_, State>, label: &str) -> Option<(Rect, Rect)> {
    let viewport = harness.ctx.input(|input| input.content_rect());
    let status = harness.get_by_label("Saved submissions: 0").rect();
    let page = Rect::from_min_max(viewport.min, egui::pos2(viewport.right(), status.top()));
    harness.output().shapes.iter().find_map(|shape| {
        if let Shape::Text(text) = &shape.shape {
            (text.galley.job.text == label).then_some((
                shape.clip_rect.intersect(page),
                text.galley.rect.translate(text.pos.to_vec2()),
            ))
        } else {
            None
        }
    })
}

/// Requires an enabled, focused action and its complete visible label and control.
///
/// # Errors
/// Returns only safe control state and geometry when the proof fails.
pub fn assert_focused_visible<State>(
    harness: &Harness<'_, State>,
    control: &Node<'_>,
    label: &str,
) -> Result<(), String> {
    let rect = control.rect();
    let paint = self::text_bounds(harness, label);
    if control.is_focused()
        && !control.accesskit_node().is_disabled()
        && paint
            .is_some_and(|(clip, painted)| clip.contains_rect(rect) && clip.contains_rect(painted))
    {
        Ok(())
    } else {
        Err(format!(
            "{label}: focused={} disabled={} control={rect:?} effective_clip_and_paint={paint:?}",
            control.is_focused(),
            control.accesskit_node().is_disabled()
        ))
    }
}
