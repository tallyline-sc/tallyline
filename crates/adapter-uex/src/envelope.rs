// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! The UEX response envelope `{"status", "http_code", "data", "message"}` (R-API-5).

use serde_json::Value;

use crate::api::{ReadError, RejectedData};

const OK_STATUS: &str = "ok";

/// Returns the `data` of an `ok` envelope, or the [`ReadError`] the answer stands for.
pub(crate) fn data(http_status: u16, body: &[u8]) -> Result<Value, ReadError> {
    let Ok(answer) = serde_json::from_slice::<Value>(body) else {
        return Err(ReadError::NotJson {
            http_status,
            cf_ray: None,
        });
    };
    let Value::Object(mut envelope) = answer else {
        return Err(ReadError::Malformed {
            detail: "the answer is not a JSON object".to_owned(),
        });
    };
    let Some(Value::String(status)) = envelope.remove("status") else {
        return Err(ReadError::Malformed {
            detail: "the envelope has no status string".to_owned(),
        });
    };
    if status != OK_STATUS {
        let message = match envelope.remove("message") {
            Some(Value::String(message)) => message,
            Some(_) | None => String::new(),
        };
        let data = envelope
            .remove("data")
            .filter(|value| !value.is_null())
            .map(|value| rejected_data(value, &status));
        return Err(ReadError::Rejected {
            http_status,
            code: status,
            message,
            data,
        });
    }
    envelope.remove("data").ok_or_else(|| ReadError::Malformed {
        detail: "the ok envelope has no data".to_owned(),
    })
}

fn rejected_data(value: Value, code: &str) -> RejectedData {
    if code == "exceeded_daily_quota"
        && let Some(reset) = value
            .get("reset")
            .and_then(Value::as_u64)
            .filter(|reset| *reset > 0)
    {
        return RejectedData::QuotaReset {
            reset,
            observed_at: None,
        };
    }
    if let Some(row) = value.get("row").and_then(Value::as_u64)
        && let Ok(row) = usize::try_from(row)
    {
        return RejectedData::Row(row);
    }
    if let Some(report) = value.get("report")
        && let (Some(id_terminal), Some(id_commodity)) = (
            report.get("id_terminal").and_then(Value::as_u64),
            report.get("id_commodity").and_then(Value::as_u64),
        )
    {
        return RejectedData::Report {
            id_terminal,
            id_commodity,
        };
    }
    RejectedData::Other
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::data;
    use crate::api::ReadError;

    #[test]
    fn a_json_value_that_is_not_an_object_is_malformed() {
        assert!(matches!(
            data(200, b"[1, 2]"),
            Err(ReadError::Malformed { .. })
        ));
    }

    #[test]
    fn an_object_without_a_status_is_malformed() {
        assert!(matches!(
            data(200, br#"{"data": 1}"#),
            Err(ReadError::Malformed { .. })
        ));
    }

    #[test]
    fn an_ok_envelope_without_data_is_malformed() {
        assert!(matches!(
            data(200, br#"{"status": "ok"}"#),
            Err(ReadError::Malformed { .. })
        ));
    }

    #[test]
    fn an_error_envelope_without_a_message_has_an_empty_message() {
        assert_eq!(
            data(500, br#"{"status": "error", "http_code": 500}"#),
            Err(ReadError::Rejected {
                http_status: 500,
                code: "error".to_owned(),
                message: String::new(),
                data: None,
            })
        );
    }

    #[test]
    fn an_ok_envelope_yields_its_data() {
        assert_eq!(
            data(200, br#"{"status": "ok", "data": {"live": "4.10.1"}}"#),
            Ok(json!({"live": "4.10.1"}))
        );
    }
}
