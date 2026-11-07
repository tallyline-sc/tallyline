// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Drawn contracts for offline legal documents.

#[path = "resources/common.rs"]
mod resources;

#[path = "licence_browser/support.rs"]
mod support;

use egui_kittest::kittest::Queryable;
use tallyline_ui::api::model::BackgroundEvent;

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_18__component_selection_shows_only_its_exact_metadata_and_documents() {
    let mut harness = support::browser().unwrap();
    harness.get_by_label("Shipped components");
    harness.get_by_label("MIT OR Apache-2.0");
    harness.get_by_label("Copyright 2025 Alpha Authors");
    assert!(harness.query_by_label("Exact Beta licence.").is_none());
    harness.get_by_label("Beta 4.5.6").click();
    harness.run();
    harness
        .get_by_label("The reviewed upstream licence and sources do not name a copyright holder.");
    assert!(
        harness
            .query_by_label("Copyright 2025 Alpha Authors")
            .is_none()
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_17_r_ui_18__list_keys_select_and_tab_enters_the_immutable_copyable_document() {
    let mut harness = support::browser().unwrap();
    harness
        .get_by_role_and_label(egui::Role::ListBox, "Shipped components")
        .focus();
    harness.run();
    harness.key_press(egui::Key::ArrowDown);
    harness.run();
    harness
        .get_by_label("The reviewed upstream licence and sources do not name a copyright holder.");
    harness.key_press(egui::Key::Home);
    harness.run();
    harness.get_by_label("Copyright 2025 Alpha Authors");
    harness.key_press(egui::Key::End);
    harness.run();
    harness
        .get_by_label("The reviewed upstream licence and sources do not name a copyright holder.");
    harness.key_press(egui::Key::ArrowUp);
    harness.run();
    harness.get_by_label("Copyright 2025 Alpha Authors");
    harness.key_press(egui::Key::Tab);
    harness.run();
    assert!(harness.get_by_role(egui::Role::Document).is_focused());
    assert_eq!(
        support::copy_document(&mut harness),
        Some("Exact Alpha licence\n\nSecond paragraph.")
    );
    harness
        .get_by_role(egui::Role::Document)
        .type_text("Replacement");
    harness.run();
    assert_eq!(
        harness.state().dialogs().licences().unwrap().components()[0].notices[0].text,
        "Exact Alpha licence\n\nSecond paragraph."
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_18__each_bundled_notice_can_be_copied_and_reopening_preserves_selection() {
    let mut harness = support::browser().unwrap();
    harness.get_by_role(egui::Role::ComboBox).click();
    harness.run();
    harness
        .get_by_role_and_label(egui::Role::Button, "Alpha/NOTICE")
        .click();
    harness.run();
    harness.get_by_role(egui::Role::Document).focus();
    harness.run();
    assert_eq!(
        support::copy_document(&mut harness),
        Some("Exact Alpha attribution.")
    );
    harness.get_by_label("Beta 4.5.6").click();
    harness.run();
    harness
        .get_by_role_and_label(egui::Role::Window, "Open-source licences")
        .get_by_label("Close window")
        .click();
    harness.run();
    harness
        .get_by_role_and_label(egui::Role::Button, "Open-source licences")
        .focus();
    harness.run();
    harness.key_press(egui::Key::Space);
    harness.run();
    harness
        .get_by_label("The reviewed upstream licence and sources do not name a copyright holder.");
    assert!(
        harness
            .query_by_label("Copyright 2025 Alpha Authors")
            .is_none()
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_18__missing_catalogue_feedback_remains_visible_without_a_partial_browser() {
    let mut harness = support::browser_with(None, tallyline_egui::Theme::Light, 1.0).unwrap();
    harness.get_by_label("The open-source licence list is unavailable. Review the legal directory supplied with this build.");
    assert!(harness.query_by_role(egui::Role::ListBox).is_none());
    harness.state_mut().receive(BackgroundEvent::LicencesLoaded(
        support::catalogue().unwrap(),
    ));
    harness.run();
    harness.get_by_role(egui::Role::ListBox);
    harness
        .state_mut()
        .receive(BackgroundEvent::LicencesUnavailable);
    harness.run();
    assert!(harness.query_by_role(egui::Role::Document).is_none());
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_17_r_ui_18__large_catalogue_stays_bounded_and_keyboard_reaches_its_last_document_in_every_theme()
 {
    for theme in [
        tallyline_egui::Theme::Dark,
        tallyline_egui::Theme::Light,
        tallyline_egui::Theme::HighContrast,
    ] {
        for zoom in [1.0, 2.0] {
            let components: Vec<_> = (0..1000).map(|index| serde_json::json!({"name":format!("Component {index}"),"version":"1.0","licence":"MIT","source":"https://example.invalid/source","copyright":{"status":"not_stated_by_upstream"},"notices":[{"path":"LICENSE","text":"First paragraph.\n\nA long wrapped legal paragraph. ".repeat(200)}]})).collect();
            let data = serde_json::json!({"schema_version":1,"components":components}).to_string();
            let mut harness = support::browser_with(
                Some(tallyline_ui::api::parse_licence_catalogue(&data).unwrap()),
                theme,
                zoom,
            )
            .unwrap();
            let list = harness.get_by_role(egui::Role::ListBox);
            assert!(list.rect().height() <= 121.0);
            assert!(list.rect().max.y < 640.0 / zoom);
            assert!(harness.query_by_label("Component 999 1.0").is_none());
            list.focus();
            harness.run();
            harness.key_press(egui::Key::End);
            harness.run();
            harness.get_by_role_and_label(egui::Role::Button, "Component 999 1.0");
            harness.key_press(egui::Key::Tab);
            harness.run();
            assert!(harness.get_by_role(egui::Role::Document).is_focused());
            assert!(
                harness.get_by_role(egui::Role::Document).rect().min.y < 640.0 / zoom,
                "focused document must enter the viewport at {zoom}x"
            );
            let before_scroll = support::text_origin(&harness, "First paragraph.").unwrap();
            harness.key_press(egui::Key::PageDown);
            harness.run();
            assert!(harness.get_by_role(egui::Role::Document).is_focused());
            let after_scroll = support::text_origin(&harness, "First paragraph.").unwrap();
            assert!(
                after_scroll < before_scroll,
                "Page Down must scroll the legal document: {theme:?} {zoom}x, {before_scroll} -> {after_scroll}"
            );
            harness.key_press(egui::Key::PageUp);
            harness.run();
            assert!(
                support::text_origin(&harness, "First paragraph.").unwrap() > after_scroll,
                "Page Up must scroll towards the start"
            );
        }
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_18__gpl_uses_the_same_focusable_document_and_copies_the_full_bundled_file() {
    let mut harness = support::browser().unwrap();
    harness
        .get_by_role_and_label(egui::Role::Window, "Open-source licences")
        .get_by_label("Close window")
        .click();
    harness.run();
    harness
        .get_by_role_and_label(egui::Role::Button, "GNU General Public License")
        .focus();
    harness.run();
    harness.key_press(egui::Key::Space);
    harness.run();
    harness.get_by_role(egui::Role::Document).focus();
    harness.run();
    assert_eq!(
        support::copy_document(&mut harness),
        Some(include_str!("../../../LICENSE"))
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_17_r_ui_18__selected_rows_and_legal_type_use_the_semantic_theme_tokens() {
    for theme in [
        tallyline_egui::Theme::Dark,
        tallyline_egui::Theme::Light,
        tallyline_egui::Theme::HighContrast,
    ] {
        let mut harness =
            support::browser_with(Some(support::catalogue().unwrap()), theme, 1.0).unwrap();
        harness.get_by_role(egui::Role::ListBox).focus();
        harness.run();
        let palette = theme.palette();
        let selected = palette.color("selection-row-bg").unwrap();
        assert!(harness.output().shapes.iter().any(|shape| matches!(&shape.shape, egui::epaint::Shape::Rect(rect) if rect.fill == selected && rect.rect.width() > 100.0 && rect.rect.height() <= 40.0)));
        let mark = palette.color("selection-row-mark").unwrap();
        assert!(harness.output().shapes.iter().any(|shape| matches!(&shape.shape, egui::epaint::Shape::Rect(rect) if rect.fill == mark && rect.rect.width() == 4.0)));
        let document = harness
            .output()
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::epaint::Shape::Text(text)
                    if text.galley.job.text == "Exact Alpha licence\n\nSecond paragraph." =>
                {
                    Some(text)
                }
                egui::epaint::Shape::Noop
                | egui::epaint::Shape::Vec(_)
                | egui::epaint::Shape::Circle(_)
                | egui::epaint::Shape::Ellipse(_)
                | egui::epaint::Shape::LineSegment { .. }
                | egui::epaint::Shape::Path(_)
                | egui::epaint::Shape::Band(_)
                | egui::epaint::Shape::Rect(_)
                | egui::epaint::Shape::Text(_)
                | egui::epaint::Shape::Mesh(_)
                | egui::epaint::Shape::QuadraticBezier(_)
                | egui::epaint::Shape::CubicBezier(_)
                | egui::epaint::Shape::Callback(_) => None,
            })
            .unwrap();
        for section in &document.galley.job.sections {
            assert_eq!(
                section.format.font_id.family,
                egui::FontFamily::Name("IBM Plex Sans".into())
            );
            assert_eq!(section.format.font_id.size, 14.0);
            assert_eq!(section.format.line_height, Some(22.0));
            assert_eq!(section.format.color, palette.color("text-primary").unwrap());
        }
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_17_r_ui_18__reloaded_document_replaces_same_component_text_and_reflows_when_resized() {
    let mut harness = support::browser().unwrap();
    harness.get_by_role(egui::Role::Document).focus();
    harness.run();
    let replacement = "A replacement legal document with a longer paragraph that must wrap at the available width. ".repeat(30);
    let data = serde_json::json!({"schema_version":1,"components":[{"name":"Alpha","version":"1.2.3","licence":"MIT","declared_licence":"MIT OR Apache-2.0","source":"https://example.invalid/alpha","copyright":{"status":"provided","notices":["Copyright 2025 Alpha Authors"]},"notices":[{"path":"Alpha/LICENSE","text":replacement}]}]}).to_string();
    harness.state_mut().receive(BackgroundEvent::LicencesLoaded(
        tallyline_ui::api::parse_licence_catalogue(&data).unwrap(),
    ));
    harness.run();
    assert!(harness.get_by_role(egui::Role::Document).is_focused());
    assert_eq!(
        support::copy_document(&mut harness),
        Some(replacement.as_str())
    );
    let before = harness
        .output()
        .shapes
        .iter()
        .find_map(|shape| {
            if let egui::epaint::Shape::Text(text) = &shape.shape
                && text.galley.job.text == replacement
            {
                Some(text.galley.rect.width())
            } else {
                None
            }
        })
        .unwrap();
    harness.set_size(egui::vec2(400.0, 640.0));
    harness.run();
    let after = harness
        .output()
        .shapes
        .iter()
        .find_map(|shape| {
            if let egui::epaint::Shape::Text(text) = &shape.shape
                && text.galley.job.text == replacement
            {
                Some(text.galley.rect.width())
            } else {
                None
            }
        })
        .unwrap();
    assert!(
        after < before,
        "legal paragraphs must reflow into a narrower viewport"
    );
}
