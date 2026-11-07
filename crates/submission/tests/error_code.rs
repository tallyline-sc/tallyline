// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Opaque technical identifiers cannot grant domain meaning by spelling.

use tallyline_submission::api::model::{RowFailureKind, UexErrorCode};

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9_r_sub_11__opaque_identifiers_require_explicit_adapter_classification() {
    for identifier in ["database_error", "duplicated_report", "Future_Code"] {
        let code = UexErrorCode::new(identifier.to_owned()).unwrap();
        assert_eq!(code.as_str(), identifier);
        assert_eq!(
            code.row_failure(),
            RowFailureKind::Unrecognised,
            "opaque spelling must not grant a semantic row cause"
        );
    }
}
