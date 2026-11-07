// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Actual visible credential control geometry through the public drawn shell.

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use tallyline_ui::api::ShellState;

/// Describes only control state and geometry without credential values.
pub fn geometry(harness: &Harness<'_, ShellState>, label: &str) -> String {
    let control = harness.get_by_label(label);
    let viewport = harness.ctx.input(|input| input.content_rect());
    let status = harness.get_by_label("Saved submissions: 0").rect();
    let paint = crate::control_paint::text_bounds(harness, label);
    format!(
        "{label}: focused={} disabled={} control={:?} viewport={viewport:?} status={status:?} paint_clip_and_bounds={paint:?} busy={} failure={:?}",
        control.is_focused(),
        control.accesskit_node().is_disabled(),
        control.rect(),
        harness.state().credentials().busy(),
        harness.state().credentials().failure()
    )
}

/// Requires real focus and complete control/label paint inside the visible page.
///
/// # Errors
/// Returns safe geometry when the control is hidden, disabled or unfocused.
pub fn assert_focused_visible(
    harness: &Harness<'_, ShellState>,
    label: &str,
) -> Result<(), String> {
    crate::control_paint::assert_focused_visible(harness, &harness.get_by_label(label), label)
}

/// Activates only a visibly focused control using a real Space key event.
///
/// # Errors
/// Returns the complete visibility proof failure before activation.
pub fn activate_visible(harness: &mut Harness<'_, ShellState>, label: &str) -> Result<(), String> {
    harness.get_by_label(label).focus();
    harness.run();
    self::assert_focused_visible(harness, label)?;
    harness.key_press(egui::Key::Space);
    harness.run();
    Ok(())
}

/// Requires the complete focused text input frame within its actual paint clip.
///
/// # Errors
/// Returns safe control geometry when its frame is clipped or absent.
pub fn assert_input_visible(harness: &Harness<'_, ShellState>, label: &str) -> Result<(), String> {
    let control = harness.get_by_label(label);
    let rect = control.rect();
    let viewport = harness.ctx.input(|input| input.content_rect());
    let label_visible = crate::control_paint::text_bounds(harness, label)
        .is_some_and(|(clip, painted)| clip.contains_rect(painted));
    let frame_visible = crate::input_frame_paint::visible(harness, rect);
    if control.is_focused()
        && !control.accesskit_node().is_disabled()
        && label_visible
        && frame_visible
        && viewport.contains_rect(rect)
        && rect.bottom() <= harness.get_by_label("Saved submissions: 0").rect().top()
    {
        Ok(())
    } else {
        Err(self::geometry(harness, label))
    }
}
