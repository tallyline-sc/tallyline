// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact SCU boundary contracts.

use super::*;

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_2_q_manual__both_scu_bands_are_strict_in_both_directions_and_at_decimal_boundaries() {
    let tolerance = ScuTolerance::new(Percent::whole(25), "16".parse().unwrap());
    for (prior, value, expected) in [
        ("0", "0", Deviation::Equal),
        ("0", "16", Deviation::Minor),
        ("86", "70", Deviation::Minor),
        ("100", "125", Deviation::Minor),
        ("100", "125.00000000000000000000000001", Deviation::Major),
        ("100", "75", Deviation::Minor),
        ("100", "74.99999999999999999999999999", Deviation::Major),
        ("40", "24", Deviation::Minor),
        ("40", "23.99999999999999999999999999", Deviation::Major),
        ("40", "56", Deviation::Minor),
        ("40", "56.00000000000000000000000001", Deviation::Major),
        ("0.1", "0.2", Deviation::Minor),
        ("12000", "1200", Deviation::Major),
        ("12000", "120000", Deviation::Major),
    ] {
        assert_eq!(
            assessment(prior, value, tolerance).unwrap().deviation(),
            Some(expected),
            "{prior} → {value}"
        );
    }
    let zero = ScuTolerance::new(Percent::whole(0), "0".parse().unwrap());
    assert_eq!(
        assessment("0.1", "0.1000000000000000000000000001", zero)
            .unwrap()
            .deviation(),
        Some(Deviation::Major)
    );
    assert_eq!(
        assessment("16", "0", tolerance).unwrap().deviation(),
        Some(Deviation::Minor)
    );
    assert_eq!(
        assessment("16.1", "0", tolerance).unwrap().deviation(),
        Some(Deviation::Major)
    );
}
