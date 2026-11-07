// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Theme settings and egui style mapping.

use egui::{Color32, Context, CornerRadius, Margin, Stroke, Style, TextStyle, Visuals};

use crate::TypeStyle;
use crate::tokens::{self, ThemeColors};

/// A Tallyline colour theme.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Theme {
    /// Dark slate surfaces.
    #[default]
    Dark,
    /// Light surfaces.
    Light,
    /// High contrast surfaces.
    HighContrast,
}

/// Widget spacing preset.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Density {
    /// Compact controls for data-dense screens.
    Compact,
    /// Comfortable controls for general screens.
    #[default]
    Comfortable,
}

/// Motion preference.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Motion {
    /// Use the design-system transitions.
    #[default]
    Full,
    /// Suppress transitions.
    Reduced,
}

impl Theme {
    /// Returns the canonical resolved colours for this theme.
    #[must_use]
    pub const fn palette(self) -> &'static ThemeColors {
        match self {
            Self::Dark => &tokens::DARK,
            Self::Light => &tokens::LIGHT,
            Self::HighContrast => &tokens::HIGH_CONTRAST,
        }
    }
}
/// Apply Tallyline colours, typography and spacing to an egui context.
pub fn apply_theme(ctx: &Context, theme: Theme, density: Density, motion: Motion) {
    let palette = theme.palette();
    let mut style = Style::default();
    style.spacing.item_spacing = egui::vec2(
        8.0,
        if density == Density::Compact {
            4.0
        } else {
            8.0
        },
    );
    style.spacing.button_padding = egui::vec2(12.0, 6.0);
    style.spacing.interact_size = egui::vec2(32.0, 32.0);
    style.animation_time = if motion == Motion::Reduced { 0.0 } else { 0.12 };
    style.text_styles = TypeStyle::ALL
        .into_iter()
        .map(|style| (style.text_style(), style.font_id()))
        .chain([
            (TextStyle::Small, TypeStyle::CAPTION.font_id()),
            (TextStyle::Body, TypeStyle::BODY.font_id()),
            (TextStyle::Button, TypeStyle::BUTTON.font_id()),
            (TextStyle::Heading, TypeStyle::HEADING_1.font_id()),
            (TextStyle::Monospace, TypeStyle::NUMERALS.font_id()),
        ])
        .collect();
    let mut visuals = Visuals::dark();
    if theme == Theme::Light {
        visuals = Visuals::light();
    }
    let primary = color(palette, "text-primary", Color32::WHITE);
    let strong_border = color(palette, "border-strong", Color32::GRAY);
    let focus = color(palette, "focus-ring", Color32::LIGHT_BLUE);
    let sunken = color(palette, "surface-sunken", Color32::BLACK);
    let raised = color(palette, "surface-3", Color32::BLACK);
    visuals.override_text_color = None;
    visuals.weak_text_color = Some(color(palette, "text-muted", primary));
    visuals.hyperlink_color = color(palette, "text-link", primary);
    visuals.warn_fg_color = color(palette, "caution-fg", primary);
    visuals.error_fg_color = color(palette, "critical-fg", primary);
    visuals.panel_fill = color(palette, "surface-1", visuals.panel_fill);
    visuals.window_fill = raised;
    visuals.window_corner_radius = CornerRadius::same(4);
    visuals.window_stroke = Stroke::new(1.0, color(palette, "border-default", strong_border));
    visuals.menu_corner_radius = CornerRadius::same(4);
    visuals.extreme_bg_color = sunken;
    visuals.text_edit_bg_color = Some(sunken);
    visuals.faint_bg_color = color(palette, "surface-0", visuals.faint_bg_color);
    visuals.code_bg_color = color(palette, "surface-2", sunken);
    visuals.selection.bg_fill = color(palette, "selection-text-bg", visuals.selection.bg_fill);
    visuals.selection.stroke = Stroke::new(1.0, primary);
    visuals.text_cursor.stroke = Stroke::new(2.0, primary);
    visuals.text_cursor.blink = false;
    visuals.image_loading_spinners = false;
    if theme == Theme::HighContrast {
        visuals.window_shadow = egui::epaint::Shadow::NONE;
        visuals.popup_shadow = egui::epaint::Shadow::NONE;
    }
    for widget in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.corner_radius = CornerRadius::same(2);
        widget.fg_stroke = Stroke::new(1.0, primary);
        widget.bg_stroke = Stroke::new(1.0, strong_border);
        widget.expansion = 0.0;
    }
    visuals.widgets.noninteractive.bg_fill = color(palette, "surface-2", sunken);
    visuals.widgets.noninteractive.weak_bg_fill = visuals.panel_fill;
    visuals.widgets.noninteractive.bg_stroke = visuals.window_stroke;
    visuals.widgets.inactive.bg_fill = sunken;
    visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    visuals.widgets.hovered.bg_fill = raised;
    visuals.widgets.hovered.weak_bg_fill = raised;
    visuals.widgets.active.bg_fill = sunken;
    visuals.widgets.active.weak_bg_fill = sunken;
    visuals.widgets.active.bg_stroke = Stroke::new(2.0, focus);
    visuals.widgets.open.bg_fill = raised;
    visuals.widgets.open.weak_bg_fill = raised;
    style.visuals = visuals;
    style.spacing.window_margin = Margin::same(16);
    let egui_theme = if theme == Theme::Light {
        egui::Theme::Light
    } else {
        egui::Theme::Dark
    };
    ctx.set_style_of(egui_theme, style);
    ctx.set_theme(egui_theme);
}

fn color(palette: &ThemeColors, name: &str, fallback: Color32) -> Color32 {
    palette.color(name).unwrap_or(fallback)
}
