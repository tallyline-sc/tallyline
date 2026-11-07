// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Fixed-mode queue presentation and version-bound cancellation intent.

use crate::api::Catalogue;
use crate::resolution_queue_action::ResolutionQueueAction;
use tallyline_shared_kernel::api::model::SubmissionMode;
use tallyline_submission::api::model::{
    JobId, JobPhase, SubmissionApplicationError, SubmissionJob, SubmissionRepositoryError,
    SubmissionResult, VerifiedReceiptMapping, VerifiedUexReport,
};
use tallyline_submission::api::model::{
    NotProcessed, SendReturnReason, SubmissionAccountCheck, SubmissionCorrection,
    SubmissionHoldReason, SubmissionPauseReason, SubmissionRetrySettings, SubmissionReturnReason,
    SubmissionServiceCause, SubmissionServiceFence, SubmissionWait,
};

/// An explicit queue action without credentials or mutable report values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueueCommand {
    /// Refreshes lifecycle evidence for the exact displayed source and ledger.
    RefreshLifecycle {
        /// Stable original job identity.
        id: JobId,
        /// Committed source revision displayed before the action.
        expected_revision: u64,
        /// Committed lifecycle revision, absent before its first admission.
        expected_lifecycle: Option<u64>,
    },
    /// Explicitly partitions the exact displayed queued revision into two releases.
    Split {
        /// Stable original job identity.
        id: JobId,
        /// Committed revision displayed before the action.
        expected_revision: u64,
    },
    /// Resolves only the exact partial or unknown origin explicitly inspected by the user.
    Resolve {
        /// Stable local submission identity.
        id: JobId,
        /// The committed version displayed before the action.
        expected_revision: u64,
        /// The explicit action without transport authority.
        action: ResolutionQueueAction,
    },
    /// Retries only the eligible exact version explicitly shown to the user.
    Retry {
        /// Stable local submission identity.
        id: JobId,
        /// The committed version observed before the action.
        expected_revision: u64,
    },
    /// Cancels only the exact unsent job version shown to the user.
    Cancel {
        /// Stable local submission identity.
        id: JobId,
        /// The committed version observed before the action.
        expected_revision: u64,
    },
}

impl QueueCommand {
    fn identity(self) -> (JobId, u64) {
        match self {
            Self::RefreshLifecycle {
                id,
                expected_revision,
                ..
            } => (id, expected_revision),
            Self::Resolve {
                id,
                expected_revision,
                ..
            } => (id, expected_revision),
            Self::Split {
                id,
                expected_revision,
            }
            | Self::Cancel {
                id,
                expected_revision,
            }
            | Self::Retry {
                id,
                expected_revision,
            } => (id, expected_revision),
        }
    }
}

/// Current committed job views and one outstanding explicit cancellation.
#[derive(Default)]
pub struct SubmissionQueue {
    client_version: Option<tallyline_shared_kernel::api::model::ReleaseVersion>,
    service_fences: Vec<SubmissionServiceFence>,
    authenticated_quotas: Vec<tallyline_submission::api::model::TokenQuotaObservation>,
    jobs: Vec<SubmissionJob>,
    mappings: Vec<VerifiedReceiptMapping>,
    lifecycles: Vec<tallyline_submission::api::model::SubmissionLifecycle>,
    lifecycle_busy: bool,
    lifecycle_account: Option<String>,
    waiting: Vec<SubmissionWait>,
    assessed_at: Option<tallyline_shared_kernel::api::model::Timestamp>,
    pending: Option<QueueCommand>,
    busy: Option<QueueCommand>,
    failure: Option<SubmissionApplicationError>,
}
impl SubmissionQueue {
    /// Publishes passive immutable compiled release context without dispatch authority.
    pub fn received_client_version(
        &mut self,
        version: Option<tallyline_shared_kernel::api::model::ReleaseVersion>,
    ) {
        self.client_version = version;
    }
    /// Retains the newest committed version of every received job.
    pub fn received(&mut self, jobs: Vec<SubmissionJob>) {
        for job in jobs {
            if let Some(old) = self.jobs.iter_mut().find(|old| old.id() == job.id()) {
                if job.revision() > old.revision() {
                    *old = job;
                }
            } else {
                self.jobs.push(job);
            }
        }
        self.mappings
            .retain(|mapping| self.jobs.iter().any(|job| mapping.matches_job(job)));
        self.jobs.sort_by_key(|job| std::cmp::Reverse(job.id()));
        if let Some((id, expected_revision)) = self.busy.map(QueueCommand::identity)
            && self
                .jobs
                .iter()
                .any(|job| job.id() == id && job.revision() > expected_revision)
        {
            self.busy = None;
            self.pending = None;
            self.failure = None;
        }
    }
    /// Publishes committed authenticated-read quota holds without submission authority.
    pub fn received_authenticated_quotas(
        &mut self,
        quotas: Vec<tallyline_submission::api::model::TokenQuotaObservation>,
    ) {
        self.authenticated_quotas = quotas;
    }
    /// Publishes durable global service reasons without granting dispatch authority.
    pub fn received_service_fences(&mut self, fences: Vec<SubmissionServiceFence>) {
        self.service_fences = fences
            .into_iter()
            .filter(|fence| {
                self.jobs
                    .iter()
                    .find(|job| job.id() == fence.source())
                    .is_none_or(|job| {
                        job.revision() < fence.source_revision() || fence.matches_source(job)
                    })
            })
            .collect();
    }
    /// Retains waits only for the exact committed queued revision currently visible.
    pub fn received_waiting(&mut self, waiting: Vec<SubmissionWait>) {
        self.waiting = waiting
            .into_iter()
            .filter(|wait| {
                self.jobs.iter().any(|job| {
                    job.id() == wait.job
                        && job.revision() == wait.revision
                        && job.phase() == JobPhase::Queued
                })
            })
            .collect();
    }
    /// Retains the queue owner's injected assessment time without granting send authority.
    pub fn received_schedule_time(&mut self, at: tallyline_shared_kernel::api::model::Timestamp) {
        self.assessed_at = Some(at);
    }
    fn split_partition(
        &self,
        id: JobId,
    ) -> Option<tallyline_submission::api::model::CooldownPartition> {
        if !self.service_fences.is_empty() || !self.authenticated_quotas.is_empty() {
            return None;
        }
        let job = self.jobs.iter().find(|job| job.id() == id)?;
        if !self.waiting.iter().any(|wait| {
            wait.job == id
                && wait.revision == job.revision()
                && matches!(
                    wait.reason,
                    tallyline_submission::api::model::SubmissionWaitReason::CooldownUntil(_)
                )
        }) {
            return None;
        }
        let partition = tallyline_submission::api::SubmissionSchedule::cooldown_partition(
            job,
            &self.jobs,
            self.assessed_at?,
        )
        .ok()?;
        (!partition.ready_rows().is_empty() && !partition.cooling_rows().is_empty())
            .then_some(partition)
    }
    /// Reports whether the exact displayed waiting revision has two nonempty cooldown groups.
    #[must_use]
    pub fn can_split(&self, id: JobId) -> bool {
        self.busy.is_none() && self.split_partition(id).is_some()
    }
    /// Queues one explicit version-bound split without changing committed row values.
    pub fn request_split(&mut self, id: JobId) -> bool {
        if !self.can_split(id) {
            return false;
        }
        let Some(job) = self.jobs.iter().find(|job| job.id() == id) else {
            return false;
        };
        let command = QueueCommand::Split {
            id,
            expected_revision: job.revision(),
        };
        self.pending = Some(command);
        self.busy = Some(command);
        self.failure = None;
        true
    }
    /// Publishes a persisted mapping only for the exact committed successful job already visible.
    pub fn received_mapping(&mut self, mapping: VerifiedReceiptMapping) -> bool {
        if !self.jobs.iter().any(|job| mapping.matches_job(job)) {
            return false;
        }
        if let Some(previous) = self
            .mappings
            .iter()
            .find(|previous| previous.job() == mapping.job())
        {
            return previous == &mapping;
        }
        self.mappings.push(mapping);
        true
    }
    /// Borrows verified identities only while their exact successful snapshot remains current.
    #[must_use]
    pub fn verified_receipts(&self, id: JobId) -> Option<&[VerifiedUexReport]> {
        let job = self.jobs.iter().find(|job| job.id() == id)?;
        self.mappings
            .iter()
            .find(|mapping| mapping.matches_job(job))
            .map(VerifiedReceiptMapping::receipts)
    }
    /// Borrows committed jobs, newest first, without inventing intermediate phases.
    #[must_use]
    pub fn jobs(&self) -> &[SubmissionJob] {
        &self.jobs
    }
    /// Returns whether this view can request cancellation of the current unsent job.
    #[must_use]
    pub fn can_cancel(&self, id: JobId) -> bool {
        self.busy.is_none()
            && self.jobs.iter().any(|job| {
                job.id() == id
                    && matches!(
                        job.phase(),
                        JobPhase::Queued | JobPhase::Held | JobPhase::Paused
                    )
                    && !matches!(
                        job.result().map(SubmissionResult::final_result),
                        Some(SubmissionResult::NeedsOutcomeCheck { .. })
                    )
            })
    }
    /// Queues at most one explicit version-bound cancellation.
    pub fn request_cancel(&mut self, id: JobId) -> bool {
        if !self.can_cancel(id) {
            return false;
        }
        let Some(job) = self.jobs.iter().find(|job| job.id() == id) else {
            return false;
        };
        let command = QueueCommand::Cancel {
            id,
            expected_revision: job.revision(),
        };
        self.pending = Some(command);
        self.busy = Some(command);
        self.failure = None;
        true
    }
    /// Queues an explicit retry only for the exact eligible committed version.
    pub fn request_retry(&mut self, id: JobId) -> bool {
        if self.busy.is_some() {
            return false;
        }
        let Some(job) = self.jobs.iter().find(|job| {
            job.id() == id && (job.can_retry() || job.can_retry_after_update(self.client_version))
        }) else {
            return false;
        };
        let command = QueueCommand::Retry {
            id,
            expected_revision: job.revision(),
        };
        self.pending = Some(command);
        self.busy = Some(command);
        self.failure = None;
        true
    }
    /// Drains the bounded command for the application worker.
    pub fn take_command(&mut self) -> Option<QueueCommand> {
        self.pending.take()
    }
    /// Applies a passive failure only to the still outstanding exact command.
    pub fn failed(
        &mut self,
        id: JobId,
        expected_revision: u64,
        failure: SubmissionApplicationError,
    ) {
        if self.busy.map(QueueCommand::identity) == Some((id, expected_revision)) {
            self.busy = None;
            self.pending = None;
            self.failure = Some(failure);
        }
    }
}

/// Draws immutable mode and account context and emits only explicit unsent cancellation intent.
pub fn show_submission_queue(
    ui: &mut egui::Ui,
    state: &mut SubmissionQueue,
    catalogue: &Catalogue,
    theme: tallyline_egui::Theme,
) -> Option<QueueCommand> {
    ui.heading(catalogue.text("queue.title"));
    for fence in &state.service_fences {
        ui.label(catalogue.text(match fence.cause() {
            SubmissionServiceCause::DailyQuota => "queue.global_daily_quota",
            SubmissionServiceCause::AccountPause => "queue.global_account_pause",
            SubmissionServiceCause::ApiChanged => "queue.global_api_changed",
        }));
        if fence.cause() == SubmissionServiceCause::DailyQuota {
            crate::submission_quota::show(ui, fence.quota_reset(), catalogue);
        }
    }
    for observation in &state.authenticated_quotas {
        ui.label(catalogue.text("queue.global_daily_quota"));
        crate::submission_quota::show(
            ui,
            observation
                .reset()
                .and_then(tallyline_submission::api::model::TokenQuotaReset::usable_reset),
            catalogue,
        );
    }
    if state.jobs.is_empty() {
        ui.label(catalogue.text("queue.empty"));
    }
    if let Some(failure) = state.failure {
        let key = match failure {
            SubmissionApplicationError::Missing => "queue.missing",
            SubmissionApplicationError::Refused(_) => "queue.cannot_cancel",
            SubmissionApplicationError::Repository(SubmissionRepositoryError::Conflict) => {
                "queue.changed"
            }
            SubmissionApplicationError::Repository(
                SubmissionRepositoryError::Invalid | SubmissionRepositoryError::Unavailable,
            ) => "queue.unavailable",
        };
        ui.label(catalogue.text(key));
    }
    let mut unknown_action = None;
    let mut cancel = None;
    let mut retry = None;
    let mut split = None;
    let mut lifecycle_refresh = None;
    for job in &state.jobs {
        ui.push_id(job.id().0, |ui| {
            let header = job.request().header();
            ui.group(|ui| {
                ui.label(format!(
                    "{} {} · {} {} · {}",
                    catalogue.text("queue.report"),
                    header.report.0,
                    catalogue.text("queue.submission"),
                    job.id().0,
                    header.account
                ));
                show_submission_mode(ui, header.mode, catalogue, theme);
                ui.label(catalogue.text(phase_key(job)));
                if let Some(wait) = state.waiting.iter().find(|wait| {
                    wait.job == job.id()
                        && wait.revision == job.revision()
                        && job.phase() == JobPhase::Queued
                }) {
                    crate::submission_waiting::show(ui, wait, catalogue);
                }
                ui.label(format!(
                    "{} {} · {} {} · {} {}",
                    catalogue.text("queue.observed"),
                    crate::attachment_views::format_time(header.observed_at),
                    job.request().rows().len(),
                    catalogue.text("queue.rows"),
                    job.attempts().len(),
                    catalogue.text("queue.attempts")
                ));
                if let Some(reason) = job.review_reason() {
                    ui.label(catalogue.text(review_reason_key(reason)));
                }
                show_result(ui, job, state.verified_receipts(job.id()), catalogue);
                if lifecycle::show(
                    ui,
                    job,
                    state.lifecycle(job.id()),
                    state.can_refresh_lifecycle(job.id()),
                    catalogue,
                ) {
                    lifecycle_refresh = Some(job.id());
                }
                if let Some(action) =
                    crate::submission_partial::show(ui, job, catalogue, state.busy.is_none())
                {
                    unknown_action = Some((job.id(), action));
                }
                if let Some(action) = crate::submission_unknown::show(
                    ui,
                    job,
                    &state.jobs,
                    catalogue,
                    state.busy.is_none(),
                ) {
                    unknown_action = Some((job.id(), action));
                }
                if let Some(action) =
                    crate::submission_rejected::show(ui, job, catalogue, state.busy.is_none())
                {
                    unknown_action = Some((job.id(), action));
                }
                if let Some(partition) = state.split_partition(job.id()) {
                    let ready = partition
                        .ready_rows()
                        .iter()
                        .map(|row| row.0.to_string())
                        .collect::<Vec<_>>()
                        .join(", ");
                    let cooling = partition
                        .cooling_rows()
                        .iter()
                        .map(|row| row.row().0.to_string())
                        .collect::<Vec<_>>()
                        .join(", ");
                    ui.label(format!("{} {ready}", catalogue.text("queue.split_ready")));
                    ui.label(format!(
                        "{} {cooling}",
                        catalogue.text("queue.split_cooling")
                    ));
                    for row in partition.cooling_rows() {
                        ui.label(format!(
                            "{} {} · {} {}",
                            catalogue.text("queue.split_row"),
                            row.row().0,
                            catalogue.text("queue.split_until"),
                            crate::attachment_views::format_time(row.until())
                        ));
                    }
                    if ui
                        .add_enabled(
                            state.busy.is_none(),
                            egui::Button::new(tallyline_egui::button_text(format!(
                                "{} {}",
                                catalogue.text("queue.split"),
                                job.id().0
                            ))),
                        )
                        .clicked()
                    {
                        split = Some(job.id());
                    }
                }
                let retry_policy = SubmissionRetrySettings::default();
                if job.result().is_some_and(SubmissionResult::is_daily_quota)
                    && job.attempts().len() >= retry_policy.maximum_attempts()
                {
                    ui.label(catalogue.text("queue.quota_attempts_exhausted"));
                }
                if let Some(at) = retry_policy.automatic_retry_at(job) {
                    ui.label(format!(
                        "{} {}",
                        catalogue.text("queue.next_retry"),
                        crate::attachment_views::format_time(at)
                    ));
                } else if job.phase() == JobPhase::Held
                    && job.can_retry()
                    && matches!(
                        job.result().map(SubmissionResult::final_result),
                        Some(
                            SubmissionResult::NotProcessed(_)
                                | SubmissionResult::RateLimitedCode(_)
                        )
                    )
                {
                    ui.label(catalogue.text("queue.retries_exhausted"));
                }
                if (job.can_retry() || job.can_retry_after_update(state.client_version))
                    && ui
                        .add_enabled(
                            state.busy.is_none(),
                            egui::Button::new(tallyline_egui::button_text(format!(
                                "{} {}",
                                catalogue.text(
                                    if job.can_acknowledge_service_pause()
                                        || (matches!(
                                            job.phase(),
                                            JobPhase::Cancelled | JobPhase::Returned
                                        ) && job.can_retry_after_update(state.client_version))
                                    {
                                        "queue.retry_queue"
                                    } else {
                                        "queue.retry_now"
                                    }
                                ),
                                job.id().0
                            ))),
                        )
                        .clicked()
                {
                    retry = Some(job.id());
                }
                if ui
                    .add_enabled(
                        state.can_cancel(job.id()),
                        egui::Button::new(tallyline_egui::button_text(format!(
                            "{} {}",
                            catalogue.text("queue.cancel"),
                            job.id().0
                        ))),
                    )
                    .clicked()
                {
                    cancel = Some(job.id());
                }
            });
        });
    }
    if let Some((id, action)) = unknown_action {
        state.request_resolution(id, action);
    }
    if let Some(id) = cancel {
        state.request_cancel(id);
    }
    if let Some(id) = retry {
        state.request_retry(id);
    }
    if let Some(id) = split {
        state.request_split(id);
    }
    if let Some(id) = lifecycle_refresh {
        state.request_lifecycle_refresh(id);
    }
    state.take_command()
}

/// Draws the fixed submission mode with its dedicated design-system tokens.
pub fn show_submission_mode(
    ui: &mut egui::Ui,
    mode: SubmissionMode,
    catalogue: &Catalogue,
    theme: tallyline_egui::Theme,
) {
    let palette = match theme {
        tallyline_egui::Theme::Dark => tallyline_egui::tokens::DARK,
        tallyline_egui::Theme::Light => tallyline_egui::tokens::LIGHT,
        tallyline_egui::Theme::HighContrast => tallyline_egui::tokens::HIGH_CONTRAST,
    };
    let (key, foreground, background) = match mode {
        SubmissionMode::DryRun => ("queue.dry_run", "env-test-fg", "env-test-bg"),
        SubmissionMode::Production => {
            ("queue.production", "env-production-fg", "env-production-bg")
        }
    };
    ui.add(tallyline_egui::Chip {
        text: catalogue.text(key),
        foreground: palette
            .color(foreground)
            .unwrap_or_else(|| ui.visuals().text_color()),
        background: palette
            .color(background)
            .unwrap_or(egui::Color32::TRANSPARENT),
    });
}
fn phase_key(job: &SubmissionJob) -> &'static str {
    match job.phase() {
        JobPhase::Queued => "queue.queued",
        JobPhase::Sending => "queue.sending",
        JobPhase::Held => "queue.held",
        JobPhase::Rejected => "queue.rejected",
        JobPhase::Paused => "queue.paused",
        JobPhase::Succeeded => match job.request().header().mode {
            SubmissionMode::DryRun => "queue.test_succeeded",
            SubmissionMode::Production => "queue.submitted",
        },
        JobPhase::PartiallyAccepted => "queue.partial",
        JobPhase::OutcomeUnknown => "queue.unknown",
        JobPhase::Returned => "queue.returned",
        JobPhase::Cancelled => "queue.cancelled",
    }
}
fn show_result(
    ui: &mut egui::Ui,
    job: &SubmissionJob,
    verified: Option<&[VerifiedUexReport]>,
    catalogue: &Catalogue,
) {
    if let Some(receipts) = verified {
        show_verified(ui, receipts, catalogue);
        return;
    }
    let Some(result) = job.result() else {
        return;
    };
    show_closed_result(ui, result, catalogue);
}
fn show_closed_result(ui: &mut egui::Ui, result: &SubmissionResult, catalogue: &Catalogue) {
    match result {
        SubmissionResult::AccountRefusal {
            code,
            reason,
            check,
            outcome,
            ..
        } => {
            if let Some(code) = code {
                ui.label(code.as_str());
            }
            if matches!(check, SubmissionAccountCheck::Pending) {
                ui.label(catalogue.text("queue.account_check_pending"));
            } else {
                show_closed_result(
                    ui,
                    &SubmissionResult::Paused(match check {
                        SubmissionAccountCheck::Completed(action) => *action,
                        SubmissionAccountCheck::Pending | SubmissionAccountCheck::NotRequired => {
                            *reason
                        }
                    }),
                    catalogue,
                );
            }
            if !matches!(
                outcome.as_ref(),
                SubmissionResult::Paused(_) | SubmissionResult::PausedCode { .. }
            ) {
                show_closed_result(ui, outcome, catalogue);
            }
        }
        SubmissionResult::PausedCode { code, reason } => {
            ui.label(code.as_str());
            show_closed_result(ui, &SubmissionResult::Paused(*reason), catalogue);
        }
        SubmissionResult::DuplicateEvidence(previous) => {
            ui.label(format!(
                "{} {}",
                catalogue.text("queue.possibly_received"),
                previous.0
            ));
        }
        SubmissionResult::Returned(SubmissionReturnReason::PossiblyAlreadyReceived) => {
            ui.label(catalogue.text("queue.possibly_received"));
        }
        SubmissionResult::RateLimitedCode(code) => {
            ui.label(format!(
                "{} {}",
                catalogue.text("queue.rate_limited"),
                code.as_str()
            ));
        }
        SubmissionResult::NotProcessed(NotProcessed::RateLimited | NotProcessed::RowBudget) => {
            ui.label(catalogue.text("queue.rate_limited"));
        }
        SubmissionResult::NotProcessed(NotProcessed::ImageUpload | NotProcessed::ImageStorage) => {
            ui.label(catalogue.text("queue.image_unavailable"));
        }
        SubmissionResult::NotProcessed(
            NotProcessed::DailyQuota | NotProcessed::DailyQuotaUntil(_),
        ) => {
            ui.label(catalogue.text("queue.daily_quota"));
        }
        SubmissionResult::Rechecked { code, outcome } => {
            ui.label(format!(
                "{} {}",
                catalogue.text("queue.checked_code"),
                code.as_str()
            ));
            show_closed_result(ui, outcome, catalogue);
        }
        SubmissionResult::PartialError { code, row, .. } => {
            ui.label(format!(
                "{} {}",
                catalogue.text("queue.partial_error"),
                code.as_str()
            ));
            if let Some(row) = row {
                ui.label(format!(
                    "{} {}",
                    catalogue.text("queue.failed_row"),
                    row + 1
                ));
            }
        }
        SubmissionResult::NeedsOutcomeCheck { code, .. } => {
            ui.label(format!(
                "{} {}",
                catalogue.text("queue.receipt_check_required"),
                code.as_str()
            ));
        }
        SubmissionResult::Rejected(refusal) => {
            let code = refusal.code();
            ui.label(format!(
                "{} {}",
                catalogue.text("queue.rejected_code"),
                code.as_str()
            ));
            ui.label(catalogue.text(
                if refusal.correction() == SubmissionCorrection::ReviewClient {
                    "queue.permanent_action"
                } else {
                    "queue.invalid_game_version"
                },
            ));
        }
        SubmissionResult::Returned(SubmissionReturnReason::MaxRowsExceeded) => {
            ui.label(catalogue.text("queue.max_rows_exceeded"));
        }
        SubmissionResult::Returned(SubmissionReturnReason::TerminalNotFound) => {
            ui.label(catalogue.text("queue.terminal_not_found"));
        }
        SubmissionResult::Returned(
            SubmissionReturnReason::InvalidScreenshot | SubmissionReturnReason::ScreenshotTooLarge,
        ) => {
            ui.label(catalogue.text("queue.invalid_screenshot"));
        }
        SubmissionResult::Held(reason) => {
            ui.label(catalogue.text(match reason {
                SubmissionHoldReason::AcceptanceClosed => "queue.acceptance_closed",
                SubmissionHoldReason::Cooldown => "queue.duplicate_cooldown",
                SubmissionHoldReason::ApiChanged => "queue.api_changed",
            }));
        }
        SubmissionResult::VersionLockedOut(_) => {
            show_closed_result(
                ui,
                &SubmissionResult::Paused(SubmissionPauseReason::UpdateRequired),
                catalogue,
            );
        }
        SubmissionResult::Paused(reason) => {
            ui.label(catalogue.text(match reason {
                SubmissionPauseReason::AppToken => "queue.app_token",
                SubmissionPauseReason::UpdateRequired => "queue.update_required",
                SubmissionPauseReason::KeyChecked => "queue.key_checked",
                SubmissionPauseReason::Reauthenticate => "queue.reauthenticate",
                SubmissionPauseReason::KeyCheckUnavailable => "queue.key_check_unavailable",
            }));
        }
        SubmissionResult::NotProcessed(NotProcessed::ServiceUnavailable) => {
            ui.label(catalogue.text("queue.service_unavailable"));
        }
        SubmissionResult::DryRunSucceeded => {
            ui.label(catalogue.text("queue.no_uex_ids"));
        }
        SubmissionResult::ProductionSucceeded(receipts) => {
            show_verified(ui, receipts, catalogue);
        }
        SubmissionResult::ProductionAccepted(ids) | SubmissionResult::Partial(ids) => {
            for id in ids {
                ui.label(format!(
                    "{} {} · {}",
                    catalogue.text("queue.unverified_id"),
                    id.id(),
                    crate::attachment_views::format_time(id.date_added())
                ));
            }
        }
        SubmissionResult::Returned(SubmissionReturnReason::PreflightChanged) => {
            ui.label(catalogue.text("queue.preflight_changed"));
        }
        SubmissionResult::Unknown => {
            ui.label(catalogue.text("queue.no_replay"));
        }
        SubmissionResult::Returned(SubmissionReturnReason::ScreenshotRequired) => {
            ui.label(catalogue.text("queue.screenshot_required"));
        }
        SubmissionResult::Returned(SubmissionReturnReason::InvalidObservationTime) => {
            ui.label(catalogue.text("queue.invalid_observation_time"));
        }
        SubmissionResult::NotProcessed(NotProcessed::ConnectFailure) => {
            ui.label(catalogue.text("queue.connect_failure"));
        }
    }
}
fn show_verified(ui: &mut egui::Ui, receipts: &[VerifiedUexReport], catalogue: &Catalogue) {
    for receipt in receipts {
        ui.label(format!(
            "{} {} · {} {}",
            catalogue.text("queue.verified_id"),
            receipt.id(),
            catalogue.text("queue.row"),
            receipt.row().0
        ));
    }
}

pub(crate) fn review_reason_key(reason: SendReturnReason) -> &'static str {
    use tallyline_submission::api::model::SendReturnReason;
    match reason {
        SendReturnReason::ReviewRequired => "queue.review_required",
        SendReturnReason::GameVersionChanged => "queue.version_changed",
        SendReturnReason::ObservationAgeUnconfirmed => "queue.age_unconfirmed",
        SendReturnReason::ObservationTooOld => "queue.observation_too_old",
        SendReturnReason::ObservationInFuture => "queue.observation_in_future",
    }
}

impl SubmissionQueue {
    /// Queues one explicit resolution action against the exact displayed partial or unknown origin.
    pub fn request_resolution(&mut self, id: JobId, action: ResolutionQueueAction) -> bool {
        if self.busy.is_some() {
            return false;
        }
        let Some(job) = self
            .jobs
            .iter()
            .find(|job| job.id() == id && action.eligible(job))
        else {
            return false;
        };
        let command = QueueCommand::Resolve {
            id,
            expected_revision: job.revision(),
            action,
        };
        self.pending = Some(command);
        self.busy = Some(command);
        self.failure = None;
        true
    }

    pub(crate) fn completed(&mut self, id: JobId, expected_revision: u64) {
        if self.busy.map(QueueCommand::identity) == Some((id, expected_revision)) {
            self.busy = None;
            self.pending = None;
        }
    }
}

mod lifecycle;
