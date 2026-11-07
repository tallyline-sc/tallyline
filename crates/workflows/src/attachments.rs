// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Report-bound private working-copy contracts.

use tallyline_reporting::api::model::{AttachmentBinding, ImageHash, Report, ReportId};
use tallyline_shared_kernel::api::model::ImageRaster;

/// A closed private-copy failure without paths or pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkingCopyFailure {
    /// Privacy was not confirmed or the raster does not match the selected region.
    Unconfirmed,
    /// The expected private copy is absent or its integrity does not match.
    MissingOrChanged,
    /// Private storage could not commit the requested copy.
    Unavailable,
}

/// Persists only confirmed source-scale private regions, independently of original files.
pub trait WorkingCopyStore {
    /// Writes a confirmed region before its digest is published in Report metadata.
    ///
    /// # Errors
    /// Refuses unconfirmed evidence, mismatched dimensions and failed durable writes.
    fn save(
        &mut self,
        report: &Report,
        raster: &ImageRaster,
    ) -> Result<ImageHash, WorkingCopyFailure>;
    /// Loads and verifies the exact persisted private-copy binding.
    ///
    /// # Errors
    /// Refuses absent, changed or unbound evidence without falling back to the original.
    fn load(&mut self, report: &Report) -> Result<ImageRaster, WorkingCopyFailure>;
}

/// Immutable request identity rechecked against the current Report before using a result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttachmentRequest {
    report: ReportId,
    version: u64,
    binding: AttachmentBinding,
}
impl AttachmentRequest {
    /// Captures the exact selected evidence and Report version for bounded background work.
    #[must_use]
    pub fn new(report: &Report) -> Option<Self> {
        Some(Self {
            report: report.id(),
            version: report.version(),
            binding: report.attachment()?.binding(),
        })
    }
    /// Rejects delayed results after any edit, replacement or removal of the Report.
    #[must_use]
    pub fn is_current(self, report: &Report) -> bool {
        self.report == report.id()
            && self.version == report.version()
            && report
                .attachment()
                .is_some_and(|value| value.binding() == self.binding)
    }
}

/// The attachment-related part of release readiness, without claiming complete release validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManualAttachmentBlock {
    /// The current account's policy could not be read.
    PolicyUnavailable,
    /// The account is known to require an image, and none is selected.
    ImageRequired,
    /// The exact selected crop needs full-size privacy confirmation.
    PreviewUnconfirmed,
    /// The imported observation time needs an explicit decision.
    TimeUncertain,
    /// The confirmed private working copy is absent or changed.
    WorkingCopyMissingOrChanged,
}

/// Evaluates only current screenshot policy and current durable attachment evidence.
///
/// # Errors
/// Returns the actionable attachment block; all other Report release gates remain separate.
pub fn manual_attachment_readiness(
    report: &Report,
    policy: Result<
        tallyline_submission::api::model::ScreenshotPolicy,
        tallyline_submission::api::model::StorageFailure,
    >,
    copies: &mut impl WorkingCopyStore,
) -> Result<(), ManualAttachmentBlock> {
    use tallyline_reporting::api::model::AttachmentBlock;
    use tallyline_submission::api::model::ScreenshotPolicy;
    let policy = policy.map_err(|_| ManualAttachmentBlock::PolicyUnavailable)?;
    if report.attachment().is_none() {
        return match policy {
            ScreenshotPolicy::Unknown => Ok(()),
            ScreenshotPolicy::Required { .. } => Err(ManualAttachmentBlock::ImageRequired),
        };
    }
    report.attachment_gate().map_err(|block| match block {
        AttachmentBlock::PreviewUnconfirmed => ManualAttachmentBlock::PreviewUnconfirmed,
        AttachmentBlock::TimeUncertain => ManualAttachmentBlock::TimeUncertain,
    })?;
    copies
        .load(report)
        .map_err(|_| ManualAttachmentBlock::WorkingCopyMissingOrChanged)?;
    Ok(())
}

/// A selected source-scale crop prepared for the user's full-size review.
#[derive(Clone, Debug)]
pub struct ManualAttachmentPreview {
    binding: AttachmentBinding,
    time: tallyline_reporting::api::model::AttachmentTime,
    raster: ImageRaster,
}
impl ManualAttachmentPreview {
    /// Checks source-scale dimensions when mapping an adapter's bounded preparation result.
    ///
    /// # Errors
    /// Refuses pixels whose dimensions differ from the selected source rectangle.
    pub fn new(
        binding: AttachmentBinding,
        time: tallyline_reporting::api::model::AttachmentTime,
        raster: ImageRaster,
    ) -> Result<Self, WorkingCopyFailure> {
        let [_, _, width, height] = binding.region().rectangle();
        if (raster.width(), raster.height()) != (width, height) {
            return Err(WorkingCopyFailure::Unconfirmed);
        }
        Ok(Self {
            binding,
            time,
            raster,
        })
    }
    /// Returns the source hash and rectangle to which the preview decision applies.
    #[must_use]
    pub const fn binding(&self) -> AttachmentBinding {
        self.binding
    }
    /// Returns the imported or explicitly confirmed time without refreshing it.
    #[must_use]
    pub const fn time(&self) -> tallyline_reporting::api::model::AttachmentTime {
        self.time
    }
    /// Returns only the selected source-scale private region.
    #[must_use]
    pub const fn raster(&self) -> &ImageRaster {
        &self.raster
    }
}

/// A private confirmation could not be committed against current Report state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManualAttachmentFailure {
    /// Input could not produce a safe source-scale private region.
    Input(ManualAttachmentInputFailure),
    /// The current Draft changed, disappeared or could not be saved.
    Report(tallyline_reporting::api::RepositoryError),
    /// Private pixels could not be committed or recovered.
    WorkingCopy(WorkingCopyFailure),
}

/// Persists a confirmed private crop before binding its digest to the current Draft.
/// A failed or superseded database edit can leave an unreferenced private crop, never approved evidence.
///
/// # Errors
/// Refuses changed Report versions/bindings, mismatched previews and failed filesystem/database writes.
pub fn confirm_manual_attachment<
    R: tallyline_reporting::api::ReportRepository,
    C: tallyline_shared_kernel::api::Clock,
>(
    application: &mut tallyline_reporting::api::ReportingApplication<R, C>,
    copies: &mut impl WorkingCopyStore,
    id: ReportId,
    expected: u64,
    preview: &ManualAttachmentPreview,
) -> Result<Report, ManualAttachmentFailure> {
    use tallyline_reporting::api::{AttachmentEdit, RepositoryError};
    let mut report = application
        .draft(id)
        .map_err(ManualAttachmentFailure::Report)?
        .ok_or(ManualAttachmentFailure::Report(RepositoryError::Invalid))?;
    if report.version() != expected {
        return Err(ManualAttachmentFailure::Report(RepositoryError::Conflict));
    }
    report
        .edit_attachment(AttachmentEdit::ConfirmPreview(preview.binding()))
        .map_err(|_| ManualAttachmentFailure::Report(RepositoryError::Invalid))?;
    let digest = copies
        .save(&report, preview.raster())
        .map_err(ManualAttachmentFailure::WorkingCopy)?;
    application
        .confirm_attachment_copy(id, expected, preview.binding(), digest)
        .map_err(ManualAttachmentFailure::Report)
}

/// Actionable local source preparation failures without technology payloads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManualAttachmentInputFailure {
    /// The user must provide an SDR copy of HDR content.
    Hdr,
    /// The source's colour metadata or format is unsupported.
    Unsupported,
    /// Header, file, pixel, profile or codec allocation exceeds the intake limits.
    InputLimit,
    /// Intake reduced source dimensions, so source coordinates cannot safely be applied.
    SourceReduced,
    /// The rectangle lies outside the source or has empty dimensions.
    InvalidRegion,
    /// The selected source could not be read within its bounds.
    Unavailable,
    /// Encoding would exceed the screenshot limit.
    EncodingLimit,
}
