// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Allowlisted evaluation output for sanitised witness facts.

use crate::game_log::GameLogFiles;
use tallyline_game::api::GameLogSource;
use tallyline_game::api::model::{WitnessSnapshot, WitnessUnavailable};

impl GameLogFiles {
    /// Explicitly opts into a read-only evaluation with allowlisted JSON output.
    #[must_use]
    pub fn evaluate(&self) -> serde_json::Value {
        match self.read() {
            WitnessSnapshot::Off => serde_json::json!({"state":"off"}),
            WitnessSnapshot::Unavailable(reason) => {
                serde_json::json!({"state":"not_available","reason":reason_code(reason)})
            }
            WitnessSnapshot::Available(facts) => {
                let rows: Vec<_> = facts
                    .rows()
                    .iter()
                    .map(|row| {
                        serde_json::json!({
                            "logged_at_unix_millis":row.logged_at().millis(),
                            "terminal_id":row.terminal().map(|id| id.0),
                            "commodity_id":row.commodity().map(|id| id.0),
                            "container_sizes":row.container_sizes(),
                        })
                    })
                    .collect();
                let (major, minor) = facts.major_minor();
                serde_json::json!({"state":"on","major_minor":[major,minor],"buy_side_rows":rows})
            }
        }
    }
}

fn reason_code(reason: WitnessUnavailable) -> &'static str {
    match reason {
        WitnessUnavailable::UnvalidatedBuild => "unvalidated_build",
        WitnessUnavailable::ReadFailure => "read_failure",
        WitnessUnavailable::UnvalidatedFormat => "unvalidated_format",
        WitnessUnavailable::NotConfigured => "not_configured",
        WitnessUnavailable::InputLimit => "input_limit",
    }
}
