// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Public installation-token value contract.

use tallyline_submission::api::model::{CredentialFailure, ReleaseToken};

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_4__installation_tokens_are_bounded_ascii_values_for_validation_only() {
    let token = ReleaseToken::new("dummy-installation-token".to_owned()).unwrap();
    assert_eq!(token.for_validation(), "dummy-installation-token");
    assert!(ReleaseToken::new("x".repeat(1024)).is_ok());
    for value in [
        String::new(),
        "dummy token".to_owned(),
        "dummy\ninvalid".to_owned(),
        "\u{2003}".to_owned(),
        "x".repeat(1025),
    ] {
        assert!(matches!(
            ReleaseToken::new(value),
            Err(CredentialFailure::InvalidInput)
        ));
    }
}
