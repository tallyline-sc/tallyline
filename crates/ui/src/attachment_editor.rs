// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Explicit private attachment commands and stale-result filtering.

use tallyline_reporting::api::AttachmentEdit;
use tallyline_reporting::api::model::{AttachmentRegion, ReportId};
use tallyline_submission::api::model::{AccountState, ScreenshotPolicy, StorageFailure};
use tallyline_workflows::api::{
    ManualAttachmentBlock, ManualAttachmentFailure, ManualAttachmentPreview,
};

/// One explicit attachment request bound to the version visible in the editor.
#[derive(Clone, Debug)]
pub struct ManualAttachmentCommand {
    /// Report selected by the user.
    pub id: ReportId,
    /// Version shown when the command was requested.
    pub expected: u64,
    /// Local work or explicit evidence decision.
    pub action: ManualAttachmentAction,
}
/// Local attachment actions; none performs a submission or automatic import.
#[derive(Clone, Debug)]
pub enum ManualAttachmentAction {
    /// Read one explicitly selected local source and prepare only its selected rectangle.
    Prepare {
        /// User-supplied local filename; never persisted in Report metadata.
        source: String,
        /// Explicit source-pixel rectangle excluding the terminal header.
        region: AttachmentRegion,
    },
    /// Save only this reviewed private region and bind it to the current Report.
    ConfirmPreview(ManualAttachmentPreview),
    /// Persist a time decision or remove/change the current evidence.
    Edit(AttachmentEdit),
    /// Restore and view only the durable private crop.
    View,
    /// Read current account policy and validate current durable evidence.
    RefreshPolicy(AccountState),
    /// Explicitly clear only the accepted account's requirement memory.
    TryWithoutScreenshot(AccountState),
}

#[derive(Default)]
pub(crate) struct AttachmentEditor {
    pub(crate) source: String,
    pub(crate) rectangle: [u32; 4],
    pub(crate) time: String,
    pub(crate) preview: Option<ManualAttachmentPreview>,
    pub(crate) preview_report: Option<ReportId>,
    pub(crate) texture: Option<egui::TextureHandle>,
    pub(crate) policy: Option<(AccountState, Result<ScreenshotPolicy, StorageFailure>)>,
    pub(crate) readiness: Option<Result<(), ManualAttachmentBlock>>,
    pub(crate) failure: Option<ManualAttachmentFailure>,
}
impl AttachmentEditor {
    pub(crate) fn clear_preview(&mut self) {
        self.preview = None;
        self.texture = None;
        self.preview_report = None;
        self.readiness = None;
    }
}
impl crate::manual_editor::ManualEditor {
    /// Returns the currently displayed private region, with no original screenshot or file reference.
    #[must_use]
    pub fn attachment_preview(&self) -> Option<&ManualAttachmentPreview> {
        self.attachment.preview.as_ref()
    }
    pub(crate) fn preview_received(
        &mut self,
        id: ReportId,
        expected: u64,
        preview: ManualAttachmentPreview,
    ) {
        self.finish_attachment_request(id, expected);
        if self.selected().is_some_and(|report| {
            report.id() == id
                && report.version() == expected
                && report
                    .attachment()
                    .is_some_and(|value| value.binding() == preview.binding())
        }) {
            self.attachment.time = self
                .selected()
                .map(|report| report.observed_at().seconds().to_string())
                .unwrap_or_default();
            self.attachment.preview = Some(preview);
            self.attachment.preview_report = Some(id);
            self.attachment.texture = None;
            self.attachment.failure = None;
        }
    }
    pub(crate) fn attachment_status(
        &mut self,
        id: ReportId,
        expected: u64,
        account: AccountState,
        policy: Result<ScreenshotPolicy, StorageFailure>,
        readiness: Result<(), ManualAttachmentBlock>,
    ) {
        self.finish_attachment_request(id, expected);
        if self
            .selected()
            .is_some_and(|report| report.id() == id && report.version() == expected)
        {
            self.attachment.policy = Some((account, policy));
            self.attachment.readiness = Some(readiness);
            self.attachment.failure = None;
        }
    }
    pub(crate) fn attachment_failed(
        &mut self,
        id: ReportId,
        expected: u64,
        failure: ManualAttachmentFailure,
    ) {
        let pending = self.finish_attachment_request(id, expected);
        if self
            .selected()
            .is_some_and(|report| report.id() == id && (pending || report.version() == expected))
        {
            self.attachment.failure = Some(failure);
        }
    }
}
