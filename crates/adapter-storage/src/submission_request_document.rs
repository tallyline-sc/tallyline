// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared exact request-field persistence mapping.

use serde::{Deserialize, Serialize};
use tallyline_shared_kernel::api::model::{SubmissionMode, TradeSide, UexEnvironment};
use tallyline_submission::api::model::{SubmissionHeader, SubmissionRow};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HeaderDocument {
    pub(crate) report: u64,
    pub(crate) report_version: u64,
    pub(crate) account: String,
    pub(crate) mode: ModeDocument,
    pub(crate) terminal: u32,
    pub(crate) side: SideDocument,
    pub(crate) environment: EnvironmentDocument,
    pub(crate) game_version: String,
    pub(crate) faction_affinity: Option<i8>,
    pub(crate) observed_at: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RowDocument {
    pub(crate) id: u64,
    pub(crate) commodity: u32,
    pub(crate) price: String,
    pub(crate) stock: String,
    pub(crate) status: u8,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ModeDocument {
    DryRun,
    Production,
}
impl From<SubmissionMode> for ModeDocument {
    fn from(value: SubmissionMode) -> Self {
        match value {
            SubmissionMode::DryRun => Self::DryRun,
            SubmissionMode::Production => Self::Production,
        }
    }
}
impl From<ModeDocument> for SubmissionMode {
    fn from(value: ModeDocument) -> Self {
        match value {
            ModeDocument::DryRun => Self::DryRun,
            ModeDocument::Production => Self::Production,
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SideDocument {
    Buy,
    Sell,
}
impl From<TradeSide> for SideDocument {
    fn from(value: TradeSide) -> Self {
        match value {
            TradeSide::Buy => Self::Buy,
            TradeSide::Sell => Self::Sell,
        }
    }
}
impl From<SideDocument> for TradeSide {
    fn from(value: SideDocument) -> Self {
        match value {
            SideDocument::Buy => Self::Buy,
            SideDocument::Sell => Self::Sell,
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EnvironmentDocument {
    Live,
    Ptu,
}
impl From<UexEnvironment> for EnvironmentDocument {
    fn from(value: UexEnvironment) -> Self {
        match value {
            UexEnvironment::Live => Self::Live,
            UexEnvironment::Ptu => Self::Ptu,
        }
    }
}
impl From<EnvironmentDocument> for UexEnvironment {
    fn from(value: EnvironmentDocument) -> Self {
        match value {
            EnvironmentDocument::Live => Self::Live,
            EnvironmentDocument::Ptu => Self::Ptu,
        }
    }
}

impl From<&SubmissionHeader> for HeaderDocument {
    fn from(header: &SubmissionHeader) -> Self {
        Self {
            report: header.report.0,
            report_version: header.report_version,
            account: header.account.clone(),
            mode: header.mode.into(),
            terminal: header.terminal.0,
            side: header.side.into(),
            environment: header.environment.into(),
            game_version: header.game_version.clone(),
            faction_affinity: header.faction_affinity.map(|value| value.value()),
            observed_at: header.observed_at.millis().to_string(),
        }
    }
}
impl From<&SubmissionRow> for RowDocument {
    fn from(row: &SubmissionRow) -> Self {
        Self {
            id: row.id().0,
            commodity: row.commodity().0,
            price: row.price().to_string(),
            stock: row.stock().to_string(),
            status: row.status(),
        }
    }
}
