// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Bounded attachment execution through real files and durable manual Reports.

use tallyline_adapter_files::api::model::ImageIntakeSettings;
use tallyline_adapter_storage::api::SqliteReferenceCache;
use tallyline_app::api::AttachmentWorker;
use tallyline_game::api::model::GameEnvironment;
use tallyline_reporting::api::model::{AttachmentRegion, ReportKey};
use tallyline_reporting::api::{AttachmentEdit, ReportingApplication};
use tallyline_shared_kernel::api::Clock;
use tallyline_shared_kernel::api::model::{TerminalId, Timestamp, TradeSide};

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_5_r_sub_7_r_nf_3__bounded_worker_prepares_and_encodes_durable_report_evidence() {
    struct Fixed;
    impl Clock for Fixed {
        fn now(&self) -> Timestamp {
            Timestamp::from_seconds(1000)
        }
    }
    let directory = std::env::temp_dir().join(format!(
        "tallyline-attachment-worker-{}",
        std::process::id()
    ));
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("source.png");
    let mut png = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png, 2000, 1200);
        encoder.set_color(png::ColorType::Rgb);
        let mut writer = encoder.write_header().unwrap();
        let mut pixels = vec![0; 2000 * 1200 * 3];
        for row in pixels
            .as_chunks_mut::<{ 2000 * 3 }>()
            .0
            .iter_mut()
            .take(100)
        {
            row.fill(255);
        }
        writer.write_image_data(&pixels).unwrap();
    }
    std::fs::write(&path, &png).unwrap();
    std::fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(
            std::fs::FileTimes::new()
                .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(900)),
        )
        .unwrap();
    let (started, blocked) = std::sync::mpsc::channel();
    let (release, continuing) = std::sync::mpsc::channel();
    let worker = AttachmentWorker::start(move || {
        started.send(()).unwrap();
        continuing.recv().unwrap_or(());
    })
    .unwrap();
    let queue_release = release;
    let region = AttachmentRegion::new(0, 100, 2000, 1100).unwrap();
    let preview = worker
        .prepare(path.clone(), region, ImageIntakeSettings::default())
        .unwrap()
        .recv()
        .unwrap()
        .unwrap();
    blocked.recv().unwrap();
    let queued = worker
        .prepare(path.clone(), region, ImageIntakeSettings::default())
        .unwrap();
    assert!(
        worker
            .prepare(path.clone(), region, ImageIntakeSettings::default())
            .is_err()
    );
    queue_release.send(()).unwrap();
    let _preview = queued.recv().unwrap().unwrap();
    blocked.recv().unwrap();
    let database = directory.join("application.db");
    let mut app = ReportingApplication::new(SqliteReferenceCache::open(&database).unwrap(), Fixed);
    let report = app
        .create(
            ReportKey::new(TerminalId(11), TradeSide::Buy, GameEnvironment::Live).unwrap(),
            Some("4.10.2"),
        )
        .unwrap();
    let report = app
        .edit_attachment(
            report.id(),
            report.version(),
            AttachmentEdit::Select(preview.binding(), preview.time()),
        )
        .unwrap();
    let report = app
        .edit_attachment(
            report.id(),
            report.version(),
            AttachmentEdit::ConfirmPreview(preview.binding()),
        )
        .unwrap();
    let report = app
        .edit_attachment(
            report.id(),
            report.version(),
            AttachmentEdit::ConfirmTime(preview.time().at()),
        )
        .unwrap();
    drop(app);
    std::fs::remove_file(&path).unwrap();
    let mut app = ReportingApplication::new(SqliteReferenceCache::open(&database).unwrap(), Fixed);
    let report = app
        .drafts()
        .unwrap()
        .into_iter()
        .find(|r| r.id() == report.id())
        .unwrap();
    let pending = worker
        .encode(report.clone(), preview, Default::default())
        .unwrap();
    let mut current = report.clone();
    let mut changed = Vec::new();
    for binding in [
        tallyline_reporting::api::model::AttachmentBinding::new(
            current.attachment().unwrap().binding().image(),
            AttachmentRegion::new(0, 101, 2000, 1099).unwrap(),
        ),
        tallyline_reporting::api::model::AttachmentBinding::new(
            tallyline_reporting::api::model::ImageHash::new([8; 32]),
            region,
        ),
    ] {
        current = app
            .edit_attachment(
                current.id(),
                current.version(),
                AttachmentEdit::Select(
                    binding,
                    tallyline_reporting::api::model::AttachmentTime::ImportedFile(
                        Timestamp::from_seconds(950),
                    ),
                ),
            )
            .unwrap();
        current = app
            .edit_attachment(
                current.id(),
                current.version(),
                AttachmentEdit::ConfirmPreview(binding),
            )
            .unwrap();
        current = app
            .edit_attachment(
                current.id(),
                current.version(),
                AttachmentEdit::ConfirmTime(Timestamp::from_seconds(900)),
            )
            .unwrap();
        assert!(current.attachment_gate().is_ok());
        changed.push(current.clone());
    }
    current = app
        .edit_attachment(current.id(), current.version(), AttachmentEdit::Remove)
        .unwrap();
    changed.push(current.clone());
    queue_release.send(()).unwrap();
    let encoded = pending.recv().unwrap().unwrap();
    for current in changed {
        assert!(!encoded.is_current(&current));
        assert!(encoded.png_for(&current).is_err());
        assert!(encoded.base64_for(&current).is_err());
    }
    assert!(
        encoded
            .png_for(&app.draft(current.id()).unwrap().unwrap())
            .is_err()
    );
    blocked.recv().unwrap();
    queue_release.send(()).unwrap();
    let tallyline_adapter_files::api::model::ImageIntake::Decoded(image) =
        tallyline_adapter_files::api::read_image(
            encoded.png_for(&report).unwrap(),
            &ImageIntakeSettings::default(),
        )
    else {
        panic!("invalid PNG")
    };
    assert_eq!((image.raster.width(), image.raster.height()), (1920, 1056));
    assert!(
        image
            .raster
            .argb()
            .iter()
            .all(|pixel| *pixel == 0xff00_0000)
    );
    assert!(encoded.base64_for(&report).unwrap().len() <= 10_000_000);
    worker.finish().unwrap();
    drop(app);
    std::fs::remove_dir_all(directory).unwrap();
}
