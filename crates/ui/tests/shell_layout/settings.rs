// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Coherent Settings topic regions in the actual shell.

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

use super::Fixture;
use tallyline_workflows::api::model::Theme;

#[test]
fn settings_exposes_one_named_region_per_topic_without_collapsed_safety_controls()
-> Result<(), String> {
    for theme in [Theme::Dark, Theme::Light, Theme::HighContrast] {
        for zoom in [1.0, 2.0] {
            let mut harness = super::configured_shell(egui::vec2(800.0, 640.0), theme, zoom)?;
            harness.state_mut().shell.show_settings();
            harness.run();
            for label in [
                "Theme",
                "SCU review tolerance",
                "Submission mode and observation age",
                "Release updates",
                "UEX environment mapping",
                "Game log witness",
            ] {
                let section = harness.get_by_role_and_label(egui::Role::Group, label);
                assert_eq!(
                    section
                        .get_all_by_role_and_label(egui::Role::Label, label)
                        .count(),
                    1
                );
            }
            harness.get_by_role_and_label(
                egui::Role::TextInput,
                "Absolute SCU minimum (0 to 16) (SCU)",
            );
        }
    }
    Ok(())
}

#[test]
fn invalid_stock_feedback_stays_in_its_topic_and_fits_the_actual_shell_clip() -> Result<(), String>
{
    let invalid = "Enter a whole percentage from 0 to 20 and an exact SCU minimum from 0 to 16. Looser or invalid values are not applied.";
    for theme in [Theme::Dark, Theme::Light, Theme::HighContrast] {
        for zoom in [1.0, 2.0] {
            let mut harness = super::configured_shell(egui::vec2(800.0, 640.0), theme, zoom)?;
            harness.state_mut().shell.show_settings();
            harness.run();
            harness
                .get_by_role_and_label(egui::Role::TextInput, "SCU percentage (0 to 20) (%)")
                .focus();
            harness.run();
            harness
                .get_by_role_and_label(egui::Role::TextInput, "SCU percentage (0 to 20) (%)")
                .type_text("5000");
            harness.run();
            assert!(harness.state().commands.is_empty());
            harness.get_by_label("Save SCU tolerance").focus();
            harness.run();
            crate::control_paint::assert_focused_visible(
                &harness,
                &harness.get_by_label("Save SCU tolerance"),
                "Save SCU tolerance",
            )?;
            harness.key_press(egui::Key::Space);
            harness.run();
            assert!(harness.state().commands.is_empty());
            assert!(!harness.state().shell.busy());
            harness
                .get_by_role_and_label(egui::Role::Group, "SCU review tolerance")
                .get_by_label(invalid);
            let (clip, painted) = crate::control_paint::text_bounds(&harness, invalid)
                .ok_or_else(|| format!("Missing painted inline validation: {theme:?} {zoom}"))?;
            assert!(
                clip.contains_rect(painted),
                "{theme:?} {zoom}: feedback={painted:?} clip={clip:?}"
            );
        }
    }
    Ok(())
}

#[test]
fn settings_fields_and_actions_paint_completely_when_focused_in_each_theme_and_scale()
-> Result<(), String> {
    for theme in [Theme::Dark, Theme::Light, Theme::HighContrast] {
        for zoom in [1.0, 2.0] {
            let mut harness = super::configured_shell(egui::vec2(800.0, 640.0), theme, zoom)?;
            harness.state_mut().shell.show_settings();
            harness.run();
            assert_eq!(harness.ctx.pixels_per_point(), zoom);
            for (role, label, painted_text) in [
                (
                    egui::Role::TextInput,
                    "Absolute SCU minimum (0 to 16) (SCU)",
                    "16",
                ),
                (
                    egui::Role::Button,
                    "Save SCU tolerance",
                    "Save SCU tolerance",
                ),
                (
                    egui::Role::RadioButton,
                    "Production for future releases",
                    "Production for future releases",
                ),
                (
                    egui::Role::CheckBox,
                    "Check for release updates daily",
                    "Check for release updates daily",
                ),
                (egui::Role::TextInput, "Game channel directory", ""),
                (
                    egui::Role::Button,
                    "Use channel and switch witness off",
                    "Use channel and switch witness off",
                ),
                (
                    egui::Role::Button,
                    "Refresh witness facts",
                    "Refresh witness facts",
                ),
            ] {
                harness.get_by_role_and_label(role, label).focus();
                harness.run();
                let node = harness.get_by_role_and_label(role, label);
                if role == egui::Role::TextInput {
                    assert_input_visible(&harness, label, painted_text)?;
                } else {
                    crate::control_paint::assert_focused_visible(&harness, &node, painted_text)?;
                }
            }
            assert!(harness.state().commands.is_empty());
        }
    }
    Ok(())
}

fn assert_input_visible(
    harness: &Harness<'_, Fixture>,
    label: &str,
    value: &str,
) -> Result<(), String> {
    let node = harness.get_by_role_and_label(egui::Role::TextInput, label);
    assert!(node.is_focused(), "{label} has actual focus");
    assert!(!node.accesskit_node().is_disabled(), "{label} is enabled");
    let control = node.rect();
    let viewport = harness.ctx.input(|input| input.content_rect());
    assert!(viewport.contains_rect(control));
    assert!(control.bottom() <= harness.get_by_label("Saved submissions: 0").rect().top());
    assert!(
        crate::input_frame_paint::visible(harness, control),
        "Missing complete input frame: {label}"
    );
    let texts: &[&str] = if label == "Game channel directory" {
        &["Game channel directory"]
    } else {
        &["Absolute SCU minimum (0 to 16)", value, "SCU"]
    };
    for text in texts {
        let (clip, painted) = crate::control_paint::text_bounds(harness, text)
            .ok_or_else(|| format!("Missing input paint: {label}"))?;
        assert!(
            clip.contains_rect(painted),
            "{label}: paint={painted:?} effective clip={clip:?}"
        );
    }
    Ok(())
}

#[test]
fn settings_activation_proof_refuses_a_disabled_save_after_its_command() -> Result<(), String> {
    let mut harness = super::configured_shell(egui::vec2(800.0, 640.0), Theme::Light, 2.0)?;
    harness.state_mut().shell.show_settings();
    harness.run();
    harness.get_by_label("Save SCU tolerance").focus();
    harness.run();
    crate::control_paint::assert_focused_visible(
        &harness,
        &harness.get_by_label("Save SCU tolerance"),
        "Save SCU tolerance",
    )?;
    harness.key_press(egui::Key::Space);
    harness.run();
    assert!(harness.state().shell.busy());
    assert_eq!(harness.state().commands.len(), 1);
    let control = harness.get_by_label("Save SCU tolerance");
    assert!(control.accesskit_node().is_disabled());
    assert!(
        crate::control_paint::assert_focused_visible(&harness, &control, "Save SCU tolerance",)
            .is_err()
    );
    Ok(())
}
