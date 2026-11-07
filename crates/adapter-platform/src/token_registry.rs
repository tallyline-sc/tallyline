// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Bounded protected token associations with independently random public identities.

use tallyline_submission::api::model::{Persistence, StorageFailure, TokenAdmission, TokenScope};
use zeroize::Zeroizing;

const HEADER: &[u8] = b"TLTR\x01";
pub(crate) const MAX_BYTES: usize = 2560;
const MAX_ENTRIES: usize = TokenAdmission::MAX_DISTINCT_SCOPES;
const ID_BYTES: usize = 16;
const ENTRY_HEADER_BYTES: usize = ID_BYTES + std::mem::size_of::<u16>();

pub(crate) struct TokenRegistry {
    identity: [u8; ID_BYTES],
    entries: Vec<(TokenScope, Zeroizing<String>)>,
}
impl TokenRegistry {
    pub(crate) fn load(bytes: Option<&[u8]>) -> Result<Self, StorageFailure> {
        let Some(bytes) = bytes else {
            return Ok(Self {
                identity: random_id()?,
                entries: Vec::new(),
            });
        };
        if bytes.len() > MAX_BYTES {
            return Err(StorageFailure::Corrupt);
        }
        let mut input = bytes;
        if take(&mut input, HEADER.len())? != HEADER {
            return Err(StorageFailure::Corrupt);
        }
        let identity: [u8; ID_BYTES] = take(&mut input, ID_BYTES)?
            .try_into()
            .map_err(|_| StorageFailure::Corrupt)?;
        if identity.iter().all(|byte| *byte == 0) {
            return Err(StorageFailure::Corrupt);
        }
        let mut registry = Self {
            identity,
            entries: Vec::new(),
        };
        while !input.is_empty() {
            if registry.entries.len() >= MAX_ENTRIES {
                return Err(StorageFailure::Corrupt);
            }
            let token_id = take(&mut input, ID_BYTES)?;
            let mut scope = [0; 32];
            scope
                .get_mut(..ID_BYTES)
                .ok_or(StorageFailure::Corrupt)?
                .copy_from_slice(&identity);
            scope
                .get_mut(ID_BYTES..)
                .ok_or(StorageFailure::Corrupt)?
                .copy_from_slice(token_id);
            let scope = TokenScope::new(scope).ok_or(StorageFailure::Corrupt)?;
            let length = u16::from_le_bytes(
                take(&mut input, 2)?
                    .try_into()
                    .map_err(|_| StorageFailure::Corrupt)?,
            );
            let token = std::str::from_utf8(take(&mut input, usize::from(length))?)
                .map_err(|_| StorageFailure::Corrupt)?;
            if !valid_token(token)
                || registry
                    .entries
                    .iter()
                    .any(|(id, value)| *id == scope || value.as_str() == token)
            {
                return Err(StorageFailure::Corrupt);
            }
            registry
                .entries
                .push((scope, Zeroizing::new(token.to_owned())));
        }
        Ok(registry)
    }
    pub(crate) fn admission(&self, token: &str) -> Result<TokenAdmission, StorageFailure> {
        if !valid_token(token) {
            return Err(StorageFailure::Corrupt);
        }
        if let Some((scope, _)) = self
            .entries
            .iter()
            .find(|(_, value)| value.as_str() == token)
        {
            return Ok(TokenAdmission::from_scope(Some(*scope)));
        }
        TokenAdmission::distinct_from(self.entries.iter().map(|(scope, _)| *scope).collect())
            .ok_or(StorageFailure::Corrupt)
    }
    pub(crate) fn associate(
        &mut self,
        token: &str,
        persistence: Persistence,
    ) -> Result<(Option<TokenScope>, bool), StorageFailure> {
        if !valid_token(token) {
            return Err(StorageFailure::Corrupt);
        }
        if let Some((scope, _)) = self
            .entries
            .iter()
            .find(|(_, value)| value.as_str() == token)
        {
            return Ok((Some(*scope), false));
        }
        if persistence == Persistence::Session {
            return Ok((None, false));
        }
        if self.entries.len() >= MAX_ENTRIES {
            return Err(StorageFailure::WriteFailed);
        }
        let used = HEADER.len()
            + ID_BYTES
            + self
                .entries
                .iter()
                .map(|(_, value)| ENTRY_HEADER_BYTES + value.len())
                .sum::<usize>();
        if used + ENTRY_HEADER_BYTES + token.len() > MAX_BYTES {
            return Err(StorageFailure::WriteFailed);
        }
        let token_id = random_id()?;
        let mut bytes = [0; 32];
        bytes
            .get_mut(..ID_BYTES)
            .ok_or(StorageFailure::Corrupt)?
            .copy_from_slice(&self.identity);
        bytes
            .get_mut(ID_BYTES..)
            .ok_or(StorageFailure::Corrupt)?
            .copy_from_slice(&token_id);
        let scope = TokenScope::new(bytes).ok_or(StorageFailure::WriteFailed)?;
        if self.entries.iter().any(|(id, _)| *id == scope) {
            return Err(StorageFailure::WriteFailed);
        }
        self.entries.push((scope, Zeroizing::new(token.to_owned())));
        Ok((Some(scope), true))
    }
    pub(crate) fn encode(&self) -> Result<Zeroizing<Vec<u8>>, StorageFailure> {
        let mut bytes = Zeroizing::new(Vec::new());
        bytes.extend_from_slice(HEADER);
        bytes.extend_from_slice(&self.identity);
        for (scope, token) in &self.entries {
            bytes.extend_from_slice(
                scope
                    .bytes()
                    .get(ID_BYTES..)
                    .ok_or(StorageFailure::Corrupt)?,
            );
            let length = u16::try_from(token.len()).map_err(|_| StorageFailure::WriteFailed)?;
            bytes.extend_from_slice(&length.to_le_bytes());
            bytes.extend_from_slice(token.as_bytes());
        }
        if bytes.len() > MAX_BYTES {
            return Err(StorageFailure::WriteFailed);
        }
        Ok(bytes)
    }
}
fn valid_token(token: &str) -> bool {
    !token.is_empty() && token.len() <= 1024 && token.bytes().all(|byte| byte.is_ascii_graphic())
}
fn take<'a>(input: &mut &'a [u8], length: usize) -> Result<&'a [u8], StorageFailure> {
    let (value, rest) = input
        .split_at_checked(length)
        .ok_or(StorageFailure::Corrupt)?;
    *input = rest;
    Ok(value)
}
fn random_id() -> Result<[u8; ID_BYTES], StorageFailure> {
    let mut bytes = [0; ID_BYTES];
    rustls::crypto::aws_lc_rs::default_provider()
        .secure_random
        .fill(&mut bytes)
        .map_err(|_| StorageFailure::Unavailable)?;
    if bytes.iter().all(|byte| *byte == 0) {
        return Err(StorageFailure::Unavailable);
    }
    Ok(bytes)
}
