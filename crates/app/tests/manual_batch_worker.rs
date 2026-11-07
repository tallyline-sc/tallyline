// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Atomic manual worker receipts through real SQLite persistence.

use std::sync::mpsc;
use std::time::Duration;
use tallyline_adapter_storage::api::SqliteReferenceCache;
use tallyline_game::api::model::{EnvironmentMapping, GameEnvironment};
use tallyline_reference_data::api::model::{
    CachedReference, EndpointData, PricePrior, PriorProvenance, PriorValues, TerminalPriors,
    UexEnvironment,
};
use tallyline_reference_data::api::{ReferenceApplication, ReferenceDataCache};
use tallyline_reporting::api::model::{Report, ReportId, ReportKey, RowId};
use tallyline_reporting::api::{ManualEdit, ReportRepository, RepositoryError};
use tallyline_shared_kernel::api::Clock;
use tallyline_shared_kernel::api::model::{CommodityId, TerminalId, Timestamp, TradeSide};
use tallyline_ui::api::model::{BackgroundEvent, ManualCommand, ManualEditRequestId};

struct Fixed;
impl Clock for Fixed {
    fn now(&self) -> Timestamp {
        Timestamp::from_seconds(1000)
    }
}
struct Cache;
impl ReferenceDataCache for Cache {
    type Error = Box<dyn std::error::Error>;
    fn load(&mut self) -> Result<Vec<CachedReference>, Self::Error> {
        Ok(vec![CachedReference::new(
            Timestamp::from_seconds(900),
            EndpointData::TerminalPriors(TerminalPriors::new(
                TerminalId(11),
                vec![PricePrior::new(
                    CommodityId(42),
                    TradeSide::Buy,
                    PriorValues {
                        price: "100".parse()?,
                        stock: "0".parse()?,
                        status: std::num::NonZeroU8::new(3),
                        container_sizes: vec![],
                    },
                    PriorProvenance {
                        game_version: "4.10.2".to_owned(),
                        modified_at: Timestamp::from_seconds(900),
                    },
                )?],
            )?),
        )])
    }
    fn replace(&mut self, _: &CachedReference) -> Result<(), Self::Error> {
        Ok(())
    }
}

fn prepare(
    name: &str,
) -> Result<(std::path::PathBuf, SqliteReferenceCache, Report), Box<dyn std::error::Error>> {
    let path = std::env::temp_dir().join(format!("{name}-{}.db", std::process::id()));
    let mut store = SqliteReferenceCache::open(&path)?;
    let key = ReportKey::new(TerminalId(11), TradeSide::Buy, GameEnvironment::Live)?;
    let mut report = store.create_manual(key, Timestamp::from_seconds(1000), Some("4.10.2"))?;
    report.add_commodity(CommodityId(42))?;
    report.add_commodity(CommodityId(43))?;
    store.save_draft(&report, 0)?;
    Ok((path, store, report))
}

fn command(
    report: &Report,
    request: u64,
    edits: Vec<ManualEdit>,
) -> Result<ManualCommand, Box<dyn std::error::Error>> {
    Ok(ManualCommand::Edit {
        request: ManualEditRequestId(request),
        id: report.id(),
        expected: report.version(),
        edits,
        snapshot: ReferenceApplication::load(Cache, Fixed)?.snapshot(UexEnvironment::Live),
    })
}
fn receipt(
    events: &mpsc::Receiver<BackgroundEvent>,
    request: u64,
    id: ReportId,
    expected: u64,
) -> Result<Result<Report, RepositoryError>, Box<dyn std::error::Error>> {
    let BackgroundEvent::ReportEditFinished {
        request: actual,
        id: actual_id,
        expected: actual_version,
        result,
    } = events.recv_timeout(Duration::from_secs(10))?
    else {
        return Err("expected a bound manual edit receipt".into());
    };
    assert_eq!(actual, ManualEditRequestId(request));
    assert_eq!(actual_id, id);
    assert_eq!(actual_version, expected);
    Ok(result.map(|report| *report))
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_2_r_man_3_r_nf_11__worker_publishes_one_bound_atomic_batch_and_reopens_exact_values() {
    let (path, store, before) = prepare("tallyline-batch-worker").unwrap();
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
        BackgroundEvent::ReportsLoaded { selected: None, .. }
    ));
    commands
        .send(
            command(
                &before,
                1,
                vec![
                    ManualEdit::Price(
                        before.rows()[1].id(),
                        Some("12345678901234567890.125".parse().unwrap()),
                    ),
                    ManualEdit::TakeOver(before.rows()[0].id()),
                ],
            )
            .unwrap(),
        )
        .unwrap();
    let committed = receipt(&events, 1, before.id(), before.version())
        .unwrap()
        .unwrap();
    assert_eq!(committed.rows()[0].price().unwrap().to_string(), "100");
    assert_eq!(
        committed.rows()[1].price().unwrap().to_string(),
        "12345678901234567890.125"
    );
    assert!(events.try_recv().is_err());
    worker.finish().unwrap();
    let mut reopened = SqliteReferenceCache::open(&path).unwrap();
    assert_eq!(
        reopened.load_draft(committed.id()).unwrap(),
        Some(committed)
    );
    drop(reopened);
    std::fs::remove_file(path).unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_2_r_nf_11__worker_binds_validation_and_stale_refusals_without_persisting_partial_edits() {
    let (path, store, before) = prepare("tallyline-batch-worker-refusal").unwrap();
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
    events.recv_timeout(Duration::from_secs(10)).unwrap();
    commands
        .send(
            command(
                &before,
                1,
                vec![
                    ManualEdit::Price(before.rows()[0].id(), Some("200".parse().unwrap())),
                    ManualEdit::Stock(RowId(99), Some("1".parse().unwrap())),
                ],
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(
        receipt(&events, 1, before.id(), before.version()).unwrap(),
        Err(RepositoryError::Invalid)
    );
    let mut stale = command(
        &before,
        2,
        vec![ManualEdit::TakeOver(before.rows()[0].id())],
    )
    .unwrap();
    let ManualCommand::Edit { expected, .. } = &mut stale else {
        panic!("expected an edit");
    };
    *expected = 0;
    commands.send(stale).unwrap();
    assert_eq!(
        receipt(&events, 2, before.id(), 0).unwrap(),
        Err(RepositoryError::Conflict)
    );
    worker.finish().unwrap();
    let mut reopened = SqliteReferenceCache::open(&path).unwrap();
    assert_eq!(reopened.load_draft(before.id()).unwrap(), Some(before));
    drop(reopened);
    std::fs::remove_file(path).unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_2_r_ui_16__mapping_failures_keep_the_exact_edit_request_binding() {
    let (path, store, report) = prepare("tallyline-batch-worker-mapping").unwrap();
    let (commands, incoming) = mpsc::sync_channel(1);
    let (outgoing, events) = mpsc::sync_channel(4);
    let worker = tallyline_app::api::ManualWorker::start(
        store,
        Fixed,
        incoming,
        outgoing,
        || Err(std::io::Error::other("mapping unavailable")),
        || (),
    )
    .unwrap();
    events.recv_timeout(Duration::from_secs(10)).unwrap();
    commands
        .send(
            command(
                &report,
                7,
                vec![ManualEdit::TakeOver(report.rows()[0].id())],
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(
        receipt(&events, 7, report.id(), report.version()).unwrap(),
        Err(RepositoryError::Unavailable)
    );
    worker.finish().unwrap();
    std::fs::remove_file(path).unwrap();
}
