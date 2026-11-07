// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Private same-user transport for Linux credential services.

use std::ffi::OsString;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::{Path, PathBuf};
use std::time::Duration;

use tallyline_submission::api::model::StorageFailure;
use tokio::net::UnixStream;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_ADDRESS_BYTES: usize = 4096;

/// Owns a local authenticated bus connection for a Linux credential adapter.
/// Callers suppress credential-library logging targets before formatting or bridging.
pub struct LinuxCredentialBus {
    pub(crate) connection: zbus::Connection,
}

impl LinuxCredentialBus {
    /// Connects one private filesystem socket on the caller's Tokio runtime.
    /// No credential is read, written or unlocked.
    ///
    /// # Errors
    /// Returns unavailable for a rejected path, peer, authentication or deadline.
    pub async fn connect(address: &str) -> Result<Self, StorageFailure> {
        let endpoint = Endpoint::parse(address)?;
        let uid = rustix::process::geteuid().as_raw();
        validate_path(&endpoint.path, uid)?;
        tokio::time::timeout(CONNECT_TIMEOUT, Self::establish(endpoint, uid))
            .await
            .map_err(|_| StorageFailure::Unavailable)?
    }

    /// Reports whether the owned bus connection remains open.
    #[must_use]
    pub fn is_connected(&self) -> bool {
        !self.connection.is_closed()
    }

    async fn establish(endpoint: Endpoint, uid: u32) -> Result<Self, StorageFailure> {
        let stream = UnixStream::connect(&endpoint.path)
            .await
            .map_err(|_| StorageFailure::Unavailable)?;
        verify_peer(&stream, uid)?;
        let connection = zbus::connection::Builder::unix_stream(stream)
            .auth_mechanism(zbus::AuthMechanism::External)
            .build()
            .await
            .map_err(|_| StorageFailure::Unavailable)?;
        if endpoint.guid.as_ref().is_some_and(|expected| {
            !expected
                .as_str()
                .eq_ignore_ascii_case(connection.server_guid().as_str())
        }) {
            return Err(StorageFailure::Unavailable);
        }
        Ok(Self { connection })
    }
}

struct Endpoint {
    path: PathBuf,
    guid: Option<zbus::Guid<'static>>,
}

impl Endpoint {
    fn parse(address: &str) -> Result<Self, StorageFailure> {
        if address.len() > MAX_ADDRESS_BYTES || address.contains(';') {
            return Err(StorageFailure::Unavailable);
        }
        let options = address
            .strip_prefix("unix:")
            .ok_or(StorageFailure::Unavailable)?;
        let mut path = None;
        let mut guid = None;
        for option in options.split(',') {
            match option.split_once('=') {
                Some(("path", value)) if path.is_none() => path = Some(decode_path(value)?),
                Some(("guid", value)) if guid.is_none() => {
                    guid = Some(
                        zbus::Guid::try_from(value.to_owned())
                            .map_err(|_| StorageFailure::Unavailable)?,
                    );
                }
                _ => return Err(StorageFailure::Unavailable),
            }
        }
        Ok(Self {
            path: path.ok_or(StorageFailure::Unavailable)?,
            guid,
        })
    }
}

fn decode_path(value: &str) -> Result<PathBuf, StorageFailure> {
    let mut source = value.bytes();
    let mut decoded = Vec::with_capacity(value.len());
    while let Some(byte) = source.next() {
        let byte = match byte {
            b'%' => {
                let high = source
                    .next()
                    .and_then(hex)
                    .ok_or(StorageFailure::Unavailable)?;
                let low = source
                    .next()
                    .and_then(hex)
                    .ok_or(StorageFailure::Unavailable)?;
                high * 16 + low
            }
            b'-' | b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z' | b'_' | b'/' | b'.' | b'\\' | b'*' => {
                byte
            }
            _ => return Err(StorageFailure::Unavailable),
        };
        if byte == 0 {
            return Err(StorageFailure::Unavailable);
        }
        decoded.push(byte);
    }
    let path = PathBuf::from(OsString::from_vec(decoded));
    if !path.is_absolute()
        || path
            .as_os_str()
            .as_bytes()
            .split(|byte| *byte == b'/')
            .skip(1)
            .any(|part| part.is_empty() || part == b"." || part == b"..")
    {
        return Err(StorageFailure::Unavailable);
    }
    Ok(path)
}

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn validate_path(path: &Path, uid: u32) -> Result<(), StorageFailure> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component);
        let metadata =
            std::fs::symlink_metadata(&current).map_err(|_| StorageFailure::Unavailable)?;
        if metadata.file_type().is_symlink() {
            return Err(StorageFailure::Unavailable);
        }
    }
    let parent = path.parent().ok_or(StorageFailure::Unavailable)?;
    let metadata = std::fs::symlink_metadata(parent).map_err(|_| StorageFailure::Unavailable)?;
    if !metadata.is_dir() || metadata.uid() != uid || metadata.mode() & 0o077 != 0 {
        return Err(StorageFailure::Unavailable);
    }
    if !std::fs::symlink_metadata(path)
        .map_err(|_| StorageFailure::Unavailable)?
        .file_type()
        .is_socket()
    {
        return Err(StorageFailure::Unavailable);
    }
    Ok(())
}

fn verify_peer(stream: &UnixStream, uid: u32) -> Result<(), StorageFailure> {
    let peer = stream
        .peer_cred()
        .map_err(|_| StorageFailure::Unavailable)?;
    if peer.uid() != uid {
        return Err(StorageFailure::Unavailable);
    }
    Ok(())
}
