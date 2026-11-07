// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact schema-twelve fixture emitted through its source-bound public library.

use std::path::Path;

pub struct PreviousFixture {
    pub report: String,
    pub job: String,
}
pub fn create(path: &Path) -> Result<PreviousFixture, Box<dyn std::error::Error>> {
    let mut fixture: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/schema-twelve.json"))?;
    let schema: Vec<String> = serde_json::from_value(
        fixture
            .get_mut("schema")
            .ok_or("missing fixture schema")?
            .take(),
    )?;
    let report: (i64, i64, String, String) = serde_json::from_value(
        fixture
            .get_mut("report")
            .ok_or("missing fixture report")?
            .take(),
    )?;
    let job: (i64, i64, i64, String, String, String, i64) =
        serde_json::from_value(fixture.get_mut("job").ok_or("missing fixture job")?.take())?;
    let db = rusqlite::Connection::open(path)?;
    for statement in schema {
        db.execute_batch(&statement)?;
    }
    db.pragma_update(None, "user_version", 12)?;
    db.execute(
        "INSERT INTO reports(id,version,payload,phase) VALUES(?1,?2,?3,?4)",
        (&report.0, &report.1, &report.2, &report.3),
    )?;
    db.execute("INSERT INTO submission_jobs(id,report_id,revision,phase,payload,release_report,release_report_version) VALUES(?1,?2,?3,?4,?5,?6,?7)",(&job.0,&job.1,&job.2,&job.3,&job.4,&job.5,&job.6))?;
    Ok(PreviousFixture {
        report: report.2,
        job: job.4,
    })
}
