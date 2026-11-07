// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Strict bounded decoding of value-free contract metadata.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

use super::shape::FieldKind;
use super::status_codes::StatusCodeSets;
use super::{
    PublicContractEndpoint, PublicContractError, PublicContractSchema, shape, status_codes,
};

const MAX_METADATA_BYTES: usize = 8 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    schema_version: u16,
    endpoints: Vec<SchemaRecord>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SchemaRecord {
    endpoint: PublicContractEndpoint,
    http_status: u16,
    fields: BTreeMap<String, BTreeSet<FieldKind>>,
    status_codes: Option<StatusCodeSets>,
}

pub(super) fn parse(
    input: &str,
    version: u16,
) -> Result<Vec<PublicContractSchema>, PublicContractError> {
    if input.len() > MAX_METADATA_BYTES {
        return Err(PublicContractError::InvalidMetadata);
    }
    let document: Document =
        serde_json::from_str(input).map_err(|_| PublicContractError::InvalidMetadata)?;
    if document.schema_version != version {
        return Err(PublicContractError::InvalidMetadata);
    }
    Ok(document
        .endpoints
        .into_iter()
        .map(|record| PublicContractSchema {
            endpoint: record.endpoint,
            http_status: record.http_status,
            fields: record.fields,
            status_codes: record.status_codes,
        })
        .collect())
}

pub(super) fn valid(schema: &PublicContractSchema) -> bool {
    let fields = &schema.fields;
    let has_kind = |path: &str, kind| {
        fields
            .get(path)
            .is_some_and(|kinds| kinds.len() == 1 && kinds.contains(&kind))
    };
    let data_kind = match schema.endpoint {
        PublicContractEndpoint::Commodities | PublicContractEndpoint::Terminals => FieldKind::Array,
        PublicContractEndpoint::GameVersions
        | PublicContractEndpoint::CommoditiesStatus
        | PublicContractEndpoint::DataParameters => FieldKind::Object,
    };
    (200..300).contains(&schema.http_status)
        && shape::valid_metadata(fields)
        && has_kind("$", FieldKind::Object)
        && has_kind("status", FieldKind::String)
        && fields.get("http_code").is_some_and(|kinds| {
            kinds
                .iter()
                .all(|kind| matches!(kind, FieldKind::Number | FieldKind::String))
        })
        && has_kind("data", data_kind)
        && match data_kind {
            FieldKind::Array => {
                has_kind("data[]", FieldKind::Object)
                    && fields.keys().any(|path| path.starts_with("data[]."))
            }
            FieldKind::Object => fields.keys().any(|path| path.starts_with("data.")),
            FieldKind::Null | FieldKind::Boolean | FieldKind::Number | FieldKind::String => false,
        }
        && status_codes::valid_metadata(schema.endpoint, schema.status_codes.as_ref())
}
