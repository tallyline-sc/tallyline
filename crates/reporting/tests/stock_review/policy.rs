// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Candidate sensitivity is distinct from calibrated shipping defaults.

use super::*;

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_2_q_manual__exploratory_candidates_do_not_establish_a_unique_calibrated_pair() {
    for percent in [10, 19, 25, 50] {
        for minimum in [0, 15, 16, 25] {
            let tolerance = ScuTolerance::new(
                Percent::whole(percent),
                minimum.to_string().parse().unwrap(),
            );
            let expected = if percent == 10 && minimum < 16 {
                Deviation::Major
            } else {
                Deviation::Minor
            };
            assert_eq!(
                assessment("86", "70", tolerance).unwrap().deviation(),
                Some(expected),
                "P={percent} A={minimum}"
            );
            assert_eq!(
                assessment("86", "86", tolerance).unwrap().deviation(),
                Some(Deviation::Equal)
            );
            for (prior, value) in [("86", "860"), ("12000", "1200"), ("12000", "120000")] {
                assert_eq!(
                    assessment(prior, value, tolerance).unwrap().deviation(),
                    Some(Deviation::Major)
                );
            }
            let small_digit = if minimum < 18 {
                Deviation::Major
            } else {
                Deviation::Minor
            };
            assert_eq!(
                assessment("2", "20", tolerance).unwrap().deviation(),
                Some(small_digit)
            );
        }
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_2_q_manual_r_nf_5__tightening_never_removes_a_major_quantity_deviation() {
    for (prior, value) in [
        ("0", "0"),
        ("0", "17"),
        ("2", "20"),
        ("86", "70"),
        ("12000", "1200"),
        ("12000", "120000"),
    ] {
        for percent in [0, 10, 19, 25, 5000] {
            for minimum in [0, 15, 16, 25] {
                let tolerance = ScuTolerance::new(
                    Percent::whole(percent),
                    minimum.to_string().parse().unwrap(),
                );
                let original = assessment(prior, value, tolerance).unwrap();
                for override_percent in [0, 10, 19, 25, 5000] {
                    for override_minimum in [0, 15, 16, 25] {
                        let tightened = tolerance.tighten(
                            Percent::whole(override_percent),
                            override_minimum.to_string().parse().unwrap(),
                        );
                        assert!(tightened.percent().get() <= tolerance.percent().get());
                        assert!(tightened.minimum() <= tolerance.minimum());
                        if original.deviation() == Some(Deviation::Major) {
                            assert_eq!(
                                assessment(prior, value, tightened).unwrap().deviation(),
                                Some(Deviation::Major)
                            );
                        }
                    }
                }
            }
        }
    }
}
