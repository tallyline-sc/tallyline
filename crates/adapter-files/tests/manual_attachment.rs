// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Real-directory manual privacy preview and upload contracts.

use tallyline_adapter_files::api::model::{
    AttachmentEncodingSettings, ImageIntake, ImageIntakeSettings,
};
use tallyline_adapter_files::api::{encode_manual_attachment, prepare_manual_attachment_file};
use tallyline_game::api::model::GameEnvironment;
use tallyline_reporting::api::AttachmentEdit;
use tallyline_reporting::api::model::{AttachmentRegion, Report, ReportId, ReportKey};
use tallyline_shared_kernel::api::model::{TerminalId, Timestamp, TradeSide};

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_5_r_sub_7_r_cap_8__only_the_exact_confirmed_region_can_be_encoded() {
    let directory =
        std::env::temp_dir().join(format!("tallyline-private-region-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("capture.not-an-image-extension");
    let mut source = image::RgbImage::from_pixel(64, 48, image::Rgb([0, 200, 0]));
    for y in 0..12 {
        for x in 0..64 {
            source.put_pixel(x, y, image::Rgb([255, 0, 255]));
        }
    }
    source
        .save_with_format(&path, image::ImageFormat::Png)
        .unwrap();
    let region = AttachmentRegion::new(8, 16, 32, 24).unwrap();
    let preview =
        prepare_manual_attachment_file(&path, region, &ImageIntakeSettings::default()).unwrap();
    assert_eq!(
        (preview.raster().width(), preview.raster().height()),
        (32, 24)
    );
    assert!(preview.raster().argb().iter().all(|p| *p == 0xff00_c800));
    let mut report = Report::manual(
        ReportId(1),
        ReportKey::new(TerminalId(11), TradeSide::Buy, GameEnvironment::Live).unwrap(),
        Timestamp::from_seconds(u64::MAX),
        Some("4.10.2"),
    )
    .unwrap();
    report
        .edit_attachment(AttachmentEdit::Select(preview.binding(), preview.time()))
        .unwrap();
    assert!(
        encode_manual_attachment(&report, &preview, &AttachmentEncodingSettings::default())
            .is_err()
    );
    report
        .edit_attachment(AttachmentEdit::ConfirmPreview(preview.binding()))
        .unwrap();
    report
        .edit_attachment(AttachmentEdit::ConfirmTime(preview.time().at()))
        .unwrap();
    let encoded =
        encode_manual_attachment(&report, &preview, &AttachmentEncodingSettings::default())
            .unwrap();
    let ImageIntake::Decoded(decoded) = tallyline_adapter_files::api::read_image(
        encoded.png_for(&report).unwrap(),
        &ImageIntakeSettings::default(),
    ) else {
        panic!("encoded crop is not a PNG")
    };
    assert_eq!(decoded.raster, *preview.raster());
    assert!(encoded.base64_for(&report).unwrap().len() <= 10_000_000);
    assert!(!format!("{encoded:?}").contains(encoded.base64_for(&report).unwrap()));
    assert!(
        encode_manual_attachment(
            &report,
            &preview,
            &AttachmentEncodingSettings::new(4).unwrap()
        )
        .is_err()
    );
    let changed_region = prepare_manual_attachment_file(
        &path,
        AttachmentRegion::new(9, 16, 32, 24).unwrap(),
        &ImageIntakeSettings::default(),
    )
    .unwrap();
    assert!(
        encode_manual_attachment(
            &report,
            &changed_region,
            &AttachmentEncodingSettings::default()
        )
        .is_err()
    );
    source.put_pixel(63, 0, image::Rgb([0, 0, 255]));
    source
        .save_with_format(&path, image::ImageFormat::Png)
        .unwrap();
    let changed_content =
        prepare_manual_attachment_file(&path, region, &ImageIntakeSettings::default()).unwrap();
    assert!(
        encode_manual_attachment(
            &report,
            &changed_content,
            &AttachmentEncodingSettings::default()
        )
        .is_err()
    );
    assert_eq!(changed_content.raster(), preview.raster());
    assert!(
        prepare_manual_attachment_file(
            &path,
            AttachmentRegion::new(63, 40, 2, 2).unwrap(),
            &ImageIntakeSettings::default()
        )
        .is_err()
    );
    assert!(
        prepare_manual_attachment_file(
            &path,
            region,
            &ImageIntakeSettings::new(200 * 1024 * 1024, 100_000_000, 1024 * 1024, 1).unwrap()
        )
        .is_err()
    );
    std::fs::write(&path, [0x76, 0x2f, 0x31, 0x01]).unwrap();
    assert_eq!(
        prepare_manual_attachment_file(&path, region, &ImageIntakeSettings::default()).unwrap_err(),
        tallyline_adapter_files::api::model::AttachmentImageError::UnsupportedImage(
            tallyline_adapter_files::api::model::SetAsideReason::HdrFormat(
                tallyline_adapter_files::api::model::HdrContainer::OpenExr
            )
        )
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_5_r_sub_7_r_cap_8__jpeg_magic_and_imported_time_are_preserved_without_original_storage() {
    let directory =
        std::env::temp_dir().join(format!("tallyline-private-jpeg-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("capture.png");
    let mut bytes = Vec::new();
    let mut pixels = vec![80; 64 * 48 * 3];
    for row in pixels.as_chunks_mut::<{ 64 * 3 }>().0.iter_mut().take(12) {
        row.fill(240);
    }
    jpeg_encoder::Encoder::new(&mut bytes, 100)
        .encode(&pixels, 64, 48, jpeg_encoder::ColorType::Rgb)
        .unwrap();
    std::fs::write(&path, bytes).unwrap();
    std::fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(
            std::fs::FileTimes::new()
                .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(900)),
        )
        .unwrap();
    let preview = prepare_manual_attachment_file(
        &path,
        AttachmentRegion::new(4, 20, 40, 24).unwrap(),
        &ImageIntakeSettings::default(),
    )
    .unwrap();
    assert_eq!(
        preview.time(),
        tallyline_reporting::api::model::AttachmentTime::ImportedFile(Timestamp::from_seconds(900))
    );
    assert!(
        preview
            .raster()
            .argb()
            .iter()
            .all(|pixel| *pixel == 0xff50_5050)
    );
    let mut report = Report::manual(
        ReportId(1),
        ReportKey::new(TerminalId(11), TradeSide::Buy, GameEnvironment::Live).unwrap(),
        Timestamp::from_seconds(1000),
        Some("4.10.2"),
    )
    .unwrap();
    report
        .edit_attachment(AttachmentEdit::Select(preview.binding(), preview.time()))
        .unwrap();
    report
        .edit_attachment(AttachmentEdit::ConfirmPreview(preview.binding()))
        .unwrap();
    report
        .edit_attachment(AttachmentEdit::ConfirmTime(Timestamp::from_seconds(899)))
        .unwrap();
    std::fs::remove_file(&path).unwrap();
    let encoded =
        encode_manual_attachment(&report, &preview, &AttachmentEncodingSettings::default())
            .unwrap();
    let ImageIntake::Decoded(decoded) = tallyline_adapter_files::api::read_image(
        encoded.png_for(&report).unwrap(),
        &ImageIntakeSettings::default(),
    ) else {
        panic!("encoded JPEG region is invalid")
    };
    assert_eq!(decoded.raster, *preview.raster());
    assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 0);
    assert_eq!(report.observed_at(), Timestamp::from_seconds(899));
    std::fs::remove_dir(directory).unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_5_r_sub_7__confirmed_private_working_copy_survives_source_deletion_and_detects_changes() {
    use tallyline_adapter_files::api::{ManualWorkingCopies, restore_manual_attachment};
    use tallyline_workflows::api::WorkingCopyStore;
    let directory =
        std::env::temp_dir().join(format!("tallyline-durable-crop-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let source = directory.join("original.png");
    let mut image = image::RgbImage::from_pixel(64, 48, image::Rgb([0, 200, 0]));
    for y in 0..12 {
        for x in 0..64 {
            image.put_pixel(x, y, image::Rgb([255, 0, 255]));
        }
    }
    image.save(&source).unwrap();
    let preview = prepare_manual_attachment_file(
        &source,
        AttachmentRegion::new(0, 12, 64, 36).unwrap(),
        &ImageIntakeSettings::default(),
    )
    .unwrap();
    let mut report = Report::manual(
        ReportId(91),
        ReportKey::new(TerminalId(11), TradeSide::Buy, GameEnvironment::Live).unwrap(),
        Timestamp::from_seconds(u64::MAX),
        Some("4.10.2"),
    )
    .unwrap();
    report
        .edit_attachment(AttachmentEdit::Select(preview.binding(), preview.time()))
        .unwrap();
    let mut store = ManualWorkingCopies::new(directory.join("working-copies")).unwrap();
    assert!(store.save(&report, preview.raster()).is_err());
    report
        .edit_attachment(AttachmentEdit::ConfirmPreview(preview.binding()))
        .unwrap();
    let digest = store.save(&report, preview.raster()).unwrap();
    report
        .edit_attachment(AttachmentEdit::BindWorkingCopy(preview.binding(), digest))
        .unwrap();
    report
        .edit_attachment(AttachmentEdit::ConfirmTime(preview.time().at()))
        .unwrap();
    std::fs::remove_file(source).unwrap();
    drop(store);
    let mut store = ManualWorkingCopies::new(directory.join("working-copies")).unwrap();
    let restored = restore_manual_attachment(&report, &mut store).unwrap();
    assert_eq!(restored.raster(), preview.raster());
    let encoded =
        encode_manual_attachment(&report, &restored, &AttachmentEncodingSettings::default())
            .unwrap();
    assert!(encoded.is_current(&report));
    let mut removed = report.clone();
    removed.edit_attachment(AttachmentEdit::Remove).unwrap();
    assert!(!encoded.is_current(&removed));
    assert!(encoded.png_for(&removed).is_err());
    assert!(encoded.base64_for(&removed).is_err());
    let files: Vec<_> = std::fs::read_dir(directory.join("working-copies"))
        .unwrap()
        .map(|v| v.unwrap().path())
        .collect();
    assert_eq!(files.len(), 1);
    let abandoned = files[0].with_extension("pending");
    std::fs::rename(&files[0], &abandoned).unwrap();
    assert!(restore_manual_attachment(&report, &mut store).is_err());
    assert_eq!(store.save(&report, preview.raster()).unwrap(), digest);
    assert_eq!(
        restore_manual_attachment(&report, &mut store)
            .unwrap()
            .raster(),
        preview.raster()
    );
    assert!(abandoned.exists());

    let mut bytes = std::fs::read(&files[0]).unwrap();
    assert!(!bytes.windows(4).any(|v| v == [255, 255, 0, 255]));
    let last = bytes.last_mut().unwrap();
    *last ^= 1;
    std::fs::write(&files[0], bytes).unwrap();
    assert!(restore_manual_attachment(&report, &mut store).is_err());
    assert_eq!(store.save(&report, preview.raster()).unwrap(), digest);
    assert_eq!(
        restore_manual_attachment(&report, &mut store)
            .unwrap()
            .raster(),
        preview.raster()
    );
    std::fs::remove_file(&files[0]).unwrap();
    assert!(restore_manual_attachment(&report, &mut store).is_err());
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_4_r_sub_7_r_nf_9__split_metadata_reuses_exact_private_copy_and_detaching_either_report_retains_it()
 {
    use tallyline_adapter_files::api::{ManualWorkingCopies, restore_manual_attachment};
    use tallyline_reporting::api::model::{
        AttachmentBinding, AttachmentTime, ImageHash, StoredAttachment, StoredReport,
        StoredReportLifecycle,
    };
    use tallyline_shared_kernel::api::model::ImageRaster;
    use tallyline_workflows::api::WorkingCopyStore;
    let root = std::env::temp_dir().join(format!(
        "tallyline-split-private-copy-{}",
        std::process::id()
    ));
    std::fs::create_dir(&root).unwrap();
    let mut copies = ManualWorkingCopies::new(root.join("private")).unwrap();
    let mut source = Report::manual(
        ReportId(1),
        ReportKey::new(TerminalId(11), TradeSide::Buy, GameEnvironment::Live).unwrap(),
        Timestamp::from_seconds(900),
        Some("4.10.2"),
    )
    .unwrap();
    let binding = AttachmentBinding::new(
        ImageHash::new([1; 32]),
        AttachmentRegion::new(0, 0, 2, 2).unwrap(),
    );
    source
        .edit_attachment(AttachmentEdit::Select(
            binding,
            AttachmentTime::User(Timestamp::from_seconds(900)),
        ))
        .unwrap();
    source
        .edit_attachment(AttachmentEdit::ConfirmPreview(binding))
        .unwrap();
    let raster = ImageRaster::new(2, 2, vec![0xff00_c800; 4]).unwrap();
    let digest = copies.save(&source, &raster).unwrap();
    source
        .edit_attachment(AttachmentEdit::BindWorkingCopy(binding, digest))
        .unwrap();
    let evidence = source.attachment().unwrap();
    let stored = StoredReport {
        id: ReportId(2),
        key: source.key(),
        observed_at: source.observed_at(),
        game_version: Some("4.10.2".to_owned()),
        faction_affinity: None,
        version: source.version(),
        rows: vec![],
        lifecycle: StoredReportLifecycle::default(),
        attachment: Some(StoredAttachment {
            binding: evidence.binding(),
            time: evidence.time(),
            preview_confirmed: evidence.preview_confirmed(),
            working_copy: evidence.working_copy(),
            working_copy_owner: evidence.working_copy_owner(),
        }),
    };
    let target = Report::reconstitute(stored.clone()).unwrap();
    assert_eq!(
        target.attachment().unwrap().working_copy_owner(),
        Some(source.id())
    );
    let mut legacy = stored.clone();
    legacy.id = source.id();
    legacy.attachment.as_mut().unwrap().working_copy_owner = None;
    assert_eq!(
        restore_manual_attachment(&Report::reconstitute(legacy).unwrap(), &mut copies)
            .unwrap()
            .raster(),
        &raster
    );
    let mut unbound = stored.clone();
    unbound.attachment.as_mut().unwrap().working_copy_owner = None;
    assert!(
        restore_manual_attachment(&Report::reconstitute(unbound).unwrap(), &mut copies).is_err()
    );
    let mut invalid = stored;
    invalid.attachment.as_mut().unwrap().working_copy_owner = Some(ReportId(0));
    assert!(Report::reconstitute(invalid).is_err());
    assert_eq!(
        restore_manual_attachment(&target, &mut copies)
            .unwrap()
            .raster(),
        &raster
    );
    assert_eq!(std::fs::read_dir(root.join("private")).unwrap().count(), 1);
    let preview = restore_manual_attachment(&target, &mut copies).unwrap();
    let encoded =
        encode_manual_attachment(&target, &preview, &AttachmentEncodingSettings::default())
            .unwrap();
    assert!(encoded.base64_for(&target).is_ok());
    let mut changed = target.clone();
    changed.edit_attachment(AttachmentEdit::Remove).unwrap();
    assert_eq!(
        restore_manual_attachment(&source, &mut copies)
            .unwrap()
            .raster(),
        &raster
    );
    changed
        .edit_attachment(AttachmentEdit::Select(
            AttachmentBinding::new(ImageHash::new([2; 32]), binding.region()),
            evidence.time(),
        ))
        .unwrap();
    assert_eq!(
        restore_manual_attachment(&source, &mut copies)
            .unwrap()
            .raster(),
        &raster
    );
    let preserved_source = source.clone();
    source.edit_attachment(AttachmentEdit::Remove).unwrap();
    assert_eq!(
        restore_manual_attachment(&target, &mut copies)
            .unwrap()
            .raster(),
        &raster
    );
    let artifact = std::fs::read_dir(root.join("private"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let mut changed_bytes = std::fs::read(&artifact).unwrap();
    *changed_bytes.first_mut().unwrap() ^= 1;
    std::fs::write(artifact, changed_bytes).unwrap();
    assert!(restore_manual_attachment(&target, &mut copies).is_err());
    assert!(restore_manual_attachment(&preserved_source, &mut copies).is_err());
    drop(copies);
    std::fs::remove_dir_all(&root).unwrap();
    assert!(!root.exists());
}
