// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Release-check preferences in the shared configuration document.

use crate::config::SettingsIssue;
use tallyline_shared_kernel::api::model::Timestamp;
use tallyline_workflows::api::model::UpdateChecks;

pub(crate) fn load(
    document: &serde_json::Value,
    errors: &mut Vec<SettingsIssue>,
) -> (UpdateChecks, Option<Timestamp>) {
    if !matches!(
        document
            .get("schema_version")
            .and_then(serde_json::Value::as_u64),
        Some(3..=6)
    ) {
        return (UpdateChecks::Disabled, None);
    }
    let app = document.get("app");
    let checks = match app.and_then(|app| app.get("update_checks")) {
        Some(serde_json::Value::Bool(true)) => UpdateChecks::Enabled,
        Some(serde_json::Value::Bool(false)) | None => UpdateChecks::Disabled,
        Some(_) => {
            errors.push(SettingsIssue {
                key: "settings.update_checks",
                rejected: "invalid boolean".to_owned(),
                default: "off",
            });
            UpdateChecks::Disabled
        }
    };
    let attempt = match app.and_then(|app| app.get("last_update_attempt_ms")) {
        None | Some(serde_json::Value::Null) => None,
        Some(value) => match value.as_u64() {
            Some(value) => Some(Timestamp::from_millis(value)),
            None => {
                errors.push(SettingsIssue {
                    key: "settings.last_update_attempt",
                    rejected: "invalid timestamp".to_owned(),
                    default: "checks off",
                });
                return (UpdateChecks::Disabled, None);
            }
        },
    };
    (checks, attempt)
}

pub(crate) fn notice(
    document: &serde_json::Value,
    errors: &mut Vec<SettingsIssue>,
) -> (
    Option<tallyline_workflows::api::model::ReleaseVersion>,
    Option<tallyline_workflows::api::model::ReleaseVersion>,
) {
    if !matches!(
        document
            .get("schema_version")
            .and_then(serde_json::Value::as_u64),
        Some(3..=6)
    ) {
        return (None, None);
    }
    let mut read = |key| match document.get("app").and_then(|app| app.get(key)) {
        None | Some(serde_json::Value::Null) => None,
        Some(value) => match value
            .as_str()
            .and_then(tallyline_workflows::api::model::ReleaseVersion::parse)
        {
            Some(version) => Some(version),
            None => {
                errors.push(SettingsIssue {
                    key: "settings.update_notice",
                    rejected: "invalid release version".to_owned(),
                    default: "no saved notice",
                });
                None
            }
        },
    };
    (read("available_release"), read("dismissed_release"))
}
