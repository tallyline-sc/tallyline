// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact accepted ONNX Runtime binding feature closure.

use std::collections::BTreeSet;

pub(crate) fn violation(name: &str, version: &str, features: &[&str]) -> Option<String> {
    let additional: &[&str] = match name {
        "ort" => &["std", "load-dynamic", "preload-dylibs"],
        "ort-sys" => &["std", "disable-linking"],
        _ => return None,
    };
    let mut expected: BTreeSet<_> = (17..=31).map(|api| format!("api-{api}")).collect();
    expected.extend(additional.iter().map(|feature| (*feature).to_owned()));
    let actual: BTreeSet<_> = features
        .iter()
        .map(|feature| (*feature).to_owned())
        .collect();
    if version != "2.0.0-rc.14" || actual != expected || actual.len() != features.len() {
        Some(format!(
            "{name} {version} must use the exact ADR-0007 rc.14 dynamic API-31 feature closure; defaults, downloads, linking and alternate providers are not approved"
        ))
    } else {
        None
    }
}

pub(crate) fn edge_violation(caller: &str, dependency: &str) -> Option<String> {
    let permitted = match dependency {
        "ort" => "tallyline-adapter-ocr",
        "ort-sys" => "ort",
        _ => return None,
    };
    if caller == permitted {
        None
    } else {
        Some(format!(
            "{caller} must not depend on {dependency}; only {permitted} may cross this private native boundary"
        ))
    }
}
