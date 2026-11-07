// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact, context-independent decimal value contracts.

use tallyline_shared_kernel::api::model::{PricePerScu, ScuQuantity};

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_api_5__prices_and_quantities_keep_decimal_and_scientific_spelling_exact() {
    for (text, expected) in [
        ("9007199254740993.125", "9007199254740993.125"),
        ("9.7e-7", "0.00000097"),
        ("1.0e-28", "0.0000000000000000000000000001"),
        ("1.25E+2", "125"),
        ("-0.000", "0"),
        (
            "79228162514264337593543950335",
            "79228162514264337593543950335",
        ),
    ] {
        let price = text.parse::<PricePerScu>().unwrap();
        let quantity = text.parse::<ScuQuantity>().unwrap();
        assert_eq!(price.to_string(), expected);
        assert_eq!(quantity.to_string(), expected);
    }
    assert!("0".parse::<PricePerScu>().unwrap().is_zero());
    assert!("0.0".parse::<ScuQuantity>().unwrap().is_zero());
    assert!(!"0.1".parse::<PricePerScu>().unwrap().is_zero());
    assert!(!"1".parse::<ScuQuantity>().unwrap().is_zero());
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_api_5__negative_nonfinite_overprecision_and_rounding_inputs_are_rejected() {
    for text in [
        "-1",
        "-0.00001",
        "NaN",
        "inf",
        "Infinity",
        "1_000",
        "1.00000000000000000000000000001e0",
        "1.00000000000000000000000000001",
        "79228162514264337593543950336",
        "1e-29",
        "1e29",
        "0.0000000000000000000000000001e-1",
        " 1",
        "1 ",
        "1,0",
        "0x1",
        "e3",
        "1e2e3",
        "",
    ] {
        assert!(text.parse::<PricePerScu>().is_err(), "price {text}");
        assert!(text.parse::<ScuQuantity>().is_err(), "quantity {text}");
    }
    let error = "NaN".parse::<PricePerScu>().unwrap_err();
    assert_eq!(
        error.to_string(),
        "value is not a supported non-negative exact decimal"
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_2_q_manual_r_ui_11_q_manual__zero_numeric_reference_has_no_percentage_and_precision_errors_are_explicit()
 {
    use tallyline_shared_kernel::api::model::{Percent, PrecisionUnavailable};
    let zero = "0".parse::<PricePerScu>().unwrap();
    for value in ["0", "5"] {
        let difference = value
            .parse::<PricePerScu>()
            .unwrap()
            .difference(zero)
            .unwrap();
        assert_eq!(difference.percentage_text(), Ok(None));
    }
    assert_eq!(
        Percent::whole(u32::MAX).doubled(),
        Err(PrecisionUnavailable)
    );
    let difference = "1000000000"
        .parse::<PricePerScu>()
        .unwrap()
        .difference("1e-28".parse().unwrap())
        .unwrap();
    assert_eq!(difference.percentage_text(), Err(PrecisionUnavailable));
    assert_eq!(
        PrecisionUnavailable.to_string(),
        "difference cannot be compared without losing precision"
    );
}
