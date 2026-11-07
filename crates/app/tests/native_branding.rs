// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Native window identity from the approved Line artwork.

#![expect(
    non_snake_case,
    reason = "Requirement tags use the prescribed separator."
)]

use tallyline_app::api;

#[test]
fn r_ui_9__native_window_uses_the_fixed_line_mark() {
    let icon = api::native_window_icon().unwrap();
    assert_eq!([icon.width, icon.height], [256, 256]);
    assert_eq!(icon.rgba.len(), 256 * 256 * 4);
    let pixel = |x: usize, y: usize| {
        let start = ((y * 8 + 4) * 256 + x * 8 + 4) * 4;
        &icon.rgba[start..start + 4]
    };
    assert_eq!(pixel(0, 0)[3], 0);
    assert_eq!(pixel(16, 4), [0x25, 0x38, 0x49, 255]);
    assert_eq!(pixel(8, 9), [0xe8, 0xee, 0xf4, 255]);
    assert_eq!(pixel(24, 9), [0xe8, 0xee, 0xf4, 255]);
    assert_eq!(pixel(8, 15), [0x7c, 0xc3, 0xef, 255]);
    assert_eq!(pixel(20, 15), [0x7c, 0xc3, 0xef, 255]);
    assert_eq!(pixel(22, 15), [0x25, 0x38, 0x49, 255]);
    assert_eq!(pixel(8, 22), [0xe8, 0xee, 0xf4, 255]);
    assert_eq!(pixel(16, 22), [0xe8, 0xee, 0xf4, 255]);
    assert_eq!(pixel(18, 22), [0x25, 0x38, 0x49, 255]);
}
