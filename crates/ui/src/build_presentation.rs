// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Build identity presentation injected by the native composition root.

/// Public build data for About; the UI makes no source-authentication decision.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum BuildPresentation {
    /// No composition-root identity was supplied; no version is invented.
    #[default]
    Unavailable,
    /// The fixed development identity, with no release claim.
    Development {
        /// Validated version text supplied by the build-info producer.
        version: String,
        /// Compiled operating-system/architecture label.
        platform: String,
    },
    /// Complete release metadata; its provenance remains the release builder's responsibility.
    Release {
        /// Version derived from the injected tag.
        version: String,
        /// Compiled operating-system/architecture label.
        platform: String,
        /// Canonical release tag used for its source-archive link.
        tag: String,
        /// Canonical source commit used for the source-code link.
        commit: String,
    },
}
