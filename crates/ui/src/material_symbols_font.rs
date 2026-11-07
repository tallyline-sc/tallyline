// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact identity validation of the approved icon font resource.

use sha2::{Digest, Sha256};

const APPROVED_BYTES: usize = 30_960;
const IDENTITY_ERROR: &str = "Material Symbols font differs from its approved identity";

const APPROVED_SHA256: [u8; 32] = [
    0x40, 0xc8, 0xc3, 0xa4, 0x72, 0xc6, 0x97, 0x8c, 0xed, 0xd5, 0x02, 0x81, 0x94, 0xfa, 0x17, 0x4b,
    0x2c, 0x63, 0xed, 0xae, 0x41, 0x65, 0xf7, 0x68, 0x6d, 0x88, 0xfb, 0xf1, 0x74, 0x60, 0x22, 0x34,
];

/// The hash-checked Material Symbols Rounded font bytes.
pub struct MaterialSymbolsFont(Vec<u8>);

impl TryFrom<Vec<u8>> for MaterialSymbolsFont {
    type Error = &'static str;

    fn try_from(bytes: Vec<u8>) -> Result<Self, Self::Error> {
        if bytes.len() != APPROVED_BYTES {
            return Err(IDENTITY_ERROR);
        }
        let digest: [u8; 32] = Sha256::digest(&bytes).into();
        if digest != APPROVED_SHA256 {
            return Err(IDENTITY_ERROR);
        }
        Ok(Self(bytes))
    }
}

impl MaterialSymbolsFont {
    pub(crate) fn into_bytes(self) -> Vec<u8> {
        self.0
    }
}
