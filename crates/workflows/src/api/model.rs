// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Application preferences shared across public boundaries.

/// A supported Tallyline colour theme.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Theme {
    /// Follow the operating system light or dark preference.
    #[default]
    System,
    /// Dark slate surfaces.
    Dark,
    /// Light surfaces.
    Light,
    /// High contrast surfaces.
    HighContrast,
}

/// User-owned application preferences.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AppSettings {
    theme: Theme,
    stock_settings: tallyline_reporting::api::model::StockSettings,
    submission_mode: Option<tallyline_shared_kernel::api::model::SubmissionMode>,
    observation_age: tallyline_shared_kernel::api::model::ObservationAgeSettings,
    onboarding_complete: bool,
    update_checks: UpdateChecks,
    available_release: Option<ReleaseVersion>,
    dismissed_release: Option<ReleaseVersion>,
    last_update_attempt: Option<tallyline_shared_kernel::api::model::Timestamp>,
}

impl AppSettings {
    /// Reconstructs persisted application preferences.
    #[must_use]
    pub const fn from_persisted(theme: Theme, onboarding_complete: bool) -> Self {
        Self {
            theme,
            stock_settings: tallyline_reporting::api::model::StockSettings::baseline(),
            submission_mode: None,
            observation_age: tallyline_shared_kernel::api::model::ObservationAgeSettings::baseline(
            ),
            onboarding_complete,
            update_checks: UpdateChecks::Disabled,
            available_release: None,
            dismissed_release: None,
            last_update_attempt: None,
        }
    }

    /// Returns whether the start screen has been acknowledged.
    #[must_use]
    pub const fn onboarding_complete(&self) -> bool {
        self.onboarding_complete
    }
    /// Returns the selected colour theme.
    #[must_use]
    pub const fn theme(&self) -> Theme {
        self.theme
    }
}

/// Permission to check the public release endpoint.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UpdateChecks {
    /// No release requests are made.
    #[default]
    Disabled,
    /// At most one release request per 24 hours.
    Enabled,
}

impl AppSettings {
    /// Returns the persisted release-check permission.
    #[must_use]
    pub const fn update_checks(&self) -> UpdateChecks {
        self.update_checks
    }

    /// Returns the last reserved release request time, including failed requests.
    #[must_use]
    pub const fn last_update_attempt(
        &self,
    ) -> Option<tallyline_shared_kernel::api::model::Timestamp> {
        self.last_update_attempt
    }

    /// Reconstructs release-check preferences without changing other settings.
    #[must_use]
    pub const fn with_update_state(
        mut self,
        checks: UpdateChecks,
        attempt: Option<tallyline_shared_kernel::api::model::Timestamp>,
    ) -> Self {
        self.update_checks = checks;
        self.last_update_attempt = attempt;
        self
    }
}

pub use tallyline_shared_kernel::api::model::ReleaseVersion;

/// The passive result of a release check.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum UpdateStatus {
    /// No check has completed in this session.
    #[default]
    NotChecked,
    /// The installed version is at least the latest standard release.
    Current,
    /// A newer standard release has an actionable release page.
    Available(ReleaseVersion),
    /// The known available release was explicitly dismissed.
    Dismissed,
    /// No answer could be validated; core work remains available.
    Unavailable,
}

/// A release request could not be completed or validated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UpdateFailure;

impl AppSettings {
    /// Returns the last validated available release.
    #[must_use]
    pub const fn available_release(&self) -> Option<ReleaseVersion> {
        self.available_release
    }

    /// Returns the release whose notice was explicitly dismissed.
    #[must_use]
    pub const fn dismissed_release(&self) -> Option<ReleaseVersion> {
        self.dismissed_release
    }

    /// Restores only validated release identities and their dismissal choice.
    #[must_use]
    pub const fn with_release_notice(
        mut self,
        available: Option<ReleaseVersion>,
        dismissed: Option<ReleaseVersion>,
    ) -> Self {
        self.available_release = available;
        self.dismissed_release = dismissed;
        self
    }

    /// Restores passive notice state against the immutable installed version.
    #[must_use]
    pub fn cached_update(&self, installed: ReleaseVersion) -> UpdateStatus {
        if self.update_checks == UpdateChecks::Disabled {
            return UpdateStatus::NotChecked;
        }
        match self.available_release {
            Some(version) if version <= installed => UpdateStatus::Current,
            Some(version) if self.dismissed_release == Some(version) => UpdateStatus::Dismissed,
            Some(version) => UpdateStatus::Available(version),
            None => UpdateStatus::NotChecked,
        }
    }
}

impl AppSettings {
    /// Retains a validated available notice when a later check is unavailable.
    #[must_use]
    pub fn resolve_update(&self, installed: ReleaseVersion, result: UpdateStatus) -> UpdateStatus {
        match self.cached_update(installed) {
            UpdateStatus::NotChecked => result,
            cached @ (UpdateStatus::Available(_)
            | UpdateStatus::Dismissed
            | UpdateStatus::Current
            | UpdateStatus::Unavailable) => cached,
        }
    }
}

impl AppSettings {
    /// Returns independently validated SCU review bands.
    #[must_use]
    pub const fn stock_settings(&self) -> tallyline_reporting::api::model::StockSettings {
        self.stock_settings
    }
    /// Reconstructs preferences with validated SCU review bands.
    #[must_use]
    pub const fn with_stock_settings(
        mut self,
        settings: tallyline_reporting::api::model::StockSettings,
    ) -> Self {
        self.stock_settings = settings;
        self
    }
}

impl AppSettings {
    /// Returns the explicit global mode, or absence for the caller's checked build default.
    #[must_use]
    pub const fn submission_mode_override(
        &self,
    ) -> Option<tallyline_shared_kernel::api::model::SubmissionMode> {
        self.submission_mode
    }
    /// Resolves the explicit preference against a build/onboarding default supplied by the app.
    #[must_use]
    pub const fn submission_mode(
        &self,
        fallback: tallyline_shared_kernel::api::model::SubmissionMode,
    ) -> tallyline_shared_kernel::api::model::SubmissionMode {
        match self.submission_mode {
            Some(mode) => mode,
            None => fallback,
        }
    }
    /// Returns the validated tighten-only observation-age and confirmation lifetime.
    #[must_use]
    pub const fn observation_age(
        &self,
    ) -> tallyline_shared_kernel::api::model::ObservationAgeSettings {
        self.observation_age
    }
    /// Selects a global mode for future releases without changing already queued snapshots.
    #[must_use]
    pub fn with_submission_mode(
        mut self,
        mode: Option<tallyline_shared_kernel::api::model::SubmissionMode>,
    ) -> Self {
        self.submission_mode = mode;
        self
    }
    /// Replaces the observation-age limit with an independently validated tightening.
    #[must_use]
    pub fn with_observation_age(
        mut self,
        age: tallyline_shared_kernel::api::model::ObservationAgeSettings,
    ) -> Self {
        self.observation_age = age;
        self
    }
}
impl AppSettings {
    pub(crate) fn with_theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }
    pub(crate) fn with_completed_onboarding(mut self) -> Self {
        self.onboarding_complete = true;
        self
    }
}
