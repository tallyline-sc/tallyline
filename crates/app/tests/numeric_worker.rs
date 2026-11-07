// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Durable numeric review through the owned manual worker.

use std::sync::mpsc;
use std::time::Duration;
use tallyline_adapter_storage::api::SqliteReferenceCache;
use tallyline_game::api::model::{EnvironmentMapping, GameEnvironment};
use tallyline_reference_data::api::model::{
    CachedReference, EndpointData, PricePrior, PriorProvenance, PriorValues, TerminalPriors,
    UexEnvironment,
};
use tallyline_reference_data::api::{ReferenceApplication, ReferenceDataCache};
use tallyline_reporting::api::model::{ReportKey, ReviewField};
use tallyline_reporting::api::{ReportRepository, RepositoryError};
use tallyline_shared_kernel::api::Clock;
use tallyline_shared_kernel::api::model::{CommodityId, TerminalId, Timestamp, TradeSide};
use tallyline_ui::api::model::{
    BackgroundEvent, ManualCommand, ManualReviewAction, ManualReviewCommand,
};
struct Fixed;
impl Clock for Fixed {
    fn now(&self) -> Timestamp {
        Timestamp::from_seconds(1000)
    }
}
struct Cache(Vec<CachedReference>);
impl ReferenceDataCache for Cache {
    type Error = std::convert::Infallible;
    fn load(&mut self) -> Result<Vec<CachedReference>, Self::Error> {
        Ok(self.0.clone())
    }
    fn replace(&mut self, _: &CachedReference) -> Result<(), Self::Error> {
        Ok(())
    }
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_12_q_manual_r_val_7_q_manual_r_nf_11__worker_publishes_saved_review_and_exact_price_approval_without_changing_other_values()
 {
    let path = std::env::temp_dir().join(format!(
        "tallyline-numeric-worker-{}-{}.db",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut store = SqliteReferenceCache::open(&path).unwrap();
    let mut report = store
        .create_manual(
            ReportKey::new(TerminalId(11), TradeSide::Buy, GameEnvironment::Live).unwrap(),
            Timestamp::from_seconds(900),
            Some("4.10.2"),
        )
        .unwrap();
    let row = report.add_commodity(CommodityId(42)).unwrap();
    report.set_price(row, Some("200".parse().unwrap())).unwrap();
    report.set_stock(row, Some("17".parse().unwrap())).unwrap();
    report.set_status(row, std::num::NonZeroU8::new(5)).unwrap();
    store.save_draft(&report, 0).unwrap();
    let prior = PricePrior::new(
        CommodityId(42),
        TradeSide::Buy,
        PriorValues {
            price: "100".parse().unwrap(),
            stock: "0".parse().unwrap(),
            status: std::num::NonZeroU8::new(3),
            container_sizes: vec![],
        },
        PriorProvenance {
            game_version: "4.10.2".to_owned(),
            modified_at: Timestamp::from_seconds(900),
        },
    )
    .unwrap();
    let snapshot = ReferenceApplication::load(
        Cache(vec![CachedReference::new(
            Timestamp::from_seconds(950),
            EndpointData::TerminalPriors(TerminalPriors::new(TerminalId(11), vec![prior]).unwrap()),
        )]),
        Fixed,
    )
    .unwrap()
    .snapshot(UexEnvironment::Live);
    let (commands, incoming) = mpsc::sync_channel(1);
    let (outgoing, events) = mpsc::sync_channel(4);
    let worker = tallyline_app::api::ManualWorker::start(
        store,
        Fixed,
        incoming,
        outgoing,
        || Ok(EnvironmentMapping::default()),
        || (),
    )
    .unwrap();
    assert!(matches!(
        events.recv_timeout(Duration::from_secs(10)).unwrap(),
        BackgroundEvent::ReportsLoaded { .. }
    ));
    commands
        .send(ManualCommand::Review(ManualReviewCommand {
            stock_settings: Default::default(),
            id: report.id(),
            expected: report.version(),
            action: ManualReviewAction::Reassess,
            snapshot: snapshot.clone(),
        }))
        .unwrap();
    let BackgroundEvent::ReportReviewed {
        review, expected, ..
    } = events.recv_timeout(Duration::from_secs(10)).unwrap()
    else {
        panic!("expected saved review");
    };
    assert_eq!(expected, report.version());
    assert_eq!(review.summary().needs_confirmation, 3);
    let expected = review.report().version();
    commands
        .send(ManualCommand::Review(ManualReviewCommand {
            stock_settings: Default::default(),
            id: report.id(),
            expected,
            action: ManualReviewAction::Confirm(ReviewField::Price(row)),
            snapshot: snapshot.clone(),
        }))
        .unwrap();
    let BackgroundEvent::ReportReviewed { review, .. } =
        events.recv_timeout(Duration::from_secs(10)).unwrap()
    else {
        panic!("expected saved exact-value approval");
    };
    assert_eq!(
        review.report().rows()[0]
            .price_confirmation()
            .unwrap()
            .value()
            .to_string(),
        "200"
    );
    assert!(review.report().rows()[0].status_needs_confirmation());
    assert_eq!(review.report().rows()[0].stock().unwrap().to_string(), "17");
    assert_eq!(review.report().observed_at(), Timestamp::from_seconds(900));
    commands
        .send(ManualCommand::Review(ManualReviewCommand {
            stock_settings: Default::default(),
            id: report.id(),
            expected,
            action: ManualReviewAction::Confirm(ReviewField::Price(row)),
            snapshot: snapshot.clone(),
        }))
        .unwrap();
    assert!(
        matches!(events.recv_timeout(Duration::from_secs(10)).unwrap(), BackgroundEvent::ReviewFailed { id, expected: failed, error: RepositoryError::Conflict } if id == report.id() && failed == expected)
    );
    let mut review = review;
    for action in [
        ManualReviewAction::Focus(ReviewField::Stock(row)),
        ManualReviewAction::Confirm(ReviewField::Stock(row)),
    ] {
        commands
            .send(ManualCommand::Review(ManualReviewCommand {
                stock_settings: Default::default(),
                id: report.id(),
                expected: review.report().version(),
                action,
                snapshot: snapshot.clone(),
            }))
            .unwrap();
        let BackgroundEvent::ReportReviewed { review: next, .. } =
            events.recv_timeout(Duration::from_secs(10)).unwrap()
        else {
            panic!("expected stock review");
        };
        review = next;
    }
    assert_eq!(
        review.report().rows()[0]
            .stock_confirmation()
            .unwrap()
            .value()
            .to_string(),
        "17"
    );
    assert!(!review.report().rows()[0].stock_needs_confirmation());
    assert!(review.report().rows()[0].price_confirmation().is_some());
    let saved = review.report().clone();
    worker.finish().unwrap();
    let mut store = SqliteReferenceCache::open(&path).unwrap();
    assert_eq!(store.load_draft(report.id()).unwrap(), Some(saved));
    drop(store);
    std::fs::remove_file(path).unwrap();
}
