// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! The base English message catalogue.

use std::collections::BTreeMap;

/// A loaded UI message catalogue.
pub struct Catalogue(BTreeMap<String, String>);

impl Catalogue {
    /// Loads the bundled English messages.
    ///
    /// # Errors
    /// Returns invalid bundled JSON instead of silently dropping messages.
    pub fn english() -> Result<Self, serde_json::Error> {
        serde_json::from_str(include_str!("../messages/en.json")).map(Self)
    }

    /// Returns a message or a visible missing-translation marker.
    #[must_use]
    pub fn text<'a>(&'a self, key: &'a str) -> &'a str {
        self.0
            .get(key)
            .map_or("Missing translation", String::as_str)
    }
}
