// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Authoritative DTCG alpha preservation in resolved theme colours.

use tallyline_egui::tokens;

#[test]
fn scrim_preserves_each_source_themes_alpha() {
    for (palette, alpha) in [
        (tokens::DARK, 204),
        (tokens::LIGHT, 230),
        (tokens::HIGH_CONTRAST, 255),
    ] {
        assert_eq!(palette.color("scrim").unwrap().a(), alpha);
    }
}
