// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Overflow targets remain due after a bounded admitted batch.

use super::accepted_fixture::error;
use super::support;

fn accepted_jobs(
    path: &std::path::Path,
) -> std::io::Result<(
    tallyline_adapter_storage::api::SqliteReferenceCache,
    Vec<tallyline_submission::api::model::SubmissionJob>,
)> {
    use std::num::NonZeroU8;
    use tallyline_adapter_storage::api::SqliteReferenceCache;
    use tallyline_game::api::model::{EnvironmentMapping, GameEnvironment};
    use tallyline_reference_data::api::ReferenceDataCache;
    use tallyline_reference_data::api::model::{
        CachedReference, Commodity, CommodityAverages, EndpointData, PricePrior, PriorProvenance,
        PriorValues, TerminalPriors,
    };
    use tallyline_reporting::api::model::{
        ReleaseAccountStanding, ReleaseContext, ReleaseScreenshotPolicy, ReleaseWorkingCopy,
        ReportKey, StockSettings,
    };
    use tallyline_reporting::api::{ManualReviewer, ReportRepository};
    use tallyline_shared_kernel::api::model::{
        CommodityId, ObservationAgeSettings, SubmissionMode, TerminalId, Timestamp, TradeSide,
        UexEnvironment,
    };
    use tallyline_submission::api::SubmissionJobRepository;
    use tallyline_submission::api::model::{
        PayloadHash, SubmissionCommand, SubmissionResult, VerifiedUexReport,
    };
    use tallyline_workflows::api::{
        ManualReleaseStore, SubmissionQueueStore, assess_release_observation_history,
    };
    let mut records = support::records().map_err(error)?;
    records.retain(|record| {
        !matches!(
            record.data(),
            EndpointData::Commodities(_) | EndpointData::TerminalPriors(_)
        )
    });
    let commodities = (1..=501)
        .map(|id| {
            Commodity::new(
                CommodityId(id),
                &format!("Commodity {id}"),
                &format!("C{id}"),
                true,
                CommodityAverages::new(None, None),
            )
        })
        .collect();
    records.push(CachedReference::new(
        Timestamp::from_seconds(950),
        EndpointData::Commodities(commodities),
    ));
    let priors = (1..=501)
        .map(|id| {
            PricePrior::new(
                CommodityId(id),
                TradeSide::Buy,
                PriorValues {
                    price: "123.45".parse().map_err(error)?,
                    stock: "0".parse().map_err(error)?,
                    status: NonZeroU8::new(3),
                    container_sizes: Vec::new(),
                },
                PriorProvenance {
                    game_version: "4.10.2".to_owned(),
                    modified_at: Timestamp::from_seconds(900),
                },
            )
            .map_err(error)
        })
        .collect::<std::io::Result<Vec<_>>>()?;
    records.push(CachedReference::new(
        Timestamp::from_seconds(950),
        EndpointData::TerminalPriors(TerminalPriors::new(TerminalId(11), priors).map_err(error)?),
    ));
    let snapshot =
        support::snapshot_records(records.clone(), UexEnvironment::Live).map_err(error)?;
    let mut store = SqliteReferenceCache::open(path).map_err(error)?;
    for record in records {
        store.replace(&record).map_err(error)?;
    }
    let reviewer = ManualReviewer::new(
        EnvironmentMapping::default(),
        StockSettings::default(),
        None,
    );
    let mut releases = Vec::new();
    for range in [1..=500, 501..=501] {
        let mut report = store
            .create_manual(
                ReportKey::new(TerminalId(11), TradeSide::Buy, GameEnvironment::Live)
                    .map_err(error)?,
                Timestamp::from_seconds(900),
                Some("4.10.2"),
            )
            .map_err(error)?;
        for id in range {
            let row = report.add_commodity(CommodityId(id)).map_err(error)?;
            report
                .set_price(row, Some("123.45".parse().map_err(error)?))
                .map_err(error)?;
            report
                .set_stock(row, Some("0".parse().map_err(error)?))
                .map_err(error)?;
            report.set_status(row, NonZeroU8::new(3)).map_err(error)?;
        }
        store.save_draft(&report, 0).map_err(error)?;
        let gate = reviewer.submission_gate(
            report,
            &snapshot,
            ReleaseContext::new(
                ReleaseAccountStanding::Accepted("dummy-a".to_owned()),
                SubmissionMode::Production,
                ReleaseScreenshotPolicy::Unknown,
                ReleaseWorkingCopy::Absent,
            ),
            ObservationAgeSettings::default(),
            Timestamp::from_seconds(1000),
        );
        let history = store.queue_jobs().map_err(error)?;
        releases.push(
            store
                .release_manual(&assess_release_observation_history(&gate, Some(&history)))
                .map_err(error)?,
        );
    }
    let mut jobs = Vec::new();
    for (release, sent_at) in releases.into_iter().zip([1000, 1002]) {
        let sending = release
            .job()
            .apply(SubmissionCommand::Begin {
                at: Timestamp::from_seconds(sent_at),
                payload_hash: PayloadHash::new([13; 32]),
                date_added: Timestamp::from_seconds(900),
            })
            .map_err(error)?;
        SubmissionJobRepository::save(&mut store, &sending).map_err(error)?;
        let request = sending.state().request();
        let proofs = request
            .rows()
            .iter()
            .map(|row| {
                VerifiedUexReport::from_lookup_at(
                    10_000 + u64::from(row.commodity().0),
                    request.header().terminal,
                    row.commodity(),
                    Timestamp::from_seconds(900),
                    request,
                    Timestamp::from_seconds(900),
                )
                .map_err(error)
            })
            .collect::<std::io::Result<Vec<_>>>()?;
        let accepted = sending
            .state()
            .apply(SubmissionCommand::Complete {
                at: Timestamp::from_seconds(sent_at + 1),
                result: SubmissionResult::ProductionSucceeded(proofs),
            })
            .map_err(error)?;
        SubmissionJobRepository::save(&mut store, &accepted).map_err(error)?;
        jobs.push(accepted.into_state());
    }
    Ok((store, jobs))
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_4_r_nf_3_r_nf_11__a_full_batch_leaves_overflow_due_and_the_next_selection_reaches_it() {
    use tallyline_shared_kernel::api::model::Timestamp;
    use tallyline_submission::api::model::LifecycleRefresh;
    use tallyline_workflows::api::SubmissionLifecycleStore;
    let directory = tempfile::tempdir_in(std::env::temp_dir()).unwrap();
    let (mut store, jobs) = accepted_jobs(&directory.path().join("bounded-batch.db")).unwrap();
    let candidates = [&jobs[0], &jobs[1]];
    let first = store
        .claim_lifecycle_batch(
            &candidates,
            LifecycleRefresh::Automatic,
            Timestamp::from_seconds(1400),
        )
        .unwrap()
        .unwrap();
    assert_eq!(first.claims().len(), 1);
    assert_eq!(first.claims()[0].lifecycle().job(), jobs[0].id());
    assert_eq!(first.claims()[0].targets().count(), 500);
    assert_eq!(store.load_lifecycle(jobs[1].id()).unwrap(), None);
    let next = store
        .claim_lifecycle_batch(
            &candidates,
            LifecycleRefresh::Automatic,
            Timestamp::from_seconds(1400),
        )
        .unwrap()
        .unwrap();
    assert_eq!(next.claims().len(), 1);
    assert_eq!(next.claims()[0].lifecycle().job(), jobs[1].id());
    assert_eq!(next.claims()[0].targets().count(), 1);
}
