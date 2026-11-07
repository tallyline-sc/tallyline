// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared raster value contracts.

use tallyline_shared_kernel::api::model::ImageRaster;

#[test]
fn raster_accepts_only_nonempty_complete_rows_and_exposes_immutable_argb() {
    assert!(ImageRaster::new(0, 1, vec![]).is_err());
    assert!(ImageRaster::new(1, 0, vec![]).is_err());
    assert!(ImageRaster::new(2, 1, vec![0xff12_3456]).is_err());
    let pixels = [
        0xff12_3456,
        0xffab_cdef,
        0xff00_0000,
        0xffff_ffff,
        0xff01_0203,
        0xfffe_dcba,
    ];
    let raster = ImageRaster::new(2, 3, pixels.to_vec()).unwrap();
    assert_eq!((raster.width(), raster.height()), (2, 3));
    assert_eq!(raster.argb(), &pixels);
}

#[test]
fn raster_debug_output_does_not_disclose_capture_pixels() {
    for (width, height, count, expected) in [
        (13, 17, 221, ["width: 13", "height: 17", "pixels: 221"]),
        (7, 11, 77, ["width: 7", "height: 11", "pixels: 77"]),
    ] {
        let raster = ImageRaster::new(width, height, vec![0xff12_3456; count]).unwrap();
        let debug = format!("{raster:?}");
        for field in expected {
            assert!(debug.contains(field), "{debug}");
        }
        assert!(!debug.contains("4279383126"));
        assert!(!debug.to_lowercase().contains("ff123456"));
    }
}
