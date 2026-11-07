// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Bounded quantity-only research without recognition or annotation writes.

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs::File, io::Read, path::Path};

mod sensitivity;

const MAX_ENTRIES: usize = 64;
const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_ROWS: usize = 65_536;

/// Counts complete whole-SCU transcriptions without treating them as recognised or verified.
///
/// # Errors
/// Refuses malformed metadata, unsupported numeric quantities and bounded-input violations.
pub fn analyse(root: &Path) -> Result<Value, String> {
    let mut paths = Vec::new();
    for entry in std::fs::read_dir(root).map_err(|_| "public corpus cannot be read")? {
        let path = entry
            .map_err(|_| "public corpus entry cannot be read")?
            .path()
            .join("expected.json");
        if path.is_file() {
            paths.push(path);
        }
        if paths.len() > MAX_ENTRIES {
            return Err("public corpus exceeds the research entry bound".to_owned());
        }
    }
    paths.sort();
    if paths.is_empty() {
        return Err("public corpus has no expected metadata".to_owned());
    }
    let mut quantities = BTreeMap::<u64, usize>::new();
    let mut sources = Vec::new();
    let mut quantity_rows = 0;
    let mut excluded_partial = 0;
    let mut missing_quantity = 0;
    let mut missing_identity = 0;
    let mut verified_entries = 0;
    let mut rows = 0;
    for path in paths {
        let mut bytes = Vec::new();
        File::open(path)
            .map_err(|_| "expected metadata cannot be opened")?
            .take(MAX_FILE_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "expected metadata cannot be read")?;
        if u64::try_from(bytes.len()).map_err(|_| "expected metadata size cannot be represented")?
            > MAX_FILE_BYTES
        {
            return Err("expected metadata exceeds the research file bound".to_owned());
        }
        let entry: Value =
            serde_json::from_slice(&bytes).map_err(|_| "expected metadata is malformed")?;
        if entry.get("schemaVersion").and_then(Value::as_u64) != Some(2) {
            return Err("research requires corpus schema two".to_owned());
        }
        let verified = entry
            .get("verified")
            .and_then(Value::as_bool)
            .ok_or("expected metadata has no verification flag")?;
        verified_entries += usize::from(verified);
        sources
            .push(json!({"sha256": Sha256::digest(&bytes).iter().map(|byte| format!("{byte:02x}")).collect::<String>(), "verified": verified}));
        for capture in entry
            .get("captures")
            .and_then(Value::as_object)
            .ok_or("expected captures are malformed")?
            .values()
        {
            for card in capture
                .get("cards")
                .and_then(Value::as_array)
                .ok_or("expected cards are malformed")?
            {
                rows += 1;
                if rows > MAX_ROWS {
                    return Err("corpus exceeds the research row bound".to_owned());
                }
                match card.get("partial") {
                    Some(Value::Bool(true)) => {
                        excluded_partial += 1;
                        continue;
                    }
                    Some(Value::Bool(false)) | None => (),
                    Some(_) => return Err("expected completeness is malformed".to_owned()),
                }
                let quantity = match card.get("scu") {
                    Some(Value::Null) | None => {
                        missing_quantity += 1;
                        continue;
                    }
                    Some(value) => value
                        .as_u64()
                        .ok_or("research supports only non-negative whole-SCU transcriptions")?,
                };
                if card
                    .get("expectedCommodityId")
                    .and_then(Value::as_u64)
                    .filter(|id| *id > 0)
                    .is_none()
                {
                    missing_identity += 1;
                }
                *quantities.entry(quantity).or_default() += 1;
                quantity_rows += 1;
            }
        }
    }
    Ok(json!({
        "schema_version": 1,
        "candidates": sensitivity::candidates(&quantities)?,
        "recognition": "not_run",
        "accuracy": "not_measured",
        "evidence": "expected quantities only; synthetic policy research is not golden accuracy",
        "sources": sources,
        "verified_entries": verified_entries,
        "quantity_rows": quantity_rows,
        "distinct_quantities": quantities.len(),
        "excluded_partial": excluded_partial,
        "missing_quantity": missing_quantity,
        "quantity_rows_without_identity": missing_identity,
        "quantities": quantities.iter().map(|(scu, rows)| json!({"scu": scu, "rows": rows})).collect::<Vec<_>>(),
    }))
}
