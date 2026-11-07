// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integrity-bound private region files in the application data directory.

use crate::api::model::AttachmentPreview;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use tallyline_reporting::api::model::{ImageHash, Report};
use tallyline_shared_kernel::api::model::{ImageRaster, ReportId};
use tallyline_workflows::api::{WorkingCopyFailure, WorkingCopyStore};

const MAGIC: &[u8; 8] = b"TLCRP001";
const HEADER_BYTES: usize = 56;
const MAX_PIXELS: usize = 100_000_000;
const MAX_STAGING_ATTEMPTS: u64 = 1024;

/// Owns confirmed manual regions under one private application data directory.
/// Draft working copies are retained; this store never performs automatic deletion.
pub struct ManualWorkingCopies {
    directory: PathBuf,
}
impl ManualWorkingCopies {
    /// Creates or checks an absolute private directory without following a directory link.
    /// Windows inherits the application data directory ACL; Unix requires owner-only access.
    ///
    /// # Errors
    /// Refuses relative, linked, shared or inaccessible directories.
    pub fn new(directory: PathBuf) -> Result<Self, WorkingCopyFailure> {
        if !directory.is_absolute() {
            return Err(WorkingCopyFailure::Unavailable);
        }
        let builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        let builder = {
            use std::os::unix::fs::DirBuilderExt;
            let mut builder = builder;
            builder.mode(0o700);
            builder
        };
        match builder.create(&directory) {
            Ok(()) => (),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => (),
            Err(_) => return Err(WorkingCopyFailure::Unavailable),
        }
        let metadata =
            std::fs::symlink_metadata(&directory).map_err(|_| WorkingCopyFailure::Unavailable)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(WorkingCopyFailure::Unavailable);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o077 != 0 {
                return Err(WorkingCopyFailure::Unavailable);
            }
        }
        Ok(Self { directory })
    }
    fn path(&self, owner: ReportId, hash: ImageHash) -> PathBuf {
        let digest: String = hash
            .bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        self.directory
            .join(format!("report-{}-{digest}.crop", owner.0))
    }
}
impl WorkingCopyStore for ManualWorkingCopies {
    fn save(
        &mut self,
        report: &Report,
        raster: &ImageRaster,
    ) -> Result<ImageHash, WorkingCopyFailure> {
        let attachment = report.attachment().ok_or(WorkingCopyFailure::Unconfirmed)?;
        let [_, _, width, height] = attachment.binding().region().rectangle();
        if !attachment.preview_confirmed()
            || (width, height) != (raster.width(), raster.height())
            || raster.argb().len() > MAX_PIXELS
        {
            return Err(WorkingCopyFailure::Unconfirmed);
        }
        let mut bytes = Vec::with_capacity(HEADER_BYTES + raster.argb().len() * 4);
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&attachment.binding().image().bytes());
        for value in attachment.binding().region().rectangle() {
            bytes.extend_from_slice(&value.to_be_bytes());
        }
        for pixel in raster.argb() {
            bytes.extend_from_slice(&pixel.to_be_bytes());
        }
        let digest = ImageHash::new(Sha256::digest(&bytes).into());
        let destination = self.path(report.id(), digest);
        if read_verified(&destination, digest).is_ok_and(|existing| existing == bytes) {
            return Ok(digest);
        }
        let mut reserved = None;
        for attempt in 0..MAX_STAGING_ATTEMPTS {
            let pending =
                destination.with_extension(format!("{}-{attempt}.pending", std::process::id()));
            match crate::api::write_private_artifact(&pending, &bytes) {
                Ok(()) => {
                    reserved = Some(pending);
                    break;
                }
                Err(crate::api::model::PrivateArtifactError::AlreadyExists) => continue,
                Err(
                    crate::api::model::PrivateArtifactError::Destination
                    | crate::api::model::PrivateArtifactError::Write,
                ) => return Err(WorkingCopyFailure::Unavailable),
            }
        }
        let pending = reserved.ok_or(WorkingCopyFailure::Unavailable)?;
        std::fs::rename(&pending, &destination).map_err(|_| WorkingCopyFailure::Unavailable)?;
        #[cfg(unix)]
        std::fs::File::open(&self.directory)
            .and_then(|file| file.sync_all())
            .map_err(|_| WorkingCopyFailure::Unavailable)?;
        Ok(digest)
    }
    fn load(&mut self, report: &Report) -> Result<ImageRaster, WorkingCopyFailure> {
        let attachment = report
            .attachment()
            .ok_or(WorkingCopyFailure::MissingOrChanged)?;
        let digest = attachment
            .working_copy()
            .ok_or(WorkingCopyFailure::MissingOrChanged)?;
        if !attachment.preview_confirmed() {
            return Err(WorkingCopyFailure::Unconfirmed);
        }
        let owner = attachment
            .working_copy_owner()
            .ok_or(WorkingCopyFailure::MissingOrChanged)?;
        let bytes = read_verified(&self.path(owner, digest), digest)?;
        let mut expected = Vec::with_capacity(HEADER_BYTES);
        expected.extend_from_slice(MAGIC);
        expected.extend_from_slice(&attachment.binding().image().bytes());
        for value in attachment.binding().region().rectangle() {
            expected.extend_from_slice(&value.to_be_bytes());
        }
        let [_, _, width, height] = attachment.binding().region().rectangle();
        let pixels = usize::try_from(u64::from(width) * u64::from(height))
            .map_err(|_| WorkingCopyFailure::MissingOrChanged)?;
        if pixels > MAX_PIXELS
            || bytes.len() != HEADER_BYTES + pixels * 4
            || bytes.get(..HEADER_BYTES) != Some(expected.as_slice())
        {
            return Err(WorkingCopyFailure::MissingOrChanged);
        }
        let raster = bytes
            .get(HEADER_BYTES..)
            .ok_or(WorkingCopyFailure::MissingOrChanged)?
            .as_chunks::<4>()
            .0
            .iter()
            .map(|chunk| u32::from_be_bytes(*chunk))
            .collect();
        ImageRaster::new(width, height, raster).map_err(|_| WorkingCopyFailure::MissingOrChanged)
    }
}
fn read_verified(path: &Path, digest: ImageHash) -> Result<Vec<u8>, WorkingCopyFailure> {
    let metadata =
        std::fs::symlink_metadata(path).map_err(|_| WorkingCopyFailure::MissingOrChanged)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() > (HEADER_BYTES + MAX_PIXELS * 4) as u64
    {
        return Err(WorkingCopyFailure::MissingOrChanged);
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .and_then(|file| {
            file.take((HEADER_BYTES + MAX_PIXELS * 4 + 1) as u64)
                .read_to_end(&mut bytes)
        })
        .map_err(|_| WorkingCopyFailure::MissingOrChanged)?;
    if ImageHash::new(Sha256::digest(&bytes).into()) != digest {
        return Err(WorkingCopyFailure::MissingOrChanged);
    }
    Ok(bytes)
}

/// Restores only the integrity-checked private crop, preserving its original binding and time.
///
/// # Errors
/// Refuses missing, changed or unconfirmed evidence; never reopens the original screenshot.
pub fn restore_manual_attachment(
    report: &Report,
    store: &mut impl WorkingCopyStore,
) -> Result<AttachmentPreview, WorkingCopyFailure> {
    let attachment = report
        .attachment()
        .ok_or(WorkingCopyFailure::MissingOrChanged)?;
    Ok(AttachmentPreview {
        binding: attachment.binding(),
        time: attachment.time(),
        raster: store.load(report)?,
    })
}
