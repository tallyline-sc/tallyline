// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Validated canonical stable release spellings in queue documents.

use serde::{Deserialize, Serialize};
use tallyline_shared_kernel::api::model::ReleaseVersion;

#[derive(Clone, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub(crate) struct ClientVersionDocument(pub(crate) ReleaseVersion);
impl TryFrom<String> for ClientVersionDocument {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        ReleaseVersion::parse(&value)
            .map(Self)
            .ok_or("invalid canonical client release")
    }
}
impl From<ClientVersionDocument> for String {
    fn from(value: ClientVersionDocument) -> Self {
        value.0.to_string()
    }
}
