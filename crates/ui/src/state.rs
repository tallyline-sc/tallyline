// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Owned shell state and worker result handling.

use crate::api::DialogPresenter;
use crate::game_witness::GameWitnessView;
use tallyline_game::api::model::WitnessMode;
use tallyline_workflows::api::model::{AppSettings, Theme};

/// A primary shell view.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum View {
    /// First-launch legal and setup entry screen.
    #[default]
    Onboarding,
    /// The main workspace and its empty capture state.
    Home,
    /// Cached environment-specific terminal and commodity browsing.
    Reference,
    /// Persistent keyboard-driven manual Report entry.
    Manual,
    /// Durable fixed-mode submission history and explicit unsent cancellation.
    Queue,
    /// Application preferences.
    Settings,
}

/// A worker result that never requests a dialog or window command.
#[derive(Debug)]
pub enum BackgroundEvent {
    /// Committed queue state and passive feedback from the session owner.
    SubmissionUpdated(Box<crate::api::model::SubmissionUpdate>),
    /// Source-scale selected region delivered only for the exact current Report version.
    AttachmentPreview {
        /// Stable Report identity.
        id: tallyline_reporting::api::model::ReportId,
        /// Version for which this result was prepared.
        expected: u64,
        /// Private region only, with its source binding and observation time.
        preview: tallyline_workflows::api::ManualAttachmentPreview,
    },
    /// Current account evidence and the attachment-related part of release readiness.
    AttachmentStatus {
        /// Stable Report identity.
        id: tallyline_reporting::api::model::ReportId,
        /// Current version checked by the worker.
        expected: u64,
        /// Accepted account snapshot used for this policy lookup.
        account: tallyline_submission::api::model::AccountState,
        /// Dated account policy, or an actionable storage failure.
        policy: Result<
            tallyline_submission::api::model::ScreenshotPolicy,
            tallyline_submission::api::model::StorageFailure,
        >,
        /// Attachment-only verdict, without claiming complete release readiness.
        readiness: Result<(), tallyline_workflows::api::ManualAttachmentBlock>,
    },
    /// A private attachment operation failed without changing the view or focus.
    AttachmentFailed {
        /// Stable Report identity.
        id: tallyline_reporting::api::model::ReportId,
        /// Version requested by the user action.
        expected: u64,
        /// Closed actionable failure with no path or pixels.
        failure: tallyline_workflows::api::ManualAttachmentFailure,
    },

    /// A persistent directory-access warning after successful read/write probing.
    StorageWarning(String),
    /// A passive credential result with no secret values.
    CredentialsUpdated {
        /// The last accepted account.
        account: tallyline_submission::api::model::AccountState,
        /// A closed failure that does not replace the accepted identity.
        failure: Option<tallyline_submission::api::model::CredentialFailure>,
        /// Whether read-only packaged installation data is usable.
        available_token: bool,
        /// Whether an independently checked Linux fallback can be offered after warning.
        fallback_available: bool,
    },
    /// Historical non-JSON metadata, without changing current cache or window state.
    ReferenceDiagnostic(crate::api::model::ReferenceDiagnostic),
    /// Durable Drafts loaded after storage commits, without changing focus or view.
    ReportsLoaded {
        /// Most recently edited Drafts first.
        reports: Vec<tallyline_reporting::api::model::Report>,
        /// Draft explicitly created or edited by this user action.
        selected: Option<tallyline_reporting::api::model::ReportId>,
    },
    /// The committed result or refusal for exactly one manual edit attempt.
    ReportEditFinished {
        /// Editor-owned unique attempt.
        request: crate::api::model::ManualEditRequestId,
        /// Requested Draft identity.
        id: tallyline_reporting::api::model::ReportId,
        /// Draft version observed when the request was admitted.
        expected: u64,
        /// Saved state or actionable refusal, without replacing unrelated input.
        result: Result<
            Box<tallyline_reporting::api::model::Report>,
            tallyline_reporting::api::RepositoryError,
        >,
    },
    /// Committed assessments for one request, without selecting a Draft or moving focus.
    ReportReviewed {
        /// Draft version used when this request was queued.
        expected: u64,
        /// The saved Report and exact assessments.
        review: Box<tallyline_reporting::api::model::ManualReview>,
        /// Reference snapshot used for these assessments.
        snapshot: tallyline_reference_data::api::model::ReferenceSnapshot,
    },
    /// A request-bound numerical review failure.
    ReviewFailed {
        /// Requested Draft identity.
        id: tallyline_reporting::api::model::ReportId,
        /// Draft version used by the request.
        expected: u64,
        /// Actionable refusal without replacing input.
        error: tallyline_reporting::api::RepositoryError,
    },
    /// A manual Report operation failed without losing the displayed input.
    ReportFailed(tallyline_reporting::api::RepositoryError),
    /// A passive refresh starts without replacing cached records.
    ReferenceRefreshStarted,
    /// Environment-specific cache state and its injected clock observation.
    ReferenceUpdated {
        /// LIVE records.
        live: tallyline_reference_data::api::model::ReferenceSnapshot,
        /// PTU records with no borrowed LIVE priors.
        ptu: tallyline_reference_data::api::model::ReferenceSnapshot,
        /// The worker clock when these snapshots were published.
        observed_at: tallyline_shared_kernel::api::model::Timestamp,
    },
    /// Passive release metadata result with no window command.
    UpdateChecked(tallyline_workflows::api::model::UpdateStatus),
    /// Local witness settings and sanitised facts from the file worker.
    GameWitnessLoaded(GameWitnessView),
    /// An actionable failure that stays visible across view changes.
    Failure(String),
    /// Preferences loaded or successfully committed by the file worker.
    SettingsLoaded(AppSettings),
    /// The validated offline catalogue, without requesting a dialog.
    LicencesLoaded(crate::api::model::LicenceCatalogue),
    /// The bundled catalogue is missing, unreadable or incomplete.
    LicencesUnavailable,
}

/// Owned shell state shared by drawn and windowless tests.
#[derive(Default)]
pub struct ShellState {
    pub(crate) submission: crate::submission_controls::SubmissionControlState,
    credentials: crate::credentials::CredentialSetup,
    pub(crate) stock_settings_editor: crate::stock_settings::StockSettingsEditor,
    view: View,
    manual: crate::manual_editor::ManualEditor,
    references: crate::reference_browser::ReferenceBrowser,
    notice: Option<String>,
    storage_warning: Option<String>,
    reference_notice: Option<String>,
    dialogs: DialogPresenter,
    settings: AppSettings,
    busy: bool,
    ready: bool,
    game_witness: GameWitnessView,
    game_channel_draft: String,
    update_status: tallyline_workflows::api::model::UpdateStatus,
}

impl ShellState {
    /// Returns the user-editable local channel selection.
    pub fn game_channel_draft(&mut self) -> &mut String {
        &mut self.game_channel_draft
    }
    /// Returns the last durable local witness observation.
    #[must_use]
    pub const fn game_witness(&self) -> &GameWitnessView {
        &self.game_witness
    }

    /// Returns whether startup established a writable settings store.
    #[must_use]
    pub const fn ready(&self) -> bool {
        self.ready
    }

    /// Opens About in direct response to a user action.
    pub fn open_about(&mut self) {
        self.dialogs.open_about();
    }

    /// Draws dialogs owned by the presenter.
    pub fn show_dialogs(
        &mut self,
        context: &egui::Context,
        catalogue: &crate::api::Catalogue,
        unit: &crate::api::FanKitUnit,
    ) {
        let theme = crate::api::resolve_theme(self.settings.theme(), context.system_theme());
        self.dialogs.show(context, catalogue, unit, theme);
    }
    /// Returns the preferences confirmed by the settings worker.
    #[must_use]
    pub const fn settings(&self) -> &AppSettings {
        &self.settings
    }

    /// Returns whether a settings update is waiting for its worker.
    #[must_use]
    pub const fn busy(&self) -> bool {
        self.busy
    }

    /// Converts a direct user action into a bounded worker command.
    pub fn act(&mut self, action: UserAction) -> Option<SettingsCommand> {
        if self.busy || !self.ready {
            return None;
        }
        self.busy = true;
        Some(match action {
            UserAction::ChangeSubmissionMode(mode) => SettingsCommand::ChangeSubmissionMode(mode),
            UserAction::ChangeObservationAge(age) => SettingsCommand::ChangeObservationAge(age),
            UserAction::ChangeEnvironmentMapping(channel, environment) => {
                SettingsCommand::ChangeEnvironmentMapping(channel, environment)
            }
            UserAction::DismissUpdate => SettingsCommand::DismissUpdate,
            UserAction::ChangeUpdateChecks(checks) => SettingsCommand::ChangeUpdateChecks(checks),
            UserAction::ChangeStockSettings(settings) => {
                SettingsCommand::ChangeStockSettings(settings)
            }
            UserAction::ChooseTheme(theme) => SettingsCommand::ChangeTheme(theme),
            UserAction::ChooseGameChannel(channel) => SettingsCommand::ChangeGameChannel(channel),
            UserAction::RefreshWitness => SettingsCommand::RefreshWitness,
            UserAction::ChangeWitnessMode(mode) => SettingsCommand::ChangeWitnessMode(mode),
            UserAction::ContinueOnboarding => SettingsCommand::CompleteOnboarding,
        })
    }
    /// Applies a worker result without changing the view or window state.
    pub fn receive(&mut self, event: BackgroundEvent) {
        match event {
            BackgroundEvent::SubmissionUpdated(update) => {
                self.submission.background_feedback = update.background_feedback;
                self.submission.queue.received(update.jobs);
                self.submission
                    .queue
                    .received_client_version(update.client_version);
                self.submission
                    .queue
                    .received_service_fences(update.service_fences);
                self.submission
                    .queue
                    .received_authenticated_quotas(update.authenticated_quotas);
                self.submission.queue.received_waiting(update.waiting);
                self.submission
                    .queue
                    .received_schedule_time(update.assessed_at);
                self.submission
                    .accounts
                    .refresh(self.credentials.account(), self.submission.queue.jobs());
                self.submission
                    .queue
                    .received_lifecycle_busy(update.lifecycle_busy);
                self.submission.queue.received_lifecycle_account(update.lifecycle_account.filter(|name| matches!(self.credentials.account(), tallyline_submission::api::model::AccountState::Validated { username, .. } if username == name)));
                for ledger in update.lifecycles {
                    self.submission.queue.received_lifecycle(ledger);
                }
                for mapping in update.mappings {
                    self.submission.queue.received_mapping(mapping);
                }
                if update.completed {
                    self.submission.busy = false;
                    self.manual.busy = false;
                    if let Some((id, expected)) = self.submission.cancel.take() {
                        if update.feedback.as_ref().is_some_and(|feedback| {
                            !matches!(feedback, crate::api::model::SubmissionFeedback::Split)
                        }) {
                            let failure = match &update.feedback {
                            Some(crate::api::model::SubmissionFeedback::Refused(reason)) => tallyline_submission::api::model::SubmissionApplicationError::Refused(*reason),
                            Some(crate::api::model::SubmissionFeedback::Changed) => tallyline_submission::api::model::SubmissionApplicationError::Repository(tallyline_submission::api::model::SubmissionRepositoryError::Conflict),
                            Some(crate::api::model::SubmissionFeedback::Blocked(_) | crate::api::model::SubmissionFeedback::Held(_) | crate::api::model::SubmissionFeedback::Returned(_) | crate::api::model::SubmissionFeedback::Unavailable | crate::api::model::SubmissionFeedback::AttachmentUnavailable) | Some(crate::api::model::SubmissionFeedback::Split) | None => tallyline_submission::api::model::SubmissionApplicationError::Repository(tallyline_submission::api::model::SubmissionRepositoryError::Unavailable),
                        };
                            self.submission.queue.failed(id, expected, failure);
                        } else {
                            self.submission.queue.completed(id, expected);
                        }
                    }
                }
                if update.completed || update.feedback.is_some() {
                    self.submission.feedback = update.feedback;
                }
                self.receive(BackgroundEvent::ReportsLoaded {
                    reports: update.reports,
                    selected: None,
                });
            }
            BackgroundEvent::AttachmentPreview {
                id,
                expected,
                preview,
            } => self.manual.preview_received(id, expected, preview),
            BackgroundEvent::AttachmentStatus {
                id,
                expected,
                account,
                policy,
                readiness,
            } => self
                .manual
                .attachment_status(id, expected, account, policy, readiness),
            BackgroundEvent::AttachmentFailed {
                id,
                expected,
                failure,
            } => self.manual.attachment_failed(id, expected, failure),

            BackgroundEvent::StorageWarning(warning) => self.storage_warning = Some(warning),
            BackgroundEvent::CredentialsUpdated {
                account,
                failure,
                available_token,
                fallback_available,
            } => {
                self.submission.queue.received_lifecycle_account(None);
                self.submission
                    .accounts
                    .changed(self.credentials.account(), &account);
                self.credentials
                    .updated(account, failure, available_token, fallback_available);
                self.submission
                    .accounts
                    .refresh(self.credentials.account(), self.submission.queue.jobs());
            }
            BackgroundEvent::ReferenceDiagnostic(diagnostic) => {
                self.references.report_diagnostic(diagnostic)
            }
            BackgroundEvent::ReportsLoaded { reports, selected } => {
                self.receive_drafts(reports, selected)
            }
            BackgroundEvent::ReportEditFinished {
                request,
                id,
                expected,
                result,
            } => self
                .manual
                .edit_finished(request, id, expected, result.map(|report| *report)),
            BackgroundEvent::ReportReviewed {
                expected,
                review,
                snapshot,
            } => self.manual.reviewed(expected, *review, snapshot),
            BackgroundEvent::ReviewFailed {
                id,
                expected,
                error,
            } => self.manual.review_failed(id, expected, error),
            BackgroundEvent::ReportFailed(error) => self.manual.failed(error),
            BackgroundEvent::ReferenceRefreshStarted => self.references.started(),
            BackgroundEvent::ReferenceUpdated {
                live,
                ptu,
                observed_at,
            } => {
                if let Some(report) = self.manual.selected() {
                    let snapshot = match self
                        .game_witness
                        .settings
                        .environment_mapping
                        .environment(report.key().environment())
                    {
                        tallyline_shared_kernel::api::model::UexEnvironment::Live => &live,
                        tallyline_shared_kernel::api::model::UexEnvironment::Ptu => &ptu,
                    };
                    self.manual.review_reference_changed(snapshot);
                }
                self.references.updated(live, ptu, observed_at);
                self.reference_notice = None;
            }
            BackgroundEvent::UpdateChecked(status) => {
                if self.settings.update_checks()
                    == tallyline_workflows::api::model::UpdateChecks::Enabled
                {
                    self.update_status = status;
                }
            }
            BackgroundEvent::GameWitnessLoaded(view) => {
                self.game_channel_draft = view.channel_directory.clone().unwrap_or_default();
                self.game_witness = view;
                self.busy = false;
            }
            BackgroundEvent::Failure(notice) => {
                self.notice = Some(notice);
                self.busy = false;
            }
            BackgroundEvent::LicencesLoaded(catalogue) => {
                self.dialogs.set_licences(Some(catalogue))
            }
            BackgroundEvent::LicencesUnavailable => self.dialogs.set_licences(None),
            BackgroundEvent::SettingsLoaded(settings) => {
                if self.view == View::Onboarding && settings.onboarding_complete() {
                    self.view = View::Home;
                }
                if settings.update_checks()
                    == tallyline_workflows::api::model::UpdateChecks::Disabled
                {
                    self.update_status = tallyline_workflows::api::model::UpdateStatus::NotChecked;
                }
                self.stock_settings_editor = settings.stock_settings().into();
                self.manual.set_stock_settings(settings.stock_settings());
                self.settings = settings;
                self.ready = true;
                self.notice = None;
                self.busy = false;
            }
        }
    }

    /// Returns the persistent action notice.
    #[must_use]
    pub fn notice(&self) -> Option<&str> {
        self.notice
            .as_deref()
            .or(self.storage_warning.as_deref())
            .or(self.reference_notice.as_deref())
    }

    /// Returns the active primary view.
    #[must_use]
    pub const fn view(&self) -> View {
        self.view
    }

    /// Returns user-driven dialog state.
    #[must_use]
    pub const fn dialogs(&self) -> &DialogPresenter {
        &self.dialogs
    }

    /// Returns to the workspace, or to unfinished onboarding.
    pub fn leave_settings(&mut self) {
        self.view = if self.settings.onboarding_complete() {
            View::Home
        } else {
            View::Onboarding
        };
    }

    /// Opens preferences after a user action.
    pub fn show_settings(&mut self) {
        self.view = View::Settings;
    }
}

/// A direct user request that requires durable preferences.
pub enum UserAction {
    /// Select the global mode for future releases only.
    ChangeSubmissionMode(Option<tallyline_shared_kernel::api::model::SubmissionMode>),
    /// Tighten observation age and confirmation lifetime without changing the fixed hard limit.
    ChangeObservationAge(tallyline_shared_kernel::api::model::ObservationAgeSettings),

    /// Persist independently tighten-only stock bands.
    ChangeStockSettings(tallyline_reporting::api::model::StockSettings),
    /// Persist an independent UEX environment for one game channel.
    ChangeEnvironmentMapping(
        tallyline_game::api::model::GameEnvironment,
        tallyline_shared_kernel::api::model::UexEnvironment,
    ),
    /// Persist dismissal of the available release.
    DismissUpdate,
    /// Change permission for daily release metadata checks.
    ChangeUpdateChecks(tallyline_workflows::api::model::UpdateChecks),
    /// Choose a channel directory and revoke the previous opt-in.
    ChooseGameChannel(String),
    /// Refresh only after an explicit user request.
    RefreshWitness,
    /// Enable or disable read-only witness access.
    ChangeWitnessMode(WitnessMode),
    /// Select a colour theme.
    ChooseTheme(Theme),
    /// Acknowledge the first-launch screen.
    ContinueOnboarding,
}

/// A settings operation handled outside the UI thread.
#[derive(Clone, PartialEq, Eq)]
pub enum SettingsCommand {
    /// Select the global mode for future releases only.
    ChangeSubmissionMode(Option<tallyline_shared_kernel::api::model::SubmissionMode>),
    /// Tighten observation age and confirmation lifetime without changing the fixed hard limit.
    ChangeObservationAge(tallyline_shared_kernel::api::model::ObservationAgeSettings),

    /// Persist independently tighten-only stock bands.
    ChangeStockSettings(tallyline_reporting::api::model::StockSettings),
    /// Persist an independent UEX environment for one game channel.
    ChangeEnvironmentMapping(
        tallyline_game::api::model::GameEnvironment,
        tallyline_shared_kernel::api::model::UexEnvironment,
    ),
    /// Save the explicit release notice dismissal.
    DismissUpdate,
    /// Persist release-check permission.
    ChangeUpdateChecks(tallyline_workflows::api::model::UpdateChecks),
    /// Persist a channel directory and disable witness access.
    ChangeGameChannel(String),
    /// Read sanitised facts under the durable opt-in preference.
    RefreshWitness,
    /// Persist explicit read-only witness access.
    ChangeWitnessMode(WitnessMode),
    /// Save and apply a colour theme.
    ChangeTheme(Theme),
    /// Save acknowledgement of the onboarding screen.
    CompleteOnboarding,
}

impl std::fmt::Debug for SettingsCommand {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ChangeSubmissionMode(mode) => formatter
                .debug_tuple("ChangeSubmissionMode")
                .field(mode)
                .finish(),
            Self::ChangeObservationAge(age) => formatter
                .debug_tuple("ChangeObservationAge")
                .field(age)
                .finish(),
            Self::ChangeEnvironmentMapping(channel, environment) => formatter
                .debug_tuple("ChangeEnvironmentMapping")
                .field(channel)
                .field(environment)
                .finish(),
            Self::ChangeStockSettings(settings) => formatter
                .debug_tuple("ChangeStockSettings")
                .field(settings)
                .finish(),
            Self::DismissUpdate => formatter.write_str("DismissUpdate"),
            Self::ChangeUpdateChecks(checks) => formatter
                .debug_tuple("ChangeUpdateChecks")
                .field(checks)
                .finish(),
            Self::ChangeGameChannel(_) => {
                formatter.write_str("ChangeGameChannel(<local selection>)")
            }
            Self::RefreshWitness => formatter.write_str("RefreshWitness"),
            Self::ChangeWitnessMode(mode) => formatter
                .debug_tuple("ChangeWitnessMode")
                .field(mode)
                .finish(),
            Self::ChangeTheme(theme) => formatter.debug_tuple("ChangeTheme").field(theme).finish(),
            Self::CompleteOnboarding => formatter.write_str("CompleteOnboarding"),
        }
    }
}

impl ShellState {
    /// Returns the most recent passive update status.
    #[must_use]
    pub const fn update_status(&self) -> &tallyline_workflows::api::model::UpdateStatus {
        &self.update_status
    }
}

mod reference;

impl ShellState {
    /// Returns manual Draft editing state.
    #[must_use]
    pub const fn manual(&self) -> &crate::api::model::ManualEditor {
        &self.manual
    }
    /// Returns mutable manual input and command state.
    pub fn manual_mut(&mut self) -> &mut crate::api::model::ManualEditor {
        &mut self.manual
    }
    /// Opens manual entry in direct response to a user action.
    pub fn show_manual(&mut self) {
        self.view = View::Manual;
    }
}

impl ShellState {
    /// Returns masked credential input and passive account feedback.
    pub fn credentials_mut(&mut self) -> &mut crate::api::model::CredentialSetup {
        &mut self.credentials
    }
    /// Returns safe credential setup state.
    #[must_use]
    pub const fn credentials(&self) -> &crate::api::model::CredentialSetup {
        &self.credentials
    }
}

mod manual_review;

impl ShellState {
    /// Injects validated build display data without deriving a version from UI package metadata.
    pub fn set_build_presentation(&mut self, build: crate::api::model::BuildPresentation) {
        self.submission.release_build =
            matches!(build, crate::api::model::BuildPresentation::Release { .. });
        self.dialogs.set_build_presentation(build);
    }
}

impl ShellState {
    pub(crate) fn show_queue_view(&mut self) {
        self.view = View::Queue;
    }
    pub(crate) fn return_to_setup(&mut self) {
        if !self.settings.onboarding_complete() {
            self.view = View::Onboarding;
        }
    }
}

mod submission_projection;
