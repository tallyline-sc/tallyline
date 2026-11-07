// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Atomic admission of typed field commits and explicit manual actions.

use crate::manual_editor::ManualEditor;
use tallyline_reference_data::api::model::ReferenceSnapshot;
use tallyline_reporting::api::ManualEdit;
use tallyline_reporting::api::model::Report;

pub(crate) struct ManualFrame {
    pub(crate) edits: Vec<ManualEdit>,
    pub(crate) actions: Vec<ManualEdit>,
    pub(crate) invalid: bool,
}

impl ManualFrame {
    pub(crate) fn new(actions: Vec<ManualEdit>) -> Self {
        Self {
            edits: Vec::new(),
            actions,
            invalid: false,
        }
    }
    pub(crate) fn finish(
        mut self,
        editor: &mut ManualEditor,
        report: &Report,
        snapshot: &ReferenceSnapshot,
        invalid_text: &str,
    ) {
        if !self.actions.is_empty() {
            self.edits.clear();
            if !self.actions.iter().any(|action| matches!(action, ManualEdit::ChangeKey(key) if key.terminal() != report.key().terminal())) {
                collect(&editor.faction_affinity, report.faction_affinity(), ManualEdit::FactionAffinity, &mut self.edits, &mut self.invalid);
            }
            for row in &editor.rows {
                if self
                    .actions
                    .iter()
                    .any(|action| matches!(action, ManualEdit::TakeOver(id) if *id == row.id))
                {
                    continue;
                }
                if let Some(stored) = report.rows().iter().find(|stored| stored.id() == row.id) {
                    collect(
                        &row.price,
                        stored.price(),
                        |value| ManualEdit::Price(row.id, value),
                        &mut self.edits,
                        &mut self.invalid,
                    );
                    collect(
                        &row.stock,
                        stored.stock(),
                        |value| ManualEdit::Stock(row.id, value),
                        &mut self.edits,
                        &mut self.invalid,
                    );
                    collect(
                        &row.status,
                        stored.status(),
                        |value| ManualEdit::Status(row.id, value),
                        &mut self.edits,
                        &mut self.invalid,
                    );
                }
            }
        }
        self.edits.retain(|edit| match edit {
            ManualEdit::FactionAffinity(value) => report.faction_affinity() != *value,
            ManualEdit::Price(id, value) => report
                .rows()
                .iter()
                .any(|row| row.id() == *id && row.price() != *value),
            ManualEdit::Stock(id, value) => report
                .rows()
                .iter()
                .any(|row| row.id() == *id && row.stock() != *value),
            ManualEdit::Status(id, value) => report
                .rows()
                .iter()
                .any(|row| row.id() == *id && row.status() != *value),
            ManualEdit::AddCommodity(_) | ManualEdit::TakeOver(_) | ManualEdit::ChangeKey(_) => {
                true
            }
        });
        let mut context = Vec::new();
        for action in self.actions {
            if matches!(action, ManualEdit::ChangeKey(_)) {
                context.push(action);
            } else {
                self.edits.push(action);
            }
        }
        context.extend(self.edits);
        if !context.is_empty() {
            editor.queue_edits(context, snapshot.clone());
        }
        if self.invalid {
            editor.notice = Some(invalid_text.to_owned());
        }
    }
}

pub(crate) fn optional<T: std::str::FromStr>(text: &str) -> Result<Option<T>, ()> {
    if text.trim().is_empty() {
        Ok(None)
    } else {
        text.trim().parse().map(Some).map_err(|_| ())
    }
}

fn collect<T: std::str::FromStr + PartialEq>(
    input: &str,
    stored: Option<T>,
    edit: impl FnOnce(Option<T>) -> ManualEdit,
    edits: &mut Vec<ManualEdit>,
    invalid: &mut bool,
) {
    match optional(input) {
        Ok(value) => {
            if value != stored {
                edits.push(edit(value));
            }
        }
        Err(()) => *invalid = true,
    }
}
