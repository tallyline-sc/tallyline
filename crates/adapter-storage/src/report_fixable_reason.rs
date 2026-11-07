// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact closed correction reasons in Report lifecycle documents.

use serde::{Deserialize, Serialize};
use tallyline_reporting::api::model::FixableReason;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FixableDocument {
    ScreenshotRequired,
    InvalidObservationTime,
    TooManyRows,
    TerminalNotFound,
    InvalidScreenshot,
    ScreenshotTooLarge,
}
impl From<FixableReason> for FixableDocument {
    fn from(value: FixableReason) -> Self {
        match value {
            FixableReason::ScreenshotRequired => Self::ScreenshotRequired,
            FixableReason::InvalidObservationTime => Self::InvalidObservationTime,
            FixableReason::TooManyRows => Self::TooManyRows,
            FixableReason::TerminalNotFound => Self::TerminalNotFound,
            FixableReason::InvalidScreenshot => Self::InvalidScreenshot,
            FixableReason::ScreenshotTooLarge => Self::ScreenshotTooLarge,
        }
    }
}
impl From<FixableDocument> for FixableReason {
    fn from(value: FixableDocument) -> Self {
        match value {
            FixableDocument::ScreenshotRequired => Self::ScreenshotRequired,
            FixableDocument::InvalidObservationTime => Self::InvalidObservationTime,
            FixableDocument::TooManyRows => Self::TooManyRows,
            FixableDocument::TerminalNotFound => Self::TerminalNotFound,
            FixableDocument::InvalidScreenshot => Self::InvalidScreenshot,
            FixableDocument::ScreenshotTooLarge => Self::ScreenshotTooLarge,
        }
    }
}
