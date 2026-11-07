// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Keyboard navigation and passive shell publications.

use egui_kittest::kittest::Queryable;
use tallyline_ui::api::model::{BackgroundEvent, View};

#[test]
fn navigation_and_help_work_from_the_keyboard_without_stealing_content_focus() {
    let mut harness = super::shell(egui::vec2(800.0, 640.0)).unwrap();
    harness.run();
    harness.get_by_label("Search terminals").focus();
    harness.run();
    assert!(harness.get_by_label("Search terminals").is_focused());
    harness.state_mut().shell.receive(BackgroundEvent::Failure(
        "Storage self-test failed".to_owned(),
    ));
    harness.run();
    assert!(harness.get_by_label("Search terminals").is_focused());
    assert_eq!(harness.state().shell.view(), View::Reference);
    harness.get_by_label("Search terminals").type_text("Orbit");
    harness.run();
    assert_eq!(
        harness.state().shell.references().terminal_search(),
        "Orbit"
    );
    harness
        .get_by_role_and_label(egui::Role::Button, "Reference data")
        .focus();
    harness.run();
    harness.key_press(egui::Key::Tab);
    harness.run();
    assert!(
        harness
            .get_by_role_and_label(egui::Role::Button, "Manual Report")
            .is_focused()
    );
    harness.key_press(egui::Key::Space);
    harness.run();
    assert_eq!(harness.state().shell.view(), View::Manual);
    harness.get_by_label("Help").focus();
    harness.run();
    harness.key_press(egui::Key::Space);
    harness.run();
    assert!(harness.query_by_label("About").is_some());
    harness.key_press(egui::Key::Escape);
    harness.run();
    assert!(harness.query_by_label("About").is_none());
    assert!(harness.get_by_label("Help").is_focused());
    assert_eq!(harness.state().shell.view(), View::Manual);
    assert!(!harness.state().shell.dialogs().about_open());
}
