// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Fixed sibling-file identity held through the native load.

use super::RuntimeLoadError;
use sha2::{Digest, Sha256};
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;

#[cfg(windows)]
const PIN: (&str, &str, &str) = (
    env!("TALLYLINE_RUNTIME_WINDOWS_FILE"),
    env!("TALLYLINE_RUNTIME_WINDOWS_SHA256"),
    env!("TALLYLINE_RUNTIME_WINDOWS_BYTES"),
);
#[cfg(target_os = "linux")]
const PIN: (&str, &str, &str) = (
    env!("TALLYLINE_RUNTIME_LINUX_FILE"),
    env!("TALLYLINE_RUNTIME_LINUX_SHA256"),
    env!("TALLYLINE_RUNTIME_LINUX_BYTES"),
);

pub(super) struct VerifiedLibrary {
    pub(super) file: std::fs::File,
    pub(super) path: PathBuf,
}

impl VerifiedLibrary {
    pub(super) fn open_sibling() -> Result<Self, RuntimeLoadError> {
        let executable = std::env::current_exe()
            .and_then(|path| path.canonicalize())
            .map_err(|_| RuntimeLoadError::Location)?;
        let directory = executable.parent().ok_or(RuntimeLoadError::Location)?;
        let path = directory.join(PIN.0);
        let metadata = std::fs::symlink_metadata(&path).map_err(|_| RuntimeLoadError::Read)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() || is_reparse(&metadata) {
            return Err(RuntimeLoadError::Location);
        }
        let path = path
            .canonicalize()
            .map_err(|_| RuntimeLoadError::Location)?;
        if path.parent() != Some(directory) {
            return Err(RuntimeLoadError::Location);
        }
        let mut options = std::fs::OpenOptions::new();
        options.read(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.share_mode(1);
        }
        let file = options.open(&path).map_err(|_| RuntimeLoadError::Read)?;
        let mut verified = Self { file, path };
        verified.recheck()?;
        Ok(verified)
    }

    pub(super) fn recheck(&mut self) -> Result<(), RuntimeLoadError> {
        let bytes: u64 = PIN.2.parse().map_err(|_| RuntimeLoadError::HashMismatch)?;
        let metadata = self.file.metadata().map_err(|_| RuntimeLoadError::Read)?;
        if !metadata.is_file() || metadata.len() != bytes {
            return Err(RuntimeLoadError::HashMismatch);
        }
        self.file
            .seek(SeekFrom::Start(0))
            .map_err(|_| RuntimeLoadError::Read)?;
        let mut reader =
            (&mut self.file).take(bytes.checked_add(1).ok_or(RuntimeLoadError::HashMismatch)?);
        let mut digest = Sha256::new();
        let mut buffer = [0_u8; 64 * 1024];
        let mut count = 0_u64;
        loop {
            let read = reader
                .read(&mut buffer)
                .map_err(|_| RuntimeLoadError::Read)?;
            if read == 0 {
                break;
            }
            digest.update(buffer.get(..read).ok_or(RuntimeLoadError::Read)?);
            count += read as u64;
        }
        let actual: String = digest
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        if count != bytes || actual != PIN.1 {
            return Err(RuntimeLoadError::HashMismatch);
        }
        Ok(())
    }
}

fn is_reparse(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        let _ = metadata;
        false
    }
}
