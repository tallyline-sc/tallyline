// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Independent fail-closed submission preference validation at the JSON boundary.

use crate::api::model::SettingsIssue;
use tallyline_shared_kernel::api::model::{ObservationAgeSettings, SubmissionMode};

pub(crate) fn migrate(document: &mut serde_json::Value) {
    if let Some(app) = document
        .get_mut("app")
        .and_then(serde_json::Value::as_object_mut)
    {
        app.insert("submission_mode".to_owned(), serde_json::Value::Null);
        app.insert(
            "observation_age_ms".to_owned(),
            serde_json::json!(3_600_000),
        );
    }
}
pub(crate) fn load(
    document: &serde_json::Value,
    errors: &mut Vec<SettingsIssue>,
) -> (Option<SubmissionMode>, ObservationAgeSettings) {
    let current = document
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        == Some(6);
    if !current {
        return (None, ObservationAgeSettings::default());
    }
    let mode = document
        .get("app")
        .and_then(|app| app.get("submission_mode"));
    let mode = match mode {
        Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::String(mode)) if mode == "dry_run" => Some(SubmissionMode::DryRun),
        Some(serde_json::Value::String(mode)) if mode == "production" => {
            Some(SubmissionMode::Production)
        }
        invalid => {
            issue(errors, "settings.submission_mode", invalid, "dry_run");
            Some(SubmissionMode::DryRun)
        }
    };
    let value = document
        .get("app")
        .and_then(|app| app.get("observation_age_ms"));
    let age = value.and_then(serde_json::Value::as_u64).and_then(|value| {
        ObservationAgeSettings::new(std::time::Duration::from_millis(value)).ok()
    });
    let age = match age {
        Some(age) => age,
        None => {
            issue(errors, "settings.observation_age_ms", value, "3600000");
            ObservationAgeSettings::default()
        }
    };
    (mode, age)
}
fn issue(
    errors: &mut Vec<SettingsIssue>,
    key: &'static str,
    value: Option<&serde_json::Value>,
    default: &'static str,
) {
    errors.push(SettingsIssue {
        key,
        rejected: value.map_or_else(|| "missing".to_owned(), serde_json::Value::to_string),
        default,
    });
}
