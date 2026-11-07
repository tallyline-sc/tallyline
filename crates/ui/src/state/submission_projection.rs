// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Version-monotonic Draft publication constrained by committed queue lifecycle.

use super::ShellState;
use tallyline_reporting::api::model::{Report, ReportId};
use tallyline_submission::api::model::{JobPhase, SubmissionJob};

impl ShellState {
    pub(super) fn receive_drafts(&mut self, reports: Vec<Report>, selected: Option<ReportId>) {
        let mut lifecycle = std::collections::BTreeMap::<u64, &SubmissionJob>::new();
        for job in self.submission.queue.jobs() {
            lifecycle
                .entry(job.request().header().report.0)
                .and_modify(|previous| {
                    if previous.id() < job.id() {
                        *previous = job;
                    }
                })
                .or_insert(job);
        }
        self.manual.loaded(reports, selected, |report| {
            lifecycle
                .get(&report.id().0)
                .is_none_or(|job| match job.phase() {
                    JobPhase::Returned | JobPhase::Cancelled => {
                        report.version() > job.request().header().report_version
                    }
                    JobPhase::Queued
                    | JobPhase::Sending
                    | JobPhase::Held
                    | JobPhase::Paused
                    | JobPhase::Rejected
                    | JobPhase::Succeeded
                    | JobPhase::PartiallyAccepted
                    | JobPhase::OutcomeUnknown => false,
                })
        });
    }
}
