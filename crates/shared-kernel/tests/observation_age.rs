// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared value-bound observation-age decisions.

use std::time::Duration;
use tallyline_shared_kernel::api::model::{
    AgeConfirmation, AgeValidity, ObservationAgeSettings, Timestamp,
};

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_6__observation_age_is_tighten_only_and_confirmation_cannot_outlive_its_value() {
    let settings = ObservationAgeSettings::default();
    let observed = Timestamp::from_seconds(100);
    assert_eq!(
        settings.assess(observed, None, Timestamp::from_seconds(3700)),
        AgeValidity::Fresh
    );
    assert_eq!(
        settings.assess(observed, None, Timestamp::from_seconds(3701)),
        AgeValidity::Unconfirmed
    );
    let confirmation = AgeConfirmation::new(observed, Timestamp::from_seconds(3700)).unwrap();
    assert_eq!(
        settings.assess(observed, Some(confirmation), Timestamp::from_seconds(7300)),
        AgeValidity::Confirmed
    );
    assert_eq!(
        settings.assess(observed, Some(confirmation), Timestamp::from_seconds(7301)),
        AgeValidity::Unconfirmed
    );
    assert_eq!(
        settings.assess(
            Timestamp::from_seconds(99),
            Some(confirmation),
            Timestamp::from_seconds(4000)
        ),
        AgeValidity::Unconfirmed
    );
    assert_eq!(
        settings.assess(observed, Some(confirmation), Timestamp::from_seconds(86501)),
        AgeValidity::TooOld
    );
    assert_eq!(
        settings.assess(observed, None, Timestamp::from_seconds(99)),
        AgeValidity::Future
    );
    assert!(ObservationAgeSettings::new(Duration::from_secs(3601)).is_err());
    assert!(ObservationAgeSettings::new(Duration::ZERO).is_err());
    assert!(ObservationAgeSettings::new(Duration::from_secs(1)).is_ok());
}
