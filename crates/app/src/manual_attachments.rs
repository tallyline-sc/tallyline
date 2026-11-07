// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Local attachment workflow composition on the bounded manual worker.

use std::path::PathBuf;
use tallyline_adapter_files::api::model::{
    AttachmentImageError, Encoding, ImageIntakeSettings, SetAsideReason,
};
use tallyline_adapter_files::api::{
    ManualWorkingCopies, prepare_manual_attachment_file, restore_manual_attachment,
};
use tallyline_reporting::api::{
    AttachmentEdit, ReportRepository, ReportingApplication, RepositoryError,
};
use tallyline_shared_kernel::api::Clock;
use tallyline_submission::api::ScreenshotPolicyApplication;
use tallyline_ui::api::model::{BackgroundEvent, ManualAttachmentAction, ManualAttachmentCommand};
use tallyline_workflows::api::{
    ManualAttachmentFailure, ManualAttachmentInputFailure, ManualAttachmentPreview,
    WorkingCopyFailure, confirm_manual_attachment, manual_attachment_readiness,
};

/// Local application data location for confirmed private crops and account policy.
pub struct ManualAttachmentStorage {
    data: PathBuf,
}
impl ManualAttachmentStorage {
    /// Retains the documented application data directory; operations refuse a relative location.
    #[must_use]
    pub const fn new(data: PathBuf) -> Self {
        Self { data }
    }
}
pub(crate) fn execute<R: ReportRepository, C: Clock>(
    application: &mut ReportingApplication<R, C>,
    command: ManualAttachmentCommand,
    storage: Option<&ManualAttachmentStorage>,
) -> Result<Vec<BackgroundEvent>, ManualAttachmentFailure> {
    let storage = storage.filter(|storage| storage.data.is_absolute()).ok_or(
        ManualAttachmentFailure::WorkingCopy(WorkingCopyFailure::Unavailable),
    )?;
    let report = application
        .draft(command.id)
        .map_err(ManualAttachmentFailure::Report)?
        .ok_or(ManualAttachmentFailure::Report(RepositoryError::Invalid))?;
    if report.version() != command.expected {
        return Err(ManualAttachmentFailure::Report(RepositoryError::Conflict));
    }
    let mut events = Vec::new();
    match command.action {
        ManualAttachmentAction::Prepare { source, region } => {
            let preview = prepare_manual_attachment_file(
                std::path::Path::new(&source),
                region,
                &ImageIntakeSettings::default(),
            )
            .map_err(input_failure)?;
            let report = application
                .edit_attachment(
                    command.id,
                    command.expected,
                    AttachmentEdit::Select(preview.binding(), preview.time()),
                )
                .map_err(ManualAttachmentFailure::Report)?;
            let preview = ManualAttachmentPreview::new(
                preview.binding(),
                preview.time(),
                preview.raster().clone(),
            )
            .map_err(ManualAttachmentFailure::WorkingCopy)?;
            events.push(BackgroundEvent::ReportsLoaded {
                reports: application
                    .drafts()
                    .map_err(ManualAttachmentFailure::Report)?,
                selected: None,
            });
            events.push(BackgroundEvent::AttachmentPreview {
                id: report.id(),
                expected: report.version(),
                preview,
            });
        }
        ManualAttachmentAction::ConfirmPreview(preview) => {
            let mut copies = ManualWorkingCopies::new(storage.data.join("manual-attachments"))
                .map_err(ManualAttachmentFailure::WorkingCopy)?;
            confirm_manual_attachment(
                application,
                &mut copies,
                command.id,
                command.expected,
                &preview,
            )?;
            events.push(BackgroundEvent::ReportsLoaded {
                reports: application
                    .drafts()
                    .map_err(ManualAttachmentFailure::Report)?,
                selected: None,
            });
        }
        ManualAttachmentAction::Edit(edit) => {
            application
                .edit_attachment(command.id, command.expected, edit)
                .map_err(ManualAttachmentFailure::Report)?;
            events.push(BackgroundEvent::ReportsLoaded {
                reports: application
                    .drafts()
                    .map_err(ManualAttachmentFailure::Report)?,
                selected: None,
            });
        }
        ManualAttachmentAction::View => {
            let mut copies = ManualWorkingCopies::new(storage.data.join("manual-attachments"))
                .map_err(ManualAttachmentFailure::WorkingCopy)?;
            let preview = restore_manual_attachment(&report, &mut copies)
                .map_err(ManualAttachmentFailure::WorkingCopy)?;
            let preview = ManualAttachmentPreview::new(
                preview.binding(),
                preview.time(),
                preview.raster().clone(),
            )
            .map_err(ManualAttachmentFailure::WorkingCopy)?;
            events.push(BackgroundEvent::AttachmentPreview {
                id: report.id(),
                expected: report.version(),
                preview,
            });
        }
        action @ (ManualAttachmentAction::RefreshPolicy(_)
        | ManualAttachmentAction::TryWithoutScreenshot(_)) => {
            let (account, clear) = match action {
                ManualAttachmentAction::RefreshPolicy(account) => (account, false),
                ManualAttachmentAction::TryWithoutScreenshot(account) => (account, true),
                ManualAttachmentAction::Prepare { .. }
                | ManualAttachmentAction::ConfirmPreview(_)
                | ManualAttachmentAction::Edit(_)
                | ManualAttachmentAction::View => {
                    return Err(ManualAttachmentFailure::Report(RepositoryError::Invalid));
                }
            };
            let policy = tallyline_adapter_storage::api::SqliteReferenceCache::open(
                storage.data.join("tallyline.db"),
            )
            .map(|store| {
                let mut application = ScreenshotPolicyApplication::new(store);
                if clear {
                    application.try_without_screenshot(&account)?;
                }
                application.current(&account)
            })
            .unwrap_or(Err(
                tallyline_submission::api::model::StorageFailure::Unavailable,
            ));
            let readiness = match ManualWorkingCopies::new(storage.data.join("manual-attachments"))
            {
                Ok(mut copies) => manual_attachment_readiness(&report, policy, &mut copies),
                Err(_) => Err(
                    tallyline_workflows::api::ManualAttachmentBlock::WorkingCopyMissingOrChanged,
                ),
            };
            events.push(BackgroundEvent::AttachmentStatus {
                id: report.id(),
                expected: report.version(),
                account,
                policy,
                readiness,
            });
        }
    }
    Ok(events)
}
fn input_failure(error: AttachmentImageError) -> ManualAttachmentFailure {
    let failure = match error {
        AttachmentImageError::FileUnavailable => ManualAttachmentInputFailure::Unavailable,
        AttachmentImageError::ReducedSource => ManualAttachmentInputFailure::SourceReduced,
        AttachmentImageError::InvalidRegion | AttachmentImageError::Unconfirmed => {
            ManualAttachmentInputFailure::InvalidRegion
        }
        AttachmentImageError::EncodingLimit => ManualAttachmentInputFailure::EncodingLimit,
        AttachmentImageError::UnsupportedImage(reason) => match reason {
            SetAsideReason::HdrFormat(_)
            | SetAsideReason::HdrOrUnsupportedEncoding(Encoding::CicpHdr { .. }) => {
                ManualAttachmentInputFailure::Hdr
            }
            SetAsideReason::HeaderTooLarge
            | SetAsideReason::MemoryLimit
            | SetAsideReason::TooManyPixels
            | SetAsideReason::ProfileTooLarge
            | SetAsideReason::FileTooLarge => ManualAttachmentInputFailure::InputLimit,
            SetAsideReason::HdrOrUnsupportedEncoding(_) | SetAsideReason::UnsupportedFormat => {
                ManualAttachmentInputFailure::Unsupported
            }
            SetAsideReason::DecodeFailed(_) | SetAsideReason::FileUnreadable(_) => {
                ManualAttachmentInputFailure::Unavailable
            }
        },
    };
    ManualAttachmentFailure::Input(failure)
}
