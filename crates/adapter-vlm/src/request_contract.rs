// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Explicit answer-request settings and a single truncation retry.

use crate::parse_answer::{self, ParseError};
use std::fmt;
use std::time::Duration;
use tallyline_recognition::api::model::RawTranscription;

/// The backend's supported completion status.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FinishReason {
    /// The backend completed its answer normally.
    Complete,
    /// The backend exhausted its output token limit.
    Length,
}

/// One complete backend response, before deterministic parsing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelAnswer {
    /// Raw Markdown answer text.
    pub text: String,
    /// Backend completion metadata, independent of answer contents.
    pub finish: FinishReason,
}

/// The fixed version-one prompt and explicit generation settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RequestContract {
    token_limit: u32,
}

impl RequestContract {
    /// Returns the exact versioned prompt bytes.
    #[must_use]
    pub const fn prompt(self) -> &'static str {
        include_str!("../prompts/shop_panel_v1.md")
    }
    /// Identifies the exact response and prompt contract.
    #[must_use]
    pub const fn prompt_version(self) -> &'static str {
        "shop_panel_v1"
    }
    /// Returns the deterministic sampling temperature.
    #[must_use]
    pub const fn temperature(self) -> u8 {
        0
    }
    /// Returns the explicit idle model retention interval.
    #[must_use]
    pub const fn keep_alive(self) -> Duration {
        Duration::from_secs(300)
    }
    /// Returns the output token bound for this attempt.
    #[must_use]
    pub const fn token_limit(self) -> u32 {
        self.token_limit
    }
}

/// A terminal answer failure without any report mutation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TranscriptionFailure<E> {
    /// Both permitted answers exhausted their output limits.
    Truncated,
    /// A completed answer does not match the versioned format.
    Unparseable(ParseError),
    /// The supplied backend operation failed.
    Backend(E),
}

impl<E: fmt::Display> fmt::Display for TranscriptionFailure<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => formatter.write_str("answer truncated"),
            Self::Unparseable(error) => error.fmt(formatter),
            Self::Backend(error) => error.fmt(formatter),
        }
    }
}
impl<E: fmt::Debug + fmt::Display> std::error::Error for TranscriptionFailure<E> {}

/// Obtains and parses answers, retrying only a token-limited answer once.
/// The callback supplies answers; this contract owns no images, reports or runtime.
///
/// # Errors
/// Returns the backend error, a format error, or truncation after both attempts.
pub fn parse_with_retry<E>(
    mut answer: impl FnMut(RequestContract) -> Result<ModelAnswer, E>,
) -> Result<RawTranscription, TranscriptionFailure<E>> {
    for token_limit in [4096, 8192] {
        let result =
            answer(RequestContract { token_limit }).map_err(TranscriptionFailure::Backend)?;
        match result.finish {
            FinishReason::Complete => {
                return parse_answer::parse_answer(&result.text)
                    .map_err(TranscriptionFailure::Unparseable);
            }
            FinishReason::Length => continue,
        }
    }
    Err(TranscriptionFailure::Truncated)
}
