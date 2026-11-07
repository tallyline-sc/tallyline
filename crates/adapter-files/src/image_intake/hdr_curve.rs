// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Recognition of standard HDR transfer curves in untagged ICC profiles.

use moxcms::{ColorProfile, ToneReprCurve};

const SIGNALS: [f32; 4] = [0.25, 0.5, 0.75, 0.875];
const HDR_CURVES: [[f32; 4]; 3] = [
    [0.000_515_417_6, 0.009_224_571, 0.098_337_784, 0.309_890_33],
    [0.020_833_334, 0.083_333_336, 0.264_962_55, 0.509_023_7],
    [0.009_605_291, 0.050_697_03, 0.203_152_15, 0.444_718_93],
];
const CURVE_TOLERANCE: f32 = 0.000_1;

pub(crate) fn is_hdr(profile: &ColorProfile) -> bool {
    [
        &profile.red_trc,
        &profile.green_trc,
        &profile.blue_trc,
        &profile.gray_trc,
    ]
    .into_iter()
    .flatten()
    .any(matches_hdr_curve)
}

fn matches_hdr_curve(curve: &ToneReprCurve) -> bool {
    let Ok(evaluator) = curve.make_linear_evaluator() else {
        return false;
    };
    HDR_CURVES.iter().any(|expected| {
        SIGNALS.iter().zip(expected).all(|(&signal, &value)| {
            (evaluator.evaluate_value(signal) - value).abs() <= CURVE_TOLERANCE
        })
    })
}
