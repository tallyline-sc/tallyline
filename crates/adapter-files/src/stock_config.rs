// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Independent SCU preference validation at the JSON boundary.

use crate::api::model::SettingsIssue;
use tallyline_reporting::api::model::StockSettings;
use tallyline_shared_kernel::api::model::{Percent, ScuQuantity};

pub(crate) fn migrate(document: &mut serde_json::Value) {
    let baseline = StockSettings::baseline().tolerance();
    if let Some(app) = document
        .get_mut("app")
        .and_then(serde_json::Value::as_object_mut)
    {
        app.entry("stock_percent")
            .or_insert(serde_json::json!(baseline.percent().get()));
        app.entry("stock_minimum")
            .or_insert(serde_json::json!(baseline.minimum().to_string()));
    }
}
pub(crate) fn load(
    document: &serde_json::Value,
    errors: &mut Vec<SettingsIssue>,
) -> std::io::Result<StockSettings> {
    let baseline = StockSettings::baseline().tolerance();
    let app = document.get("app");
    let current = document
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        .is_some_and(|version| version >= 5);
    let percent = app.and_then(|app| app.get("stock_percent"));
    let minimum = app.and_then(|app| app.get("stock_minimum"));
    let percent = match percent
        .and_then(serde_json::Value::as_u64)
        .filter(|value| *value <= u64::from(baseline.percent().get()))
    {
        Some(value) => Percent::whole(u32::try_from(value).map_err(std::io::Error::other)?),
        None => {
            if current || percent.is_some() {
                issue(errors, "settings.stock_percent", percent, "20");
            }
            baseline.percent()
        }
    };
    let minimum = match minimum
        .and_then(serde_json::Value::as_str)
        .and_then(|value| value.parse::<ScuQuantity>().ok())
        .filter(|value| *value <= baseline.minimum())
    {
        Some(value) => value,
        None => {
            if current || minimum.is_some() {
                issue(errors, "settings.stock_minimum", minimum, "16");
            }
            baseline.minimum()
        }
    };
    StockSettings::new(percent, minimum)
        .map_err(|_| std::io::Error::other("validated stock settings are invalid"))
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
