// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! User-owned manual editor state and bounded persistence commands.

use tallyline_game::api::model::GameEnvironment;
use tallyline_reference_data::api::model::ReferenceSnapshot;
use tallyline_reporting::api::model::{Report, ReportId, ReportKey, RowId};
use tallyline_reporting::api::{ManualEdit, RepositoryError};
use tallyline_shared_kernel::api::model::{CommodityId, TerminalId, TradeSide};

pub use edits::ManualEditRequestId;

/// A manual Report operation handled by the app-owned storage worker.
#[derive(Clone, Debug)]
pub enum ManualCommand {
    /// Review, focus or explicitly approve a saved price or status.
    Review(crate::manual_review::ManualReviewCommand),
    /// Explicit local attachment work, performed only by the bounded app worker.
    Attachment(crate::attachment_editor::ManualAttachmentCommand),
    /// Create an empty Draft with explicit context.
    Create {
        /// User-selected context.
        key: ReportKey,
        /// Immutable vocabulary and values shown for this decision.
        snapshot: ReferenceSnapshot,
    },
    /// Persist an explicit edit against its observed version and reference snapshot.
    Edit {
        /// Unique attempt owned by this editor session.
        request: ManualEditRequestId,
        /// Persistent Draft identity.
        id: ReportId,
        /// Version last displayed to the user.
        expected: u64,
        /// Ordered explicit inputs and actions.
        edits: Vec<ManualEdit>,
        /// Immutable data used for this decision.
        snapshot: ReferenceSnapshot,
    },
}

pub(crate) struct RowInput {
    pub(crate) id: RowId,
    pub(crate) commodity: CommodityId,
    pub(crate) price: String,
    pub(crate) stock: String,
    pub(crate) status: String,
}

/// Manual Draft choices, with reference values kept outside the inputs.
#[derive(Default)]
pub struct ManualEditor {
    pub(crate) reports: Vec<Report>,
    pub(crate) selected: Option<ReportId>,
    pub(crate) environment: Option<GameEnvironment>,
    pub(crate) terminal: Option<TerminalId>,
    pub(crate) side: Option<TradeSide>,
    pub(crate) terminal_query: String,
    pub(crate) location_query: String,
    pub(crate) commodity_query: String,
    pub(crate) rows: Vec<RowInput>,
    pub(crate) faction_affinity: String,
    pub(crate) ready: bool,
    pub(crate) busy: bool,
    pub(crate) notice: Option<String>,
    command: Option<ManualCommand>,
    pending_edit: Option<edits::PendingEdit>,
    next_edit_request: u64,
    input_version: Option<u64>,
    stock_settings: tallyline_reporting::api::model::StockSettings,
    pending_reviews: Vec<(ReportId, u64)>,
    review_attempt: Option<(ReportId, u64, ReferenceSnapshot)>,
    pub(crate) review: crate::manual_review::ManualReviewState,
    pending_attachments: Vec<(ReportId, u64)>,
    pub(crate) attachment: crate::attachment_editor::AttachmentEditor,
}
impl ManualEditor {
    pub(crate) fn set_stock_settings(
        &mut self,
        settings: tallyline_reporting::api::model::StockSettings,
    ) {
        if self.stock_settings != settings {
            self.stock_settings = settings;
            self.review.invalidate();
            self.review_attempt = None;
        }
    }
    /// Returns the selected durable Draft.
    #[must_use]
    pub fn selected(&self) -> Option<&Report> {
        self.reports
            .iter()
            .find(|report| Some(report.id()) == self.selected)
    }
    /// Returns the next explicit request without changing keyboard focus.
    pub fn take_command(&mut self) -> Option<ManualCommand> {
        self.command.take()
    }
    /// Clears a failed delivery while keeping the user's input visible.
    pub fn failed(&mut self, error: RepositoryError) {
        if self.pending_edit.is_some() {
            return;
        }
        self.busy = false;
        self.pending_attachments
            .retain(|(id, _)| Some(*id) != self.selected);
        self.pending_reviews
            .retain(|(id, _)| Some(*id) != self.selected);
        self.notice = Some(error.to_string());
    }
    fn refresh_busy(&mut self) {
        self.busy = self.pending_edit.is_some()
            || self
                .selected
                .is_some_and(|id| self.review_or_attachment_pending(id));
    }
    pub(crate) fn queue(&mut self, command: ManualCommand) {
        if self.ready && !self.busy && self.pending_edit.is_none() {
            let pending = match &command {
                ManualCommand::Attachment(request) => Some((request.id, request.expected)),
                ManualCommand::Review(request) => {
                    self.pending_reviews.retain(|(id, _)| *id != request.id);
                    self.pending_reviews.push((request.id, request.expected));
                    if let crate::manual_review::ManualReviewAction::Focus(field) = request.action {
                        self.review.mark_focus_pending(field);
                    }
                    self.review_attempt =
                        Some((request.id, request.expected, request.snapshot.clone()));
                    if request.action == crate::manual_review::ManualReviewAction::Reassess {
                        self.review.suspend();
                    }
                    None
                }
                ManualCommand::Create { .. } | ManualCommand::Edit { .. } => {
                    self.review.invalidate();
                    self.review_attempt = None;
                    None
                }
            };
            if let Some((id, expected)) = pending {
                self.pending_attachments.retain(|(old, _)| *old != id);
                self.pending_attachments.push((id, expected));
            }
            self.command = Some(command);
            self.busy = true;
            self.notice = None;
        }
    }
    pub(crate) fn loaded(
        &mut self,
        mut reports: Vec<Report>,
        selected: Option<ReportId>,
        visible: impl Fn(&Report) -> bool,
    ) {
        let before = self
            .selected()
            .map(|report| (report.id(), report.version()));
        let mut positions = reports
            .iter()
            .enumerate()
            .map(|(index, report)| (report.id().0, index))
            .collect::<std::collections::BTreeMap<_, _>>();
        for previous in std::mem::take(&mut self.reports) {
            if let Some(current) = positions
                .get(&previous.id().0)
                .and_then(|index| reports.get_mut(*index))
            {
                if current.version() < previous.version() {
                    *current = previous;
                } else if self.selected == Some(previous.id())
                    && self.input_version == Some(previous.version())
                    && self.pending_edit.is_none()
                    && edits::same_input_values(&previous, current)
                {
                    self.input_version = Some(current.version());
                }
            } else {
                positions.insert(previous.id().0, reports.len());
                reports.push(previous);
            }
        }
        reports.retain(visible);
        let selected = selected
            .filter(|_| self.pending_edit.is_none())
            .filter(|id| reports.iter().any(|report| report.id() == *id));
        self.pending_attachments.retain(|(id, expected)| {
            reports
                .iter()
                .any(|report| report.id() == *id && report.version() == *expected)
        });
        self.reports = reports;
        self.ready = true;
        let changed = before
            != self
                .selected()
                .map(|report| (report.id(), report.version()));
        if changed {
            self.review.invalidate();
        }
        if let Some(id) = selected {
            self.busy = false;
            self.notice = None;
            self.attachment.readiness = None;
            self.select(id);
        } else if changed || self.selected.is_none() {
            self.attachment.readiness = None;
            self.busy = self
                .selected
                .is_some_and(|id| self.review_or_attachment_pending(id));
        }
        self.busy |= self.pending_edit.is_some();
        if self.selected.is_none() && self.environment.is_none() {
            self.environment = self
                .reports
                .first()
                .map(|report| report.key().environment());
        }
        if self.attachment.preview.as_ref().is_some_and(|preview| {
            self.selected().is_none_or(|report| {
                report
                    .attachment()
                    .is_none_or(|attachment| attachment.binding() != preview.binding())
            })
        }) {
            self.attachment.clear_preview();
        }
    }
    pub(crate) fn finish_attachment_request(&mut self, id: ReportId, expected: u64) -> bool {
        let matched = self.pending_attachments.contains(&(id, expected));
        if matched {
            self.pending_attachments
                .retain(|request| *request != (id, expected));
            if self.selected == Some(id) {
                self.busy = self.pending_edit.is_some();
            }
        }
        matched
    }
    pub(crate) fn select(&mut self, id: ReportId) {
        if let Some(report) = self.reports.iter().find(|report| report.id() == id) {
            if self.selected != Some(id) {
                self.busy = self.pending_edit.is_some() || self.review_or_attachment_pending(id);
                self.review = crate::manual_review::ManualReviewState::default();
                self.review_attempt = None;
            }
            if self.selected != Some(id)
                || self.attachment.preview.as_ref().is_some_and(|preview| {
                    report
                        .attachment()
                        .is_none_or(|attachment| attachment.binding() != preview.binding())
                })
            {
                self.attachment.clear_preview();
            }
            self.selected = Some(id);
            self.input_version = Some(report.version());
            self.environment = Some(report.key().environment());
            self.terminal = Some(report.key().terminal());
            self.side = Some(report.key().side());
            self.faction_affinity = report
                .faction_affinity()
                .map(|value| value.to_string())
                .unwrap_or_default();
            self.rows = report
                .rows()
                .iter()
                .map(|row| RowInput {
                    id: row.id(),
                    commodity: row.commodity(),
                    price: row.price().map(|v| v.to_string()).unwrap_or_default(),
                    stock: row.stock().map(|v| v.to_string()).unwrap_or_default(),
                    status: row.status().map(|v| v.to_string()).unwrap_or_default(),
                })
                .collect();
        }
    }
    pub(crate) fn new_draft(&mut self) {
        self.review = crate::manual_review::ManualReviewState::default();
        self.review_attempt = None;
        self.attachment.clear_preview();
        self.busy = self.pending_edit.is_some();
        self.selected = None;
        self.input_version = None;
        self.terminal = None;
        self.side = None;
        self.rows.clear();
        self.faction_affinity.clear();
    }
}

mod review;

mod edits;
