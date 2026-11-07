// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Frozen reference metadata and entry-scoped public recordings.

use std::{collections::BTreeSet, fs, path::Path};

use super::field;
use serde_json::{Value, json};

pub(super) fn inspect(
    directory: &Path,
    entry: &Value,
    pins: Option<&BTreeSet<String>>,
) -> Result<Value, String> {
    let path = directory.join("reference/metadata.json");
    if !path.exists() {
        return Ok(json!({"status":"skipped: frozen reference metadata unavailable"}));
    }
    require_pin(pins, "reference/metadata.json")?;
    let metadata = read_json(&super::pinned::contained(
        directory,
        "reference/metadata.json",
    )?)?;
    if field(&metadata, "fetchedAt")
        .as_str()
        .is_none_or(str::is_empty)
        || !matches!(
            field(&metadata, "priorProvenance").as_str(),
            Some("pre-capture" | "post-capture")
        )
    {
        return Err(
            "reference metadata needs fetchedAt and pre-capture or post-capture priorProvenance"
                .to_owned(),
        );
    }
    let recordings = field(&metadata, "recordings")
        .as_object()
        .ok_or_else(|| "reference recordings must map endpoints to files".to_owned())?;
    for (endpoint, file) in recordings {
        let file = file
            .as_str()
            .ok_or_else(|| "recording file must be a string".to_owned())?;
        let relative = format!("reference/{file}");
        let path = super::pinned::contained(directory, &relative)?;
        if !path.exists() {
            return Ok(
                json!({"status":"skipped: frozen reference recording unavailable", "recording":file,
                "fetchedAt":field(&metadata, "fetchedAt"),"priorProvenance":field(&metadata,"priorProvenance"),
                "priorDependentMetrics":"skipped: recognition is not implemented"}),
            );
        }
        require_pin(pins, &relative)?;
        let recording = read_json(&path)?;
        if pins.is_none() {
            public_fixture(endpoint, &recording, entry)?;
        }
    }
    let mut result = metadata;
    let object = result
        .as_object_mut()
        .ok_or_else(|| "reference metadata must be an object".to_owned())?;
    object.insert(
        "status".to_owned(),
        json!("inspected metadata; snapshot replay is not implemented"),
    );
    object.insert(
        "priorDependentMetrics".to_owned(),
        json!("skipped: recognition is not implemented"),
    );
    Ok(result)
}

fn require_pin(pins: Option<&BTreeSet<String>>, name: &str) -> Result<(), String> {
    if pins.is_some_and(|pins| !pins.contains(name)) {
        return Err(format!("private input {name} is not pinned in SHA256SUMS"));
    }
    Ok(())
}

fn read_json(path: &Path) -> Result<Value, String> {
    serde_json::from_str(&fs::read_to_string(path).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())
}

fn public_fixture(endpoint: &str, recording: &Value, entry: &Value) -> Result<(), String> {
    if !matches!(
        endpoint,
        "/commodities"
            | "/terminals?type=commodity"
            | "/commodities_prices"
            | "/commodities_status"
            | "/data_parameters"
            | "/game_versions"
            | "/star_systems"
            | "/planets"
            | "/moons"
            | "/space_stations"
            | "/cities"
            | "/outposts"
            | "/orbits"
    ) {
        return Err(format!(
            "public reference endpoint is not allowed: {endpoint}"
        ));
    }
    reject_personal_fields(recording)?;
    if matches!(
        endpoint,
        "/game_versions" | "/commodities_status" | "/data_parameters"
    ) {
        return if field(recording, "data").is_object() {
            Ok(())
        } else {
            Err("recorded global response data must be an object".to_owned())
        };
    }
    let rows = field(recording, "data")
        .as_array()
        .ok_or_else(|| "recorded response data must be an array".to_owned())?;
    let commodities = field(entry, "captures")
        .as_object()
        .into_iter()
        .flat_map(|captures| captures.values())
        .flat_map(|capture| field(capture, "cards").as_array().into_iter().flatten())
        .filter_map(|card| card["expectedCommodityId"].as_u64())
        .collect::<BTreeSet<_>>();
    let terminal = field(entry, "expectedTerminalId").as_u64();
    for row in rows {
        let outside = match endpoint {
            "/commodities_prices" => {
                field(row, "id_terminal")
                    .as_u64()
                    .is_none_or(|id| Some(id) != terminal)
                    || field(row, "id_commodity")
                        .as_u64()
                        .is_none_or(|id| !commodities.contains(&id))
            }
            "/terminals?type=commodity" => field(row, "id")
                .as_u64()
                .is_none_or(|id| Some(id) != terminal),
            "/commodities" => field(row, "id")
                .as_u64()
                .is_none_or(|id| !commodities.contains(&id)),
            _ => false,
        };
        if outside {
            return Err(format!(
                "public {endpoint} fixture contains rows outside the entry"
            ));
        }
    }
    Ok(())
}

fn reject_personal_fields(value: &Value) -> Result<(), String> {
    match value {
        Value::Object(fields) => {
            for (key, value) in fields {
                if matches!(
                    key.to_ascii_lowercase().as_str(),
                    "username" | "email" | "discord" | "secret_key" | "app_token" | "authorization"
                ) {
                    return Err(format!(
                        "public reference contains a personal or secret field: {key}"
                    ));
                }
                reject_personal_fields(value)?;
            }
        }
        Value::Array(values) => {
            for value in values {
                reject_personal_fields(value)?;
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
    Ok(())
}
