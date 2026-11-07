// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Attachment commands through the Report aggregate.

use super::Report;
use crate::api::AttachmentEdit;
use crate::api::model::{AttachmentBlock, AttachmentTime, InvalidReport, ManualAttachment};

impl Report {
    /// Applies one explicit attachment decision without refreshing observation time.
    ///
    /// # Errors
    /// Refuses stale preview bindings and observation times later than the shown time.
    pub fn edit_attachment(&mut self, edit: AttachmentEdit) -> Result<(), InvalidReport> {
        self.ensure_draft()?;
        let previous_time = self.observed_at;
        match edit {
            AttachmentEdit::Select(binding, time) => {
                if self.attachment.is_some_and(|old| old.binding == binding) {
                    return Ok(());
                }
                self.observed_at = self.observed_at.min(time.at());
                self.attachment = Some(ManualAttachment {
                    binding,
                    time,
                    preview_confirmed: false,
                    working_copy: None,
                    working_copy_owner: None,
                });
            }
            AttachmentEdit::ConfirmPreview(binding) => {
                let attachment = self.attachment.as_mut().ok_or(InvalidReport)?;
                if attachment.binding != binding {
                    return Err(InvalidReport);
                }
                attachment.preview_confirmed = true;
            }
            AttachmentEdit::BindWorkingCopy(binding, digest) => {
                let attachment = self.attachment.as_mut().ok_or(InvalidReport)?;
                if attachment.binding != binding || !attachment.preview_confirmed {
                    return Err(InvalidReport);
                }
                attachment.working_copy = Some(digest);
                attachment.working_copy_owner = Some(self.id);
            }
            AttachmentEdit::ConfirmTime(at) => {
                let attachment = self.attachment.as_mut().ok_or(InvalidReport)?;
                if at > self.observed_at {
                    return Err(InvalidReport);
                }
                self.observed_at = at;
                attachment.time = AttachmentTime::User(at);
            }
            AttachmentEdit::Remove => {
                self.attachment = None;
            }
        }
        if self.observed_at != previous_time {
            self.revoke_observation_age();
            self.revoke_all_newer_observations();
        }
        self.version += 1;
        Ok(())
    }
    /// Returns the selected attachment metadata without image bytes or a source path.
    #[must_use]
    pub const fn attachment(&self) -> Option<ManualAttachment> {
        self.attachment
    }
    /// Checks only attachment privacy and capture-time decisions, not Report release readiness.
    ///
    /// # Errors
    /// Returns the first unresolved preview or capture-time decision.
    pub fn attachment_gate(&self) -> Result<(), AttachmentBlock> {
        if let Some(attachment) = self.attachment {
            if !attachment.preview_confirmed {
                return Err(AttachmentBlock::PreviewUnconfirmed);
            }
            match attachment.time {
                AttachmentTime::ImportedFile(_) => return Err(AttachmentBlock::TimeUncertain),
                AttachmentTime::User(_) => {}
            }
        }
        Ok(())
    }
}
