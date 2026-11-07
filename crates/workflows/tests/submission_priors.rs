// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Only accepted production rows may update the local prior record.

use tallyline_shared_kernel::api::model::{
    CommodityId, ReportId, RowId, SubmissionMode, TerminalId, Timestamp, TradeSide, UexEnvironment,
};
use tallyline_submission::api::model::{
    JobId, PayloadHash, SubmissionHeader, SubmissionJob, SubmissionRequest, SubmissionResult,
    SubmissionRow,
};
use tallyline_workflows::api::accepted_submission_priors;
fn sending(mode: SubmissionMode, environment: UexEnvironment) -> Option<SubmissionJob> {
    let request = SubmissionRequest::new(
        SubmissionHeader {
            report: ReportId(9),
            report_version: 3,
            account: "test-account".to_owned(),
            mode,
            terminal: TerminalId(89),
            side: TradeSide::Sell,
            environment,
            game_version: "4.10.2".to_owned(),
            faction_affinity: None,
            observed_at: Timestamp::from_seconds(100),
        },
        vec![
            SubmissionRow::new(
                RowId(1),
                CommodityId(7),
                "120.125".parse().ok()?,
                "0".parse().ok()?,
                1,
            )
            .ok()?,
        ],
    )
    .ok()?;
    SubmissionJob::queued(JobId(7), request)
        .ok()?
        .begin(Timestamp::from_seconds(110), PayloadHash::new([4; 32]))
        .ok()
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_7_r_sub_8__dry_run_unknown_and_unmapped_partial_outcomes_leave_local_priors_unchanged() {
    use tallyline_shared_kernel::api::model::UexReportId;
    let dry = sending(SubmissionMode::DryRun, UexEnvironment::Live)
        .unwrap()
        .complete(
            SubmissionResult::DryRunSucceeded,
            Timestamp::from_seconds(120),
        )
        .unwrap();
    assert!(accepted_submission_priors(&dry).unwrap().is_empty());
    let production = sending(SubmissionMode::Production, UexEnvironment::Live).unwrap();
    for result in [
        SubmissionResult::Unknown,
        SubmissionResult::Partial(vec![
            UexReportId::new(42, Timestamp::from_seconds(100)).unwrap(),
        ]),
    ] {
        let finished = production
            .complete(result, Timestamp::from_seconds(120))
            .unwrap();
        assert!(accepted_submission_priors(&finished).unwrap().is_empty());
    }
    assert!(accepted_submission_priors(&production).unwrap().is_empty());
    for environment in [UexEnvironment::Live, UexEnvironment::Ptu] {
        let accepted = sending(SubmissionMode::Production, environment)
            .unwrap()
            .complete(
                SubmissionResult::ProductionAccepted(vec![
                    UexReportId::new(42, Timestamp::from_seconds(100)).unwrap(),
                ]),
                Timestamp::from_seconds(120),
            )
            .unwrap();
        let records = accepted_submission_priors(&accepted).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].context().submission, JobId(7));
        assert_eq!(records[0].context().environment, environment);
        assert_eq!(
            records[0].context().observed_at,
            Timestamp::from_seconds(100)
        );
        assert_eq!(
            records[0].context().accepted_at,
            Timestamp::from_seconds(120)
        );
        assert_eq!(records[0].values().price.to_string(), "120.125");
        assert_eq!(records[0].values().stock.to_string(), "0");
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_4_r_val_7_q_manual__decline_and_expiry_revoke_only_the_matching_submission_prior() {
    use tallyline_reference_data::api::model::{
        AcceptedPriorHistory, LocalPriorProjection, LocalPriorRevocation,
    };
    use tallyline_shared_kernel::api::model::UexReportId;
    use tallyline_submission::api::model::{
        LifecycleRefresh, RowEditability, SubmissionLifecycle, UexLifecycleObservation,
        UexReportStatus, VerifiedReceiptMapping, VerifiedUexReport,
    };
    use tallyline_workflows::api::lifecycle_prior_history;
    let accepted = sending(SubmissionMode::Production, UexEnvironment::Live)
        .unwrap()
        .complete(
            SubmissionResult::ProductionAccepted(vec![
                UexReportId::new(42, Timestamp::from_seconds(100)).unwrap(),
            ]),
            Timestamp::from_seconds(120),
        )
        .unwrap();
    let other = SubmissionJob::queued(JobId(8), accepted.request().clone())
        .unwrap()
        .begin(Timestamp::from_seconds(111), PayloadHash::new([5; 32]))
        .unwrap()
        .complete(
            SubmissionResult::ProductionAccepted(vec![
                UexReportId::new(43, Timestamp::from_seconds(100)).unwrap(),
            ]),
            Timestamp::from_seconds(121),
        )
        .unwrap();
    let receipt = VerifiedUexReport::from_lookup(
        42,
        TerminalId(89),
        CommodityId(7),
        Timestamp::from_seconds(100),
        accepted.request(),
    )
    .unwrap();
    let mapping = VerifiedReceiptMapping::new(&accepted, vec![receipt.clone()]).unwrap();
    let lifecycle = SubmissionLifecycle::try_new(&accepted, &mapping).unwrap();
    let original = AcceptedPriorHistory::default()
        .accepted(&accepted_submission_priors(&accepted).unwrap())
        .unwrap()
        .accepted(&accepted_submission_priors(&other).unwrap())
        .unwrap();
    for (status, reason) in [
        (UexReportStatus::Declined, LocalPriorRevocation::Declined),
        (UexReportStatus::Expired, LocalPriorRevocation::Expired),
    ] {
        let claim = lifecycle
            .claim(LifecycleRefresh::Manual, Timestamp::from_seconds(120))
            .unwrap()
            .unwrap();
        let observed = claim
            .observed(&[UexLifecycleObservation::new(
                receipt.clone(),
                status,
                RowEditability::NotEditable,
                Timestamp::from_seconds(121),
            )])
            .unwrap();
        let next = lifecycle_prior_history(&observed, &original).unwrap();
        assert_eq!(
            next.records()[0].projection(),
            LocalPriorProjection::Revoked(reason)
        );
        assert_eq!(next.records()[0].prior(), original.records()[0].prior());
        assert_eq!(next.records()[1], original.records()[1]);
        assert_eq!(
            next.active(UexEnvironment::Live)[0].context().submission,
            JobId(8)
        );
        let refreshed = original.refreshed(UexEnvironment::Live);
        assert!(
            lifecycle_prior_history(&observed, &refreshed)
                .unwrap()
                .active(UexEnvironment::Live)
                .is_empty()
        );
    }
}
