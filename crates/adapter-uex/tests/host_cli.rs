// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Expert-host disclosure at the public technical CLI boundary.

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_api_3__custom_credential_commands_require_confirmation_before_reading_credentials() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_tallyline-uex-spike"))
        .args(["--host", "space", "account"])
        .env_remove("TALLYLINE_UEX_SECRET_KEY")
        .env_remove("TALLYLINE_UEX_APP_TOKEN")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(error.contains("requires --confirm-expert-host"), "{error}");
    assert!(!error.contains("TALLYLINE_UEX_SECRET_KEY is not set"));
    let confirmed = std::process::Command::new(env!("CARGO_BIN_EXE_tallyline-uex-spike"))
        .args(["--confirm-expert-host", "--host", "space", "account"])
        .env_remove("TALLYLINE_UEX_SECRET_KEY")
        .env_remove("TALLYLINE_UEX_APP_TOKEN")
        .output()
        .unwrap();
    assert!(!confirmed.status.success());
    assert!(
        String::from_utf8(confirmed.stdout)
            .unwrap()
            .contains("Warning: this expert host receives your UEX secret key and app token.")
    );
    assert!(
        String::from_utf8(confirmed.stderr)
            .unwrap()
            .contains("TALLYLINE_UEX_SECRET_KEY is not set")
    );
}
