// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Navigation paint follows semantic selection and the approved icon family.

use egui_kittest::kittest::{NodeT, Queryable};
use tallyline_workflows::api::model::Theme;

#[test]
fn navigation_paints_named_material_symbols_and_the_selected_accent_surface_in_each_theme() {
    for (theme, ui_theme) in [
        (Theme::Dark, tallyline_egui::Theme::Dark),
        (Theme::Light, tallyline_egui::Theme::Light),
        (Theme::HighContrast, tallyline_egui::Theme::HighContrast),
    ] {
        for width in [800.0, 1100.0] {
            let mut harness =
                super::configured_shell(egui::vec2(width, 640.0), theme, 1.0).unwrap();
            harness.run();
            let selected = harness
                .get_by_role_and_label(egui::Role::Button, "Reference data")
                .rect();
            let expected = ui_theme.palette().color("accent-subtle-bg").unwrap();
            assert!(harness.output().shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Rect(rect) if rect.fill == expected && rect.rect.contains_rect(selected))), "{theme:?} {width}: selected navigation uses accent-subtle-bg");
            assert!(harness.output().shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text == tallyline_egui::symbols::SETTINGS && text.galley.job.sections[0].format.font_id.family == egui::FontFamily::Name(tallyline_egui::symbols::FAMILY.into()) && text.galley.job.sections[0].format.font_id.size == 20.0)));
            assert!(
                harness
                    .get_by_role_and_label(egui::Role::Button, "Home")
                    .accesskit_node()
                    .is_disabled()
            );
            harness
                .get_by_role_and_label(egui::Role::Button, "Settings")
                .click();
            harness.run();
            assert_eq!(
                harness.state().shell.view(),
                tallyline_ui::api::model::View::Settings
            );
            harness.get_by_label("Back to setup").click();
            harness.run();
            assert_eq!(
                harness.state().shell.view(),
                tallyline_ui::api::model::View::Onboarding
            );
        }
    }
}
