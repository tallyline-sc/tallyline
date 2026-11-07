// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Dispatch checks against current account and independently fresh reference facts.

use tallyline_shared_kernel::api::model::{
    CommodityId, ObservationAgeSettings, ReportId, RowId, SubmissionMode, TerminalId, Timestamp,
    TradeSide, UexEnvironment,
};
use tallyline_submission::api::SubmissionPreflight;
use tallyline_submission::api::model::{
    AccountState, Persistence, SendHoldReason, SendReadiness, SendReferenceState, SendReturnReason,
    SubmissionAcceptance, SubmissionHeader, SubmissionRequest, SubmissionRow,
};

fn request() -> Option<SubmissionRequest> {
    SubmissionRequest::new(
        SubmissionHeader {
            report: ReportId(1),
            report_version: 3,
            account: "dummy-a".to_owned(),
            mode: SubmissionMode::DryRun,
            terminal: TerminalId(89),
            side: TradeSide::Buy,
            environment: UexEnvironment::Live,
            game_version: "4.10.2".to_owned(),
            faction_affinity: None,
            observed_at: Timestamp::from_seconds(100),
        },
        vec![
            SubmissionRow::new(
                RowId(1),
                CommodityId(7),
                "2".parse().ok()?,
                "5".parse().ok()?,
                2,
            )
            .ok()?,
        ],
    )
    .ok()
}
fn account() -> AccountState {
    AccountState::Validated {
        username: "dummy-a".to_owned(),
        validated_at: Timestamp::from_seconds(100),
        persistence: Persistence::Session,
    }
}
fn references() -> SendReferenceState {
    SendReferenceState {
        environment: UexEnvironment::Live,
        version: Some("4.10.2".to_owned()),
        versions_fetched_at: Timestamp::from_seconds(100),
        acceptance: SubmissionAcceptance::Open,
        parameters_fetched_at: Timestamp::from_seconds(100),
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_5_r_sub_8__dispatch_requires_the_bound_account_and_independently_fresh_open_reference_state()
 {
    let request = request().unwrap();
    let account = account();
    let references = references();
    let settings = ObservationAgeSettings::default();
    assert_eq!(
        SubmissionPreflight::evaluate(
            &request,
            &account,
            Some(&references),
            None,
            settings,
            Timestamp::from_seconds(1000)
        ),
        SendReadiness::Ready
    );
    assert_eq!(
        SubmissionPreflight::evaluate(
            &request,
            &account,
            Some(&references),
            None,
            settings,
            Timestamp::from_millis(1000001)
        ),
        SendReadiness::Held(SendHoldReason::ReferenceUnavailable)
    );
    let mut closed = references.clone();
    closed.acceptance = SubmissionAcceptance::Closed;
    assert_eq!(
        SubmissionPreflight::evaluate(
            &request,
            &account,
            Some(&closed),
            None,
            settings,
            Timestamp::from_seconds(1000)
        ),
        SendReadiness::Held(SendHoldReason::AcceptanceClosed)
    );
    let mut patched = references.clone();
    patched.version = Some("4.10.3".to_owned());
    assert_eq!(
        SubmissionPreflight::evaluate(
            &request,
            &account,
            Some(&patched),
            None,
            settings,
            Timestamp::from_seconds(1000)
        ),
        SendReadiness::Returned(SendReturnReason::GameVersionChanged)
    );
    let other = AccountState::Validated {
        username: "dummy-b".to_owned(),
        validated_at: Timestamp::from_seconds(1000),
        persistence: Persistence::Session,
    };
    assert_eq!(
        SubmissionPreflight::evaluate(
            &request,
            &other,
            Some(&references),
            None,
            settings,
            Timestamp::from_seconds(1000)
        ),
        SendReadiness::Held(SendHoldReason::AccountChanged)
    );
    let mut stale = references.clone();
    stale.parameters_fetched_at = Timestamp::from_seconds(99);
    assert_eq!(
        SubmissionPreflight::evaluate(
            &request,
            &account,
            Some(&stale),
            None,
            settings,
            Timestamp::from_seconds(1000)
        ),
        SendReadiness::Held(SendHoldReason::ReferenceUnavailable)
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_6__dispatch_rechecks_age_confirmation_and_the_fixed_hard_limit() {
    use tallyline_shared_kernel::api::model::AgeConfirmation;
    let request = request().unwrap();
    let account = account();
    let mut references = references();
    let settings = ObservationAgeSettings::default();
    references.versions_fetched_at = Timestamp::from_seconds(3701);
    references.parameters_fetched_at = Timestamp::from_seconds(3701);
    assert_eq!(
        SubmissionPreflight::evaluate(
            &request,
            &account,
            Some(&references),
            None,
            settings,
            Timestamp::from_seconds(3701)
        ),
        SendReadiness::Returned(SendReturnReason::ObservationAgeUnconfirmed)
    );
    let decision =
        AgeConfirmation::new(Timestamp::from_seconds(100), Timestamp::from_seconds(3701)).unwrap();
    assert_eq!(
        SubmissionPreflight::evaluate(
            &request,
            &account,
            Some(&references),
            Some(decision),
            settings,
            Timestamp::from_seconds(3701)
        ),
        SendReadiness::Ready
    );
    references.versions_fetched_at = Timestamp::from_seconds(86501);
    references.parameters_fetched_at = Timestamp::from_seconds(86501);
    assert_eq!(
        SubmissionPreflight::evaluate(
            &request,
            &account,
            Some(&references),
            Some(decision),
            settings,
            Timestamp::from_seconds(86501)
        ),
        SendReadiness::Returned(SendReturnReason::ObservationTooOld)
    );
}
