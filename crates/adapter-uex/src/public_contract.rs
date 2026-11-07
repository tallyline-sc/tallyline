// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Credential-free reference samples and value-free schema metadata.

mod comparison;
mod envelope;
mod rejection;
mod shape;
mod snapshot;
mod snapshot_wire;
mod status_codes;

pub use rejection::PublicContractCode;
pub use snapshot::PublicContractSnapshot;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::api::{ReadError, UexReader};

/// A fixed public reference route that requires neither credential.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicContractEndpoint {
    /// The LIVE and PTU version response.
    GameVersions,
    /// The buy and sell status vocabulary.
    CommoditiesStatus,
    /// Public server parameters.
    DataParameters,
    /// The commodity vocabulary.
    Commodities,
    /// Commodity terminals, including non-LIVE rows.
    Terminals,
}

impl PublicContractEndpoint {
    /// Every fixed credential-free route included in a complete contract snapshot.
    pub const ALL: [Self; 5] = [
        Self::GameVersions,
        Self::CommoditiesStatus,
        Self::DataParameters,
        Self::Commodities,
        Self::Terminals,
    ];

    pub(crate) fn resource(self) -> &'static str {
        match self {
            Self::GameVersions => "game_versions/",
            Self::CommoditiesStatus => "commodities_status/",
            Self::DataParameters => "data_parameters/",
            Self::Commodities => "commodities/",
            Self::Terminals => "terminals/",
        }
    }

    pub(crate) fn query(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Self::Terminals => &[("type", "commodity")],
            Self::GameVersions
            | Self::CommoditiesStatus
            | Self::DataParameters
            | Self::Commodities => &[],
        }
    }
}

/// A capped public response that exposes only schema metadata when consumed.
pub struct PublicContractPayload {
    endpoint: PublicContractEndpoint,
    http_status: u16,
    body: Vec<u8>,
}

impl fmt::Debug for PublicContractPayload {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PublicContractPayload(<redacted>)")
    }
}

impl PublicContractPayload {
    /// Parses field names and types without retaining scalar response values.
    /// Run this bounded CPU work outside the asynchronous I/O runtime.
    ///
    /// # Errors
    /// Returns a closed reason for an invalid envelope, JSON or schema complexity.
    pub fn into_schema(self) -> Result<PublicContractSchema, PublicContractError> {
        let value = serde_json::from_slice(&self.body).map_err(|_| PublicContractError::NotJson)?;
        envelope::validate(&value, self.http_status)?;
        envelope::validate_data(&value, self.endpoint)?;
        let fields = shape::fields(&value)?;
        let status_codes = status_codes::extract(self.endpoint, &value)?;
        Ok(PublicContractSchema {
            endpoint: self.endpoint,
            http_status: self.http_status,
            fields,
            status_codes,
        })
    }
}

/// Field names, primitive kinds and the HTTP code of one public response.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PublicContractSchema {
    endpoint: PublicContractEndpoint,
    http_status: u16,
    fields: BTreeMap<String, BTreeSet<shape::FieldKind>>,
    status_codes: Option<status_codes::StatusCodeSets>,
}

impl PublicContractSchema {
    /// Encodes only field/type metadata and codes, with no scalar response values.
    ///
    /// # Errors
    /// Returns a closed encoding failure.
    pub fn to_json(&self) -> Result<String, PublicContractError> {
        serde_json::to_string_pretty(self).map_err(|_| PublicContractError::MetadataEncoding)
    }
}

/// A public-contract refusal containing only closed reasons and bounded technical codes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PublicContractError {
    /// The envelope reports a bounded technical error identifier.
    Rejected {
        /// The actual response's HTTP code.
        http_status: u16,
        /// The validated technical identifier, without message or data values.
        code: PublicContractCode,
    },
    /// The public endpoint returned an unsuccessful HTTP code.
    HttpStatus {
        /// The received HTTP code, without response content.
        code: u16,
    },
    /// The envelope is not an object with the expected success status.
    EnvelopeStatus,
    /// The envelope's HTTP code is absent, malformed or contradicts the response.
    EnvelopeCode,
    /// The envelope has no data field.
    MissingData,
    /// No nonempty reference object or object-row sample was observed.
    UnobservableData,
    /// A status vocabulary is missing, malformed, empty, duplicated or has a non-positive code.
    StatusVocabulary,
    /// The response is not valid bounded JSON.
    NotJson,
    /// A field name cannot be represented safely in diagnostic metadata.
    UnsupportedFieldName,
    /// The bounded field, path, node or depth budget was exceeded.
    Complexity,
    /// Schema metadata could not be encoded.
    MetadataEncoding,
    /// The supplied metadata is incomplete, unsupported or outside its bounds.
    InvalidMetadata,
}

impl UexReader {
    /// Reads one fixed public route without credentials and retains no printable body.
    ///
    /// # Errors
    /// Returns the existing transport, redirect or response-size failure.
    pub async fn public_contract_sample(
        &self,
        endpoint: PublicContractEndpoint,
    ) -> Result<PublicContractPayload, ReadError> {
        let response = self.transport.public_contract_response(endpoint).await?;
        Ok(PublicContractPayload {
            endpoint,
            http_status: response.http_status,
            body: response.body,
        })
    }
}
