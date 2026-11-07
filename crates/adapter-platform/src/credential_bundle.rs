// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Bounded credential bundle encoding for platform stores.

use tallyline_shared_kernel::api::model::Timestamp;
use tallyline_submission::api::model::{
    CredentialFailure, CredentialValues, SavedCredentials, StorageFailure, ValidatedAccount,
};
use zeroize::Zeroizing;

const HEADER: &[u8] = b"TLCR\x01";
pub(crate) const MAX_BUNDLE_BYTES: usize = 2560;

pub(crate) fn encode(saved: &SavedCredentials) -> Result<Zeroizing<Vec<u8>>, StorageFailure> {
    let fields = [
        saved.values().secret_key(),
        saved.values().own_token().unwrap_or(""),
        saved.username(),
    ];
    let length = HEADER.len() + 6 + 16 + 1 + fields.iter().map(|field| field.len()).sum::<usize>();
    if length > MAX_BUNDLE_BYTES
        || saved.username().is_empty()
        || saved.username().chars().any(char::is_control)
    {
        return Err(StorageFailure::WriteFailed);
    }
    let mut bytes = Zeroizing::new(Vec::with_capacity(length));
    bytes.extend_from_slice(HEADER);
    for field in fields {
        let length = u16::try_from(field.len()).map_err(|_| StorageFailure::WriteFailed)?;
        bytes.extend_from_slice(&length.to_le_bytes());
    }
    bytes.extend_from_slice(&saved.validated_at().millis().to_le_bytes());
    bytes.push(match saved.refusal() {
        None => 0,
        Some(CredentialFailure::InvalidKey) => 1,
        Some(CredentialFailure::NotAllowed) => 2,
        Some(CredentialFailure::InvalidToken) => 3,
        Some(CredentialFailure::UpdateRequired) => 4,
        Some(_) => return Err(StorageFailure::Corrupt),
    });
    for field in fields {
        bytes.extend_from_slice(field.as_bytes());
    }
    Ok(bytes)
}

pub(crate) fn decode(bytes: &[u8]) -> Result<SavedCredentials, StorageFailure> {
    if bytes.is_empty() || bytes.len() > MAX_BUNDLE_BYTES {
        return Err(StorageFailure::Corrupt);
    }
    let mut remaining = bytes;
    if take(&mut remaining, HEADER.len())? != HEADER {
        return Err(StorageFailure::Corrupt);
    }
    let key_length = length(&mut remaining)?;
    let token_length = length(&mut remaining)?;
    let username_length = length(&mut remaining)?;
    let millis = u128::from_le_bytes(
        take(&mut remaining, 16)?
            .try_into()
            .map_err(|_| StorageFailure::Corrupt)?,
    );
    let timestamp = match u64::try_from(millis) {
        Ok(millis) => Timestamp::from_millis(millis),
        Err(_) if millis.is_multiple_of(1000) => Timestamp::from_seconds(
            u64::try_from(millis / 1000).map_err(|_| StorageFailure::Corrupt)?,
        ),
        Err(_) => return Err(StorageFailure::Corrupt),
    };
    let refusal = match take(&mut remaining, 1)? {
        [0] => None,
        [1] => Some(CredentialFailure::InvalidKey),
        [2] => Some(CredentialFailure::NotAllowed),
        [3] => Some(CredentialFailure::InvalidToken),
        [4] => Some(CredentialFailure::UpdateRequired),
        _ => return Err(StorageFailure::Corrupt),
    };
    let key = text(&mut remaining, key_length)?;
    let token = text(&mut remaining, token_length)?;
    let username = text(&mut remaining, username_length)?;
    if !remaining.is_empty() || username.is_empty() || username.chars().any(char::is_control) {
        return Err(StorageFailure::Corrupt);
    }
    let values = CredentialValues::new(
        key.to_owned(),
        if token.is_empty() {
            None
        } else {
            Some(token.to_owned())
        },
    )
    .map_err(|_| StorageFailure::Corrupt)?;
    SavedCredentials::from_storage(
        values,
        ValidatedAccount::new(username.to_owned()),
        timestamp,
        refusal,
    )
}

fn take<'a>(remaining: &mut &'a [u8], count: usize) -> Result<&'a [u8], StorageFailure> {
    let (value, rest) = remaining
        .split_at_checked(count)
        .ok_or(StorageFailure::Corrupt)?;
    *remaining = rest;
    Ok(value)
}

fn length(remaining: &mut &[u8]) -> Result<usize, StorageFailure> {
    Ok(usize::from(u16::from_le_bytes(
        take(remaining, 2)?
            .try_into()
            .map_err(|_| StorageFailure::Corrupt)?,
    )))
}

fn text<'a>(remaining: &mut &'a [u8], count: usize) -> Result<&'a str, StorageFailure> {
    std::str::from_utf8(take(remaining, count)?).map_err(|_| StorageFailure::Corrupt)
}
