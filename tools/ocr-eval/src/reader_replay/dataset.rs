// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Hash-bound retained observations and exact verified field identities.

use super::ReplayError;
use super::observation::{Field, Observation};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::Read;
use std::path::Path;

const MAX_PACKET_BYTES: u64 = 16 * 1024 * 1024;
const MAX_REFERENCE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_ROWS: usize = 4096;
const MAX_READERS: usize = 16;
const MAX_REFERENCES: usize = 64;

pub(super) struct Dataset {
    pub readers: Vec<String>,
    pub rows: Vec<Row>,
    pub references: Vec<Value>,
    pub sha256: String,
}

pub(super) struct Row {
    pub capture: String,
    pub card: String,
    pub field: Field,
    pub expected: Option<String>,
    pub observations: Vec<Observation>,
}

impl Dataset {
    pub fn read(public: &Path, path: &Path, expected_hash: &str) -> Result<Self, ReplayError> {
        let bytes = bounded_read(path, MAX_PACKET_BYTES)?;
        let sha256 = digest(&bytes);
        if sha256 != expected_hash {
            return Err(ReplayError::PacketHashMismatch);
        }
        let packet: Value =
            serde_json::from_slice(&bytes).map_err(|_| ReplayError::MalformedPacket)?;
        if packet.get("schema_version").and_then(Value::as_u64) != Some(1) {
            return Err(ReplayError::UnsupportedSchema);
        }
        let readers = array(&packet, "readers")?
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .filter(|value| safe_component(value))
                    .map(str::to_owned)
                    .ok_or(ReplayError::InvalidReaders)
            })
            .collect::<Result<Vec<_>, _>>()?;
        if !(2..=MAX_READERS).contains(&readers.len())
            || readers.iter().collect::<BTreeSet<_>>().len() != readers.len()
        {
            return Err(ReplayError::InvalidReaders);
        }
        let source_rows = array(&packet, "rows")?;
        if source_rows.is_empty() || source_rows.len() > MAX_ROWS {
            return Err(ReplayError::RowCountOutOfBounds);
        }
        let mut references = BTreeMap::<String, (String, Value)>::new();
        let mut identities = BTreeSet::new();
        let mut rows = Vec::new();
        for source in source_rows {
            let row = read_row(public, source, readers.len(), &mut references)?;
            if !identities.insert((row.card.clone(), row.field.name())) {
                return Err(ReplayError::DuplicateField);
            }
            rows.push(row);
        }
        Ok(Self {
            readers,
            rows,
            references: references
                .into_iter()
                .map(|(entry, (sha256, _))| json!({"entry":entry,"sha256":sha256,"verified":true}))
                .collect(),
            sha256,
        })
    }
}

fn read_row(
    public: &Path,
    source: &Value,
    readers: usize,
    references: &mut BTreeMap<String, (String, Value)>,
) -> Result<Row, ReplayError> {
    let entry = text(source, "entry")?;
    let capture = text(source, "capture")?;
    if !safe_component(entry) || !safe_component(capture) {
        return Err(ReplayError::UnsafeReferencePath);
    }
    if !references.contains_key(entry) {
        if references.len() >= MAX_REFERENCES {
            return Err(ReplayError::TooManyReferences);
        }
        let root = public
            .canonicalize()
            .map_err(ReplayError::PublicCorpusUnavailable)?;
        let path = public
            .join(entry)
            .join("expected.json")
            .canonicalize()
            .map_err(ReplayError::ReferenceUnavailable)?;
        if !path.starts_with(root) {
            return Err(ReplayError::UnsafeReferencePath);
        }
        let bytes = bounded_read(&path, MAX_REFERENCE_BYTES)?;
        let reference: Value =
            serde_json::from_slice(&bytes).map_err(|_| ReplayError::MalformedReference)?;
        if reference.get("verified").and_then(Value::as_bool) != Some(true) {
            return Err(ReplayError::UnverifiedReference);
        }
        references.insert(entry.to_owned(), (digest(&bytes), reference));
    }
    let (hash, reference) = references
        .get(entry)
        .ok_or(ReplayError::ReferenceFieldUnavailable)?;
    if hash != text(source, "reference_sha256")? {
        return Err(ReplayError::ReferenceHashMismatch);
    }
    let screen_order = source
        .get("screen_order")
        .and_then(Value::as_u64)
        .ok_or(ReplayError::ReferenceFieldUnavailable)?;
    let cards = reference
        .get("captures")
        .and_then(|value| value.get(capture))
        .and_then(|value| value.get("cards"))
        .and_then(Value::as_array)
        .ok_or(ReplayError::ReferenceFieldUnavailable)?;
    let mut cards = cards
        .iter()
        .filter(|card| card.get("screenOrder").and_then(Value::as_u64) == Some(screen_order));
    let card = cards.next().ok_or(ReplayError::ReferenceFieldUnavailable)?;
    if cards.next().is_some() || card.get("partial").and_then(Value::as_bool) == Some(true) {
        return Err(ReplayError::AmbiguousCard);
    }
    let field = Field::parse(text(source, "field")?)?;
    let expected = match card.get(field.name()) {
        None | Some(Value::Null) => None,
        Some(value) => Some(field.canonical(value)?),
    };
    let observations = array(source, "observations")?;
    if observations.len() != readers {
        return Err(ReplayError::IncompleteObservations);
    }
    let observations = observations
        .iter()
        .map(|value| Observation::read(value, field, expected.as_deref()))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Row {
        capture: format!("{entry}/{capture}"),
        card: format!("{entry}/{capture}/{screen_order}"),
        field,
        expected,
        observations,
    })
}

fn safe_component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value != "."
        && value != ".."
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
}

fn text<'a>(value: &'a Value, name: &'static str) -> Result<&'a str, ReplayError> {
    value
        .get(name)
        .and_then(Value::as_str)
        .ok_or(ReplayError::TextRequired(name))
}

fn array<'a>(value: &'a Value, name: &'static str) -> Result<&'a [Value], ReplayError> {
    value
        .get(name)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or(ReplayError::ArrayRequired(name))
}

fn bounded_read(path: &Path, limit: u64) -> Result<Vec<u8>, ReplayError> {
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(ReplayError::InputOpen)?
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(ReplayError::InputRead)?;
    if u64::try_from(bytes.len()).map_err(|_| ReplayError::InputSizeOverflow)? > limit {
        return Err(ReplayError::InputSizeExceeded);
    }
    Ok(bytes)
}

fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
