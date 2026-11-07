// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Immutable compiled identity; authentication of the release source remains upstream.

use tallyline_workflows::api::model::ReleaseVersion;

const DEVELOPMENT_VERSION: &str = "0.0.0";
const DEVELOPMENT_TAG_BASELINE: &str = "v0.0.0";
const MAX_RELEASE_TAG_BYTES: usize = 64;
const COMMIT_HEX_BYTES: usize = 40;

/// Whether compile-time metadata claims a release or the fixed local development identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildKind {
    /// No release metadata was injected; this is never release provenance.
    Development,
    /// Complete canonical tag/commit metadata was injected by the source build.
    ///
    /// This value alone authenticates no signature, CI run or source commit.
    Release,
}

/// Supported target identity derived from the app's compilation configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildTarget {
    /// Windows x64 using the reviewed MSVC target.
    WindowsX64,
    /// Linux x64 using the reviewed GNU target.
    LinuxX64,
}

impl BuildTarget {
    /// Returns the exact target spelling used by prebuilt package admission.
    #[must_use]
    pub const fn triple(self) -> &'static str {
        match self {
            Self::WindowsX64 => "x86_64-pc-windows-msvc",
            Self::LinuxX64 => "x86_64-unknown-linux-gnu",
        }
    }

    /// Returns the existing About platform label without reading the running machine.
    #[must_use]
    pub const fn platform(self) -> &'static str {
        match self {
            Self::WindowsX64 => "windows/x86_64",
            Self::LinuxX64 => "linux/x86_64",
        }
    }
}

/// Invalid or incomplete compile-time identity; raw input values are never retained.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildInformationError {
    /// Exactly one release metadata field was supplied.
    PartialReleaseMetadata,
    /// The tag is not the canonical stable release-process format.
    InvalidReleaseTag,
    /// The source commit is not a canonical lower-case forty-digit Git object ID.
    InvalidSourceCommit,
    /// The compilation target is outside the approved application targets.
    UnsupportedTarget,
}

impl std::fmt::Display for BuildInformationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::PartialReleaseMetadata => "incomplete compiled release metadata",
            Self::InvalidReleaseTag => "invalid compiled release tag",
            Self::InvalidSourceCommit => "invalid compiled source commit",
            Self::UnsupportedTarget => "unsupported compiled application target",
        })
    }
}

impl std::error::Error for BuildInformationError {}

/// Immutable version information consumed consistently by UI, logs and client headers.
///
/// The source builder must authenticate the signed tag before injection. This type
/// validates consistency and spelling, not source or signature authenticity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildInformation {
    kind: BuildKind,
    version: &'static str,
    tag: Option<&'static str>,
    commit: Option<&'static str>,
    target: BuildTarget,
    update_baseline: ReleaseVersion,
}

impl BuildInformation {
    /// Validates immutable compile-time release fields or chooses the fixed Development identity.
    ///
    /// Neither the Cargo package version nor optimisation profile determines a release.
    ///
    /// # Errors
    /// Returns partial fields, a malformed tag/commit or invalid fixed development data.
    pub fn new(
        tag: Option<&'static str>,
        commit: Option<&'static str>,
        target: BuildTarget,
    ) -> Result<Self, BuildInformationError> {
        match (tag, commit) {
            (None, None) => {
                let update_baseline = ReleaseVersion::parse(DEVELOPMENT_TAG_BASELINE)
                    .ok_or(BuildInformationError::InvalidReleaseTag)?;
                Ok(Self {
                    kind: BuildKind::Development,
                    version: DEVELOPMENT_VERSION,
                    tag: None,
                    commit: None,
                    target,
                    update_baseline,
                })
            }
            (Some(tag), Some(commit)) => {
                if tag.len() > MAX_RELEASE_TAG_BYTES {
                    return Err(BuildInformationError::InvalidReleaseTag);
                }
                let update_baseline =
                    ReleaseVersion::parse(tag).ok_or(BuildInformationError::InvalidReleaseTag)?;
                let version = tag
                    .strip_prefix('v')
                    .ok_or(BuildInformationError::InvalidReleaseTag)?;
                if commit.len() != COMMIT_HEX_BYTES
                    || !commit
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                {
                    return Err(BuildInformationError::InvalidSourceCommit);
                }
                Ok(Self {
                    kind: BuildKind::Release,
                    version,
                    tag: Some(tag),
                    commit: Some(commit),
                    target,
                    update_baseline,
                })
            }
            (Some(_), None) | (None, Some(_)) => Err(BuildInformationError::PartialReleaseMetadata),
        }
    }

    /// Returns the explicit development/release metadata classification.
    #[must_use]
    pub const fn kind(self) -> BuildKind {
        self.kind
    }

    /// Returns the one version string used by all application identity consumers.
    #[must_use]
    pub const fn version(self) -> &'static str {
        self.version
    }

    /// Returns the injected tag only for a complete release metadata tuple.
    #[must_use]
    pub const fn release_tag(self) -> Option<&'static str> {
        self.tag
    }

    /// Returns the injected source commit; it is not a verified signature claim.
    #[must_use]
    pub const fn source_commit(self) -> Option<&'static str> {
        self.commit
    }

    /// Returns the compiled target identity.
    #[must_use]
    pub const fn target(self) -> BuildTarget {
        self.target
    }

    /// Returns the release-notice comparison baseline; Development uses zero without claiming a tag.
    #[must_use]
    pub const fn update_baseline(self) -> ReleaseVersion {
        self.update_baseline
    }
}

/// Reads only immutable compile-time metadata, without runtime environment, filesystem or provider access.
///
/// # Errors
/// Returns incomplete/invalid release fields or an unsupported compiled target.
pub fn compiled_build_information() -> Result<BuildInformation, BuildInformationError> {
    let target = if cfg!(all(
        target_os = "windows",
        target_arch = "x86_64",
        target_env = "msvc"
    )) {
        BuildTarget::WindowsX64
    } else if cfg!(all(
        target_os = "linux",
        target_arch = "x86_64",
        target_env = "gnu"
    )) {
        BuildTarget::LinuxX64
    } else {
        return Err(BuildInformationError::UnsupportedTarget);
    };
    BuildInformation::new(
        option_env!("TALLYLINE_RELEASE_TAG"),
        option_env!("TALLYLINE_RELEASE_COMMIT"),
        target,
    )
}
