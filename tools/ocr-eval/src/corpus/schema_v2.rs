// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Type checks for schema-version-two metadata.

use super::field;
use serde_json::Value;

pub(super) fn validate(entry: &Value) -> Result<(), String> {
    for key in ["priceCapHeightPx", "displayHeightFraction"] {
        let value = field(field(entry, "measured"), key);
        if !value.is_null() && value.as_f64().is_none_or(|number| number <= 0.0) {
            return Err(format!("measured.{key} must be positive or null"));
        }
    }
    text_or_null(
        field(field(entry, "measured"), "textHeightBand"),
        "measured.textHeightBand",
    )?;
    if let Some(captures) = field(entry, "captures").as_object() {
        for capture in captures.values() {
            text_or_null(field(capture, "capturedAt"), "capturedAt")?;
            for key in [
                "fileResolution",
                "gameResolution",
                "displayResolution",
                "windowMode",
                "renderer",
                "upscaler",
                "windowsHdr",
                "gameHdr",
                "captureMethod",
                "captureToolVersion",
                "captureToolOptions",
                "os",
                "channel",
                "gameVersion",
                "container",
                "transfer",
                "sourceFidelity",
                "originalFile",
                "originalSha256",
            ] {
                text_or_null(field(field(capture, "conditions"), key), key)?;
            }
            for key in [
                "fovSetting",
                "refWhiteNits",
                "peakNits",
                "windowsSdrContentBrightness",
            ] {
                let value = field(field(capture, "conditions"), key);
                if !value.is_null() && !value.is_number() {
                    return Err(format!("condition {key} must be numeric or null"));
                }
            }
            if let Some(headers) = field(capture, "sectionHeaders").as_array() {
                for header in headers {
                    if field(header, "name").as_str().is_none()
                        || !field(header, "expanded").is_boolean()
                        || (!field(header, "beforeCard").is_null()
                            && field(header, "beforeCard").as_u64().is_none())
                    {
                        return Err("section header needs a name, expanded flag and integer or null beforeCard".to_owned());
                    }
                }
            }
            if let Some(cards) = field(capture, "cards").as_array() {
                for card in cards {
                    let id = field(card, "expectedCommodityId");
                    if !id.is_null() && id.as_u64().is_none() {
                        return Err("expectedCommodityId must be an integer or null".to_owned());
                    }
                    if field(card, "partial") == &Value::Bool(true)
                        && !matches!(field(card, "partialEdge").as_str(), Some("top" | "bottom"))
                    {
                        return Err("partial card must have a top or bottom partialEdge".to_owned());
                    }
                }
            }
        }
    }
    Ok(())
}

fn text_or_null(value: &Value, context: &str) -> Result<(), String> {
    if value.is_null() || value.is_string() {
        Ok(())
    } else {
        Err(format!("{context} must be a string or null"))
    }
}
