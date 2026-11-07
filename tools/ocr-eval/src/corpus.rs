// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Validation of the hand-authored public OCR corpus.

#[cfg(test)]
use std::{fs, path::Path};

use serde_json::Value;

fn object<'a>(
    value: &'a Value,
    context: &str,
) -> Result<&'a serde_json::Map<String, Value>, String> {
    value
        .as_object()
        .ok_or_else(|| format!("{context} must be an object"))
}

fn required<'a>(value: &'a Value, key: &str, context: &str) -> Result<&'a Value, String> {
    value
        .get(key)
        .ok_or_else(|| format!("{context} is missing `{key}`"))
}

fn string(value: &Value, context: &str) -> Result<(), String> {
    value
        .as_str()
        .map(|_| ())
        .ok_or_else(|| format!("{context} must be a string"))
}

fn validate_entry(value: &Value) -> Result<(), String> {
    let root = object(value, "entry")?;
    for key in ["id", "transcription", "theme"] {
        string(required(value, key, "entry")?, &format!("entry.{key}"))?;
    }
    if !required(value, "verified", "entry")?.is_boolean() {
        return Err("entry.verified must be a boolean".to_owned());
    }
    let version = required(value, "schemaVersion", "entry")?
        .as_u64()
        .ok_or_else(|| "entry.schemaVersion must be an integer".to_owned())?;
    if !matches!(version, 1 | 2) {
        return Err(format!(
            "entry.schemaVersion {version} is not supported (expected 1 or 2)"
        ));
    }
    for key in ["source", "location"] {
        object(required(value, key, "entry")?, &format!("entry.{key}"))?;
    }
    let captures = object(required(value, "captures", "entry")?, "entry.captures")?;
    if captures.is_empty() {
        return Err("entry.captures must contain at least one capture".to_owned());
    }
    for (name, capture) in captures {
        let context = format!("capture `{name}`");
        object(capture, &context)?;
        let side = required(capture, "side", &context)?;
        string(side, &format!("{context}.side"))?;
        if !matches!(side.as_str(), Some("BUY" | "SELL")) {
            return Err(format!("{context}.side must be BUY or SELL"));
        }
        let cards = required(capture, "cards", &context)?
            .as_array()
            .ok_or_else(|| format!("{context}.cards must be an array"))?;
        for (index, card) in cards.iter().enumerate() {
            let card_context = format!("{context}.cards[{index}]");
            object(card, &card_context)?;
            if !required(card, "screenOrder", &card_context)?.is_number() {
                return Err(format!("{card_context}.screenOrder must be a number"));
            }
            if card.get("partial") != Some(&Value::Bool(true)) {
                for key in ["name", "status", "scu", "pricePerScu", "containerSizes"] {
                    let field = required(card, key, &card_context)?;
                    match key {
                        "name" | "status" => string(field, &format!("{card_context}.{key}"))?,
                        "containerSizes" if !field.is_array() && !field.is_null() => {
                            return Err(format!(
                                "{card_context}.containerSizes must be an array or null"
                            ));
                        }
                        "scu" | "pricePerScu" if !field.is_number() && !field.is_null() => {
                            return Err(format!("{card_context}.{key} must be a number or null"));
                        }
                        _ => {}
                    }
                }
            }
            if version == 2 {
                string(
                    required(card, "section", &card_context)?,
                    &format!("{card_context}.section"),
                )?;
            }
        }
        if version == 2 {
            object(
                required(capture, "conditions", &context)?,
                &format!("{context}.conditions"),
            )?;
            required(capture, "capturedAt", &context)?;
            required(capture, "sectionHeaders", &context)?
                .as_array()
                .ok_or_else(|| format!("{context}.sectionHeaders must be an array"))?;
        } else if capture.get("section").is_none() {
            return Err(format!("{context} is missing `section`"));
        }
    }
    if version == 2 {
        object(required(value, "measured", "entry")?, "entry.measured")?;
        schema_v2::validate(value)?;
        if let Some(id) = root.get("expectedTerminalId")
            && !id.is_null()
            && id.as_u64().is_none()
        {
            return Err("entry.expectedTerminalId must be a number".to_owned());
        }
    }
    Ok(())
}

/// Validates one JSON corpus entry against the documented schema.
///
/// # Errors
///
/// Returns an error when the text is invalid JSON or the entry does not match
/// schema version 1 or 2.
pub fn validate_json(text: &str) -> Result<(), String> {
    let value: Value = serde_json::from_str(text).map_err(|error| error.to_string())?;
    validate_entry(&value)
}

#[cfg(test)]
fn public_entries(root: &Path) -> Result<(), String> {
    let public = root.join("../../corpus/public");
    for entry in fs::read_dir(&public).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        if !entry
            .file_type()
            .map_err(|error| error.to_string())?
            .is_dir()
        {
            continue;
        }
        let path = entry.path().join("expected.json");
        let text =
            fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        validate_json(&text).map_err(|error| format!("{}: {error}", path.display()))?;
    }
    Ok(())
}

#[cfg(test)]
#[expect(
    non_snake_case,
    reason = "tagged test names use a double underscore (11 B4)"
)]
mod tests {
    use std::path::Path;

    use super::{public_entries, validate_json};

    #[test]
    fn r_qa_1__every_public_corpus_entry_matches_its_schema() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf();
        let result = public_entries(&root);
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn invalid_schema_version_is_rejected() {
        let fixture = r#"{"schemaVersion":3}"#;
        assert!(validate_json(fixture).is_err());
    }
}

#[cfg(test)]
#[expect(
    non_snake_case,
    reason = "tagged test names use a double underscore (11 B4)"
)]
#[path = "corpus/tests.rs"]
mod inspection_tests;

mod inspect;
pub use inspect::inspect;

mod classes;

mod annotate;
pub use annotate::annotate;

mod pinned;

mod reference;

pub(super) fn field<'a>(value: &'a Value, key: &str) -> &'a Value {
    value.get(key).unwrap_or(&Value::Null)
}

mod schema_v2;
