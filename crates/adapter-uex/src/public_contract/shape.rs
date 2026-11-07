// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Bounded structural projection of untrusted public JSON.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::PublicContractError;

const MAX_DEPTH: usize = 16;
const MAX_FIELDS: usize = 4_096;
const MAX_PATH_BYTES: usize = 256;
const MAX_NAME_BYTES: usize = 64;
const MAX_NODES: usize = 1_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum FieldKind {
    Null,
    Boolean,
    Number,
    String,
    Object,
    Array,
}

pub(super) fn fields(
    value: &Value,
) -> Result<BTreeMap<String, BTreeSet<FieldKind>>, PublicContractError> {
    let mut fields = BTreeMap::new();
    let mut nodes = 0;
    visit(value, "$", 0, &mut nodes, &mut fields)?;
    Ok(fields)
}

fn visit(
    value: &Value,
    path: &str,
    depth: usize,
    nodes: &mut usize,
    fields: &mut BTreeMap<String, BTreeSet<FieldKind>>,
) -> Result<(), PublicContractError> {
    *nodes = nodes.saturating_add(1);
    if depth > MAX_DEPTH || *nodes > MAX_NODES || path.len() > MAX_PATH_BYTES {
        return Err(PublicContractError::Complexity);
    }
    let kind = match value {
        Value::Null => FieldKind::Null,
        Value::Bool(_) => FieldKind::Boolean,
        Value::Number(_) => FieldKind::Number,
        Value::String(_) => FieldKind::String,
        Value::Object(_) => FieldKind::Object,
        Value::Array(_) => FieldKind::Array,
    };
    fields.entry(path.to_owned()).or_default().insert(kind);
    if fields.len() > MAX_FIELDS {
        return Err(PublicContractError::Complexity);
    }
    match value {
        Value::Object(object) => {
            for (name, child) in object {
                if !valid_name(name) {
                    return Err(PublicContractError::UnsupportedFieldName);
                }
                let child_path = if path == "$" {
                    name.clone()
                } else {
                    format!("{path}.{name}")
                };
                visit(child, &child_path, depth + 1, nodes, fields)?;
            }
            Ok(())
        }
        Value::Array(array) => {
            let child_path = format!("{path}[]");
            for child in array {
                visit(child, &child_path, depth + 1, nodes, fields)?;
            }
            Ok(())
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => Ok(()),
    }
}

fn valid_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    !name.is_empty()
        && name.len() <= MAX_NAME_BYTES
        && bytes
            .next()
            .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

pub(super) fn valid_metadata(fields: &BTreeMap<String, BTreeSet<FieldKind>>) -> bool {
    !fields.is_empty()
        && fields.len() <= MAX_FIELDS
        && fields.iter().all(|(path, kinds)| {
            !kinds.is_empty()
                && path.len() <= MAX_PATH_BYTES
                && (path == "$"
                    || path
                        .split('.')
                        .all(|part| valid_name(part.trim_end_matches("[]"))))
        })
}
