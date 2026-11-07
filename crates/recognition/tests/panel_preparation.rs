// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Public supplied-panel preparation contracts.

use tallyline_recognition::api::model::{
    GeometryProvenance, PanelGeometry, PanelLimits, SuppliedPanels,
};
use tallyline_recognition::api::prepare_panels;
use tallyline_shared_kernel::api::model::ImageRaster;

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags contain a double underscore."
)]
fn r_ocr_2__supplied_skewed_and_tall_panels_keep_upright_mapping_and_source_pixels() {
    let image = ImageRaster::new(8, 8, (0..64).map(|v| 0xff00_0000 | v).collect()).unwrap();
    let before = image.clone();
    let supplied = SuppliedPanels {
        shop: PanelGeometry {
            corners: [[2., 2.], [6., 2.], [5., 6.], [3., 6.]],
            output: [4, 4],
        },
        location: PanelGeometry {
            corners: [[0., 2.], [2., 2.], [2., 8.], [0., 8.]],
            output: [2, 6],
        },
        header: [[0., 0.], [8., 0.], [8., 1.], [0., 1.]],
        provenance: GeometryProvenance::Manual,
    };
    let prepared = prepare_panels(&image, &supplied, PanelLimits::default()).unwrap();
    assert_eq!(prepared.provenance(), GeometryProvenance::Manual);
    assert!(prepared.requires_privacy_confirmation());
    assert_eq!(
        [
            prepared.location().raster().width(),
            prepared.location().raster().height()
        ],
        [2, 6]
    );
    assert_eq!(prepared.location().raster().argb()[0], 0xff00_0010);
    assert_eq!(prepared.location().raster().argb()[11], 0xff00_0039);
    let source = prepared.shop().transform().to_source([2., 2.]).unwrap();
    assert!((source[0] - 4.).abs() < 1e-10);
    assert!((source[1] - 14. / 3.).abs() < 1e-10);
    let normalised = prepared
        .shop()
        .transform()
        .from_source([4., 14. / 3.])
        .unwrap();
    assert!((normalised[0] - 2.).abs() < 1e-10);
    assert!((normalised[1] - 2.).abs() < 1e-10);
    assert_eq!(image, before);
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags contain a double underscore."
)]
fn r_cap_7_r_sub_7__fractional_header_pixels_are_masked_before_projective_bilinear_filtering() {
    let supplied = SuppliedPanels {
        shop: PanelGeometry {
            corners: [[1.25, 2.1], [5.75, 2.4], [5.25, 6.], [1.75, 6.]],
            output: [9, 8],
        },
        location: PanelGeometry {
            corners: [[0., 2.], [2., 2.], [2., 5.], [0., 5.]],
            output: [4, 6],
        },
        header: [[0., 0.], [8., 0.], [8., 2.25], [0., 2.25]],
        provenance: GeometryProvenance::Profile,
    };
    let make_image = |secret| {
        ImageRaster::new(
            8,
            8,
            (0..64)
                .map(|i| if i / 8 < 3 { secret } else { 0xff22_4466 })
                .collect(),
        )
        .unwrap()
    };
    let first =
        prepare_panels(&make_image(0xffff_00ff), &supplied, PanelLimits::default()).unwrap();
    let second =
        prepare_panels(&make_image(0xffff_ffff), &supplied, PanelLimits::default()).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.location().raster().argb()[0], 0xff00_0000);
    assert_eq!(
        *first.location().raster().argb().last().unwrap(),
        0xff22_4466
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags contain a double underscore."
)]
fn r_ocr_2_r_cap_7__minification_averages_source_footprints_after_privacy_masking() {
    let image = ImageRaster::new(
        8,
        8,
        (0..64)
            .map(|i| if i % 8 == 1 { 0xffff_ffff } else { 0xff00_0000 })
            .collect(),
    )
    .unwrap();
    for (header_bottom, expected) in [(1., 0xff40_4040), (2.25, 0xff30_3030)] {
        let supplied = SuppliedPanels {
            shop: PanelGeometry {
                corners: [[0., 2.], [4., 2.], [4., 6.], [0., 6.]],
                output: [1, 1],
            },
            location: PanelGeometry {
                corners: [[4., 2.], [6., 2.], [6., 6.], [4., 6.]],
                output: [1, 1],
            },
            header: [[0., 0.], [8., 0.], [8., header_bottom], [0., header_bottom]],
            provenance: GeometryProvenance::Manual,
        };
        let result = prepare_panels(&image, &supplied, PanelLimits::default()).unwrap();
        assert_eq!(result.shop().raster().argb(), [expected]);
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags contain a double underscore."
)]
fn r_ocr_2__invalid_geometry_and_excessive_output_or_sampling_are_typed_refusals() {
    use tallyline_recognition::api::model::PanelRefusal;
    let image = ImageRaster::new(8, 8, vec![0xff22_4466; 64]).unwrap();
    let valid = SuppliedPanels {
        shop: PanelGeometry {
            corners: [[2., 2.], [6., 2.], [6., 6.], [2., 6.]],
            output: [4, 4],
        },
        location: PanelGeometry {
            corners: [[0., 2.], [2., 2.], [2., 6.], [0., 6.]],
            output: [2, 4],
        },
        header: [[0., 0.], [8., 0.], [8., 1.], [0., 1.]],
        provenance: GeometryProvenance::Manual,
    };
    for corners in [
        [[0., 0.]; 4],
        [[0., 0.], [2., 2.], [2., 0.], [0., 2.]],
        [[1., 1.], [2., 1.], [3., 1.], [4., 1.]],
        [[f64::NAN, 1.], [6., 2.], [6., 6.], [2., 6.]],
        [[f64::INFINITY, 1.], [6., 2.], [6., 6.], [2., 6.]],
        [[-0.1, 1.], [6., 2.], [6., 6.], [2., 6.]],
        [[2., 2.], [9., 2.], [6., 6.], [2., 6.]],
    ] {
        let mut supplied = valid;
        supplied.shop.corners = corners;
        assert_eq!(
            prepare_panels(&image, &supplied, PanelLimits::default()),
            Err(PanelRefusal::Geometry)
        );
    }
    let mut oversized = valid;
    oversized.shop.output = [u32::MAX, u32::MAX];
    oversized.location.output = [u32::MAX, u32::MAX];
    assert_eq!(
        prepare_panels(&image, &oversized, PanelLimits::default()),
        Err(PanelRefusal::OutputLimit)
    );
    let limits = PanelLimits::new(4096, 4_194_304, 1).unwrap();
    assert_eq!(
        prepare_panels(&image, &valid, limits),
        Err(PanelRefusal::SamplingLimit)
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags contain a double underscore."
)]
fn r_ocr_2_r_cap_7__bounded_mapping_translation_and_privacy_invariants_hold_across_working_scales()
{
    for output in [[1, 1], [3, 4], [9, 11]] {
        let supplied = SuppliedPanels {
            shop: PanelGeometry {
                corners: [[2., 2.1], [6., 2.3], [5.5, 6.], [2.5, 6.]],
                output,
            },
            location: PanelGeometry {
                corners: [[1., 3.], [2., 3.], [2., 5.], [1., 5.]],
                output: [2, 4],
            },
            header: [[0., 0.], [8., 0.], [8., 2.25], [0., 2.25]],
            provenance: GeometryProvenance::Manual,
        };
        let mut baseline = None;
        for secret in [0xff00_0000, 0xffff_ffff, 0xfff1_02e3] {
            let image = ImageRaster::new(
                8,
                8,
                (0..64)
                    .map(|i| {
                        if i / 8 < 3 {
                            secret
                        } else {
                            0xff00_0000 | ((i * 3) << 8) | i
                        }
                    })
                    .collect(),
            )
            .unwrap();
            let prepared = prepare_panels(&image, &supplied, PanelLimits::default()).unwrap();
            if let Some(first) = &baseline {
                assert_eq!(first, &prepared);
            } else {
                baseline = Some(prepared);
            }
        }
        let image = ImageRaster::new(8, 8, (0..64).map(|i| 0xff00_0000 | i).collect()).unwrap();
        let original = prepare_panels(&image, &supplied, PanelLimits::default()).unwrap();
        let mut padded = vec![0xff12_3456; 12 * 12];
        for y in 0..8 {
            for x in 0..8 {
                padded[(y + 1) * 12 + x + 2] = image.argb()[y * 8 + x];
            }
        }
        let padded = ImageRaster::new(12, 12, padded).unwrap();
        let shift = |corners: [[f64; 2]; 4]| corners.map(|[x, y]| [x + 2., y + 1.]);
        let translated = SuppliedPanels {
            shop: PanelGeometry {
                corners: shift(supplied.shop.corners),
                ..supplied.shop
            },
            location: PanelGeometry {
                corners: shift(supplied.location.corners),
                ..supplied.location
            },
            header: shift(supplied.header),
            ..supplied
        };
        let translated = prepare_panels(&padded, &translated, PanelLimits::default()).unwrap();
        assert_eq!(translated.shop().raster(), original.shop().raster());
        assert_eq!(translated.location().raster(), original.location().raster());
        for point in [[0., 0.], [0.5, 0.5], [1., 1.]] {
            let point = [
                point[0] * f64::from(output[0]),
                point[1] * f64::from(output[1]),
            ];
            let source = original.shop().transform().to_source(point).unwrap();
            let shifted = translated.shop().transform().to_source(point).unwrap();
            assert!((shifted[0] - source[0] - 2.).abs() < 1e-9);
            assert!((shifted[1] - source[1] - 1.).abs() < 1e-9);
            let roundtrip = original.shop().transform().from_source(source).unwrap();
            assert!((roundtrip[0] - point[0]).abs() < 1e-9);
            assert!((roundtrip[1] - point[1]).abs() < 1e-9);
        }
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags contain a double underscore."
)]
fn r_cap_7__a_panel_with_no_extent_outside_the_header_is_refused() {
    use tallyline_recognition::api::model::PanelRefusal;
    let image = ImageRaster::new(8, 8, vec![0xff00_0000; 64]).unwrap();
    let supplied = SuppliedPanels {
        shop: PanelGeometry {
            corners: [[2., 2.], [6., 2.], [6., 6.], [2., 6.]],
            output: [4, 4],
        },
        location: PanelGeometry {
            corners: [[0., 2.], [2., 2.], [2., 5.], [0., 5.]],
            output: [2, 3],
        },
        header: [[0., 0.], [8., 0.], [8., 8.], [0., 8.]],
        provenance: GeometryProvenance::Manual,
    };
    for header in [
        [[0., 0.], [8., 0.], [8., 8.], [0., 8.]],
        [[0., 0.], [8., 0.], [8., 8.], [0., 6.]],
        [[0., 0.], [8., 0.], [8., 6.], [0., 8.]],
    ] {
        let supplied = SuppliedPanels { header, ..supplied };
        assert_eq!(
            prepare_panels(&image, &supplied, PanelLimits::default()),
            Err(PanelRefusal::PrivacyExtent),
            "header {header:?}"
        );
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags contain a double underscore."
)]
fn r_ocr_2__mapping_refuses_nonfinite_points_and_overflowing_inverse_outputs() {
    let image = ImageRaster::new(8, 8, vec![0xff22_4466; 64]).unwrap();
    let supplied = SuppliedPanels {
        shop: PanelGeometry {
            corners: [[1., 2.], [2., 2.], [2., 3.], [1., 3.]],
            output: [4096, 1],
        },
        location: PanelGeometry {
            corners: [[0., 3.], [2., 3.], [2., 5.], [0., 5.]],
            output: [2, 2],
        },
        header: [[0., 0.], [8., 0.], [8., 1.], [0., 1.]],
        provenance: GeometryProvenance::Manual,
    };
    let prepared = prepare_panels(&image, &supplied, PanelLimits::default()).unwrap();
    assert!(
        prepared
            .shop()
            .transform()
            .to_source([f64::NAN, 0.])
            .is_none()
    );
    assert!(
        prepared
            .shop()
            .transform()
            .from_source([f64::MAX, 0.])
            .is_none()
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags contain a double underscore."
)]
fn r_nf_5__panel_limits_validate_tighten_only_values_at_construction() {
    let limits = PanelLimits::new(64, 1024, 4096).unwrap();
    assert_eq!(limits.max_side(), 64);
    assert_eq!(limits.max_total_pixels(), 1024);
    assert_eq!(limits.max_sampled_pixels(), 4096);
    for (side, pixels, samples) in [
        (0, 1, 1),
        (4097, 1, 1),
        (1, 0, 1),
        (1, 4_194_305, 1),
        (1, 1, 0),
        (1, 1, 128_000_001),
    ] {
        assert!(PanelLimits::new(side, pixels, samples).is_err());
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags contain a double underscore."
)]
fn r_cap_7_r_sub_7__header_boundary_excludes_the_upper_region_before_every_filter() {
    for (header, corners, output, last_rows) in [
        (
            [[0., 2.], [8., 2.], [8., 3.], [0., 3.]],
            [[0., 0.], [8., 0.], [8., 6.], [0., 6.]],
            [8, 6],
            [3; 8],
        ),
        (
            [[0., 1.8], [8., 2.2], [8., 3.2], [0., 2.8]],
            [[0.5, 0.2], [7.5, 0.4], [6.5, 6.5], [1.5, 6.5]],
            [14, 12],
            [2, 2, 2, 3, 3, 3, 3, 3],
        ),
        (
            [[0., 1.8], [8., 2.2], [8., 3.2], [0., 2.8]],
            [[0.5, 0.2], [7.5, 0.4], [6.5, 6.5], [1.5, 6.5]],
            [3, 2],
            [2, 2, 2, 3, 3, 3, 3, 3],
        ),
    ] {
        let mut unpadded = None;
        for [dx, dy] in [[0_u32, 0_u32], [2, 1]] {
            let width = 8 + 2 * dx;
            let height = 8 + 2 * dy;
            let shift =
                |quad: [[f64; 2]; 4]| quad.map(|[x, y]| [x + f64::from(dx), y + f64::from(dy)]);
            let supplied = SuppliedPanels {
                shop: PanelGeometry {
                    corners: shift(corners),
                    output,
                },
                location: PanelGeometry {
                    corners: shift([[0., 4.], [2., 4.], [2., 8.], [0., 8.]]),
                    output: [2, 4],
                },
                header: shift(header),
                provenance: GeometryProvenance::Profile,
            };
            let image = |secret| {
                ImageRaster::new(
                    width,
                    height,
                    (0..width * height)
                        .map(|index| {
                            let x = index % width;
                            let y = index / width;
                            if x >= dx
                                && x < dx + 8
                                && y >= dy
                                && y - dy <= last_rows[(x - dx) as usize]
                            {
                                secret
                            } else {
                                0xff22_4466
                            }
                        })
                        .collect(),
                )
                .unwrap()
            };
            let first_source = image(0xffff_00ff);
            let before = first_source.clone();
            let first = prepare_panels(&first_source, &supplied, PanelLimits::default()).unwrap();
            let second =
                prepare_panels(&image(0xff00_ff11), &supplied, PanelLimits::default()).unwrap();
            assert_eq!(first, second);
            assert_eq!(first_source, before);

            assert!(
                first
                    .shop()
                    .raster()
                    .argb()
                    .iter()
                    .any(|pixel| *pixel != 0xff00_0000)
            );
            assert!(
                first
                    .location()
                    .raster()
                    .argb()
                    .iter()
                    .all(|pixel| *pixel == 0xff22_4466)
            );
            if let Some(reference) = &unpadded {
                assert_eq!(first.shop().raster(), reference);
            } else {
                unpadded = Some(first.shop().raster().clone());
            }
        }
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags contain a double underscore."
)]
fn r_ocr_2_r_cap_7__positive_skew_preserves_safe_cell_outside_header_width() {
    let mut pixels = vec![0xff22_4466; 64];
    pixels[16] = 0xff12_3456;
    let image = ImageRaster::new(8, 8, pixels).unwrap();
    let supplied = SuppliedPanels {
        shop: PanelGeometry {
            corners: [[0., 0.], [8., 0.], [8., 8.], [0., 8.]],
            output: [8, 8],
        },
        location: PanelGeometry {
            corners: [[0., 6.], [2., 6.], [2., 8.], [0., 8.]],
            output: [2, 2],
        },
        header: [[2., 0.], [6., 0.], [6., 4.], [2., 2.]],
        provenance: GeometryProvenance::Manual,
    };
    let prepared = prepare_panels(&image, &supplied, PanelLimits::default()).unwrap();
    assert_eq!(prepared.shop().raster().argb()[16], 0xff12_3456);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags contain a double underscore."
)]
fn r_cap_7_r_sub_7__both_skews_exclude_contact_cells_before_both_filters() {
    for (header, last_rows, witness) in [
        (
            [[2., 0.], [6., 0.], [6., 4.], [2., 2.]],
            [1, 2, 2, 3, 3, 4, 4, 5],
            16,
        ),
        (
            [[2., 0.], [6., 0.], [6., 2.], [2., 4.]],
            [5, 4, 4, 3, 3, 2, 2, 1],
            23,
        ),
    ] {
        for (output, expected) in [
            ([8, 8], 0xff80_8080),
            ([16, 16], 0xff80_8080),
            ([1, 1], 0xff40_4040),
        ] {
            let supplied = SuppliedPanels {
                shop: PanelGeometry {
                    corners: [[0., 0.], [8., 0.], [8., 8.], [0., 8.]],
                    output,
                },
                location: PanelGeometry {
                    corners: [[0., 6.], [8., 6.], [8., 8.], [0., 8.]],
                    output: [8, 2],
                },
                header,
                provenance: GeometryProvenance::Manual,
            };
            let image = |secret| {
                ImageRaster::new(
                    8,
                    8,
                    (0..64)
                        .map(|i| {
                            if i / 8 <= last_rows[i % 8] {
                                secret
                            } else {
                                0xff80_8080
                            }
                        })
                        .collect(),
                )
                .unwrap()
            };
            let source = image(0xffff_00ff);
            let before = source.clone();
            let first = prepare_panels(&source, &supplied, PanelLimits::default()).unwrap();
            let second =
                prepare_panels(&image(0xff00_ff00), &supplied, PanelLimits::default()).unwrap();
            assert_eq!(first, second);
            assert_eq!(source, before);
            if output == [8, 8] {
                assert_eq!(first.shop().raster().argb()[witness], expected);
                for (x, y) in last_rows.into_iter().enumerate() {
                    assert_eq!(first.shop().raster().argb()[y * 8 + x], 0xff00_0000);
                    assert_eq!(first.shop().raster().argb()[(y + 1) * 8 + x], expected);
                }
            } else if output == [1, 1] {
                assert_eq!(first.shop().raster().argb(), [expected]);
            } else {
                assert_eq!(*first.shop().raster().argb().last().unwrap(), expected);
            }
        }
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags contain a double underscore."
)]
fn r_cap_7__unsupported_header_labels_and_invalid_coordinates_refuse_geometry() {
    use tallyline_recognition::api::model::PanelRefusal;
    let image = ImageRaster::new(8, 8, vec![0xff80_8080; 64]).unwrap();
    for header in [
        [[2., 0.], [2., 2.], [6., 4.], [6., 0.]],
        [[6., 0.], [6., 4.], [2., 2.], [2., 0.]],
        [[6., 4.], [2., 2.], [2., 0.], [6., 0.]],
        [[1., 1.], [3., 1.], [3., 5.], [3., 3.]],
        [[0., 0.]; 4],
        [[0., 0.], [8., 0.], [8., 1e-12], [0., 1e-12]],
        [[0., 0.], [8., 0.], [2., 1.], [0., 4.]],
        [[f64::NAN, 0.], [8., 0.], [8., 2.], [0., 2.]],
        [[0., 0.], [f64::INFINITY, 0.], [8., 2.], [0., 2.]],
        [[0., 0.], [f64::from(u32::MAX), 0.], [8., 2.], [0., 2.]],
    ] {
        let supplied = SuppliedPanels {
            shop: PanelGeometry {
                corners: [[0., 0.], [8., 0.], [8., 8.], [0., 8.]],
                output: [8, 8],
            },
            location: PanelGeometry {
                corners: [[0., 6.], [2., 6.], [2., 8.], [0., 8.]],
                output: [2, 2],
            },
            header,
            provenance: GeometryProvenance::Profile,
        };
        assert_eq!(
            prepare_panels(&image, &supplied, PanelLimits::default()),
            Err(PanelRefusal::Geometry),
            "{header:?}"
        );
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags contain a double underscore."
)]
fn r_cap_7__fractional_skew_contacts_and_neighbouring_boundaries_keep_safe_cells() {
    for (bottom, rows) in [
        ([3.875, 1.875], [1, 1, 2, 2, 3, 3, 4, 4]),
        ([4., 2.], [1, 2, 2, 3, 3, 4, 4, 5]),
        ([4.125, 2.125], [1, 2, 2, 3, 3, 4, 4, 5]),
        (
            [4_f64.next_down(), 2_f64.next_down()],
            [1, 2, 2, 3, 3, 4, 4, 5],
        ),
        ([4_f64.next_up(), 2_f64.next_up()], [1, 2, 2, 3, 3, 4, 4, 5]),
    ] {
        for mirrored in [false, true] {
            let (header, rows) = if mirrored {
                let mut rows = rows;
                rows.reverse();
                ([[2., 0.], [6., 0.], [6., bottom[1]], [2., bottom[0]]], rows)
            } else {
                ([[2., 0.], [6., 0.], [6., bottom[0]], [2., bottom[1]]], rows)
            };
            let source = ImageRaster::new(8, 8, vec![0xff12_3456; 64]).unwrap();
            let supplied = SuppliedPanels {
                shop: PanelGeometry {
                    corners: [[0., 0.], [8., 0.], [8., 8.], [0., 8.]],
                    output: [8, 8],
                },
                location: PanelGeometry {
                    corners: [[0., 6.], [2., 6.], [2., 8.], [0., 8.]],
                    output: [2, 2],
                },
                header,
                provenance: GeometryProvenance::Manual,
            };
            let result = prepare_panels(&source, &supplied, PanelLimits::default()).unwrap();
            for (x, last) in rows.into_iter().enumerate() {
                let prohibited = if bottom == [4_f64.next_down(), 2_f64.next_down()] {
                    last - 1
                } else {
                    last
                };
                assert_eq!(
                    result.shop().raster().argb()[prohibited * 8 + x],
                    0xff00_0000,
                    "{bottom:?} {mirrored} {x}"
                );
                assert_eq!(
                    result.shop().raster().argb()[(last + 1) * 8 + x],
                    0xff12_3456
                );
            }
        }
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags contain a double underscore."
)]
fn r_cap_7__allocated_translated_thin_raster_keeps_safe_cells_with_small_edge_offsets() {
    let width = 262_144;
    let image = ImageRaster::new(width, 8, vec![0xff12_3456; width as usize * 8]).unwrap();
    let base = f64::from(width) - 8.;
    let supplied = SuppliedPanels {
        shop: PanelGeometry {
            corners: [[base, 0.], [base + 8., 0.], [base + 8., 8.], [base, 8.]],
            output: [8, 8],
        },
        location: PanelGeometry {
            corners: [[base, 6.], [base + 2., 6.], [base + 2., 8.], [base, 8.]],
            output: [2, 2],
        },
        header: [
            [base + 2., 0.],
            [base + 6., 0.],
            [base + 6., 2.000_000_000_2],
            [base + 2., 2.000_000_000_1],
        ],
        provenance: GeometryProvenance::Profile,
    };
    let result = prepare_panels(&image, &supplied, PanelLimits::default()).unwrap();
    assert_eq!(result.shop().raster().argb()[18], 0xff00_0000);
    assert_eq!(result.shop().raster().argb()[26], 0xff12_3456);
}
