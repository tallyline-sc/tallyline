// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Connected private attachment workflow with real SQLite and files.

use tallyline_adapter_files::api::ManualWorkingCopies;
use tallyline_adapter_storage::api::SqliteReferenceCache;
use tallyline_game::api::model::GameEnvironment;
use tallyline_reporting::api::model::{
    AttachmentBinding, AttachmentRegion, AttachmentTime, ImageHash, ReportKey,
};
use tallyline_reporting::api::{AttachmentEdit, ReportingApplication};
use tallyline_shared_kernel::api::{
    Clock,
    model::{ImageRaster, TerminalId, Timestamp, TradeSide},
};
use tallyline_submission::api::ScreenshotPolicyApplication;
use tallyline_submission::api::model::{
    AccountState, Persistence, ScreenshotPolicy, ScreenshotRequirement,
};
use tallyline_workflows::api::{
    AttachmentRequest, ManualAttachmentBlock, ManualAttachmentPreview, confirm_manual_attachment,
    manual_attachment_readiness,
};
struct Fixed;
impl Clock for Fixed {
    fn now(&self) -> Timestamp {
        Timestamp::from_seconds(1000)
    }
}
fn account(name: &str) -> AccountState {
    AccountState::Validated {
        username: name.to_owned(),
        validated_at: Timestamp::from_seconds(1000),
        persistence: Persistence::Session,
    }
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_5_r_sub_7_r_val_6_q_manual__current_account_policy_and_current_durable_evidence_control_attachment_readiness()
 {
    let directory = std::env::temp_dir().join(format!(
        "tallyline-attachment-workflow-{}",
        std::process::id()
    ));
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("application.db");
    let mut app = ReportingApplication::new(SqliteReferenceCache::open(&path).unwrap(), Fixed);
    let mut policy = ScreenshotPolicyApplication::new(SqliteReferenceCache::open(&path).unwrap());
    let mut copies = ManualWorkingCopies::new(directory.join("working-copies")).unwrap();
    let mut report = app
        .create(
            ReportKey::new(TerminalId(11), TradeSide::Buy, GameEnvironment::Live).unwrap(),
            Some("4.10.2"),
        )
        .unwrap();
    assert_eq!(
        manual_attachment_readiness(&report, policy.current(&account("dummy-a")), &mut copies),
        Ok(())
    );
    assert_eq!(
        manual_attachment_readiness(
            &report,
            Err(tallyline_submission::api::model::StorageFailure::Unavailable),
            &mut copies
        ),
        Err(ManualAttachmentBlock::PolicyUnavailable)
    );
    policy
        .observe(
            &account("dummy-a"),
            ScreenshotRequirement::Required,
            Timestamp::from_seconds(990),
        )
        .unwrap();
    assert_eq!(
        manual_attachment_readiness(&report, policy.current(&account("dummy-a")), &mut copies),
        Err(ManualAttachmentBlock::ImageRequired)
    );
    assert_eq!(
        manual_attachment_readiness(&report, policy.current(&account("dummy-b")), &mut copies),
        Ok(())
    );
    let binding = AttachmentBinding::new(
        ImageHash::new([1; 32]),
        AttachmentRegion::new(0, 10, 2, 1).unwrap(),
    );
    report = app
        .edit_attachment(
            report.id(),
            report.version(),
            AttachmentEdit::Select(
                binding,
                AttachmentTime::ImportedFile(Timestamp::from_seconds(900)),
            ),
        )
        .unwrap();
    let preview = ManualAttachmentPreview::new(
        binding,
        report.attachment().unwrap().time(),
        ImageRaster::new(2, 1, vec![0xff00_c800; 2]).unwrap(),
    )
    .unwrap();
    assert_eq!(
        manual_attachment_readiness(&report, policy.current(&account("dummy-a")), &mut copies),
        Err(ManualAttachmentBlock::PreviewUnconfirmed)
    );
    let request = AttachmentRequest::new(&report).unwrap();
    report = confirm_manual_attachment(
        &mut app,
        &mut copies,
        report.id(),
        report.version(),
        &preview,
    )
    .unwrap();
    assert!(!request.is_current(&report));
    let mut same_version = tallyline_reporting::api::model::Report::manual(
        report.id(),
        report.key(),
        Timestamp::from_seconds(1000),
        Some("4.10.2"),
    )
    .unwrap();
    same_version
        .edit_attachment(AttachmentEdit::Select(
            AttachmentBinding::new(ImageHash::new([2; 32]), binding.region()),
            AttachmentTime::ImportedFile(Timestamp::from_seconds(900)),
        ))
        .unwrap();
    assert!(!request.is_current(&same_version));
    assert_eq!(
        manual_attachment_readiness(&report, policy.current(&account("dummy-a")), &mut copies),
        Err(ManualAttachmentBlock::TimeUncertain)
    );
    report = app
        .edit_attachment(
            report.id(),
            report.version(),
            AttachmentEdit::ConfirmTime(Timestamp::from_seconds(899)),
        )
        .unwrap();
    assert_eq!(
        manual_attachment_readiness(&report, policy.current(&account("dummy-a")), &mut copies),
        Ok(())
    );
    let verified = report.clone();
    let file = std::fs::read_dir(directory.join("working-copies"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let bytes = std::fs::read(&file).unwrap();
    std::fs::write(&file, b"changed private copy").unwrap();
    assert_eq!(
        manual_attachment_readiness(&report, policy.current(&account("dummy-a")), &mut copies),
        Err(ManualAttachmentBlock::WorkingCopyMissingOrChanged)
    );
    std::fs::write(&file, bytes).unwrap();
    assert!(
        AttachmentRequest::new(&report)
            .unwrap()
            .is_current(&verified)
    );
    let mut other = tallyline_reporting::api::model::Report::manual(
        tallyline_reporting::api::model::ReportId(report.id().0 + 1),
        report.key(),
        Timestamp::from_seconds(1000),
        Some("4.10.2"),
    )
    .unwrap();
    other
        .edit_attachment(AttachmentEdit::Select(
            binding,
            AttachmentTime::ImportedFile(Timestamp::from_seconds(900)),
        ))
        .unwrap();
    other
        .edit_attachment(AttachmentEdit::ConfirmPreview(binding))
        .unwrap();
    other
        .edit_attachment(AttachmentEdit::BindWorkingCopy(
            binding,
            report.attachment().unwrap().working_copy().unwrap(),
        ))
        .unwrap();
    other
        .edit_attachment(AttachmentEdit::ConfirmTime(Timestamp::from_seconds(899)))
        .unwrap();
    assert_eq!(other.version(), report.version());
    assert!(!AttachmentRequest::new(&other).unwrap().is_current(&report));
    let expected = report.version();
    report = app
        .edit_attachment(report.id(), expected, AttachmentEdit::Remove)
        .unwrap();
    assert!(
        confirm_manual_attachment(&mut app, &mut copies, report.id(), expected, &preview).is_err()
    );
    assert!(
        confirm_manual_attachment(
            &mut app,
            &mut copies,
            report.id(),
            report.version(),
            &preview
        )
        .is_err()
    );
    policy
        .observe(
            &account("dummy-a"),
            ScreenshotRequirement::Required,
            Timestamp::from_seconds(900),
        )
        .unwrap();
    drop(app);
    drop(policy);
    let mut policy = ScreenshotPolicyApplication::new(SqliteReferenceCache::open(&path).unwrap());
    assert_eq!(
        policy.current(&account("dummy-a")).unwrap(),
        ScreenshotPolicy::Required {
            observed_at: Timestamp::from_seconds(990)
        }
    );
    policy.try_without_screenshot(&account("dummy-b")).unwrap();
    assert!(matches!(
        policy.current(&account("dummy-a")).unwrap(),
        ScreenshotPolicy::Required { .. }
    ));
    policy.try_without_screenshot(&account("dummy-a")).unwrap();
    assert_eq!(
        manual_attachment_readiness(&report, policy.current(&account("dummy-a")), &mut copies),
        Ok(())
    );
    drop(policy);
    drop(copies);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_5_r_sub_7_r_val_6_q_manual__private_copy_failure_and_a_locked_database_never_publish_confirmed_metadata()
 {
    use tallyline_reporting::api::RepositoryError;
    use tallyline_workflows::api::{ManualAttachmentFailure, WorkingCopyFailure, WorkingCopyStore};
    struct LockedAfterCopy {
        copies: ManualWorkingCopies,
        lock: rusqlite::Connection,
    }
    impl WorkingCopyStore for LockedAfterCopy {
        fn save(
            &mut self,
            report: &tallyline_reporting::api::model::Report,
            raster: &ImageRaster,
        ) -> Result<ImageHash, WorkingCopyFailure> {
            let digest = self.copies.save(report, raster)?;
            self.lock.execute_batch("BEGIN IMMEDIATE").unwrap();
            Ok(digest)
        }
        fn load(
            &mut self,
            report: &tallyline_reporting::api::model::Report,
        ) -> Result<ImageRaster, WorkingCopyFailure> {
            self.copies.load(report)
        }
    }
    struct Unavailable;
    impl WorkingCopyStore for Unavailable {
        fn save(
            &mut self,
            _: &tallyline_reporting::api::model::Report,
            _: &ImageRaster,
        ) -> Result<ImageHash, WorkingCopyFailure> {
            Err(WorkingCopyFailure::Unavailable)
        }
        fn load(
            &mut self,
            _: &tallyline_reporting::api::model::Report,
        ) -> Result<ImageRaster, WorkingCopyFailure> {
            Err(WorkingCopyFailure::Unavailable)
        }
    }
    let directory = std::env::temp_dir().join(format!(
        "tallyline-attachment-failure-{}",
        std::process::id()
    ));
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("application.db");
    let mut app = ReportingApplication::new(SqliteReferenceCache::open(&path).unwrap(), Fixed);
    let mut report = app
        .create(
            ReportKey::new(TerminalId(11), TradeSide::Buy, GameEnvironment::Live).unwrap(),
            Some("4.10.2"),
        )
        .unwrap();
    assert_eq!(AttachmentRequest::new(&report), None);
    let binding = AttachmentBinding::new(
        ImageHash::new([4; 32]),
        AttachmentRegion::new(1, 10, 2, 1).unwrap(),
    );
    report = app
        .edit_attachment(
            report.id(),
            report.version(),
            AttachmentEdit::Select(
                binding,
                AttachmentTime::ImportedFile(Timestamp::from_seconds(900)),
            ),
        )
        .unwrap();
    for (width, height) in [(3, 1), (2, 2)] {
        assert!(
            ManualAttachmentPreview::new(
                binding,
                report.attachment().unwrap().time(),
                ImageRaster::new(width, height, vec![0; (width * height) as usize]).unwrap()
            )
            .is_err()
        );
    }
    let preview = ManualAttachmentPreview::new(
        binding,
        report.attachment().unwrap().time(),
        ImageRaster::new(2, 1, vec![0xff00_c800; 2]).unwrap(),
    )
    .unwrap();
    assert_eq!(
        preview.time(),
        AttachmentTime::ImportedFile(Timestamp::from_seconds(900))
    );
    assert_eq!(
        confirm_manual_attachment(
            &mut app,
            &mut Unavailable,
            report.id(),
            report.version(),
            &preview
        ),
        Err(ManualAttachmentFailure::WorkingCopy(
            WorkingCopyFailure::Unavailable
        ))
    );
    assert_eq!(app.draft(report.id()).unwrap(), Some(report.clone()));
    let mut locked = LockedAfterCopy {
        copies: ManualWorkingCopies::new(directory.join("working-copies")).unwrap(),
        lock: rusqlite::Connection::open(&path).unwrap(),
    };
    assert_eq!(
        confirm_manual_attachment(
            &mut app,
            &mut locked,
            report.id(),
            report.version(),
            &preview
        ),
        Err(ManualAttachmentFailure::Report(
            RepositoryError::Unavailable
        ))
    );
    locked.lock.execute_batch("ROLLBACK").unwrap();
    assert_eq!(app.draft(report.id()).unwrap(), Some(report.clone()));
    let changed = AttachmentBinding::new(ImageHash::new([5; 32]), binding.region());
    report = app
        .edit_attachment(
            report.id(),
            report.version(),
            AttachmentEdit::Select(
                changed,
                AttachmentTime::ImportedFile(Timestamp::from_seconds(950)),
            ),
        )
        .unwrap();
    assert!(
        confirm_manual_attachment(
            &mut app,
            &mut locked.copies,
            report.id(),
            report.version(),
            &preview
        )
        .is_err()
    );
    assert_eq!(app.draft(report.id()).unwrap(), Some(report));
    drop(locked);
    drop(app);
    std::fs::remove_dir_all(directory).unwrap();
}
