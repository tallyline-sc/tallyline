// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Canonical stable release identity shared by update and submission policy.

/// A standard numeric release tag without prerelease or build metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ReleaseVersion(u64, u64, u64);

impl ReleaseVersion {
    /// Parses the release process's exact `vMAJOR.MINOR.PATCH` format.
    #[must_use]
    pub fn parse(tag: &str) -> Option<Self> {
        let mut parts = tag.strip_prefix('v')?.split('.');
        let major = version_component(parts.next()?)?;
        let minor = version_component(parts.next()?)?;
        let patch = version_component(parts.next()?)?;
        if parts.next().is_some() {
            return None;
        }
        Some(Self(major, minor, patch))
    }

    /// Returns the fixed release-page URL for this version.
    #[must_use]
    pub fn release_url(self) -> String {
        format!("https://github.com/tallyline-sc/tallyline/releases/tag/{self}")
    }
}

fn version_component(value: &str) -> Option<u64> {
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    value.parse().ok()
}

impl std::fmt::Display for ReleaseVersion {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "v{}.{}.{}", self.0, self.1, self.2)
    }
}
