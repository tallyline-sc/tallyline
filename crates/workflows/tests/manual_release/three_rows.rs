// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared three-row manual release fixture through public reference and Reporting APIs.

use super::support;
use std::num::NonZeroU8;
use tallyline_adapter_storage::api::SqliteReferenceCache;
use tallyline_game::api::model::EnvironmentMapping;
use tallyline_reference_data::api::ReferenceDataCache;
use tallyline_reference_data::api::model::{
    CachedReference, Commodity, CommodityAverages, EndpointData, PricePrior, PriorProvenance,
    PriorValues, TerminalPriors,
};
use tallyline_reporting::api::model::{
    ReleaseAccountStanding, ReleaseContext, ReleaseScreenshotPolicy, ReleaseWorkingCopy,
    StockSettings, SubmissionGate,
};
use tallyline_reporting::api::{ManualReviewer, ReportRepository};
use tallyline_shared_kernel::api::model::{
    CommodityId, ObservationAgeSettings, SubmissionMode, TerminalId, Timestamp, TradeSide,
    UexEnvironment,
};

pub(super) fn gate(path: &std::path::Path) -> Option<(SqliteReferenceCache, SubmissionGate)> {
    gate_with_affinity(path, None)
}
pub(super) fn gate_with_affinity(
    path: &std::path::Path,
    affinity: Option<tallyline_shared_kernel::api::model::FactionAffinity>,
) -> Option<(SqliteReferenceCache, SubmissionGate)> {
    configured_gate(path, affinity, None)
}
pub(super) fn configured_gate(
    path: &std::path::Path,
    affinity: Option<tallyline_shared_kernel::api::model::FactionAffinity>,
    placeholder: Option<CommodityId>,
) -> Option<(SqliteReferenceCache, SubmissionGate)> {
    let (mut store, initial) =
        support::stored_gate_with_mode(path, SubmissionMode::Production).ok()?;
    let commodities = [
        (42, "Gold", "GOLD"),
        (43, "Silver", "SILV"),
        (44, "Iron", "IRON"),
    ]
    .into_iter()
    .map(|(id, name, code)| {
        Commodity::new(
            CommodityId(id),
            name,
            code,
            true,
            CommodityAverages::new(None, None),
        )
    })
    .collect();
    store
        .replace(&CachedReference::new(
            Timestamp::from_seconds(950),
            EndpointData::Commodities(commodities),
        ))
        .ok()?;
    let priors = [42, 43, 44]
        .into_iter()
        .map(|id| {
            PricePrior::new(
                CommodityId(id),
                TradeSide::Buy,
                PriorValues {
                    price: "123.45".parse().ok()?,
                    stock: "0".parse().ok()?,
                    status: NonZeroU8::new(3),
                    container_sizes: vec![],
                },
                PriorProvenance {
                    game_version: "4.10.2".to_owned(),
                    modified_at: Timestamp::from_seconds(900),
                },
            )
            .ok()
        })
        .collect::<Option<Vec<_>>>()?;
    store
        .replace(&CachedReference::new(
            Timestamp::from_seconds(950),
            EndpointData::TerminalPriors(TerminalPriors::new(TerminalId(11), priors).ok()?),
        ))
        .ok()?;
    let mut report = initial.source().clone();
    report = report.set_faction_affinity(affinity).ok()?;
    for id in [43, 44] {
        let row = report.add_commodity(CommodityId(id)).ok()?;
        report.set_price(row, Some("123.45".parse().ok()?)).ok()?;
        report.set_stock(row, Some("0".parse().ok()?)).ok()?;
        report.set_status(row, NonZeroU8::new(3)).ok()?;
    }
    if let Some(commodity) = placeholder {
        report.add_commodity(commodity).ok()?;
        let row = report.rows().first()?.id();
        report.set_price(row, Some("200".parse().ok()?)).ok()?;
    }
    store.save_draft(&report, initial.source().version()).ok()?;
    if placeholder.is_some() {
        store
            .replace(&CachedReference::new(
                Timestamp::from_seconds(800),
                EndpointData::GameVersions(tallyline_reference_data::api::model::GameVersions {
                    live: "4.10.2".to_owned(),
                    ptu: None,
                }),
            ))
            .ok()?;
    }
    let records = ReferenceDataCache::load(&mut store).ok()?;
    let snapshot = support::snapshot_records(records, UexEnvironment::Live).ok()?;
    let working_copy = if placeholder.is_some() {
        use tallyline_reporting::api::model::{
            AttachmentBinding, AttachmentRegion, AttachmentTime, ImageHash,
        };
        use tallyline_reporting::api::{AttachmentEdit, ReportingApplication};
        let row = report.rows().first()?.id();
        let mut application = ReportingApplication::new(store, support::Fixed);
        report = application
            .confirm_price(report.id(), report.version(), row, &snapshot)
            .ok()?;
        drop(application);
        store = SqliteReferenceCache::open(path).ok()?;
        let expected = report.version();
        let binding = AttachmentBinding::new(
            ImageHash::new([1; 32]),
            AttachmentRegion::new(0, 0, 10, 10).ok()?,
        );
        let digest = ImageHash::new([2; 32]);
        report
            .edit_attachment(AttachmentEdit::Select(
                binding,
                AttachmentTime::ImportedFile(Timestamp::from_seconds(1000)),
            ))
            .ok()?;
        report
            .edit_attachment(AttachmentEdit::ConfirmPreview(binding))
            .ok()?;
        report
            .edit_attachment(AttachmentEdit::BindWorkingCopy(binding, digest))
            .ok()?;
        report
            .edit_attachment(AttachmentEdit::ConfirmTime(Timestamp::from_seconds(900)))
            .ok()?;
        report = report
            .confirm_observation_age(Timestamp::from_seconds(1000))
            .ok()?
            .into_state();
        store.save_draft(&report, expected).ok()?;
        ReleaseWorkingCopy::Verified { binding, digest }
    } else {
        ReleaseWorkingCopy::Absent
    };

    let reviewer = ManualReviewer::new(
        EnvironmentMapping::default(),
        StockSettings::default(),
        None,
    );
    let gate = reviewer.submission_gate(
        report,
        &snapshot,
        ReleaseContext::new(
            ReleaseAccountStanding::Accepted("dummy-a".to_owned()),
            SubmissionMode::Production,
            ReleaseScreenshotPolicy::Unknown,
            working_copy,
        ),
        ObservationAgeSettings::default(),
        Timestamp::from_seconds(1000),
    );
    let gate = tallyline_workflows::api::assess_release_observation_history(&gate, Some(&[]));
    Some((store, gate))
}
