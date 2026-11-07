// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Event-driven resolution of persisted theme preferences.

use tallyline_egui::Theme as ResolvedTheme;
use tallyline_workflows::api::model::Theme;

/// Resolves a persisted preference against the latest operating-system theme event.
#[must_use]
pub fn resolve_theme(preference: Theme, system: Option<egui::Theme>) -> ResolvedTheme {
    match preference {
        Theme::System => match system.unwrap_or(egui::Theme::Dark) {
            egui::Theme::Dark => ResolvedTheme::Dark,
            egui::Theme::Light => ResolvedTheme::Light,
        },
        Theme::Dark => ResolvedTheme::Dark,
        Theme::Light => ResolvedTheme::Light,
        Theme::HighContrast => ResolvedTheme::HighContrast,
    }
}
