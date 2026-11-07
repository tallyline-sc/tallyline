// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Typed permanent-refusal copying without receipt or retry authority.

use tallyline_shared_kernel::api::model::{ReportId, SubmissionMode, Timestamp};
use tallyline_submission::api::model::{
    JobId, PayloadHash, ReceiptDecision, SubmissionAccountCheck, SubmissionCorrection,
    SubmissionJob, SubmissionPauseReason, SubmissionRejection, SubmissionRequest,
    SubmissionResolutionAction, SubmissionResult, UexErrorCode, VerifiedUexReport,
};

fn rejected(result: SubmissionResult) -> Result<SubmissionJob, String> {
    let mut header = super::header();
    if result != SubmissionResult::DryRunSucceeded {
        header.mode = SubmissionMode::Production;
    }
    let sending = SubmissionJob::queued(
        JobId(81),
        SubmissionRequest::new(header, vec![super::row().ok_or("invalid fixture row")?])
            .map_err(fixture_error)?,
    )
    .map_err(fixture_error)?
    .begin_prepared(
        Timestamp::from_seconds(110),
        PayloadHash::new([8; 32]),
        Timestamp::from_seconds(100),
    )
    .map_err(fixture_error)?;
    if let SubmissionResult::Rechecked { code, outcome } = result {
        sending
            .complete(
                SubmissionResult::NeedsOutcomeCheck {
                    code,
                    http_status: 400,
                },
                Timestamp::from_seconds(120),
            )
            .map_err(fixture_error)?
            .resolve_outcome_check(*outcome)
            .map_err(fixture_error)
    } else {
        sending
            .complete(result, Timestamp::from_seconds(120))
            .map_err(fixture_error)
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_11__only_typed_client_review_refusal_can_copy_without_other_resolution_authority() {
    for code in ["invalid_game_version", "future_refusal"] {
        let origin = rejected(SubmissionResult::Rejected(SubmissionRejection::new(
            UexErrorCode::new(code.to_owned()).unwrap(),
            SubmissionCorrection::NewCapture,
        )))
        .unwrap();
        assert!(!origin.can_duplicate_rejected());
        assert!(
            origin
                .resolve_delivery(SubmissionResolutionAction::Duplicated {
                    report: ReportId(88),
                    at: Timestamp::from_seconds(130)
                })
                .is_err()
        );
    }
    let refusal = SubmissionResult::Rejected(SubmissionRejection::new(
        UexErrorCode::new("opaque_review_code".to_owned()).unwrap(),
        SubmissionCorrection::ReviewClient,
    ));
    for result in [
        SubmissionResult::AccountRefusal {
            code: UexErrorCode::new("opaque_review_code".to_owned()),
            http_status: 400,
            reason: SubmissionPauseReason::KeyCheckUnavailable,
            check: SubmissionAccountCheck::Pending,
            outcome: Box::new(refusal.clone()),
        },
        refusal.clone(),
        SubmissionResult::Rechecked {
            code: UexErrorCode::new("original".to_owned()).unwrap(),
            outcome: Box::new(refusal),
        },
    ] {
        let origin = rejected(result).unwrap();
        assert!(origin.can_duplicate_rejected());
        assert!(!origin.can_decide_unknown());
        assert!(!origin.can_check_receipts());
        assert!(origin.retry().is_err());
        assert!(
            origin
                .resolve_delivery(SubmissionResolutionAction::Decision {
                    decision: ReceiptDecision::NotReceived,
                    at: Timestamp::from_seconds(130)
                })
                .is_err()
        );
        let proof = VerifiedUexReport::from_lookup_at(
            718,
            origin.request().header().terminal,
            origin.request().rows()[0].commodity(),
            Timestamp::from_seconds(100),
            origin.request(),
            Timestamp::from_seconds(100),
        )
        .unwrap();
        assert!(
            origin
                .resolve_delivery(SubmissionResolutionAction::Verified {
                    receipts: vec![proof],
                    at: Timestamp::from_seconds(130)
                })
                .is_err(),
            "permanent rejection cannot acquire receipt resolution authority"
        );
        assert!(
            origin
                .resolve_delivery(SubmissionResolutionAction::Duplicated {
                    report: ReportId(88),
                    at: Timestamp::from_seconds(119)
                })
                .is_err()
        );
        let copy = origin
            .resolve_delivery(SubmissionResolutionAction::Duplicated {
                report: ReportId(88),
                at: Timestamp::from_seconds(130),
            })
            .unwrap();
        assert_eq!(copy.request(), origin.request());
        assert_eq!(copy.result(), origin.result());
        assert_eq!(copy.attempts(), origin.attempts());
        assert!(!copy.can_duplicate_rejected());
        assert!(
            copy.resolve_delivery(SubmissionResolutionAction::Duplicated {
                report: ReportId(89),
                at: Timestamp::from_seconds(140)
            })
            .is_err()
        );
    }
    assert!(
        !rejected(SubmissionResult::DryRunSucceeded)
            .unwrap()
            .can_duplicate_rejected()
    );
}

fn fixture_error(error: impl std::fmt::Debug) -> String {
    format!("{error:?}")
}
