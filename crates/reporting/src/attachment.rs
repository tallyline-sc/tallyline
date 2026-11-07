// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Value-bound manual screenshot evidence.

use crate::api::model::InvalidReport;
use tallyline_shared_kernel::api::model::{ReportId, Timestamp};

/// SHA-256 of source image bytes or a private crop artifact, according to its owning binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImageHash([u8; 32]);
impl ImageHash {
    /// Wraps a digest computed by the image adapter.
    #[must_use]
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    /// Returns the digest bytes for persistence.
    #[must_use]
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// A nonempty source-pixel rectangle selected by the user.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttachmentRegion {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}
impl AttachmentRegion {
    /// Validates a rectangle without overflowing its far edges.
    ///
    /// # Errors
    /// Refuses empty dimensions or coordinate overflow.
    pub fn new(x: u32, y: u32, width: u32, height: u32) -> Result<Self, InvalidReport> {
        if width == 0
            || height == 0
            || x.checked_add(width).is_none()
            || y.checked_add(height).is_none()
        {
            return Err(InvalidReport);
        }
        Ok(Self {
            x,
            y,
            width,
            height,
        })
    }
    /// Returns source-pixel coordinates and dimensions.
    #[must_use]
    pub const fn rectangle(self) -> [u32; 4] {
        [self.x, self.y, self.width, self.height]
    }
}

/// The exact content and geometry to which a privacy decision applies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttachmentBinding {
    image: ImageHash,
    region: AttachmentRegion,
}
impl AttachmentBinding {
    /// Binds the original digest and selected source rectangle.
    #[must_use]
    pub const fn new(image: ImageHash, region: AttachmentRegion) -> Self {
        Self { image, region }
    }
    /// Returns the complete source-image digest.
    #[must_use]
    pub const fn image(self) -> ImageHash {
        self.image
    }
    /// Returns the selected source rectangle.
    #[must_use]
    pub const fn region(self) -> AttachmentRegion {
        self.region
    }
}

/// The source and certainty of a manually attached image's observation time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttachmentTime {
    /// File modification time from an explicit import, requiring confirmation.
    ImportedFile(Timestamp),
    /// Time explicitly entered or confirmed by the user.
    User(Timestamp),
}
impl AttachmentTime {
    /// Returns the original or explicitly confirmed observation time.
    #[must_use]
    pub const fn at(self) -> Timestamp {
        match self {
            Self::ImportedFile(at) | Self::User(at) => at,
        }
    }
}

/// Report-owned evidence metadata without original pixels or file paths.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ManualAttachment {
    pub(crate) binding: AttachmentBinding,
    pub(crate) time: AttachmentTime,
    pub(crate) preview_confirmed: bool,
    pub(crate) working_copy: Option<ImageHash>,
    pub(crate) working_copy_owner: Option<ReportId>,
}
impl ManualAttachment {
    /// Returns the exact content and geometry selected for this Report.
    #[must_use]
    pub const fn binding(self) -> AttachmentBinding {
        self.binding
    }
    /// Returns the digest of the durably stored private region, when committed.
    #[must_use]
    pub const fn working_copy(self) -> Option<ImageHash> {
        self.working_copy
    }
    /// Returns the Report namespace owning the exact persisted private crop.
    #[must_use]
    pub const fn working_copy_owner(self) -> Option<ReportId> {
        self.working_copy_owner
    }
    /// Returns the attachment's time source and certainty.
    #[must_use]
    pub const fn time(self) -> AttachmentTime {
        self.time
    }
    /// Reports whether the user approved this exact full-size preview.
    #[must_use]
    pub const fn preview_confirmed(self) -> bool {
        self.preview_confirmed
    }
}

/// An unresolved attachment decision that blocks attachment eligibility.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttachmentBlock {
    /// This content and geometry have not received a privacy confirmation.
    PreviewUnconfirmed,
    /// The imported file time needs an explicit user decision.
    TimeUncertain,
}

/// Explicit user decisions for the manual attachment of one Draft.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttachmentEdit {
    /// Replace the selected image or region, revoking its preview confirmation.
    Select(AttachmentBinding, AttachmentTime),
    /// Confirm that this exact full-size preview contains no private data.
    ConfirmPreview(AttachmentBinding),
    /// Bind an already persisted private region to the exact confirmed selection.
    BindWorkingCopy(AttachmentBinding, ImageHash),
    /// Confirm the shown observation time or move it earlier.
    ConfirmTime(Timestamp),
    /// Detach the evidence without advancing the Report's observation time.
    Remove,
}

/// Persistence-only attachment state, restored through the Report mapper.
#[derive(Clone, Copy, Debug)]
pub struct StoredAttachment {
    /// Exact source content and selected geometry.
    pub binding: AttachmentBinding,
    /// Original or explicitly confirmed observation time.
    pub time: AttachmentTime,
    /// Privacy decision bound to this exact binding.
    pub preview_confirmed: bool,
    /// Digest of the persisted private region, absent for legacy selections.
    pub working_copy: Option<ImageHash>,
    /// Original private-copy namespace, absent in legacy documents and unbound selections.
    pub working_copy_owner: Option<ReportId>,
}
