// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Permanent refusal policy translated independently of opaque technical codes.

use crate::api::model::UexErrorCode;

/// The required correction after a permanent refusal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubmissionCorrection {
    /// Review a suspected client defect before creating another Draft.
    ReviewClient,
    /// Capture a new observation with a valid game version.
    NewCapture,
}
/// A permanent refusal with a closed correction and an opaque display identifier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubmissionRejection {
    code: UexErrorCode,
    correction: SubmissionCorrection,
}
impl SubmissionRejection {
    /// Retains the adapter's closed classification and bounded technical identifier.
    #[must_use]
    pub const fn new(code: UexErrorCode, correction: SubmissionCorrection) -> Self {
        Self { code, correction }
    }
    /// Borrows the opaque technical identifier for presentation.
    #[must_use]
    pub const fn code(&self) -> &UexErrorCode {
        &self.code
    }
    /// Returns the correction without interpreting the technical identifier.
    #[must_use]
    pub const fn correction(&self) -> SubmissionCorrection {
        self.correction
    }
}
