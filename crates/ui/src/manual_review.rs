// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Explicit navigation through Reporting-owned manual review decisions.

use tallyline_reference_data::api::model::ReferenceSnapshot;
use tallyline_reporting::api::model::{ManualReview, ReportId, ReviewField};

/// An explicit direction through unresolved price and status decisions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewDirection {
    /// Move to the next required decision, wrapping at the end.
    Next,
    /// Move to the previous required decision, wrapping at the start.
    Previous,
}
/// A review operation performed by the app-owned manual worker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManualReviewAction {
    /// Reassess against the latest displayed reference snapshot.
    Reassess,
    /// Explicitly approve one displayed value at its current deviation.
    Confirm(ReviewField),
    /// Record keyboard focus without approving a value.
    Focus(ReviewField),
}
/// A manual review request bound to the displayed Draft and references.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManualReviewCommand {
    /// Validated SCU policy bound to this request.
    pub stock_settings: tallyline_reporting::api::model::StockSettings,
    /// Persistent Draft identity.
    pub id: ReportId,
    /// Version displayed when the user acted.
    pub expected: u64,
    /// Explicit action or passive reassessment.
    pub action: ManualReviewAction,
    /// Immutable reference data used for this decision.
    pub snapshot: ReferenceSnapshot,
}
/// Desktop review state that never requests focus from a passive result.
#[derive(Default)]
pub struct ManualReviewState {
    review: Option<ManualReview>,
    snapshot: Option<ReferenceSnapshot>,
    report: Option<ReportId>,
    focused: Option<ReviewField>,
    in_flight_focus: Option<ReviewField>,
    deferred_focus: Option<ReviewField>,
    current: bool,
}
impl ManualReviewState {
    /// Returns the committed review currently displayed.
    #[must_use]
    pub const fn review(&self) -> Option<&ManualReview> {
        self.review.as_ref()
    }
    /// Returns the field explicitly focused by the user.
    #[must_use]
    pub const fn focused(&self) -> Option<ReviewField> {
        self.focused
    }
    /// Applies a committed result without moving keyboard focus.
    pub fn received(&mut self, review: ManualReview, snapshot: ReferenceSnapshot) {
        if self.report != Some(review.report().id()) {
            self.focused = None;
            self.deferred_focus = None;
        }
        self.in_flight_focus = None;
        self.current = true;
        self.report = Some(review.report().id());
        self.review = Some(review);
        self.snapshot = Some(snapshot);
    }
    /// Hides stale assessment data until a new committed review arrives.
    pub fn invalidate(&mut self) {
        self.current = false;
        self.in_flight_focus = None;
        self.deferred_focus = None;
        self.review = None;
        self.snapshot = None;
    }
    pub(crate) fn updating(&self) -> bool {
        self.review.is_some() && !self.current
    }
    pub(crate) fn suspend(&mut self) {
        self.current = false;
        self.in_flight_focus = None;
        self.deferred_focus = None;
    }
    /// Selects a required decision explicitly and returns its version-bound focus request.
    #[must_use]
    pub fn navigate(&mut self, direction: ReviewDirection) -> Option<ManualReviewCommand> {
        if !self.current {
            return None;
        }
        let targets = self.review.as_ref()?.confirmation_fields();
        if targets.is_empty() {
            return None;
        }
        let position = self
            .focused
            .and_then(|focused| targets.iter().position(|field| *field == focused));
        let index = match (position, direction) {
            (None, ReviewDirection::Next) => 0,
            (None, ReviewDirection::Previous) => targets.len() - 1,
            (Some(index), ReviewDirection::Next) => (index + 1) % targets.len(),
            (Some(index), ReviewDirection::Previous) => (index + targets.len() - 1) % targets.len(),
        };
        let field = *targets.get(index)?;
        self.focused = Some(field);
        self.command(ManualReviewAction::Focus(field))
    }
    /// Records explicit focus only for a current required decision.
    #[must_use]
    pub fn focus(&mut self, field: ReviewField) -> Option<ManualReviewCommand> {
        if !self.current {
            return None;
        }
        if !self.review.as_ref()?.confirmation_fields().contains(&field) {
            return None;
        }
        self.focused = Some(field);
        self.command(ManualReviewAction::Focus(field))
    }
    /// Requests explicit approval of the focused unresolved value.
    #[must_use]
    pub fn confirm_focused(&self) -> Option<ManualReviewCommand> {
        let field = self.focused?;
        self.confirm(field)
    }
    /// Reports a current Reporting-owned confirmation requirement without building a command.
    #[must_use]
    pub fn needs_confirmation(&self, field: ReviewField) -> bool {
        self.review
            .as_ref()
            .and_then(|review| {
                review
                    .report()
                    .rows()
                    .iter()
                    .find(|row| row.id() == field.row())
            })
            .is_some_and(|row| match field {
                ReviewField::Price(_) => row.price_needs_confirmation(),
                ReviewField::Status(_) => row.status_needs_confirmation(),
                ReviewField::Stock(_) => row.stock_needs_confirmation(),
            })
    }
    pub(crate) fn mark_focus_pending(&mut self, field: ReviewField) {
        self.in_flight_focus = Some(field);
    }
    pub(crate) fn defer_focus(&mut self, field: ReviewField) {
        self.deferred_focus = (self.in_flight_focus != Some(field)).then_some(field);
    }
    pub(crate) fn take_deferred_focus(&mut self) -> Option<ManualReviewCommand> {
        if !self.current {
            return None;
        }
        self.review.as_ref()?;
        let field = self.deferred_focus.take()?;
        self.focus(field)
    }
    pub(crate) fn reference_matches(&self, snapshot: &ReferenceSnapshot) -> bool {
        self.snapshot.as_ref() == Some(snapshot)
    }
    pub(crate) fn is_current(&self, command: &ManualReviewCommand) -> bool {
        if !self.current {
            return false;
        }
        self.review.as_ref().is_some_and(|review| {
            review.report().id() == command.id && review.report().version() == command.expected
        }) && self
            .review
            .as_ref()
            .is_some_and(|review| review.stock_settings() == command.stock_settings)
            && self.snapshot.as_ref() == Some(&command.snapshot)
    }
    /// Requests explicit approval only for a Reporting-owned required decision.
    #[must_use]
    pub fn confirm(&self, field: ReviewField) -> Option<ManualReviewCommand> {
        if !self.current {
            return None;
        }
        self.needs_confirmation(field)
            .then(|| self.command(ManualReviewAction::Confirm(field)))
            .flatten()
    }
    fn command(&self, action: ManualReviewAction) -> Option<ManualReviewCommand> {
        let review = self.review.as_ref()?;
        Some(ManualReviewCommand {
            stock_settings: review.stock_settings(),
            id: review.report().id(),
            expected: review.report().version(),
            action,
            snapshot: self.snapshot.as_ref()?.clone(),
        })
    }
}
