// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Explicit cooldown partition through public workflows and real SQLite.

use super::{support, three_rows};
use tallyline_adapter_storage::api::SqliteReferenceCache;
use tallyline_shared_kernel::api::model::{SubmissionMode, Timestamp, UexReportId};
use tallyline_submission::api::model::{
    JobPhase, PayloadHash, ScheduleDecision, SubmissionCommand, SubmissionResult,
};
use tallyline_submission::api::{SubmissionJobRepository, SubmissionSchedule};
use tallyline_workflows::api::{CooldownSplitStore, ManualReleaseStore, SubmissionQueueStore};

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_4_r_nf_11__explicit_mixed_split_preserves_release_history_and_reopens_without_replay() {
    let root =
        std::env::temp_dir().join(format!("tallyline-cooldown-split-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("queue.db");
    let (mut store, gate) =
        support::stored_gate_with_mode(&path, SubmissionMode::Production).unwrap();
    let prior = store.release_manual(&gate).unwrap();
    let sending = prior
        .job()
        .apply(SubmissionCommand::Begin {
            at: Timestamp::from_seconds(1000),
            payload_hash: PayloadHash::new([4; 32]),
            date_added: Timestamp::from_seconds(900),
        })
        .unwrap();
    SubmissionJobRepository::save(&mut store, &sending).unwrap();
    let accepted = sending
        .state()
        .apply(SubmissionCommand::Complete {
            result: SubmissionResult::ProductionAccepted(vec![
                UexReportId::new(42, Timestamp::from_seconds(900)).unwrap(),
            ]),
            at: Timestamp::from_seconds(1001),
        })
        .unwrap();
    SubmissionJobRepository::save(&mut store, &accepted).unwrap();
    drop(store);
    let (mut store, gate) = three_rows::configured_gate(
        &path,
        Some("0".parse().unwrap()),
        Some(tallyline_shared_kernel::api::model::CommodityId(45)),
    )
    .unwrap();
    let released = store.release_manual(&gate).unwrap();
    let original = released.job().clone();
    assert!(released.report().rows()[0].price_confirmation().is_some());
    assert!(released.report().attachment().is_some());
    assert!(released.report().age_confirmation().is_some());
    let all_rows = original
        .request()
        .rows()
        .iter()
        .map(|row| row.id())
        .collect::<Vec<_>>();
    assert!(
        released
            .report()
            .split_off_cooldown_rows(
                &all_rows,
                tallyline_shared_kernel::api::model::ReportId(800)
            )
            .is_err()
    );
    let waiting = released
        .report()
        .apply_delivery(
            original.id(),
            tallyline_reporting::api::model::ReportDelivery::Cooldown(Timestamp::from_seconds(
                1200,
            )),
        )
        .unwrap()
        .into_state();
    let waiting_split = tallyline_workflows::api::CooldownSplit::prepare(
        &waiting,
        &original,
        &store.queue_jobs().unwrap(),
        tallyline_workflows::api::CooldownSplitIdentities {
            ready_report: tallyline_shared_kernel::api::model::ReportId(800),
            ready_job: tallyline_submission::api::model::JobId(900),
            cooling_job: tallyline_submission::api::model::JobId(901),
        },
        Timestamp::from_seconds(1100),
    )
    .unwrap();
    assert_eq!(
        waiting_split.cooling().report().observed_at(),
        original.request().header().observed_at
    );
    assert!(
        store
            .split_cooldown(
                original.id(),
                original.revision() + 1,
                Timestamp::from_seconds(1100)
            )
            .is_err()
    );
    assert!(
        store
            .split_cooldown(
                original.id(),
                original.revision(),
                Timestamp::from_seconds(1200)
            )
            .is_err()
    );
    let db = rusqlite::Connection::open(&path).unwrap();
    for trigger in [
        "CREATE TRIGGER refuse_split BEFORE INSERT ON domain_outbox WHEN NEW.event_kind='cancelled_split' BEGIN SELECT RAISE(ABORT,'injected outbox failure'); END;",
        "CREATE TRIGGER refuse_split BEFORE INSERT ON submission_splits BEGIN SELECT RAISE(ABORT,'injected final split failure'); END;",
    ] {
        db.execute_batch(trigger).unwrap();
        assert!(
            store
                .split_cooldown(
                    original.id(),
                    original.revision(),
                    Timestamp::from_seconds(1100)
                )
                .is_err()
        );
        assert_eq!(
            store.queue_jobs().unwrap(),
            vec![accepted.state().clone(), original.clone()]
        );
        assert_eq!(
            store.released_report(original.id()).unwrap().as_ref(),
            Some(released.report())
        );
        db.execute_batch("DROP TRIGGER refuse_split").unwrap();
    }
    drop(db);
    let split = store
        .split_cooldown(
            original.id(),
            original.revision(),
            Timestamp::from_seconds(1100),
        )
        .unwrap();
    assert_eq!(split.origin().phase(), JobPhase::Cancelled);
    assert_eq!(split.origin().request(), original.request());
    assert_eq!(split.origin().attempts(), original.attempts());
    assert_eq!(
        split.ready().report().rows(),
        &released.report().rows()[1..3]
    );
    assert_eq!(
        split.cooling().report().rows(),
        &[
            released.report().rows()[0].clone(),
            released.report().rows()[3].clone()
        ]
    );
    assert_eq!(
        split.ready().job().request().rows(),
        &original.request().rows()[1..]
    );
    assert_eq!(
        split.cooling().job().request().rows(),
        &original.request().rows()[..1]
    );
    assert!(!split.cooling().report().rows()[1].entered());
    assert_eq!(split.cooling().report().id(), released.report().id());
    assert_ne!(split.ready().report().id(), released.report().id());
    for part in [split.ready(), split.cooling()] {
        assert_eq!(part.report().observed_at(), released.report().observed_at());
        assert_eq!(part.report().attachment(), released.report().attachment());
        assert_eq!(
            part.report().age_confirmation(),
            released.report().age_confirmation()
        );
        assert_eq!(
            part.report().faction_affinity(),
            released.report().faction_affinity()
        );
        assert_ne!(part.job().id(), original.id());
        assert_eq!(
            part.job().request().header().report_version,
            part.report().version()
        );
        assert!(
            tallyline_workflows::api::apply_submission_delivery(part.report(), split.origin())
                .is_err()
        );
    }
    let history = store.queue_jobs().unwrap();
    assert_eq!(history.len(), 4);
    assert_eq!(
        SubmissionSchedule::assess(split.ready().job(), &history, Timestamp::from_seconds(1100)),
        ScheduleDecision::Ready
    );
    assert_eq!(
        SubmissionSchedule::assess(
            split.cooling().job(),
            &history,
            Timestamp::from_seconds(1100)
        ),
        ScheduleDecision::WaitingUntil(Timestamp::from_seconds(1200))
    );
    assert_eq!(
        store.released_report(original.id()).unwrap().as_ref(),
        Some(released.report())
    );
    assert!(
        store
            .split_cooldown(
                original.id(),
                original.revision(),
                Timestamp::from_seconds(1100)
            )
            .is_err()
    );
    assert!(split.origin().apply(SubmissionCommand::Cancel).is_err());
    use tallyline_workflows::api::{EventOutbox, OutboxKind, dispatch_pending_events};
    assert!(
        store
            .pending_events()
            .unwrap()
            .iter()
            .any(|event| event.kind() == OutboxKind::SubmissionSplit)
    );
    let before = store.queue_jobs().unwrap();
    assert!(dispatch_pending_events(&mut store, |_| Ok(())).unwrap() > 0);
    assert_eq!(
        dispatch_pending_events(&mut store, |_| panic!(
            "delivered split notifications must not replay"
        )),
        Ok(0)
    );
    assert_eq!(store.queue_jobs().unwrap(), before);
    drop(store);
    let mut reopened = SqliteReferenceCache::open(&path).unwrap();
    assert_eq!(
        SubmissionJobRepository::load(&mut reopened, prior.job().id())
            .unwrap()
            .as_ref(),
        Some(accepted.state())
    );
    assert_eq!(
        SubmissionJobRepository::load(&mut reopened, original.id())
            .unwrap()
            .as_ref(),
        Some(split.origin())
    );
    assert_eq!(reopened.queue_jobs().unwrap().len(), 4);
    assert_eq!(
        reopened.released_report(original.id()).unwrap().as_ref(),
        Some(released.report())
    );
    let recovered = reopened
        .recover_unfinished(Timestamp::from_seconds(1101))
        .unwrap();
    assert_eq!(recovered.len(), 2);
    let drafts = reopened.drafts().unwrap();
    assert_eq!(drafts.len(), 2);
    assert!(
        drafts
            .iter()
            .any(|report| report.id() == split.ready().report().id())
    );
    assert!(
        drafts
            .iter()
            .any(|report| report.id() == split.cooling().report().id())
    );
    assert!(
        recovered
            .iter()
            .all(|job| job.phase() == JobPhase::Returned)
    );
    use tallyline_reporting::api::ReportRepository;
    for part in [split.ready(), split.cooling()] {
        assert_eq!(
            reopened
                .load_draft(part.report().id())
                .unwrap()
                .unwrap()
                .state(),
            tallyline_reporting::api::model::ReportState::Draft
        );
    }
    assert_eq!(
        SubmissionJobRepository::load(&mut reopened, original.id())
            .unwrap()
            .as_ref(),
        Some(split.origin())
    );
    assert_eq!(
        reopened.released_report(original.id()).unwrap().as_ref(),
        Some(released.report())
    );
    drop(reopened);
    std::fs::remove_dir_all(root).unwrap();
}
