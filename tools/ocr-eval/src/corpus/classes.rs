// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Sample readiness per condition, theme and measured text-height band.

use std::collections::{BTreeMap, BTreeSet};

use super::field;
use serde_json::{Value, json};

#[derive(Default)]
struct Samples {
    real: usize,
    synthetic: usize,
    verified: usize,
    numeric: usize,
    terminals: BTreeSet<u64>,
}

pub(super) fn report(entries: &[Value]) -> Value {
    let mut classes = BTreeMap::<String, Samples>::new();
    for (dimension, values) in [
        (
            "fileResolution",
            &[
                "1920x1080",
                "2560x1440",
                "3840x2160",
                "3440x1440",
                "5120x1440",
            ][..],
        ),
        ("windowMode", &["windowed"][..]),
        (
            "captureMethod",
            &["sc-key", "gamebar-png", "win-prtscn", "snipping-tool"][..],
        ),
        ("renderer", &["DX11", "Vulkan"][..]),
        ("windowsHdr", &["off", "on"][..]),
        ("gameHdr", &["off", "on"][..]),
        (
            "textHeightBand",
            &["<6", "6-8", "8-10", "10-14", ">=14"][..],
        ),
        ("theme", &["blue", "orange", "Nyx", "Gateway"][..]),
        ("side", &["BUY", "SELL"][..]),
    ] {
        for value in values {
            classes.entry(format!("{dimension}={value}")).or_default();
        }
    }
    for entry in entries {
        if let Some(captures) = field(field(entry, "expected"), "captures").as_object() {
            for (name, capture) in captures {
                if field(entry, "availableCaptures")
                    .as_array()
                    .is_some_and(|names| names.contains(&json!(name)))
                {
                    add_capture(&mut classes, field(entry, "expected"), capture);
                }
            }
        }
    }
    Value::Object(classes.into_iter().map(|(key, samples)| {
        let status = if samples.verified >= 4 && samples.terminals.len() >= 2 && samples.numeric >= 40 {
            "gated"
        } else if samples.real > 0 { "insufficient" }
        else if samples.synthetic > 0 { "synthetic only" }
        else { "missing" };
        (key, json!({"status":status,"captures":samples.real,"verifiedCaptures":samples.verified,
            "syntheticCaptures":samples.synthetic,"terminals":samples.terminals.len(),"numericFields":samples.numeric,
            "recognition":"not evaluated"}))
    }).collect())
}

fn add_capture(classes: &mut BTreeMap<String, Samples>, entry: &Value, capture: &Value) {
    let synthetic = field(capture, "synthetic")
        .as_str()
        .is_some_and(|id| id.starts_with("synthetic:"));
    let fidelity = field(field(capture, "conditions"), "sourceFidelity").as_str();
    let geometry_known = matches!(fidelity, Some("original" | "original-redacted-lossless"));
    let mut dimensions = BTreeMap::new();
    if let Some(conditions) = field(capture, "conditions").as_object() {
        for (key, value) in conditions {
            if value.is_null() || matches!(key.as_str(), "originalFile" | "originalSha256") {
                continue;
            }
            if !geometry_known && !synthetic {
                continue;
            }
            dimensions.insert(key.clone(), value.clone());
        }
    }
    let theme = field(entry, "theme").as_str().unwrap_or("");
    let family = if theme.to_ascii_lowercase().contains("blue") {
        "blue"
    } else if theme.to_ascii_lowercase().contains("orange") {
        "orange"
    } else if theme.to_ascii_lowercase().contains("nyx")
        || theme.to_ascii_lowercase().contains("grey")
    {
        "Nyx"
    } else {
        theme
    };
    dimensions.insert("theme".to_owned(), json!(family));
    if geometry_known || synthetic {
        dimensions.insert("side".to_owned(), field(capture, "side").clone());
    }
    if !field(field(entry, "measured"), "textHeightBand").is_null() {
        dimensions.insert(
            "textHeightBand".to_owned(),
            field(field(entry, "measured"), "textHeightBand").clone(),
        );
    }
    let compound = Value::Object(dimensions.clone().into_iter().collect()).to_string();
    let mut keys = dimensions
        .into_iter()
        .map(|(key, value)| {
            format!(
                "{key}={}",
                value
                    .as_str()
                    .map_or_else(|| value.to_string(), str::to_owned)
            )
        })
        .collect::<Vec<_>>();
    keys.push(format!("conditions={compound}"));
    if field(entry, "location")
        .to_string()
        .to_ascii_lowercase()
        .contains("gateway")
    {
        keys.push("theme=Gateway".to_owned());
    }
    for key in keys {
        let samples = classes.entry(key).or_default();
        if synthetic {
            samples.synthetic += 1;
            continue;
        }
        samples.real += 1;
        if field(entry, "verified") != &json!(true) {
            continue;
        }
        samples.verified += 1;
        if let Some(terminal) = field(entry, "expectedTerminalId").as_u64() {
            samples.terminals.insert(terminal);
        }
        if let Some(cards) = field(capture, "cards").as_array() {
            samples.numeric += cards
                .iter()
                .filter(|card| card["partial"] != true)
                .map(|card| {
                    ["scu", "pricePerScu"]
                        .iter()
                        .filter(|field| card[**field].is_number())
                        .count()
                })
                .sum::<usize>();
        }
    }
}
