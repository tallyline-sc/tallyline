// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! App-owned manual storage worker restart contract.

use std::sync::mpsc;
use std::time::Duration;
use tallyline_adapter_storage::api::SqliteReferenceCache;
use tallyline_game::api::model::{EnvironmentMapping, GameEnvironment};
use tallyline_reporting::api::ReportRepository;
use tallyline_reporting::api::model::ReportKey;
use tallyline_shared_kernel::api::model::{TerminalId, Timestamp, TradeSide};
use tallyline_ui::api::model::{BackgroundEvent, ManualCommand};
struct Fixed;
impl tallyline_shared_kernel::api::Clock for Fixed {
    fn now(&self) -> Timestamp {
        Timestamp::from_seconds(1000)
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_2_r_ui_16__manual_worker_publishes_only_durable_drafts_and_reopens_them_after_restart() {
    let path =
        std::env::temp_dir().join(format!("tallyline-manual-worker-{}.db", std::process::id()));
    let store = SqliteReferenceCache::open(&path).unwrap();
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
    struct Cache;
    impl tallyline_reference_data::api::ReferenceDataCache for Cache {
        type Error = std::convert::Infallible;
        fn load(
            &mut self,
        ) -> Result<Vec<tallyline_reference_data::api::model::CachedReference>, Self::Error>
        {
            use tallyline_reference_data::api::model::{
                CachedReference, EndpointData, Terminal, TerminalText,
            };
            Ok(vec![CachedReference::new(
                Timestamp::from_seconds(900),
                EndpointData::Terminals(vec![Terminal::new(
                    TerminalId(11),
                    TerminalText {
                        name: "Depot".to_owned(),
                        nickname: "DP".to_owned(),
                        display_name: "Shop".to_owned(),
                        location: "Pyro".to_owned(),
                    },
                    true,
                )]),
            )])
        }
        fn replace(
            &mut self,
            _: &tallyline_reference_data::api::model::CachedReference,
        ) -> Result<(), Self::Error> {
            Ok(())
        }
    }
    let snapshot = tallyline_reference_data::api::ReferenceApplication::load(Cache, Fixed)
        .unwrap()
        .snapshot(tallyline_reference_data::api::model::UexEnvironment::Live);
    commands
        .send(ManualCommand::Create {
            key: ReportKey::new(TerminalId(11), TradeSide::Buy, GameEnvironment::Live).unwrap(),
            snapshot,
        })
        .unwrap();
    let BackgroundEvent::ReportsLoaded { reports, .. } =
        events.recv_timeout(Duration::from_secs(10)).unwrap()
    else {
        panic!("expected durable Draft result");
    };
    let report = reports.first().unwrap().clone();
    worker.finish().unwrap();
    let mut reopened = SqliteReferenceCache::open(&path).unwrap();
    assert_eq!(reopened.load_draft(report.id()).unwrap(), Some(report));
    drop(reopened);
    std::fs::remove_file(path).unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_2_r_ui_16__unavailable_mapping_returns_a_passive_failure_instead_of_leaving_the_editor_waiting()
 {
    struct Empty;
    impl tallyline_reference_data::api::ReferenceDataCache for Empty {
        type Error = std::convert::Infallible;
        fn load(
            &mut self,
        ) -> Result<Vec<tallyline_reference_data::api::model::CachedReference>, Self::Error>
        {
            Ok(vec![])
        }
        fn replace(
            &mut self,
            _: &tallyline_reference_data::api::model::CachedReference,
        ) -> Result<(), Self::Error> {
            Ok(())
        }
    }
    let snapshot = tallyline_reference_data::api::ReferenceApplication::load(Empty, Fixed)
        .unwrap()
        .snapshot(tallyline_reference_data::api::model::UexEnvironment::Live);
    let path = std::env::temp_dir().join(format!(
        "tallyline-manual-mapping-{}.db",
        std::process::id()
    ));
    let store = SqliteReferenceCache::open(&path).unwrap();
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
    let _loaded = events.recv_timeout(Duration::from_secs(10)).unwrap();
    commands
        .send(ManualCommand::Create {
            key: ReportKey::new(TerminalId(11), TradeSide::Buy, GameEnvironment::Live).unwrap(),
            snapshot,
        })
        .unwrap();
    assert!(matches!(
        events.recv_timeout(Duration::from_secs(10)).unwrap(),
        BackgroundEvent::ReportFailed(tallyline_reporting::api::RepositoryError::Unavailable)
    ));
    worker.finish().unwrap();
    std::fs::remove_file(path).unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_5_r_sub_7__private_attachment_controls_use_the_bounded_worker_and_recover_after_restart() {
    use tallyline_app::api::ManualAttachmentStorage;
    use tallyline_reporting::api::ReportingApplication;
    use tallyline_reporting::api::model::AttachmentRegion;
    use tallyline_ui::api::model::{ManualAttachmentAction, ManualAttachmentCommand};
    let directory = std::env::temp_dir().join(format!(
        "tallyline-connected-attachment-{}",
        std::process::id()
    ));
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("tallyline.db");
    let mut app = ReportingApplication::new(SqliteReferenceCache::open(&path).unwrap(), Fixed);
    let report = app
        .create(
            ReportKey::new(TerminalId(11), TradeSide::Buy, GameEnvironment::Live).unwrap(),
            Some("4.10.2"),
        )
        .unwrap();
    drop(app);
    let source = directory.join("source.png");
    let mut image = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut image, 4, 4);
        encoder.set_color(png::ColorType::Rgb);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&[0; 48]).unwrap();
    }
    std::fs::write(&source, image).unwrap();
    let (commands, incoming) = mpsc::sync_channel(1);
    let (outgoing, events) = mpsc::sync_channel(8);
    let worker = tallyline_app::api::ManualWorker::start_with_attachments(
        SqliteReferenceCache::open(&path).unwrap(),
        Fixed,
        incoming,
        outgoing,
        ManualAttachmentStorage::new(directory.clone()),
        || Ok(Default::default()),
        || (),
    )
    .unwrap();
    let _loaded = events.recv_timeout(Duration::from_secs(10)).unwrap();
    commands
        .send(ManualCommand::Attachment(ManualAttachmentCommand {
            id: report.id(),
            expected: report.version(),
            action: ManualAttachmentAction::Prepare {
                source: source.to_string_lossy().into_owned(),
                region: AttachmentRegion::new(0, 1, 4, 3).unwrap(),
            },
        }))
        .unwrap();
    let BackgroundEvent::ReportsLoaded { reports, selected } =
        events.recv_timeout(Duration::from_secs(10)).unwrap()
    else {
        panic!("no saved Report")
    };
    assert_eq!(selected, None);
    let report = reports[0].clone();
    let BackgroundEvent::AttachmentPreview { preview, .. } =
        events.recv_timeout(Duration::from_secs(10)).unwrap()
    else {
        panic!("no source-scale preview")
    };
    commands
        .send(ManualCommand::Attachment(ManualAttachmentCommand {
            id: report.id(),
            expected: report.version(),
            action: ManualAttachmentAction::ConfirmPreview(preview),
        }))
        .unwrap();
    let BackgroundEvent::ReportsLoaded { reports, selected } =
        events.recv_timeout(Duration::from_secs(10)).unwrap()
    else {
        panic!("no confirmed Report")
    };
    assert_eq!(selected, None);
    let report = reports[0].clone();
    assert!(report.attachment().unwrap().working_copy().is_some());
    worker.finish().unwrap();
    std::fs::remove_file(source).unwrap();
    let (commands, incoming) = mpsc::sync_channel(1);
    let (outgoing, events) = mpsc::sync_channel(8);
    let worker = tallyline_app::api::ManualWorker::start_with_attachments(
        SqliteReferenceCache::open(&path).unwrap(),
        Fixed,
        incoming,
        outgoing,
        ManualAttachmentStorage::new(directory.clone()),
        || Ok(Default::default()),
        || (),
    )
    .unwrap();
    let _loaded = events.recv_timeout(Duration::from_secs(10)).unwrap();
    commands
        .send(ManualCommand::Attachment(ManualAttachmentCommand {
            id: report.id(),
            expected: report.version(),
            action: ManualAttachmentAction::View,
        }))
        .unwrap();
    let BackgroundEvent::AttachmentPreview { preview, .. } =
        events.recv_timeout(Duration::from_secs(10)).unwrap()
    else {
        panic!("no recovered preview")
    };
    assert_eq!(
        (preview.raster().width(), preview.raster().height()),
        (4, 3)
    );
    worker.finish().unwrap();
    std::fs::remove_dir_all(directory).unwrap();
}
