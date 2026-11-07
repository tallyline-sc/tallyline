// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Widget geometry and semantic colours observed through painted output.

use super::resources;

use egui::{Color32, CornerRadius, RawInput, Rect, Shape, Vec2};
use tallyline_egui::{Chip, DeltaBadge, Density, Field, Motion, Panel, Theme, Toast};

#[test]
fn custom_widgets_use_token_geometry_and_semantic_surfaces_in_every_theme_and_density() {
    for (theme, palette) in [
        (Theme::Dark, tallyline_egui::tokens::DARK),
        (Theme::Light, tallyline_egui::tokens::LIGHT),
        (Theme::HighContrast, tallyline_egui::tokens::HIGH_CONTRAST),
    ] {
        for density in [Density::Compact, Density::Comfortable] {
            let context = egui::Context::default();
            tallyline_ui::api::configure_fonts(&context, resources::font_files().unwrap());
            tallyline_egui::apply_theme(&context, theme, density, Motion::Reduced);
            let mut panel = Rect::NOTHING;
            let mut content = Rect::NOTHING;
            let mut output = context.run_ui(RawInput::default(), |ui| {
                ui.add(Chip {
                    text: "Reference unavailable",
                    foreground: Color32::WHITE,
                    background: Color32::BLUE,
                });
                ui.add(DeltaBadge {
                    text: "Within tolerance",
                    major: false,
                    theme,
                });
                ui.add(Field {
                    label: "Price",
                    value: "12.50",
                    reason: Some("Check the value"),
                    theme,
                });
                let mut child = |ui: &mut egui::Ui| {
                    content = ui
                        .allocate_exact_size(egui::vec2(40.0, 20.0), egui::Sense::hover())
                        .0;
                };
                panel = ui
                    .add(Panel {
                        content: &mut child,
                        theme,
                    })
                    .rect;
                ui.add(Toast {
                    text: "Saved",
                    tone: tallyline_egui::SemanticTone::Success,
                    theme,
                });
            });
            output.textures_delta.clear();
            for (label, face, size, line_height) in [
                (
                    "Reference unavailable",
                    "IBM Plex Sans SemiBold",
                    13.0,
                    18.0,
                ),
                ("Within tolerance", "IBM Plex Sans SemiBold", 13.0, 18.0),
                ("Price", "IBM Plex Sans", 13.0, 18.0),
                ("12.50", "IBM Plex Sans", 14.0, 20.0),
                ("Check the value", "IBM Plex Sans", 12.0, 16.0),
                ("Saved", "IBM Plex Sans", 14.0, 20.0),
            ] {
                let text = output
                    .shapes
                    .iter()
                    .filter_map(super::paint_text::shape)
                    .find(|text| text.galley.job.text == label)
                    .unwrap();
                let format = &text.galley.job.sections[0].format;
                assert_eq!(
                    format.font_id.family,
                    egui::FontFamily::Name(face.into()),
                    "{label}"
                );
                assert_eq!(format.font_id.size, size, "{label}");
                assert_eq!(format.line_height, Some(line_height), "{label}");
            }
            let rectangles = painted_rectangles(&output.shapes);
            let chip = rectangles
                .iter()
                .find(|rectangle| rectangle.fill == Color32::BLUE)
                .unwrap();
            assert_eq!(
                chip.corner_radius,
                CornerRadius::same(2),
                "{theme:?} {density:?}"
            );
            let field = rectangles
                .iter()
                .find(|rectangle| rectangle.fill == palette.color("surface-sunken").unwrap())
                .unwrap();
            assert_eq!(field.corner_radius, CornerRadius::same(2));
            assert_eq!(field.stroke.color, palette.color("border-strong").unwrap());
            let raised = rectangles
                .iter()
                .find(|rectangle| rectangle.rect == panel)
                .unwrap();
            assert_eq!(raised.fill, palette.color("surface-2").unwrap());
            assert_eq!(raised.corner_radius, CornerRadius::same(2));
            assert_eq!(panel.size() - content.size(), Vec2::new(34.0, 26.0));
            assert!(rectangles.iter().any(|rectangle| rectangle.stroke.color
                == palette.color("deviation-minor-border").unwrap()
                && rectangle.corner_radius == CornerRadius::same(2)));
            let toast = rectangles
                .iter()
                .find(|rectangle| rectangle.fill == palette.color("success-bg").unwrap())
                .unwrap();
            assert_eq!(toast.stroke.color, palette.color("success-border").unwrap());
            assert!(output.shapes.iter().any(|shape| matches!(&shape.shape, Shape::Text(text) if text.galley.job.text == "Saved" && text.galley.job.sections[0].format.color == palette.color("success-fg").unwrap())));
        }
    }
}
#[test]
fn built_in_controls_and_dialogs_paint_themed_borders_and_keyboard_focus() {
    for (theme, palette) in [
        (Theme::Dark, tallyline_egui::tokens::DARK),
        (Theme::Light, tallyline_egui::tokens::LIGHT),
        (Theme::HighContrast, tallyline_egui::tokens::HIGH_CONTRAST),
    ] {
        for density in [Density::Compact, Density::Comfortable] {
            let context = egui::Context::default();
            tallyline_ui::api::configure_fonts(&context, resources::font_files().unwrap());
            tallyline_egui::apply_theme(&context, theme, density, Motion::Reduced);
            let mut button_id = None;
            let mut focused_output = None;
            for _ in 0..3 {
                let mut output = context.run_ui(RawInput::default(), |ui| {
                    let button = ui.button(tallyline_egui::button_text("Check references"));
                    button_id = Some(button.id);
                    egui::Frame::window(ui.style()).show(ui, |ui| {
                        ui.label("Dialog content");
                    });
                    ui.label(egui::RichText::new("Supporting text").weak());
                });
                output.textures_delta.clear();
                context.memory_mut(|memory| memory.request_focus(button_id.unwrap()));
                focused_output = Some(output);
            }
            let output = focused_output.unwrap();
            let rectangles = painted_rectangles(&output.shapes);
            assert!(
                rectangles
                    .iter()
                    .any(|rectangle| rectangle.corner_radius == CornerRadius::same(2)
                        && rectangle.stroke.width == 2.0
                        && rectangle.stroke.color == palette.color("focus-ring").unwrap()),
                "Missing keyboard focus: {theme:?} {density:?}"
            );
            assert!(
                rectangles
                    .iter()
                    .any(|rectangle| rectangle.corner_radius == CornerRadius::same(4)
                        && rectangle.fill == palette.color("surface-3").unwrap()),
                "Missing dialog surface: {theme:?}"
            );
            assert!(output.shapes.iter().any(|shape| matches!(&shape.shape, Shape::Text(text) if text.galley.job.text == "Supporting text" && text.galley.job.sections[0].format.color == palette.color("text-muted").unwrap())));
            assert_eq!(
                context.global_style().visuals.selection.bg_fill,
                palette.color("selection-text-bg").unwrap()
            );
        }
    }
}
fn painted_rectangles(shapes: &[egui::epaint::ClippedShape]) -> Vec<&egui::epaint::RectShape> {
    fn visit<'a>(shape: &'a Shape, output: &mut Vec<&'a egui::epaint::RectShape>) {
        if let Shape::Rect(rectangle) = shape {
            output.push(rectangle);
        } else if let Shape::Vec(shapes) = shape {
            for shape in shapes {
                visit(shape, output);
            }
        }
    }
    let mut rectangles = Vec::new();
    for shape in shapes {
        visit(&shape.shape, &mut rectangles);
    }
    rectangles
}
