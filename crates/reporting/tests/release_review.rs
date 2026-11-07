// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Fresh immutable review preparation for cross-context release transactions.

mod support;

use support::{Fixed, Repository, key, snapshot};
use tallyline_reference_data::api::model::UexEnvironment;
use tallyline_reporting::api::model::{NumericBlock, Report, ReportId, ReviewField};
use tallyline_reporting::api::{ReportingApplication, RepositoryError};
use tallyline_shared_kernel::api::model::{CommodityId, Timestamp};

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_7_q_manual_r_sub_5__fresh_report_review_prepares_real_numeric_decisions_without_saving() {
    let mut report = Report::manual(
        ReportId(1),
        key().unwrap(),
        Timestamp::from_seconds(900),
        Some("4.10.2"),
    )
    .unwrap();
    let row = report.add_commodity(CommodityId(42)).unwrap();
    report
        .set_price(row, Some("123.45".parse().unwrap()))
        .unwrap();
    report.set_stock(row, Some("17".parse().unwrap())).unwrap();
    report.set_status(row, std::num::NonZeroU8::new(3)).unwrap();
    let mut app = ReportingApplication::new(Repository(vec![report.clone()]), Fixed);
    let review = app
        .reassess_report(report.clone(), &snapshot(UexEnvironment::Live).unwrap())
        .unwrap();
    assert_eq!(
        review.numeric_readiness(),
        Err(vec![NumericBlock::NeedsConfirmation(ReviewField::Stock(
            row
        ))])
    );
    let independent = app
        .manual_reviewer()
        .reassess(
            report.clone(),
            &snapshot(UexEnvironment::Live).unwrap(),
            Timestamp::from_seconds(1000),
        )
        .unwrap();
    assert_eq!(independent, review);
    assert!(review.report().version() > report.version());
    assert_eq!(review.report().rows()[0].price(), report.rows()[0].price());
    assert_eq!(review.report().rows()[0].stock(), report.rows()[0].stock());
    assert_eq!(
        review.report().rows()[0].status(),
        report.rows()[0].status()
    );
    assert_eq!(review.report().observed_at(), Timestamp::from_seconds(900));
    assert_eq!(app.drafts().unwrap(), vec![report.clone()]);
    assert_eq!(
        app.reassess_report(report.clone(), &snapshot(UexEnvironment::Ptu).unwrap()),
        Err(RepositoryError::Invalid)
    );
    assert_eq!(app.drafts().unwrap(), vec![report]);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_2_r_sub_2_r_sub_3_r_sub_8__valid_manual_release_is_immutable_and_queued_values_cannot_be_edited()
 {
    use tallyline_reporting::api::model::{
        ReleaseAccountStanding, ReleaseContext, ReleaseScreenshotPolicy, ReleaseWorkingCopy,
        ReportState,
    };
    use tallyline_shared_kernel::api::model::{
        ObservationAgeSettings, SubmissionJobId, SubmissionMode,
    };
    let mut report = Report::manual(
        ReportId(1),
        key().unwrap(),
        Timestamp::from_seconds(900),
        Some("4.10.2"),
    )
    .unwrap();
    let row = report.add_commodity(CommodityId(42)).unwrap();
    report
        .set_price(row, Some("123.45".parse().unwrap()))
        .unwrap();
    report.set_stock(row, Some("0".parse().unwrap())).unwrap();
    report.set_status(row, std::num::NonZeroU8::new(3)).unwrap();
    let app = ReportingApplication::new(Repository(vec![report.clone()]), Fixed);
    let context = ReleaseContext::new(
        ReleaseAccountStanding::Accepted("dummy-a".to_owned()),
        SubmissionMode::DryRun,
        ReleaseScreenshotPolicy::Unknown,
        ReleaseWorkingCopy::Absent,
    );
    let gate = app.manual_reviewer().submission_gate(
        report.clone(),
        &snapshot(UexEnvironment::Live).unwrap(),
        context,
        ObservationAgeSettings::default(),
        Timestamp::from_seconds(1000),
    );
    let history = tallyline_reporting::api::model::ObservationHistoryAssessment::new(
        gate.report(),
        "dummy-a",
        SubmissionMode::DryRun,
        UexEnvironment::Live,
        Vec::new(),
    );
    let gate = gate.with_observation_history(Some(history));
    let released = gate.report().release(&gate).unwrap();
    assert_eq!(report.state(), ReportState::Draft);
    assert_eq!(released.state().state(), ReportState::Released);
    assert_eq!(
        released.state().release_binding().unwrap().account(),
        "dummy-a"
    );
    assert_eq!(
        released.state().release_binding().unwrap().mode(),
        SubmissionMode::DryRun
    );
    assert_eq!(
        released.state().release_binding().unwrap().environment(),
        UexEnvironment::Live
    );
    assert_eq!(released.events().len(), 1);
    let queued = released.state().enqueue(SubmissionJobId(7)).unwrap();
    assert_eq!(queued.state().state(), ReportState::Queued);
    assert_eq!(
        queued.state().release_binding().unwrap().job(),
        Some(SubmissionJobId(7))
    );
    assert_eq!(queued.state().observed_at(), Timestamp::from_seconds(900));
    assert_eq!(queued.state().rows()[0].stock(), report.rows()[0].stock());
    let mut immutable = queued.state().clone();
    assert!(
        immutable
            .set_price(row, Some("124".parse().unwrap()))
            .is_err()
    );
    assert_eq!(&immutable, queued.state());
    assert!(
        immutable
            .edit_attachment(tallyline_reporting::api::AttachmentEdit::Remove)
            .is_err()
    );
    assert_eq!(&immutable, queued.state());
    assert!(
        immutable
            .set_faction_affinity(Some("25".parse().unwrap()))
            .is_err()
    );
    assert!(immutable.change_key(report.key()).is_err());
    assert_eq!(&immutable, queued.state());
    assert!(queued.state().release(&gate).is_err());
    assert_eq!(
        queued.state().enqueue(SubmissionJobId(7)).unwrap().state(),
        queued.state()
    );
    assert!(queued.state().enqueue(SubmissionJobId(8)).is_err());
    use tallyline_reporting::api::model::{ReportDelivery, ReportFinding};
    let returned = queued
        .state()
        .apply_delivery(
            SubmissionJobId(7),
            ReportDelivery::Return(ReportFinding::AppRestart),
        )
        .unwrap();
    assert_eq!(returned.state().state(), ReportState::Draft);
    assert_eq!(returned.state().finding(), Some(ReportFinding::AppRestart));
    assert_eq!(returned.state().rows(), queued.state().rows());
    assert_eq!(
        returned.state().release_binding().unwrap().job(),
        Some(SubmissionJobId(7))
    );
    let accepted = queued
        .state()
        .apply_delivery(SubmissionJobId(7), ReportDelivery::FullAcceptance)
        .unwrap();
    assert_eq!(accepted.state().state(), ReportState::Submitted);
    assert_eq!(accepted.events().len(), 1);
    assert_eq!(
        accepted
            .state()
            .apply_delivery(SubmissionJobId(7), ReportDelivery::FullAcceptance)
            .unwrap()
            .state(),
        accepted.state()
    );
    assert!(
        accepted
            .state()
            .apply_delivery(SubmissionJobId(7), ReportDelivery::Unknown)
            .is_err()
    );
    assert!(
        accepted
            .state()
            .apply_delivery(SubmissionJobId(8), ReportDelivery::FullAcceptance)
            .is_err()
    );
    let unknown = queued
        .state()
        .apply_delivery(SubmissionJobId(7), ReportDelivery::Unknown)
        .unwrap();
    assert_eq!(unknown.state().state(), ReportState::OutcomeUnknown);
    assert!(
        unknown
            .state()
            .apply_delivery(
                SubmissionJobId(7),
                ReportDelivery::Return(ReportFinding::AppRestart)
            )
            .is_err()
    );
    let partial = queued
        .state()
        .apply_delivery(SubmissionJobId(7), ReportDelivery::PartialAcceptance)
        .unwrap();
    assert_eq!(partial.state().state(), ReportState::PartiallyAccepted);
    assert!(
        partial
            .state()
            .apply_delivery(
                SubmissionJobId(7),
                ReportDelivery::Return(ReportFinding::AppRestart)
            )
            .is_err()
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_6_r_sub_2__age_confirmation_is_explicit_value_bound_and_cannot_relax_the_hard_limit() {
    use tallyline_reporting::api::AttachmentEdit;
    use tallyline_reporting::api::model::{
        AttachmentBinding, AttachmentRegion, AttachmentTime, ImageHash,
    };
    use tallyline_shared_kernel::api::model::{AgeValidity, ObservationAgeSettings};
    let report = Report::manual(
        ReportId(1),
        key().unwrap(),
        Timestamp::from_seconds(1000),
        Some("4.10.2"),
    )
    .unwrap();
    let settings = ObservationAgeSettings::default();
    assert_eq!(
        settings.assess(
            report.observed_at(),
            report.age_confirmation(),
            Timestamp::from_seconds(5000)
        ),
        AgeValidity::Unconfirmed
    );
    let confirmation = report
        .confirm_observation_age(Timestamp::from_seconds(5000))
        .unwrap();
    assert_eq!(report.age_confirmation(), None);
    assert_eq!(
        settings.assess(
            confirmation.state().observed_at(),
            confirmation.state().age_confirmation(),
            Timestamp::from_seconds(5000)
        ),
        AgeValidity::Confirmed
    );
    assert!(
        report
            .confirm_observation_age(Timestamp::from_seconds(999))
            .is_err()
    );
    assert!(
        report
            .confirm_observation_age(Timestamp::from_seconds(87401))
            .is_err()
    );
    let mut changed = confirmation.into_state();
    let region = AttachmentRegion::new(1, 1, 10, 10).unwrap();
    let binding = AttachmentBinding::new(ImageHash::new([1; 32]), region);
    changed
        .edit_attachment(AttachmentEdit::Select(
            binding,
            AttachmentTime::ImportedFile(Timestamp::from_seconds(999)),
        ))
        .unwrap();
    assert_eq!(changed.age_confirmation(), None);
    assert_eq!(changed.observed_at(), Timestamp::from_seconds(999));
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_7_r_api_6__live_accepted_local_values_keep_distinct_provenance_and_never_repair_ptu() {
    use tallyline_game::api::model::{EnvironmentMapping, GameEnvironment};
    use tallyline_reference_data::api::model::{
        AcceptedLocalPrior, AcceptedPriorContext, PriorValues,
    };
    use tallyline_reporting::api::ManualReviewer;
    use tallyline_reporting::api::model::{Deviation, StockSettings};
    use tallyline_shared_kernel::api::model::SubmissionJobId;
    let mut report = Report::manual(
        ReportId(1),
        key().unwrap(),
        Timestamp::from_seconds(900),
        Some("4.10.2"),
    )
    .unwrap();
    let row = report.add_commodity(CommodityId(42)).unwrap();
    report.set_price(row, Some("200".parse().unwrap())).unwrap();
    report.set_stock(row, Some("20".parse().unwrap())).unwrap();
    report.set_status(row, std::num::NonZeroU8::new(5)).unwrap();
    let local = AcceptedLocalPrior::new(
        std::sync::Arc::new(AcceptedPriorContext {
            report: ReportId(9),
            report_version: 10,
            submission: SubmissionJobId(3),
            environment: UexEnvironment::Live,
            terminal: report.key().terminal(),
            side: report.key().side(),
            game_version: "4.10.2".to_owned(),
            observed_at: Timestamp::from_seconds(950),
            accepted_at: Timestamp::from_seconds(975),
        }),
        CommodityId(42),
        PriorValues {
            price: "200".parse().unwrap(),
            stock: "20".parse().unwrap(),
            status: std::num::NonZeroU8::new(5),
            container_sizes: vec![],
        },
    )
    .unwrap();
    let reviewer = ManualReviewer::new(
        EnvironmentMapping::default(),
        StockSettings::default(),
        None,
    );
    let live = snapshot(UexEnvironment::Live)
        .unwrap()
        .with_local_priors(std::slice::from_ref(&local))
        .unwrap();
    let review = reviewer
        .reassess(report.clone(), &live, Timestamp::from_seconds(1000))
        .unwrap();
    assert_eq!(review.prices()[0].deviation(), Some(Deviation::Equal));
    assert_eq!(review.stocks()[0].deviation(), Some(Deviation::Equal));
    assert_eq!(review.statuses()[0].deviation(), Deviation::Equal);
    assert_eq!(review.accepted_reference(row), Some(&local));
    assert_eq!(review.prices()[0].reference_age_millis(), Some(50_000));
    assert_eq!(review.stocks()[0].reference_age_millis(), Some(50_000));
    assert_eq!(review.report().observed_at(), report.observed_at());
    report
        .change_key(
            tallyline_reporting::api::model::ReportKey::new(
                report.key().terminal(),
                report.key().side(),
                GameEnvironment::Ptu,
            )
            .unwrap(),
        )
        .unwrap();
    let ptu = snapshot(UexEnvironment::Ptu)
        .unwrap()
        .with_local_priors(&[local])
        .unwrap();
    let review = reviewer
        .reassess(report, &ptu, Timestamp::from_seconds(1000))
        .unwrap();
    assert_eq!(review.prices()[0].deviation(), Some(Deviation::NoReference));
    assert_eq!(review.stocks()[0].deviation(), Some(Deviation::NoReference));
    assert_eq!(review.statuses()[0].deviation(), Deviation::NoReference);
    assert_eq!(review.accepted_reference(row), None);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_8_r_sub_2__newer_observation_confirmation_is_bound_to_exact_row_values_and_context() {
    use tallyline_reporting::api::model::NewerObservationEvidence;
    use tallyline_shared_kernel::api::model::SubmissionMode;
    let mut report = Report::manual(
        ReportId(1),
        key().unwrap(),
        Timestamp::from_seconds(900),
        Some("4.10.2"),
    )
    .unwrap();
    let row = report.add_commodity(CommodityId(42)).unwrap();
    report
        .set_price(row, Some("123.45".parse().unwrap()))
        .unwrap();
    report.set_stock(row, Some("0".parse().unwrap())).unwrap();
    report.set_status(row, std::num::NonZeroU8::new(3)).unwrap();
    let approved = report
        .confirm_newer_observation(
            row,
            NewerObservationEvidence::new(
                "dummy-a",
                SubmissionMode::Production,
                UexEnvironment::Live,
                Timestamp::from_seconds(1100),
            )
            .unwrap(),
            Timestamp::from_seconds(1200),
        )
        .unwrap();
    assert!(report.newer_observation_confirmations().is_empty());
    assert_eq!(approved.state().newer_observation_confirmations().len(), 1);
    assert_eq!(
        approved.state().newer_observation_confirmations()[0].row(),
        row
    );
    assert_eq!(
        approved.state().newer_observation_confirmations()[0].observed_at(),
        Timestamp::from_seconds(1100)
    );
    assert!(
        report
            .confirm_newer_observation(
                row,
                NewerObservationEvidence::new(
                    "dummy-a",
                    SubmissionMode::Production,
                    UexEnvironment::Live,
                    report.observed_at()
                )
                .unwrap(),
                Timestamp::from_seconds(1200)
            )
            .is_err()
    );
    let other_target = approved
        .state()
        .confirm_newer_observation(
            row,
            NewerObservationEvidence::new(
                "dummy-a",
                SubmissionMode::Production,
                UexEnvironment::Ptu,
                Timestamp::from_seconds(1100),
            )
            .unwrap(),
            Timestamp::from_seconds(1200),
        )
        .unwrap();
    assert_eq!(
        other_target.state().newer_observation_confirmations().len(),
        2
    );
    let mut changed = approved.state().clone();
    changed
        .set_price(row, Some("123.45".parse().unwrap()))
        .unwrap();
    assert_eq!(changed.newer_observation_confirmations().len(), 1);
    changed.set_stock(row, Some("1".parse().unwrap())).unwrap();
    assert!(changed.newer_observation_confirmations().is_empty());
    let mut changed = approved.state().clone();
    changed
        .change_key(
            tallyline_reporting::api::model::ReportKey::new(
                tallyline_shared_kernel::api::model::TerminalId(12),
                changed.key().side(),
                changed.key().environment(),
            )
            .unwrap(),
        )
        .unwrap();
    assert!(changed.newer_observation_confirmations().is_empty());
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_val_8__unavailable_observation_history_cannot_authorise_manual_release() {
    use tallyline_game::api::model::EnvironmentMapping;
    use tallyline_reporting::api::ManualReviewer;
    use tallyline_reporting::api::model::{
        ReleaseAccountStanding, ReleaseContext, ReleaseScreenshotPolicy, ReleaseWorkingCopy,
        StockSettings,
    };
    use tallyline_shared_kernel::api::model::{ObservationAgeSettings, SubmissionMode};
    let mut report = Report::manual(
        ReportId(1),
        key().unwrap(),
        Timestamp::from_seconds(900),
        Some("4.10.2"),
    )
    .unwrap();
    let row = report.add_commodity(CommodityId(42)).unwrap();
    report
        .set_price(row, Some("123.45".parse().unwrap()))
        .unwrap();
    report.set_stock(row, Some("0".parse().unwrap())).unwrap();
    report.set_status(row, std::num::NonZeroU8::new(3)).unwrap();
    let reviewer = ManualReviewer::new(
        EnvironmentMapping::default(),
        StockSettings::default(),
        None,
    );
    let gate = reviewer.submission_gate(
        report,
        &snapshot(UexEnvironment::Live).unwrap(),
        ReleaseContext::new(
            ReleaseAccountStanding::Accepted("dummy-a".to_owned()),
            SubmissionMode::DryRun,
            ReleaseScreenshotPolicy::Unknown,
            ReleaseWorkingCopy::Absent,
        ),
        ObservationAgeSettings::default(),
        Timestamp::from_seconds(1000),
    );
    assert!(
        gate.report().release(&gate).is_err(),
        "A complete numeric review cannot supply absent committed observation history"
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_sub_5__fractional_inventory_cannot_be_released_even_after_numeric_review() {
    use tallyline_reporting::api::model::{
        ObservationHistoryAssessment, ReleaseAccountStanding, ReleaseContext,
        ReleaseScreenshotPolicy, ReleaseWorkingCopy,
    };
    use tallyline_shared_kernel::api::model::{ObservationAgeSettings, SubmissionMode};
    let mut report = Report::manual(
        ReportId(1),
        key().unwrap(),
        Timestamp::from_seconds(900),
        Some("4.10.2"),
    )
    .unwrap();
    let row = report.add_commodity(CommodityId(42)).unwrap();
    report
        .set_price(row, Some("123.45".parse().unwrap()))
        .unwrap();
    report.set_stock(row, Some("0.5".parse().unwrap())).unwrap();
    report.set_status(row, std::num::NonZeroU8::new(3)).unwrap();
    let app = ReportingApplication::new(Repository(vec![]), Fixed);
    let gate = app.manual_reviewer().submission_gate(
        report,
        &snapshot(UexEnvironment::Live).unwrap(),
        ReleaseContext::new(
            ReleaseAccountStanding::Accepted("dummy-a".to_owned()),
            SubmissionMode::DryRun,
            ReleaseScreenshotPolicy::Unknown,
            ReleaseWorkingCopy::Absent,
        ),
        ObservationAgeSettings::default(),
        Timestamp::from_seconds(1000),
    );
    assert_eq!(gate.review().unwrap().numeric_readiness(), Ok(()));
    let history = ObservationHistoryAssessment::new(
        gate.report(),
        "dummy-a",
        SubmissionMode::DryRun,
        UexEnvironment::Live,
        Vec::new(),
    );
    let gate = gate.with_observation_history(Some(history));
    assert!(
        gate.report().release(&gate).is_err(),
        "A fractional quantity has no valid UEX integer payload and must not enter the queue"
    );
    assert_eq!(
        gate.report().rows()[0].stock(),
        Some("0.5".parse().unwrap())
    );
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_sub_5__release_preserves_five_hundred_rows_and_refuses_the_next_without_trimming() {
    use tallyline_reference_data::api::model::{Commodity, CommodityAverages, EndpointData};
    use tallyline_reporting::api::model::{
        ObservationHistoryAssessment, ReleaseAccountStanding, ReleaseContext,
        ReleaseScreenshotPolicy, ReleaseWorkingCopy,
    };
    use tallyline_shared_kernel::api::model::{ObservationAgeSettings, SubmissionMode};
    let base = snapshot(UexEnvironment::Live).unwrap();
    let references = support::cached_snapshot(
        UexEnvironment::Live,
        vec![
            EndpointData::Terminals(base.terminals("").into_iter().cloned().collect()),
            EndpointData::Commodities(
                (1..=501)
                    .map(|id| {
                        Commodity::new(
                            CommodityId(id),
                            "Recorded commodity",
                            "FIXTURE",
                            true,
                            CommodityAverages::new(None, None),
                        )
                    })
                    .collect(),
            ),
            EndpointData::GameVersions(base.game_versions().unwrap().clone()),
            EndpointData::StatusLevels(base.status_levels().unwrap().clone()),
        ],
    );
    let app = ReportingApplication::new(Repository(vec![]), Fixed);
    let mut report = Report::manual(
        ReportId(1),
        key().unwrap(),
        Timestamp::from_seconds(900),
        Some("4.10.2"),
    )
    .unwrap();
    for id in 1..=501 {
        let row = report.add_commodity(CommodityId(id)).unwrap();
        report.set_price(row, Some("1".parse().unwrap())).unwrap();
        report.set_stock(row, Some("0".parse().unwrap())).unwrap();
        report.set_status(row, std::num::NonZeroU8::new(3)).unwrap();
        if id >= 500 {
            let gate = app.manual_reviewer().submission_gate(
                report.clone(),
                &references,
                ReleaseContext::new(
                    ReleaseAccountStanding::Accepted("dummy-a".to_owned()),
                    SubmissionMode::DryRun,
                    ReleaseScreenshotPolicy::Unknown,
                    ReleaseWorkingCopy::Absent,
                ),
                ObservationAgeSettings::default(),
                Timestamp::from_seconds(1000),
            );
            assert_eq!(gate.review().unwrap().numeric_readiness(), Ok(()));
            let history = ObservationHistoryAssessment::new(
                gate.report(),
                "dummy-a",
                SubmissionMode::DryRun,
                UexEnvironment::Live,
                Vec::new(),
            );
            let gate = gate.with_observation_history(Some(history));
            if id == 500 {
                assert_eq!(
                    gate.report().release(&gate).unwrap().state().rows().len(),
                    500
                );
            } else {
                assert!(
                    gate.report().release(&gate).is_err(),
                    "The protocol limit must refuse a 501-row release before any transaction, without trimming it"
                );
                assert_eq!(gate.report().rows().len(), 501);
            }
        }
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_cap_3b_r_sub_2__current_version_string_cannot_certify_an_older_private_attachment() {
    use tallyline_reporting::api::AttachmentEdit;
    use tallyline_reporting::api::model::{
        AttachmentBinding, AttachmentRegion, AttachmentTime, ImageHash,
        ObservationHistoryAssessment, ReleaseAccountStanding, ReleaseContext,
        ReleaseScreenshotPolicy, ReleaseWorkingCopy,
    };
    use tallyline_shared_kernel::api::model::{ObservationAgeSettings, SubmissionMode};
    for (attachment_time, may_release) in [(900, false), (1000, true)] {
        let mut report = Report::manual(
            ReportId(1),
            key().unwrap(),
            Timestamp::from_seconds(1000),
            Some("4.10.2"),
        )
        .unwrap();
        let row = report.add_commodity(CommodityId(42)).unwrap();
        report
            .set_price(row, Some("123.45".parse().unwrap()))
            .unwrap();
        report.set_stock(row, Some("0".parse().unwrap())).unwrap();
        report.set_status(row, std::num::NonZeroU8::new(3)).unwrap();
        let binding = AttachmentBinding::new(
            ImageHash::new([1; 32]),
            AttachmentRegion::new(0, 0, 10, 10).unwrap(),
        );
        let digest = ImageHash::new([2; 32]);
        report
            .edit_attachment(AttachmentEdit::Select(
                binding,
                AttachmentTime::ImportedFile(Timestamp::from_seconds(attachment_time)),
            ))
            .unwrap();
        report
            .edit_attachment(AttachmentEdit::ConfirmPreview(binding))
            .unwrap();
        report
            .edit_attachment(AttachmentEdit::BindWorkingCopy(binding, digest))
            .unwrap();
        report
            .edit_attachment(AttachmentEdit::ConfirmTime(Timestamp::from_seconds(
                attachment_time,
            )))
            .unwrap();
        let app = ReportingApplication::new(Repository(vec![]), Fixed);
        let gate = app.manual_reviewer().submission_gate(
            report,
            &snapshot(UexEnvironment::Live).unwrap(),
            ReleaseContext::new(
                ReleaseAccountStanding::Accepted("dummy-a".to_owned()),
                SubmissionMode::DryRun,
                ReleaseScreenshotPolicy::Required,
                ReleaseWorkingCopy::Verified { binding, digest },
            ),
            ObservationAgeSettings::default(),
            Timestamp::from_seconds(1000),
        );
        assert_eq!(gate.review().unwrap().numeric_readiness(), Ok(()));
        assert_eq!(gate.report().attachment_gate(), Ok(()));
        let history = ObservationHistoryAssessment::new(
            gate.report(),
            "dummy-a",
            SubmissionMode::DryRun,
            UexEnvironment::Live,
            Vec::new(),
        );
        let gate = gate.with_observation_history(Some(history));
        assert_eq!(
            gate.report().release(&gate).is_ok(),
            may_release,
            "A current version string supplies no observed historical certainty for an earlier attachment time"
        );
        if !may_release {
            assert!(gate.blocks(gate.report()).contains(
                &tallyline_reporting::api::model::ReleaseBlockReason::VersionHistoryUnavailable
            ));
        }
    }
}
