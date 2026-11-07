// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Application settings workflow and persistence port.

pub mod model;

pub use crate::diagnostics::{
    ConfirmedDiagnosticExport, DiagnosticEntry, DiagnosticExportApplication,
    DiagnosticExportFailure, DiagnosticExportSink, DiagnosticInput, DiagnosticPreview,
    DiagnosticPrivacy, DiagnosticSection,
};

use model::{AppSettings, Theme};

/// Persists application preferences as one atomic update.
pub trait AppSettingsStore {
    /// A persistence failure.
    type Error;

    /// Saves preferences without changing the previous version on failure.
    ///
    /// # Errors
    /// Returns the storage failure when the update cannot be committed.
    fn save(&mut self, settings: &AppSettings) -> Result<(), Self::Error>;
}

/// Applies settings only after their persistence succeeds.
pub struct SettingsApplication<S> {
    settings: AppSettings,
    store: S,
}

impl<S: AppSettingsStore> SettingsApplication<S> {
    /// Creates the workflow from loaded preferences and their store.
    #[must_use]
    pub const fn new(settings: AppSettings, store: S) -> Self {
        Self { settings, store }
    }

    /// Returns the active preferences.
    #[must_use]
    pub const fn settings(&self) -> &AppSettings {
        &self.settings
    }

    /// Persists and applies the selected theme.
    ///
    /// # Errors
    /// Returns a storage error and keeps the active preferences unchanged.
    pub fn change_theme(&mut self, theme: Theme) -> Result<(), S::Error> {
        let next = self.settings.clone().with_theme(theme);
        self.store.save(&next)?;
        self.settings = next;
        Ok(())
    }

    /// Persists acknowledgement of the onboarding start screen.
    ///
    /// # Errors
    /// Returns a storage error and keeps onboarding incomplete.
    pub fn complete_onboarding(&mut self) -> Result<(), S::Error> {
        let next = self.settings.clone().with_completed_onboarding();
        self.store.save(&next)?;
        self.settings = next;
        Ok(())
    }

    /// Returns the owned settings store.
    #[must_use]
    pub fn into_store(self) -> S {
        self.store
    }
}

impl<S: AppSettingsStore> SettingsApplication<S> {
    /// Persists release-check permission before applying it.
    ///
    /// # Errors
    /// Returns the storage failure and leaves permission unchanged.
    pub fn change_update_checks(&mut self, checks: model::UpdateChecks) -> Result<(), S::Error> {
        let next = self
            .settings
            .clone()
            .with_update_state(checks, self.settings.last_update_attempt());
        self.store.save(&next)?;
        self.settings = next;
        Ok(())
    }

    /// Reserves one request durably when enabled and a full day has elapsed.
    /// A backwards clock delays requests until the persisted time plus one day.
    ///
    /// # Errors
    /// Returns a storage failure without authorising a network request.
    pub fn begin_update_check(
        &mut self,
        clock: &impl tallyline_shared_kernel::api::Clock,
    ) -> Result<bool, S::Error> {
        const DAY_MILLIS: u128 = 86_400_000;
        if self.settings.update_checks() == model::UpdateChecks::Disabled {
            return Ok(false);
        }
        let now = clock.now();
        if self
            .settings
            .last_update_attempt()
            .is_some_and(|last| now.millis().saturating_sub(last.millis()) < DAY_MILLIS)
        {
            return Ok(false);
        }
        let next = self
            .settings
            .clone()
            .with_update_state(model::UpdateChecks::Enabled, Some(now));
        self.store.save(&next)?;
        self.settings = next;
        Ok(true)
    }
}

/// Reads only the latest published standard release metadata.
pub trait UpdateCheck {
    /// Returns no version when no eligible release exists.
    fn latest(
        &self,
    ) -> impl std::future::Future<
        Output = Result<Option<model::ReleaseVersion>, model::UpdateFailure>,
    > + Send;
}

/// Checks release metadata and returns passive state without UI side effects.
pub async fn check_update(
    source: &impl UpdateCheck,
    installed: model::ReleaseVersion,
) -> model::UpdateStatus {
    match source.latest().await {
        Ok(Some(version)) if version > installed => model::UpdateStatus::Available(version),
        Ok(Some(_) | None) => model::UpdateStatus::Current,
        Err(model::UpdateFailure) => model::UpdateStatus::Unavailable,
    }
}

impl<S: AppSettingsStore> SettingsApplication<S> {
    /// Persists only validated release identities, retaining known updates on failures.
    ///
    /// # Errors
    /// Returns a save failure without changing the known notice.
    pub fn record_update(
        &mut self,
        status: model::UpdateStatus,
        installed: model::ReleaseVersion,
    ) -> Result<model::UpdateStatus, S::Error> {
        let available = match status {
            model::UpdateStatus::Available(version) => Some(version),
            model::UpdateStatus::Current => None,
            model::UpdateStatus::NotChecked
            | model::UpdateStatus::Dismissed
            | model::UpdateStatus::Unavailable => {
                return Ok(self.settings.resolve_update(installed, status));
            }
        };
        let next = self
            .settings
            .clone()
            .with_release_notice(available, self.settings.dismissed_release());
        self.store.save(&next)?;
        self.settings = next;
        Ok(self.settings.resolve_update(installed, status))
    }

    /// Persists dismissal of the known release without changing check permission.
    ///
    /// # Errors
    /// Returns a save failure and keeps the notice visible.
    pub fn dismiss_update(&mut self) -> Result<(), S::Error> {
        let next = self.settings.clone().with_release_notice(
            self.settings.available_release(),
            self.settings.available_release(),
        );
        self.store.save(&next)?;
        self.settings = next;
        Ok(())
    }
}

pub use crate::attachments::{AttachmentRequest, WorkingCopyFailure, WorkingCopyStore};

pub use crate::attachments::{
    ManualAttachmentBlock, ManualAttachmentPreview, manual_attachment_readiness,
};

pub use crate::attachments::{ManualAttachmentFailure, confirm_manual_attachment};

pub use crate::attachments::ManualAttachmentInputFailure;

impl<S: AppSettingsStore> SettingsApplication<S> {
    /// Saves SCU review bands before applying them.
    ///
    /// # Errors
    /// Returns a storage failure without changing active preferences.
    pub fn change_stock_settings(
        &mut self,
        settings: tallyline_reporting::api::model::StockSettings,
    ) -> Result<(), S::Error> {
        let next = self.settings.clone().with_stock_settings(settings);
        self.store.save(&next)?;
        self.settings = next;
        Ok(())
    }
}

pub use crate::submission_priors::accepted_submission_priors;

pub use crate::submission_request::manual_submission_request;

pub use crate::manual_release::{ManualRelease, ManualReleaseError, ManualReleaseStore};

pub use crate::submission_delivery::apply_submission_delivery;

pub use crate::submission_queue::SubmissionQueueStore;

pub use crate::submission_history::{
    assess_release_observation_history, confirm_older_observation,
};

pub use crate::receipt_enrichment::ReceiptEnrichmentStore;

pub use crate::manual_dispatch::{
    ManualDispatch, ManualDispatchError, ManualDispatchStore, assess_manual_send_context,
};

impl<S: AppSettingsStore> SettingsApplication<S> {
    /// Saves the global mode for future releases before making it active.
    ///
    /// # Errors
    /// Returns a failed save without changing the current mode or any queued request.
    pub fn change_submission_mode(
        &mut self,
        mode: Option<tallyline_shared_kernel::api::model::SubmissionMode>,
    ) -> Result<(), S::Error> {
        let next = self.settings.clone().with_submission_mode(mode);
        self.store.save(&next)?;
        self.settings = next;
        Ok(())
    }
    /// Saves the tighten-only observation-age policy before applying it.
    ///
    /// # Errors
    /// Returns a failed save without changing the current policy or observation values.
    pub fn change_observation_age(
        &mut self,
        age: tallyline_shared_kernel::api::model::ObservationAgeSettings,
    ) -> Result<(), S::Error> {
        let next = self.settings.clone().with_observation_age(age);
        self.store.save(&next)?;
        self.settings = next;
        Ok(())
    }
}

pub use crate::event_outbox::{
    EventOutbox, OUTBOX_BATCH_LIMIT, OutboxError, OutboxEvent, OutboxKind, dispatch_pending_events,
};

pub use crate::cooldown_split::{
    CooldownSplit, CooldownSplitIdentities, CooldownSplitPart, CooldownSplitStore,
};
pub use crate::resolution_draft::{ResolutionDraft, ResolutionDraftStore};

pub use crate::submission_lifecycle::{SubmissionLifecycleStore, lifecycle_prior_history};
