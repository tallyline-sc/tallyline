// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Passive review admission and request-bound result application.

use super::{ManualCommand, ManualEditor, ReferenceSnapshot, ReportId, RepositoryError};
use crate::manual_review::{ManualReviewAction, ManualReviewCommand};
use tallyline_reporting::api::model::ManualReview;
impl ManualEditor {
    /// Returns the committed numerical review for the selected Draft.
    #[must_use]
    pub fn review(&self) -> Option<&ManualReview> {
        self.review.review()
    }
    pub(super) fn review_or_attachment_pending(&self, id: ReportId) -> bool {
        self.pending_attachments
            .iter()
            .chain(&self.pending_reviews)
            .any(|(pending, _)| *pending == id)
    }
    pub(crate) fn inputs_pending(&self) -> bool {
        self.selected().is_some_and(|report| {
            !same(&self.faction_affinity, report.faction_affinity())
                || self.rows.iter().any(|input| {
                    report
                        .rows()
                        .iter()
                        .find(|row| row.id() == input.id)
                        .is_none_or(|row| {
                            !same(&input.price, row.price())
                                || !same(&input.stock, row.stock())
                                || !same(&input.status, row.status())
                        })
                })
        })
    }
    pub(crate) fn request_review(&mut self, snapshot: &ReferenceSnapshot) {
        if !self.ready || self.busy || self.inputs_pending() {
            return;
        }
        let Some(report) = self.selected() else {
            return;
        };
        if !report
            .rows()
            .iter()
            .any(|row| row.price().is_some() || row.stock().is_some() || row.status().is_some())
        {
            return;
        }
        if self
            .review_attempt
            .as_ref()
            .is_some_and(|(id, version, prior)| {
                *id == report.id() && *version == report.version() && prior == snapshot
            })
        {
            return;
        }
        self.queue(ManualCommand::Review(ManualReviewCommand {
            stock_settings: self.stock_settings,
            id: report.id(),
            expected: report.version(),
            action: ManualReviewAction::Reassess,
            snapshot: snapshot.clone(),
        }));
    }
    pub(crate) fn retry_review(&mut self, snapshot: &ReferenceSnapshot) {
        if !self.busy {
            self.review_attempt = None;
            self.request_review(snapshot);
        }
    }
    pub(crate) fn reviewed(
        &mut self,
        expected: u64,
        review: ManualReview,
        snapshot: ReferenceSnapshot,
    ) {
        let id = review.report().id();
        if !self.pending_reviews.contains(&(id, expected)) {
            return;
        }
        self.pending_reviews
            .retain(|request| *request != (id, expected));
        let Some(stored) = self
            .reports
            .iter_mut()
            .find(|report| report.id() == id && report.version() == expected)
        else {
            self.refresh_busy();
            return;
        };
        *stored = review.report().clone();
        if self.selected == Some(id) && self.input_version == Some(expected) {
            self.input_version = Some(review.report().version());
        }
        if self.selected == Some(id) && review.stock_settings() == self.stock_settings {
            self.review_attempt = Some((id, review.report().version(), snapshot.clone()));
            self.review.received(review, snapshot);
            self.notice = None;
        }
        self.refresh_busy();
    }
    pub(crate) fn review_failed(&mut self, id: ReportId, expected: u64, error: RepositoryError) {
        if !self.pending_reviews.contains(&(id, expected)) {
            return;
        }
        self.pending_reviews
            .retain(|request| *request != (id, expected));
        self.refresh_busy();
        if self.selected == Some(id) {
            self.review.invalidate();
            self.notice = Some(error.to_string());
        }
    }
    pub(crate) fn review_reference_changed(&mut self, snapshot: &ReferenceSnapshot) {
        if self.review.review().is_some() && !self.review.reference_matches(snapshot) {
            self.review.suspend();
        }
    }
}
fn same<T: std::str::FromStr + PartialEq>(text: &str, value: Option<T>) -> bool {
    if text.trim().is_empty() {
        value.is_none()
    } else {
        text.trim()
            .parse::<T>()
            .is_ok_and(|entered| value.as_ref() == Some(&entered))
    }
}
