// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Durable submission boundary contracts.

use tallyline_shared_kernel::api::model::{CommodityId, PricePerScu, RowId, ScuQuantity};
use tallyline_submission::api::model::{InvalidSubmission, SubmissionRow};

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_1__exact_manual_row_preserves_zero_stock_as_an_observation() {
    let row = SubmissionRow::new(
        RowId(1),
        CommodityId(7),
        "120.125".parse::<PricePerScu>().unwrap(),
        "0".parse::<ScuQuantity>().unwrap(),
        1,
    )
    .unwrap();
    assert_eq!(row.price().to_string(), "120.125");
    assert_eq!(row.stock().to_string(), "0");
    assert_eq!(row.commodity(), CommodityId(7));
    assert_eq!(
        SubmissionRow::new(RowId(0), CommodityId(7), row.price(), row.stock(), 1),
        Err(InvalidSubmission)
    );
    assert_eq!(
        SubmissionRow::new(RowId(1), CommodityId(7), row.price(), row.stock(), 0),
        Err(InvalidSubmission)
    );
}

use tallyline_shared_kernel::api::model::{
    ReportId, SubmissionMode, TerminalId, Timestamp, TradeSide, UexEnvironment,
};
use tallyline_submission::api::model::{SubmissionHeader, SubmissionRequest};

fn header() -> SubmissionHeader {
    SubmissionHeader {
        report: ReportId(9),
        report_version: 3,
        account: "dummy-a".to_owned(),
        mode: SubmissionMode::DryRun,
        terminal: TerminalId(89),
        side: TradeSide::Sell,
        environment: UexEnvironment::Live,
        game_version: "4.10.2".to_owned(),
        faction_affinity: None,
        observed_at: Timestamp::from_seconds(100),
    }
}
fn row() -> Option<SubmissionRow> {
    SubmissionRow::new(
        RowId(1),
        CommodityId(7),
        "120.125".parse().ok()?,
        "0".parse().ok()?,
        1,
    )
    .ok()
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_sub_8__release_snapshot_binds_account_mode_and_valid_unique_rows() {
    let request = SubmissionRequest::new(header(), vec![row().unwrap()]).unwrap();
    assert_eq!(request.header().account, "dummy-a");
    assert_eq!(request.header().mode, SubmissionMode::DryRun);
    assert_eq!(request.header().observed_at, Timestamp::from_seconds(100));
    assert_eq!(
        SubmissionRequest::new(header(), vec![]),
        Err(InvalidSubmission)
    );
    assert_eq!(
        SubmissionRequest::new(header(), vec![row().unwrap(), row().unwrap()]),
        Err(InvalidSubmission)
    );
    let mut missing_version = header();
    missing_version.report_version = 0;
    assert_eq!(
        SubmissionRequest::new(missing_version, vec![row().unwrap()]),
        Err(InvalidSubmission)
    );
    let rows = (1..=501)
        .map(|id| {
            SubmissionRow::new(
                RowId(u64::from(id)),
                CommodityId(id),
                row().unwrap().price(),
                row().unwrap().stock(),
                1,
            )
            .unwrap()
        })
        .collect();
    assert_eq!(
        SubmissionRequest::new(header(), rows),
        Err(InvalidSubmission)
    );
}

use tallyline_submission::api::model::{
    AttemptOutcome, JobId, JobPhase, PayloadHash, QueueRefusal, SubmissionJob,
};

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_sub_9__sending_cannot_cancel_and_interrupted_attempt_is_never_replayed() {
    let queued = SubmissionJob::queued(
        JobId(1),
        SubmissionRequest::new(header(), vec![row().unwrap()]).unwrap(),
    )
    .unwrap();
    let sending = queued
        .begin(Timestamp::from_seconds(110), PayloadHash::new([7; 32]))
        .unwrap();
    assert_eq!(queued.phase(), JobPhase::Queued);
    assert_eq!(sending.cancel(), Err(QueueRefusal::AlreadySending));
    assert_eq!(sending.attempts().len(), 1);
    let recovered = sending.recover(Timestamp::from_seconds(120)).unwrap();
    assert_eq!(recovered.phase(), JobPhase::OutcomeUnknown);
    assert_eq!(
        recovered.attempts()[0].outcome(),
        Some(AttemptOutcome::Unknown)
    );
    assert_eq!(
        recovered.begin(Timestamp::from_seconds(121), PayloadHash::new([7; 32])),
        Err(QueueRefusal::InvalidTransition)
    );
    assert_eq!(
        recovered.recover(Timestamp::from_seconds(130)).unwrap(),
        recovered
    );
    assert_eq!(
        queued
            .recover(Timestamp::from_seconds(120))
            .unwrap()
            .phase(),
        JobPhase::Returned
    );
}

use tallyline_submission::api::model::SubmissionResult;

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_8_r_sub_9__dry_run_success_has_no_uex_ids_and_partial_results_close_delivery() {
    let queued = SubmissionJob::queued(
        JobId(1),
        SubmissionRequest::new(header(), vec![row().unwrap()]).unwrap(),
    )
    .unwrap();
    let sending = queued
        .begin(Timestamp::from_seconds(110), PayloadHash::new([7; 32]))
        .unwrap();
    let finished = sending
        .complete(
            SubmissionResult::DryRunSucceeded,
            Timestamp::from_seconds(115),
        )
        .unwrap();
    assert_eq!(finished.phase(), JobPhase::Succeeded);
    assert_eq!(finished.result(), Some(&SubmissionResult::DryRunSucceeded));
    assert_eq!(
        finished.attempts()[0].outcome(),
        Some(AttemptOutcome::DryRunSucceeded)
    );
    assert_eq!(
        finished.complete(
            SubmissionResult::DryRunSucceeded,
            Timestamp::from_seconds(116)
        ),
        Err(QueueRefusal::InvalidTransition)
    );
    let partial = sending
        .complete(
            SubmissionResult::Partial(Vec::new()),
            Timestamp::from_seconds(115),
        )
        .unwrap();
    assert_eq!(partial.phase(), JobPhase::PartiallyAccepted);
    assert_eq!(
        partial.result(),
        Some(&SubmissionResult::Partial(Vec::new()))
    );
    assert_eq!(
        partial.begin(Timestamp::from_seconds(116), PayloadHash::new([7; 32])),
        Err(QueueRefusal::InvalidTransition)
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_1__fractional_or_unsupported_scu_cannot_be_released_for_integer_wire_fields() {
    let price = "1.25".parse().unwrap();
    assert_eq!(
        SubmissionRow::new(RowId(1), CommodityId(7), price, "1.25".parse().unwrap(), 1),
        Err(InvalidSubmission)
    );
    assert_eq!(
        SubmissionRow::new(
            RowId(1),
            CommodityId(7),
            price,
            "18446744073709551616".parse().unwrap(),
            1
        ),
        Err(InvalidSubmission)
    );
    assert!(
        SubmissionRow::new(
            RowId(1),
            CommodityId(7),
            price,
            "18446744073709551615".parse().unwrap(),
            1
        )
        .is_ok()
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_sub_9__known_unprocessed_attempt_is_held_and_restart_requires_new_release() {
    use tallyline_submission::api::model::NotProcessed;
    let queued = SubmissionJob::queued(
        JobId(1),
        SubmissionRequest::new(header(), vec![row().unwrap()]).unwrap(),
    )
    .unwrap();
    let sending = queued
        .begin(Timestamp::from_seconds(110), PayloadHash::new([7; 32]))
        .unwrap();
    let held = sending
        .complete(
            SubmissionResult::NotProcessed(NotProcessed::ConnectFailure),
            Timestamp::from_seconds(111),
        )
        .unwrap();
    assert_eq!(held.phase(), JobPhase::Held);
    assert_eq!(held.cancel().unwrap().phase(), JobPhase::Cancelled);
    assert_eq!(
        held.recover(Timestamp::from_seconds(120)).unwrap().phase(),
        JobPhase::Returned
    );
    assert_eq!(
        held.attempts()[0].outcome(),
        Some(AttemptOutcome::NotProcessed(NotProcessed::ConnectFailure))
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_8_r_sub_9__dry_run_cannot_acquire_ids_and_production_needs_exact_complete_receipts() {
    use tallyline_submission::api::model::VerifiedUexReport;
    let mut production_header = header();
    production_header.mode = SubmissionMode::Production;
    let production = SubmissionRequest::new(production_header, vec![row().unwrap()]).unwrap();
    let receipt = VerifiedUexReport::from_lookup(
        42,
        TerminalId(89),
        CommodityId(7),
        Timestamp::from_seconds(100),
        &production,
    )
    .unwrap();
    let dry_request = SubmissionRequest::new(header(), vec![row().unwrap()]).unwrap();
    assert!(
        VerifiedUexReport::from_lookup(
            42,
            TerminalId(89),
            CommodityId(7),
            Timestamp::from_seconds(100),
            &dry_request
        )
        .is_err()
    );
    let dry_run = SubmissionJob::queued(JobId(1), dry_request)
        .unwrap()
        .begin(Timestamp::from_seconds(110), PayloadHash::new([7; 32]))
        .unwrap();
    assert_eq!(
        dry_run.complete(
            SubmissionResult::ProductionSucceeded(vec![receipt.clone()]),
            Timestamp::from_seconds(115)
        ),
        Err(QueueRefusal::InvalidTransition)
    );
    assert_eq!(
        dry_run.complete(
            SubmissionResult::Partial(vec![
                tallyline_shared_kernel::api::model::UexReportId::new(
                    42,
                    Timestamp::from_seconds(100)
                )
                .unwrap()
            ]),
            Timestamp::from_seconds(115)
        ),
        Err(QueueRefusal::InvalidTransition)
    );
    let sending = SubmissionJob::queued(JobId(2), production)
        .unwrap()
        .begin(Timestamp::from_seconds(110), PayloadHash::new([7; 32]))
        .unwrap();
    assert_eq!(
        sending.complete(
            SubmissionResult::ProductionSucceeded(vec![]),
            Timestamp::from_seconds(115)
        ),
        Err(QueueRefusal::InvalidTransition)
    );
    assert_eq!(
        sending.complete(
            SubmissionResult::ProductionSucceeded(vec![receipt.clone(), receipt.clone()]),
            Timestamp::from_seconds(115)
        ),
        Err(QueueRefusal::InvalidTransition)
    );
    assert_eq!(
        sending
            .complete(
                SubmissionResult::ProductionSucceeded(vec![receipt]),
                Timestamp::from_seconds(115)
            )
            .unwrap()
            .phase(),
        JobPhase::Succeeded
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2__row_budget_counts_unknown_production_attempts_by_account_and_exact_sliding_window() {
    use tallyline_submission::api::SubmissionBudget;
    use tallyline_submission::api::model::{AccountAttempt, RowBudgetDecision};
    let records = vec![
        AccountAttempt::new(
            "dummy-a".to_owned(),
            SubmissionMode::Production,
            Timestamp::from_millis(100001),
            500,
            Some(AttemptOutcome::Unknown),
        )
        .unwrap(),
        AccountAttempt::new(
            "dummy-a".to_owned(),
            SubmissionMode::Production,
            Timestamp::from_millis(100001),
            500,
            None,
        )
        .unwrap(),
    ];
    let mut production_header = header();
    production_header.mode = SubmissionMode::Production;
    let request = SubmissionRequest::new(production_header, vec![row().unwrap()]).unwrap();
    assert_eq!(
        SubmissionBudget::assess(&request, &records, Timestamp::from_millis(1900000)),
        RowBudgetDecision::Held
    );
    assert_eq!(
        SubmissionBudget::assess(&request, &records, Timestamp::from_millis(1900001)),
        RowBudgetDecision::Ready
    );
    let dry_run = SubmissionRequest::new(header(), vec![row().unwrap()]).unwrap();
    assert_eq!(
        SubmissionBudget::assess(&dry_run, &records, Timestamp::from_millis(100010)),
        RowBudgetDecision::Ready
    );
    let excluded = vec![
        AccountAttempt::new(
            "dummy-other".to_owned(),
            SubmissionMode::Production,
            Timestamp::from_seconds(100),
            500,
            None,
        )
        .unwrap(),
        AccountAttempt::new(
            "dummy-a".to_owned(),
            SubmissionMode::DryRun,
            Timestamp::from_seconds(100),
            500,
            None,
        )
        .unwrap(),
        AccountAttempt::new(
            "dummy-a".to_owned(),
            SubmissionMode::Production,
            Timestamp::from_seconds(100),
            500,
            Some(AttemptOutcome::NotProcessed(
                tallyline_submission::api::model::NotProcessed::ConnectFailure,
            )),
        )
        .unwrap(),
    ];
    assert_eq!(
        SubmissionBudget::assess(&request, &excluded, Timestamp::from_seconds(110)),
        RowBudgetDecision::Ready
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_6_r_sub_9__attempt_records_the_exact_earlier_wire_time_and_verifies_receipts_against_it() {
    use tallyline_submission::api::model::VerifiedUexReport;
    let mut selected = header();
    selected.mode = SubmissionMode::Production;
    let request = SubmissionRequest::new(selected, vec![row().unwrap()]).unwrap();
    let queued = SubmissionJob::queued(JobId(1), request.clone()).unwrap();
    assert_eq!(
        queued.begin_prepared(
            Timestamp::from_seconds(110),
            PayloadHash::new([7; 32]),
            Timestamp::from_seconds(101)
        ),
        Err(QueueRefusal::InvalidPreparedPayload)
    );
    let sending = queued
        .begin_prepared(
            Timestamp::from_seconds(110),
            PayloadHash::new([7; 32]),
            Timestamp::from_seconds(95),
        )
        .unwrap();
    assert_eq!(
        sending.attempts()[0].date_added(),
        Timestamp::from_seconds(95)
    );
    assert_eq!(
        sending.request().header().observed_at,
        Timestamp::from_seconds(100)
    );
    let wrong = VerifiedUexReport::from_lookup_at(
        42,
        TerminalId(89),
        CommodityId(7),
        Timestamp::from_seconds(90),
        &request,
        Timestamp::from_seconds(90),
    )
    .unwrap();
    assert_eq!(
        sending.complete(
            SubmissionResult::ProductionSucceeded(vec![wrong]),
            Timestamp::from_seconds(115)
        ),
        Err(QueueRefusal::InvalidTransition)
    );
    let matched = VerifiedUexReport::from_lookup_at(
        42,
        TerminalId(89),
        CommodityId(7),
        Timestamp::from_seconds(95),
        &request,
        Timestamp::from_seconds(95),
    )
    .unwrap();
    assert_eq!(
        sending
            .complete(
                SubmissionResult::ProductionSucceeded(vec![matched]),
                Timestamp::from_seconds(115)
            )
            .unwrap()
            .phase(),
        JobPhase::Succeeded
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9__verified_receipt_cannot_cross_a_different_terminal_commodity_or_account_snapshot() {
    use tallyline_submission::api::model::VerifiedUexReport;
    let mut base = header();
    base.mode = SubmissionMode::Production;
    let original = SubmissionRequest::new(base.clone(), vec![row().unwrap()]).unwrap();
    let receipt = VerifiedUexReport::from_lookup(
        42,
        TerminalId(89),
        CommodityId(7),
        Timestamp::from_seconds(100),
        &original,
    )
    .unwrap();
    let other_row = SubmissionRow::new(
        RowId(1),
        CommodityId(8),
        row().unwrap().price(),
        row().unwrap().stock(),
        1,
    )
    .unwrap();
    let wrong_commodity = SubmissionRequest::new(base.clone(), vec![other_row]).unwrap();
    let mut terminal = base.clone();
    terminal.terminal = TerminalId(90);
    let wrong_terminal = SubmissionRequest::new(terminal, vec![row().unwrap()]).unwrap();
    let mut account = base;
    account.account = "dummy-b".to_owned();
    let wrong_account = SubmissionRequest::new(account, vec![row().unwrap()]).unwrap();
    for request in [wrong_commodity, wrong_terminal, wrong_account] {
        let sending = SubmissionJob::queued(JobId(2), request)
            .unwrap()
            .begin(Timestamp::from_seconds(110), PayloadHash::new([7; 32]))
            .unwrap();
        assert_eq!(
            sending.complete(
                SubmissionResult::ProductionSucceeded(vec![receipt.clone()]),
                Timestamp::from_seconds(115)
            ),
            Err(QueueRefusal::InvalidTransition)
        );
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9__recovery_advances_once_and_retains_the_attempt_identity() {
    let id = tallyline_shared_kernel::api::model::SubmissionJobId(44);
    let queued = SubmissionJob::queued(
        id,
        SubmissionRequest::new(header(), vec![row().unwrap()]).unwrap(),
    )
    .unwrap();
    assert_eq!(queued.revision(), 1);
    let sending = queued
        .begin(Timestamp::from_seconds(110), PayloadHash::new([9; 32]))
        .unwrap();
    assert_eq!(sending.revision(), 2);
    let unknown = sending.recover(Timestamp::from_seconds(120)).unwrap();
    assert_eq!(unknown.revision(), 3);
    assert_eq!(
        unknown.attempts()[0].payload_hash(),
        PayloadHash::new([9; 32])
    );
    let reopened = unknown.recover(Timestamp::from_seconds(130)).unwrap();
    assert_eq!(reopened, unknown);
    assert_eq!(queued.cancel().unwrap().revision(), 2);
    assert_eq!(
        queued
            .recover(Timestamp::from_seconds(120))
            .unwrap()
            .revision(),
        2
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9__outcome_events_bind_exact_job_revision_and_recovery_does_not_republish() {
    use tallyline_submission::api::model::{SubmissionCommand, SubmissionEventKind};
    let queued = SubmissionJob::queued(
        JobId(45),
        SubmissionRequest::new(header(), vec![row().unwrap()]).unwrap(),
    )
    .unwrap();
    let sending = queued
        .apply(SubmissionCommand::Begin {
            at: Timestamp::from_seconds(110),
            payload_hash: PayloadHash::new([4; 32]),
            date_added: Timestamp::from_seconds(100),
        })
        .unwrap();
    assert!(sending.event().is_none());
    let interrupted = sending
        .state()
        .apply(SubmissionCommand::Recover {
            at: Timestamp::from_seconds(120),
        })
        .unwrap();
    let event = interrupted.event().unwrap();
    assert_eq!(event.kind(), &SubmissionEventKind::OutcomeUnknown);
    assert_eq!(event.id().aggregate_id(), 45);
    assert_eq!(event.id().version(), interrupted.state().revision());
    assert_eq!(event.report(), ReportId(9));
    let reopened = interrupted
        .state()
        .apply(SubmissionCommand::Recover {
            at: Timestamp::from_seconds(130),
        })
        .unwrap();
    assert!(reopened.event().is_none());
    assert_eq!(reopened.state(), interrupted.state());
    let cancelled = queued.apply(SubmissionCommand::Cancel).unwrap();
    assert_eq!(
        cancelled.event().unwrap().kind(),
        &SubmissionEventKind::Cancelled(tallyline_submission::api::model::CancellationReason::User)
    );
    let event_id = cancelled.event().unwrap().id();
    assert_ne!(event_id, event.id());
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9__full_count_acceptance_is_final_while_remote_ids_remain_unmapped() {
    use tallyline_shared_kernel::api::model::UexReportId;
    use tallyline_submission::api::model::SubmissionResult;
    let mut production = header();
    production.mode = SubmissionMode::Production;
    let queued = SubmissionJob::queued(
        JobId(46),
        SubmissionRequest::new(production, vec![row().unwrap()]).unwrap(),
    )
    .unwrap();
    let sending = queued
        .begin(Timestamp::from_seconds(110), PayloadHash::new([5; 32]))
        .unwrap();
    let ids = vec![UexReportId::new(42, Timestamp::from_seconds(100)).unwrap()];
    let accepted = sending
        .complete(
            SubmissionResult::ProductionAccepted(ids),
            Timestamp::from_seconds(115),
        )
        .unwrap();
    assert_eq!(accepted.phase(), JobPhase::Succeeded);
    assert!(
        accepted
            .begin(Timestamp::from_seconds(120), PayloadHash::new([5; 32]))
            .is_err()
    );
    assert_eq!(
        accepted.recover(Timestamp::from_seconds(130)).unwrap(),
        accepted
    );
    for ids in [
        vec![],
        vec![UexReportId::new(42, Timestamp::from_seconds(101)).unwrap()],
    ] {
        assert_eq!(
            sending.complete(
                SubmissionResult::ProductionAccepted(ids),
                Timestamp::from_seconds(115)
            ),
            Err(QueueRefusal::InvalidTransition)
        );
    }
    let dry = SubmissionJob::queued(
        JobId(47),
        SubmissionRequest::new(header(), vec![row().unwrap()]).unwrap(),
    )
    .unwrap()
    .begin(Timestamp::from_seconds(110), PayloadHash::new([5; 32]))
    .unwrap();
    assert!(
        dry.complete(
            SubmissionResult::ProductionAccepted(vec![
                UexReportId::new(42, Timestamp::from_seconds(100)).unwrap()
            ]),
            Timestamp::from_seconds(115)
        )
        .is_err()
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_4_r_val_8__dispatch_waits_for_all_row_cooldowns_and_preserves_observation_order() {
    use tallyline_submission::api::SubmissionSchedule;
    use tallyline_submission::api::model::{ScheduleDecision, SubmissionResult};
    let candidate = SubmissionJob::queued(
        JobId(100),
        SubmissionRequest::new(header(), vec![row().unwrap()]).unwrap(),
    )
    .unwrap();
    let mut older = header();
    older.observed_at = Timestamp::from_seconds(90);
    let queued = SubmissionJob::queued(
        JobId(99),
        SubmissionRequest::new(older, vec![row().unwrap()]).unwrap(),
    )
    .unwrap();
    assert_eq!(
        SubmissionSchedule::assess(
            &candidate,
            std::slice::from_ref(&queued),
            Timestamp::from_seconds(200)
        ),
        ScheduleDecision::EarlierObservationPending
    );
    let sending = queued
        .begin(Timestamp::from_seconds(100), PayloadHash::new([1; 32]))
        .unwrap();
    assert_eq!(
        SubmissionSchedule::assess(
            &candidate,
            std::slice::from_ref(&sending),
            Timestamp::from_seconds(200)
        ),
        ScheduleDecision::InFlight
    );
    let success = sending
        .complete(
            SubmissionResult::DryRunSucceeded,
            Timestamp::from_seconds(110),
        )
        .unwrap();
    assert_eq!(
        SubmissionSchedule::assess(
            &candidate,
            std::slice::from_ref(&success),
            Timestamp::from_millis(409_999)
        ),
        ScheduleDecision::WaitingUntil(Timestamp::from_seconds(410))
    );
    assert_eq!(
        SubmissionSchedule::assess(
            &candidate,
            std::slice::from_ref(&success),
            Timestamp::from_seconds(410)
        ),
        ScheduleDecision::Ready
    );
    let mut opposite = header();
    opposite.side = TradeSide::Buy;
    opposite.environment = UexEnvironment::Ptu;
    let opposite = SubmissionJob::queued(
        JobId(101),
        SubmissionRequest::new(opposite, vec![row().unwrap()]).unwrap(),
    )
    .unwrap();
    assert_eq!(
        SubmissionSchedule::assess(
            &opposite,
            std::slice::from_ref(&success),
            Timestamp::from_seconds(200)
        ),
        ScheduleDecision::WaitingUntil(Timestamp::from_seconds(410))
    );
    let mut another = header();
    another.account = "dummy-b".to_owned();
    let another = SubmissionJob::queued(
        JobId(102),
        SubmissionRequest::new(another, vec![row().unwrap()]).unwrap(),
    )
    .unwrap();
    assert_eq!(
        SubmissionSchedule::assess(&another, &[success], Timestamp::from_seconds(200)),
        ScheduleDecision::Ready
    );
    let mut newer = header();
    newer.observed_at = Timestamp::from_seconds(200);
    let newer = SubmissionJob::queued(
        JobId(103),
        SubmissionRequest::new(newer, vec![row().unwrap()]).unwrap(),
    )
    .unwrap()
    .begin(Timestamp::from_seconds(210), PayloadHash::new([2; 32]))
    .unwrap();
    let unknown = newer
        .complete(SubmissionResult::Unknown, Timestamp::from_seconds(220))
        .unwrap();
    assert_eq!(
        SubmissionSchedule::assess(&candidate, &[unknown], Timestamp::from_seconds(1000)),
        ScheduleDecision::UnresolvedNewerObservation
    );
    let success = newer
        .complete(
            SubmissionResult::DryRunSucceeded,
            Timestamp::from_seconds(220),
        )
        .unwrap();
    assert_eq!(
        SubmissionSchedule::assess(&candidate, &[success], Timestamp::from_seconds(1000)),
        ScheduleDecision::NewerObservationSent
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9__verified_receipt_cannot_move_to_different_row_values_with_the_same_metadata() {
    use tallyline_submission::api::model::{SubmissionResult, VerifiedUexReport};
    let mut production = header();
    production.mode = SubmissionMode::Production;
    let original = SubmissionRequest::new(production.clone(), vec![row().unwrap()]).unwrap();
    let receipt = VerifiedUexReport::from_lookup(
        42,
        TerminalId(89),
        CommodityId(7),
        Timestamp::from_seconds(100),
        &original,
    )
    .unwrap();
    for changed in [
        SubmissionRow::new(
            RowId(1),
            CommodityId(7),
            "121".parse().unwrap(),
            "0".parse().unwrap(),
            1,
        )
        .unwrap(),
        SubmissionRow::new(
            RowId(1),
            CommodityId(7),
            row().unwrap().price(),
            "1".parse().unwrap(),
            1,
        )
        .unwrap(),
        SubmissionRow::new(
            RowId(1),
            CommodityId(7),
            row().unwrap().price(),
            "0".parse().unwrap(),
            2,
        )
        .unwrap(),
    ] {
        let changed = SubmissionRequest::new(production.clone(), vec![changed]).unwrap();
        let sending = SubmissionJob::queued(JobId(110), changed)
            .unwrap()
            .begin(Timestamp::from_seconds(110), PayloadHash::new([3; 32]))
            .unwrap();
        assert_eq!(
            sending.complete(
                SubmissionResult::ProductionSucceeded(vec![receipt.clone()]),
                Timestamp::from_seconds(120)
            ),
            Err(QueueRefusal::InvalidTransition)
        );
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_8__newer_observation_approval_is_bound_to_exact_released_values_and_latest_history() {
    use tallyline_submission::api::SubmissionSchedule;
    use tallyline_submission::api::model::{ScheduleDecision, SubmissionResult};
    let request = SubmissionRequest::new(header(), vec![row().unwrap()]).unwrap();
    let approved = request
        .with_confirmed_newer_observation(RowId(1), Timestamp::from_seconds(200))
        .unwrap();
    assert_eq!(request.newer_observation_confirmations().len(), 0);
    assert_eq!(
        approved.newer_observation_confirmations()[0].row(),
        RowId(1)
    );
    assert_eq!(
        approved.newer_observation_confirmations()[0].observed_at(),
        Timestamp::from_seconds(200)
    );
    assert!(
        request
            .with_confirmed_newer_observation(RowId(2), Timestamp::from_seconds(200))
            .is_err()
    );
    assert!(
        request
            .with_confirmed_newer_observation(RowId(1), Timestamp::from_seconds(100))
            .is_err()
    );
    let candidate = SubmissionJob::queued(JobId(111), approved).unwrap();
    for (observed_at, expected) in [
        (200, ScheduleDecision::Ready),
        (201, ScheduleDecision::NewerObservationSent),
    ] {
        let mut newer = header();
        newer.observed_at = Timestamp::from_seconds(observed_at);
        let success = SubmissionJob::queued(
            JobId(112),
            SubmissionRequest::new(newer, vec![row().unwrap()]).unwrap(),
        )
        .unwrap()
        .begin(Timestamp::from_seconds(210), PayloadHash::new([2; 32]))
        .unwrap()
        .complete(
            SubmissionResult::DryRunSucceeded,
            Timestamp::from_seconds(220),
        )
        .unwrap();
        assert_eq!(
            SubmissionSchedule::assess(
                &candidate,
                std::slice::from_ref(&success),
                Timestamp::from_seconds(1000)
            ),
            expected
        );
        let changed = SubmissionRequest::new(
            header(),
            vec![
                SubmissionRow::new(
                    RowId(1),
                    CommodityId(7),
                    "121".parse().unwrap(),
                    "0".parse().unwrap(),
                    1,
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let changed = SubmissionJob::queued(JobId(113), changed).unwrap();
        assert_eq!(
            SubmissionSchedule::assess(&changed, &[success], Timestamp::from_seconds(1000)),
            ScheduleDecision::NewerObservationSent
        );
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_4_r_val_6__accepted_production_cooldown_uses_wire_time_while_unknown_uses_attempt_end() {
    use tallyline_shared_kernel::api::model::UexReportId;
    use tallyline_submission::api::SubmissionSchedule;
    use tallyline_submission::api::model::ScheduleDecision;
    let mut production = header();
    production.mode = SubmissionMode::Production;
    let request = SubmissionRequest::new(production, vec![row().unwrap()]).unwrap();
    let candidate = SubmissionJob::queued(JobId(201), request.clone()).unwrap();
    let sending = SubmissionJob::queued(JobId(200), request)
        .unwrap()
        .begin_prepared(
            Timestamp::from_seconds(110),
            PayloadHash::new([1; 32]),
            Timestamp::from_seconds(95),
        )
        .unwrap();
    let accepted = sending
        .complete(
            SubmissionResult::ProductionAccepted(vec![
                UexReportId::new(42, Timestamp::from_seconds(95)).unwrap(),
            ]),
            Timestamp::from_seconds(115),
        )
        .unwrap();
    assert_eq!(
        SubmissionSchedule::assess(
            &candidate,
            std::slice::from_ref(&accepted),
            Timestamp::from_seconds(394)
        ),
        ScheduleDecision::WaitingUntil(Timestamp::from_seconds(395))
    );
    assert_eq!(
        SubmissionSchedule::assess(&candidate, &[accepted], Timestamp::from_seconds(395)),
        ScheduleDecision::Ready
    );
    let unknown = sending
        .complete(SubmissionResult::Unknown, Timestamp::from_seconds(115))
        .unwrap();
    assert_eq!(
        SubmissionSchedule::assess(
            &candidate,
            std::slice::from_ref(&unknown),
            Timestamp::from_seconds(395)
        ),
        ScheduleDecision::WaitingUntil(Timestamp::from_seconds(415))
    );
    assert_eq!(
        SubmissionSchedule::assess(&candidate, &[unknown], Timestamp::from_seconds(415)),
        ScheduleDecision::Ready
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_5_r_val_6__preflight_return_requires_renewed_release_without_inventing_an_attempt() {
    use tallyline_submission::api::model::{
        SendReturnReason, SubmissionCommand, SubmissionEventKind, SubmissionReturn,
    };
    let queued = SubmissionJob::queued(
        JobId(210),
        SubmissionRequest::new(header(), vec![row().unwrap()]).unwrap(),
    )
    .unwrap();
    let change = queued
        .apply(SubmissionCommand::Return(
            SendReturnReason::ObservationAgeUnconfirmed,
        ))
        .unwrap();
    assert_eq!(change.state().phase(), JobPhase::Returned);
    assert_eq!(change.state().request(), queued.request());
    assert!(change.state().attempts().is_empty());
    assert_eq!(
        change.state().review_reason(),
        Some(SendReturnReason::ObservationAgeUnconfirmed)
    );
    assert_eq!(
        change.event().unwrap().kind(),
        &SubmissionEventKind::Returned(SubmissionReturn::Preflight(
            SendReturnReason::ObservationAgeUnconfirmed
        ))
    );
    assert_eq!(
        change
            .state()
            .recover(Timestamp::from_seconds(120))
            .unwrap(),
        *change.state()
    );
    assert!(
        change
            .state()
            .begin(Timestamp::from_seconds(120), PayloadHash::new([1; 32]))
            .is_err()
    );
    let sending = queued
        .begin(Timestamp::from_seconds(110), PayloadHash::new([1; 32]))
        .unwrap();
    assert!(
        sending
            .apply(SubmissionCommand::Return(
                SendReturnReason::GameVersionChanged
            ))
            .is_err()
    );
    assert_eq!(sending.phase(), JobPhase::Sending);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_8_r_sub_2__release_observation_order_is_independent_of_other_inflight_work() {
    use tallyline_submission::api::ObservationOrder;
    use tallyline_submission::api::model::ObservationOrderBlock;
    let request = SubmissionRequest::new(header(), vec![row().unwrap()]).unwrap();
    let mut newer_header = header();
    newer_header.observed_at = Timestamp::from_seconds(200);
    let newer = SubmissionJob::queued(
        JobId(200),
        SubmissionRequest::new(newer_header.clone(), vec![row().unwrap()]).unwrap(),
    )
    .unwrap()
    .begin(Timestamp::from_seconds(210), PayloadHash::new([2; 32]))
    .unwrap()
    .complete(
        SubmissionResult::DryRunSucceeded,
        Timestamp::from_seconds(220),
    )
    .unwrap();
    let mut unrelated_header = header();
    unrelated_header.account = "dummy-b".to_owned();
    let unrelated = SubmissionJob::queued(
        JobId(201),
        SubmissionRequest::new(unrelated_header, vec![row().unwrap()]).unwrap(),
    )
    .unwrap()
    .begin(Timestamp::from_seconds(210), PayloadHash::new([3; 32]))
    .unwrap();
    let history = vec![newer, unrelated];
    assert_eq!(
        ObservationOrder::assess(&request, &history),
        vec![ObservationOrderBlock::NeedsConfirmation {
            row: RowId(1),
            observed_at: Timestamp::from_seconds(200)
        }]
    );
    let approved = request
        .with_confirmed_newer_observation(RowId(1), Timestamp::from_seconds(200))
        .unwrap();
    assert!(ObservationOrder::assess(&approved, &history).is_empty());
    newer_header.observed_at = Timestamp::from_seconds(201);
    let sending = SubmissionJob::queued(
        JobId(202),
        SubmissionRequest::new(newer_header, vec![row().unwrap()]).unwrap(),
    )
    .unwrap()
    .begin(Timestamp::from_seconds(210), PayloadHash::new([4; 32]))
    .unwrap();
    for job in [
        sending.clone(),
        sending
            .complete(SubmissionResult::Unknown, Timestamp::from_seconds(220))
            .unwrap(),
        sending
            .complete(
                SubmissionResult::Partial(Vec::new()),
                Timestamp::from_seconds(220),
            )
            .unwrap(),
    ] {
        assert_eq!(
            ObservationOrder::assess(&approved, &[job]),
            vec![ObservationOrderBlock::Unresolved {
                row: RowId(1),
                submission: JobId(202),
                observed_at: Timestamp::from_seconds(201)
            }]
        );
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9__received_decision_retains_unknown_attempt_without_replay_authority() {
    use tallyline_submission::api::model::{
        JobId, JobPhase, PayloadHash, ReceiptDecision, SubmissionCommand, SubmissionJob,
        SubmissionResolutionAction, SubmissionResult,
    };
    let mut production = header();
    production.mode = SubmissionMode::Production;
    let request = SubmissionRequest::new(production, vec![row().unwrap()]).unwrap();
    let unknown = SubmissionJob::queued(JobId(88), request)
        .unwrap()
        .begin(Timestamp::from_seconds(110), PayloadHash::new([8; 32]))
        .unwrap()
        .complete(SubmissionResult::Unknown, Timestamp::from_seconds(120))
        .unwrap();
    let resolved = unknown.apply(SubmissionCommand::ResolveDelivery(
        SubmissionResolutionAction::Decision {
            decision: ReceiptDecision::Received,
            at: Timestamp::from_seconds(130),
        },
    ));
    assert!(
        resolved.is_ok(),
        "a user can record received without fabricating remote acceptance: {resolved:?}"
    );
    let resolved = resolved.unwrap().into_state();
    assert_eq!(resolved.phase(), JobPhase::OutcomeUnknown);
    assert_eq!(resolved.attempts(), unknown.attempts());
    assert_eq!(resolved.result(), Some(&SubmissionResult::Unknown));
    assert_eq!(resolved.revision(), unknown.revision() + 1);
    assert!(resolved.retry().is_err());
    assert!(resolved.cancel().is_err());
    assert!(
        resolved
            .begin(Timestamp::from_seconds(140), PayloadHash::new([9; 32]))
            .is_err()
    );
}

#[path = "queue/unknown_resolution.rs"]
mod unknown_resolution;

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_4__mixed_cooldown_partition_preserves_row_order_and_exact_deadlines() {
    use tallyline_submission::api::SubmissionSchedule;
    use tallyline_submission::api::model::SubmissionResult;

    let first = row().unwrap();
    let second = SubmissionRow::new(
        RowId(2),
        CommodityId(8),
        first.price(),
        first.stock(),
        first.status(),
    )
    .unwrap();
    let third = SubmissionRow::new(
        RowId(3),
        CommodityId(9),
        first.price(),
        first.stock(),
        first.status(),
    )
    .unwrap();
    let candidate = SubmissionJob::queued(
        JobId(100),
        SubmissionRequest::new(header(), vec![third.clone(), second, first.clone()]).unwrap(),
    )
    .unwrap();
    let mut prior_header = header();
    prior_header.observed_at = Timestamp::from_seconds(90);
    prior_header.side = TradeSide::Buy;
    prior_header.environment = UexEnvironment::Ptu;
    let prior = SubmissionJob::queued(
        JobId(99),
        SubmissionRequest::new(prior_header, vec![first]).unwrap(),
    )
    .unwrap()
    .begin(Timestamp::from_seconds(100), PayloadHash::new([1; 32]))
    .unwrap()
    .complete(
        SubmissionResult::DryRunSucceeded,
        Timestamp::from_seconds(110),
    )
    .unwrap();
    let history = vec![prior];
    let partition =
        SubmissionSchedule::cooldown_partition(&candidate, &history, Timestamp::from_seconds(200))
            .unwrap();
    assert_eq!(partition.ready_rows(), &[RowId(3), RowId(2)]);
    assert_eq!(
        partition
            .cooling_rows()
            .iter()
            .map(|row| (row.row(), row.until()))
            .collect::<Vec<_>>(),
        vec![(RowId(1), Timestamp::from_seconds(410))],
    );
    assert_eq!(
        candidate
            .request()
            .rows()
            .iter()
            .map(SubmissionRow::id)
            .collect::<Vec<_>>(),
        vec![RowId(3), RowId(2), RowId(1)]
    );
    assert_eq!(history[0].attempts().len(), 1);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_4_r_sub_9__unrelated_queued_report_is_ready_without_history() {
    use tallyline_submission::api::SubmissionSchedule;
    use tallyline_submission::api::model::ScheduleDecision;

    let candidate = SubmissionJob::queued(
        JobId(1),
        SubmissionRequest::new(header(), vec![row().unwrap()]).unwrap(),
    )
    .unwrap();
    assert_eq!(
        SubmissionSchedule::assess(&candidate, &[], Timestamp::from_seconds(1000)),
        ScheduleDecision::Ready
    );
}

#[path = "queue/duplicate_ancestry.rs"]
mod duplicate_ancestry;

#[path = "queue/duplicate_row_proof.rs"]
mod duplicate_row_proof;

#[path = "queue/rejected_copy.rs"]
mod rejected_copy;
