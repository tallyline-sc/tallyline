// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Queue timer publications wake visible views without repainting an iconified window.

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_3_r_ui_16__queue_updates_request_repaint_only_while_the_viewport_is_not_minimised() {
    let context = egui::Context::default();
    let requests = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&requests);
    context.set_repaint_observer(move |_| {
        observed.fetch_add(1, Ordering::SeqCst);
    });
    let mut input = egui::RawInput::default();
    input
        .viewports
        .get_mut(&egui::ViewportId::ROOT)
        .unwrap()
        .minimized = Some(true);
    let _output = context.run_logic(&input, |context| {
        tallyline_ui::api::wake_submission(context)
    });
    assert_eq!(requests.load(Ordering::SeqCst), 0);
    input
        .viewports
        .get_mut(&egui::ViewportId::ROOT)
        .unwrap()
        .minimized = Some(false);
    let _output = context.run_logic(&input, |context| {
        tallyline_ui::api::wake_submission(context)
    });
    assert_eq!(requests.load(Ordering::SeqCst), 1);
}
