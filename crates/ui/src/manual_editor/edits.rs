// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Request-bound manual edit admission and input reconciliation.

use super::{ManualCommand, ManualEditor, RowInput};
use tallyline_reference_data::api::model::ReferenceSnapshot;
use tallyline_reporting::api::model::{Report, ReportId, RowId};
use tallyline_reporting::api::{ManualEdit, RepositoryError};

/// Identifies one manual edit attempt within an editor session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ManualEditRequestId(
    /// Session-local monotonic sequence.
    pub u64,
);

pub(super) struct PendingEdit {
    request: ManualEditRequestId,
    id: ReportId,
    expected: u64,
    takeovers: Vec<RowId>,
}

impl ManualEditor {
    /// Refuses an undelivered command without releasing another edit attempt.
    pub fn failed_delivery(&mut self, command: ManualCommand) {
        match command {
            ManualCommand::Edit {
                request,
                id,
                expected,
                ..
            } => self.edit_finished(request, id, expected, Err(RepositoryError::Unavailable)),
            ManualCommand::Review(request) => {
                self.review_failed(request.id, request.expected, RepositoryError::Unavailable)
            }
            ManualCommand::Create { .. } | ManualCommand::Attachment(_) => {
                self.failed(RepositoryError::Unavailable)
            }
        }
    }
    pub(crate) fn queue_edits(&mut self, edits: Vec<ManualEdit>, snapshot: ReferenceSnapshot) {
        if edits.is_empty() || !self.ready || self.busy || self.pending_edit.is_some() {
            return;
        }
        let Some(report) = self.selected() else {
            return;
        };
        let (id, expected) = (report.id(), self.input_version.unwrap_or(report.version()));
        let Some(next) = self.next_edit_request.checked_add(1) else {
            self.notice = Some(RepositoryError::Unavailable.to_string());
            return;
        };
        let request = ManualEditRequestId(next);
        let takeovers = edits
            .iter()
            .filter_map(|edit| match edit {
                ManualEdit::TakeOver(row) => Some(*row),
                ManualEdit::AddCommodity(_)
                | ManualEdit::FactionAffinity(_)
                | ManualEdit::Price(..)
                | ManualEdit::Stock(..)
                | ManualEdit::Status(..)
                | ManualEdit::ChangeKey(_) => None,
            })
            .collect();
        let command = ManualCommand::Edit {
            request,
            id,
            expected,
            edits,
            snapshot,
        };
        self.queue(command);
        self.next_edit_request = next;
        self.pending_edit = Some(PendingEdit {
            request,
            id,
            expected,
            takeovers,
        });
    }

    pub(crate) fn edit_finished(
        &mut self,
        request: ManualEditRequestId,
        id: ReportId,
        expected: u64,
        result: Result<Report, RepositoryError>,
    ) {
        if !self.pending_edit.as_ref().is_some_and(|pending| {
            pending.request == request && pending.id == id && pending.expected == expected
        }) {
            return;
        }
        let Some(pending) = self.pending_edit.take() else {
            return;
        };
        match result {
            Ok(report) if report.id() == id && report.version() >= expected => {
                if let Some((index, before)) =
                    self.reports.iter().enumerate().find(|(_, stored)| {
                        stored.id() == id && stored.version() <= report.version()
                    })
                {
                    if self.selected == Some(id) {
                        let affinity_input = std::mem::take(&mut self.faction_affinity);
                        self.faction_affinity = text(report.faction_affinity());
                        if before.key().terminal() == report.key().terminal() {
                            retain(
                                &mut self.faction_affinity,
                                &affinity_input,
                                before.faction_affinity(),
                            );
                        }
                        reconcile(&mut self.rows, before, &report, &pending.takeovers);
                        self.input_version = Some(report.version());
                        self.environment = Some(report.key().environment());
                        self.terminal = Some(report.key().terminal());
                        self.side = Some(report.key().side());
                    }
                    self.reports.remove(index);
                    self.reports.insert(0, report);
                    if self.selected == Some(id) {
                        if !self.inputs_pending() {
                            self.notice = None;
                        }
                        self.review.invalidate();
                        self.review_attempt = None;
                    }
                }
            }
            Ok(_) => self.edit_refused(id, RepositoryError::Invalid),
            Err(error) => self.edit_refused(id, error),
        }
        self.refresh_busy();
    }

    fn edit_refused(&mut self, id: ReportId, error: RepositoryError) {
        if self.selected == Some(id) {
            if let Some(report) = self.selected() {
                let key = report.key();
                self.environment = Some(key.environment());
                self.terminal = Some(key.terminal());
                self.side = Some(key.side());
            }
            self.notice = Some(error.to_string());
        }
    }
}

fn reconcile(inputs: &mut Vec<RowInput>, before: &Report, after: &Report, takeovers: &[RowId]) {
    let previous = std::mem::take(inputs);
    *inputs = after
        .rows()
        .iter()
        .map(|row| {
            let mut input = RowInput {
                id: row.id(),
                commodity: row.commodity(),
                price: text(row.price()),
                stock: text(row.stock()),
                status: text(row.status()),
            };
            if !takeovers.contains(&row.id())
                && let Some(old) = previous.iter().find(|old| old.id == row.id())
                && let Some(stored) = before.rows().iter().find(|stored| stored.id() == row.id())
            {
                retain(&mut input.price, &old.price, stored.price());
                retain(&mut input.stock, &old.stock, stored.stock());
                retain(&mut input.status, &old.status, stored.status());
            }
            input
        })
        .collect();
}

fn retain<T: std::fmt::Display>(current: &mut String, input: &str, stored: Option<T>) {
    if input != text(stored) {
        *current = input.to_owned();
    }
}

fn text<T: std::fmt::Display>(value: Option<T>) -> String {
    value.map(|value| value.to_string()).unwrap_or_default()
}

pub(super) fn same_input_values(before: &Report, after: &Report) -> bool {
    before.key() == after.key()
        && before.faction_affinity() == after.faction_affinity()
        && before.rows().len() == after.rows().len()
        && before
            .rows()
            .iter()
            .zip(after.rows())
            .all(|(before, after)| {
                before.id() == after.id()
                    && before.commodity() == after.commodity()
                    && before.price() == after.price()
                    && before.stock() == after.stock()
                    && before.status() == after.status()
            })
}
