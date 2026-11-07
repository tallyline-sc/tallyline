// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Separate proposals from hand-placed source-pixel anchors.

use std::{fs, io::Write, path::Path};

use super::field;
use serde_json::{Value, json};

/// Writes an unverified measurement proposal without changing the expected entry.
///
/// # Errors
/// Returns an error for invalid anchors, a non-v2 entry or an existing output file.
pub fn annotate(expected: &Path, anchors: &Path, proposal: &Path) -> Result<(), String> {
    let text = fs::read_to_string(expected).map_err(|error| error.to_string())?;
    super::validate_json(&text)?;
    let mut entry: Value = serde_json::from_str(&text).map_err(|error| error.to_string())?;
    if field(&entry, "schemaVersion") != &json!(2) {
        return Err("annotation requires schema version 2".to_owned());
    }
    let anchors: Value =
        serde_json::from_str(&fs::read_to_string(anchors).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    let top = coordinate(&anchors, "priceTop")?;
    let bottom = coordinate(&anchors, "priceBottom")?;
    let display = coordinate(&anchors, "displayHeight")?;
    if top < 0.0 || bottom <= top || display < bottom {
        return Err("anchors must satisfy 0 <= priceTop < priceBottom <= displayHeight".to_owned());
    }
    let height = bottom - top;
    let band = if height < 6.0 {
        "<6"
    } else if height < 8.0 {
        "6-8"
    } else if height < 10.0 {
        "8-10"
    } else if height < 14.0 {
        "10-14"
    } else {
        ">=14"
    };
    let object = entry
        .as_object_mut()
        .ok_or_else(|| "entry must be an object".to_owned())?;
    object.insert("verified".to_owned(), json!(false));
    object.insert("measured".to_owned(), json!({"priceCapHeightPx":height,"displayHeightFraction":height/display,"textHeightBand":band}));
    let output = serde_json::to_vec_pretty(&entry).map_err(|error| error.to_string())?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(proposal)
        .map_err(|error| error.to_string())?;
    file.write_all(&output).map_err(|error| error.to_string())?;
    file.write_all(b"\n").map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())
}

fn coordinate(anchors: &Value, key: &str) -> Result<f64, String> {
    anchors[key]
        .as_f64()
        .filter(|value| value.is_finite())
        .ok_or_else(|| format!("anchor {key} must be finite"))
}
