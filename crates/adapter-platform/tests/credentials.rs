// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Platform credential contracts exercised with dummy temporary files.

use tallyline_adapter_platform::api::ReleaseTokenFile;
use tallyline_submission::api::model::ReleaseTokenFailure;

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_4__packaged_token_is_read_from_the_fixed_installation_filename() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("tallyline-app-token"),
        "dummy-packaged-token\n",
    )
    .unwrap();
    let reader = ReleaseTokenFile::new(directory.path().to_path_buf());
    let token = reader.read().unwrap().unwrap();
    assert_eq!(token.for_validation(), "dummy-packaged-token");
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_4__missing_or_empty_packaged_data_requests_an_own_token() {
    let directory = tempfile::tempdir().unwrap();
    let reader = ReleaseTokenFile::new(directory.path().to_path_buf());
    assert!(reader.read().unwrap().is_none());
    for text in ["", " \r\n\t"] {
        std::fs::write(directory.path().join("tallyline-app-token"), text).unwrap();
        assert!(reader.read().unwrap().is_none());
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_4__malformed_or_excessive_installation_data_has_a_safe_actionable_failure() {
    let directory = tempfile::tempdir().unwrap();
    let reader = ReleaseTokenFile::new(directory.path().to_path_buf());
    for bytes in [
        vec![b' '; 4097],
        vec![b'x'; 1025],
        vec![0xff],
        b"dummy\ninvalid".to_vec(),
        "\u{2003}".as_bytes().to_vec(),
    ] {
        std::fs::write(directory.path().join("tallyline-app-token"), bytes).unwrap();
        assert!(matches!(reader.read(), Err(ReleaseTokenFailure::Malformed)));
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_4__unreadable_installation_data_is_not_misclassified_as_a_missing_token() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir(directory.path().join("tallyline-app-token")).unwrap();
    let reader = ReleaseTokenFile::new(directory.path().to_path_buf());
    assert!(matches!(
        reader.read(),
        Err(ReleaseTokenFailure::Unreadable)
    ));
}
