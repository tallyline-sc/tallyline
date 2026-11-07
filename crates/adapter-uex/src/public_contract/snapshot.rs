// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Complete, versioned public metadata baselines.

use std::collections::BTreeSet;

use serde::Serialize;

use super::{PublicContractEndpoint, PublicContractError, PublicContractSchema, snapshot_wire};

const SCHEMA_VERSION: u16 = 1;

/// Value-free metadata for every fixed public reference route.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PublicContractSnapshot {
    schema_version: u16,
    endpoints: Vec<PublicContractSchema>,
}

impl PublicContractSnapshot {
    /// Creates a complete snapshot with a deterministic route order.
    ///
    /// # Errors
    /// Refuses missing, duplicate, unsupported or malformed route metadata.
    pub fn new(mut endpoints: Vec<PublicContractSchema>) -> Result<Self, PublicContractError> {
        let actual = endpoints
            .iter()
            .map(|schema| schema.endpoint)
            .collect::<BTreeSet<_>>();
        if endpoints.len() != PublicContractEndpoint::ALL.len()
            || actual != BTreeSet::from(PublicContractEndpoint::ALL)
            || endpoints.iter().any(|schema| !snapshot_wire::valid(schema))
        {
            return Err(PublicContractError::InvalidMetadata);
        }
        endpoints.sort_by_key(|schema| schema.endpoint);
        Ok(Self {
            schema_version: SCHEMA_VERSION,
            endpoints,
        })
    }

    /// Decodes a bounded metadata baseline without reflecting rejected input.
    ///
    /// # Errors
    /// Refuses unsupported versions, malformed JSON, names, kinds or route inventories.
    pub fn from_json(json: &str) -> Result<Self, PublicContractError> {
        Self::new(snapshot_wire::parse(json, SCHEMA_VERSION)?)
    }

    /// Encodes only the versioned field/type and code metadata.
    ///
    /// # Errors
    /// Returns a closed encoding failure.
    pub fn to_json(&self) -> Result<String, PublicContractError> {
        serde_json::to_string_pretty(self).map_err(|_| PublicContractError::MetadataEncoding)
    }

    /// Lists sampled field/type and code differences for every public route.
    #[must_use]
    pub fn changes_from(&self, reference: &Self) -> Vec<String> {
        self.endpoints
            .iter()
            .zip(&reference.endpoints)
            .flat_map(|(current, previous)| current.changes_from(previous))
            .collect()
    }
}
