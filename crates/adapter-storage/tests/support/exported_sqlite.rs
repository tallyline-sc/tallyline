// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Restores retained SQLite exports without modifying their schema provenance.

pub(super) fn restore(
    path: &std::path::Path,
    encoded: &str,
    expected_version: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let fixture: serde_json::Value = serde_json::from_str(encoded)?;
    if fixture
        .get("user_version")
        .and_then(serde_json::Value::as_u64)
        != Some(u64::from(expected_version))
    {
        return Err("invalid fixture schema version".into());
    }
    let db = rusqlite::Connection::open(path)?;
    for statement in fixture
        .get("schema")
        .ok_or("missing fixture schema")?
        .as_array()
        .ok_or("missing fixture schema")?
    {
        db.execute_batch(statement.as_str().ok_or("invalid fixture statement")?)?;
    }
    db.execute_batch("BEGIN; PRAGMA defer_foreign_keys=ON;")?;
    for table in fixture
        .get("tables")
        .ok_or("missing fixture tables")?
        .as_array()
        .ok_or("missing fixture tables")?
    {
        for row in table
            .get("rows")
            .ok_or("missing fixture rows")?
            .as_array()
            .ok_or("missing fixture rows")?
        {
            let values = row
                .as_array()
                .ok_or("invalid fixture row")?
                .iter()
                .map(|value| {
                    Ok(match value {
                        serde_json::Value::Null => rusqlite::types::Value::Null,
                        serde_json::Value::String(value) => {
                            rusqlite::types::Value::Text(value.clone())
                        }
                        serde_json::Value::Number(value) => rusqlite::types::Value::Integer(
                            value.as_i64().ok_or("invalid fixture integer")?,
                        ),
                        serde_json::Value::Bool(_)
                        | serde_json::Value::Array(_)
                        | serde_json::Value::Object(_) => {
                            return Err("unsupported fixture value".into());
                        }
                    })
                })
                .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;
            let placeholders = vec!["?"; values.len()].join(",");
            db.execute(
                &format!(
                    "INSERT INTO {} VALUES({placeholders})",
                    table
                        .get("name")
                        .ok_or("missing fixture name")?
                        .as_str()
                        .ok_or("invalid fixture table")?
                ),
                rusqlite::params_from_iter(values),
            )?;
        }
    }
    db.execute_batch("COMMIT;")?;
    db.pragma_update(None, "user_version", expected_version)?;
    Ok(())
}
