// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Actual SQLite, review policy and session validation for queue-owner contracts.

use tallyline_app::api::{HttpClient, SubmissionQueueController, SubmissionQueuePreferences};
use tallyline_game::api::model::EnvironmentMapping;
use tallyline_reference_data::api::ReferenceDataCache;
use tallyline_reference_data::api::model::{
    AcceptanceSwitch, CachedReference, CommodityConstraints, EndpointData, ReferenceParameters,
    SubmissionAcceptance,
};
use tallyline_reporting::api::ManualReviewer;
use tallyline_reporting::api::model::{Report, StockSettings};
use tallyline_shared_kernel::api::model::{ObservationAgeSettings, SubmissionMode, Timestamp};
use tallyline_submission::api::CredentialApplication;
use tallyline_submission::api::model::{
    CredentialCandidate, CredentialValues, Persistence, ValidatedAccount,
};

pub struct Fixture {
    pub controller: SubmissionQueueController,
    pub application: CredentialApplication,
    pub report: Report,
    pub path: std::path::PathBuf,
}
pub fn fixture(
    client: HttpClient,
    mode: SubmissionMode,
    name: &str,
) -> Result<Fixture, Box<dyn std::error::Error>> {
    fixture_with_clock(client, mode, name, super::support::Fixed)
}
pub fn fixture_with_clock(
    client: HttpClient,
    mode: SubmissionMode,
    name: &str,
    clock: impl tallyline_shared_kernel::api::Clock + Send + 'static,
) -> Result<Fixture, Box<dyn std::error::Error>> {
    let path = std::env::temp_dir().join(format!(
        "tallyline-controller-{name}-{}.db",
        std::process::id()
    ));
    let (mut database, draft) = super::support::stored_gate(&path)?;
    database.replace(&CachedReference::new(
        Timestamp::from_seconds(950),
        EndpointData::Parameters(ReferenceParameters::new(
            SubmissionAcceptance {
                datacenter: AcceptanceSwitch::Enabled,
                reports: AcceptanceSwitch::Enabled,
                commodity: AcceptanceSwitch::Enabled,
                ptu: AcceptanceSwitch::Disabled,
            },
            CommodityConstraints {
                price_variation_percent: 20,
                prior_ttl_days: std::num::NonZeroU32::new(15)
                    .ok_or_else(|| std::io::Error::other("invalid fixture prior lifetime"))?,
                scu_variation_percent: 20,
            },
            None,
        )),
    ))?;
    let mut application = CredentialApplication::default();
    let request = application.begin(CredentialCandidate::new(
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned()))
            .map_err(|_| std::io::Error::other("fixture credentials unavailable"))?,
        Persistence::Session,
    ));
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(application.accept(
            request,
            Ok(ValidatedAccount::new("dummy-a".to_owned())),
            Timestamp::from_seconds(1000),
            &mut super::Store,
        ))
        .map_err(|_| std::io::Error::other("fixture validation unavailable"))?;
    let controller = SubmissionQueueController::new(
        database,
        client,
        move || {
            Ok(SubmissionQueuePreferences {
                reviewer: ManualReviewer::new(
                    EnvironmentMapping::default(),
                    StockSettings::default(),
                    None,
                ),
                age: ObservationAgeSettings::default(),
                mode,
            })
        },
        None,
        clock,
    )
    .map_err(|_| std::io::Error::other("fixture queue unavailable"))?;
    Ok(Fixture {
        controller,
        application,
        report: draft.source().clone(),
        path,
    })
}

pub fn open_with_build_information(
    path: &std::path::Path,
    client: HttpClient,
    information: tallyline_app::api::model::BuildInformation,
) -> Result<SubmissionQueueController, Box<dyn std::error::Error>> {
    Ok(SubmissionQueueController::new_with_build_information(
        tallyline_adapter_storage::api::SqliteReferenceCache::open(path)?,
        client,
        || {
            Ok(SubmissionQueuePreferences {
                reviewer: ManualReviewer::new(
                    EnvironmentMapping::default(),
                    StockSettings::default(),
                    None,
                ),
                age: ObservationAgeSettings::default(),
                mode: SubmissionMode::DryRun,
            })
        },
        None,
        super::support::Fixed,
        information,
    )
    .map_err(|_| std::io::Error::other("fixture queue unavailable"))?)
}
