// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact manual review tied to one Report version.

use crate::api::model::{PriceAssessment, Report, StatusAssessment, StockAssessment};

/// Price, stock and status assessments of one exact manual Report snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManualReview {
    report: Report,
    stock_settings: crate::api::model::StockSettings,
    prices: Vec<PriceAssessment>,
    stocks: Vec<StockAssessment>,
    statuses: Vec<StatusAssessment>,
    accepted_references: Vec<AcceptedReference>,
}
impl ManualReview {
    pub(crate) fn new(
        report: Report,
        stock_settings: crate::api::model::StockSettings,
        prices: Vec<PriceAssessment>,
        stocks: Vec<StockAssessment>,
        statuses: Vec<StatusAssessment>,
        accepted_references: Vec<AcceptedReference>,
    ) -> Self {
        Self {
            report,
            stock_settings,
            prices,
            stocks,
            statuses,
            accepted_references,
        }
    }
    /// Returns the exact validated SCU policy used by this immutable review.
    #[must_use]
    pub const fn stock_settings(&self) -> crate::api::model::StockSettings {
        self.stock_settings
    }
    /// Returns the exact Report snapshot assessed by this review.
    #[must_use]
    pub const fn report(&self) -> &Report {
        &self.report
    }
    /// Returns exact price assessments without reading confidence for USER values.
    #[must_use]
    pub fn prices(&self) -> &[PriceAssessment] {
        &self.prices
    }
    /// Returns exact stock assessments using the effective shipping bands.
    #[must_use]
    pub fn stocks(&self) -> &[StockAssessment] {
        &self.stocks
    }
    /// Returns status assessments without altering inventory quantities.
    #[must_use]
    pub fn statuses(&self) -> &[StatusAssessment] {
        &self.statuses
    }
}

/// A reviewed manual field identified independently of its value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewField {
    /// An explicitly entered price.
    Price(tallyline_shared_kernel::api::model::RowId),
    /// An explicitly entered inventory quantity.
    Stock(tallyline_shared_kernel::api::model::RowId),
    /// An explicitly selected status.
    Status(tallyline_shared_kernel::api::model::RowId),
}
impl ReviewField {
    /// Returns the stable row identity.
    #[must_use]
    pub const fn row(self) -> tallyline_shared_kernel::api::model::RowId {
        match self {
            Self::Price(row) | Self::Stock(row) | Self::Status(row) => row,
        }
    }
}

/// Counts for assessed prices, stocks and statuses, without claiming complete release readiness.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ManualReviewSummary {
    /// Fields differing within tolerance before stale display lowering.
    pub minor: usize,
    /// Fields differing beyond tolerance before stale display lowering.
    pub major: usize,
    /// Assessed fields still requiring explicit approval.
    pub needs_confirmation: usize,
    /// Required decisions that have not received keyboard focus.
    pub unreviewed: usize,
}
impl ManualReview {
    /// Returns every price, stock or status decision still requiring explicit approval in row order.
    #[must_use]
    pub fn confirmation_fields(&self) -> Vec<ReviewField> {
        self.report
            .rows()
            .iter()
            .flat_map(|row| {
                [
                    row.price_needs_confirmation()
                        .then_some(ReviewField::Price(row.id())),
                    row.stock_needs_confirmation()
                        .then_some(ReviewField::Stock(row.id())),
                    row.status_needs_confirmation()
                        .then_some(ReviewField::Status(row.id())),
                ]
            })
            .flatten()
            .collect()
    }
    /// Returns the price, stock and status counters from Reporting-owned decision state.
    #[must_use]
    pub fn summary(&self) -> ManualReviewSummary {
        use crate::api::model::Deviation;
        let deviations = self
            .prices
            .iter()
            .filter_map(|price| price.deviation())
            .chain(self.stocks.iter().filter_map(|stock| stock.deviation()))
            .chain(self.statuses.iter().map(|status| status.deviation()));
        let mut summary = ManualReviewSummary::default();
        for deviation in deviations {
            match deviation {
                Deviation::Minor => summary.minor += 1,
                Deviation::Major => summary.major += 1,
                Deviation::Equal | Deviation::NoReference => (),
            }
        }
        summary.needs_confirmation = self.confirmation_fields().len();
        summary.unreviewed = self
            .report
            .rows()
            .iter()
            .map(|row| {
                usize::from(row.price_unreviewed())
                    + usize::from(row.stock_unreviewed())
                    + usize::from(row.status_unreviewed())
            })
            .sum();
        summary
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AcceptedReference {
    pub(crate) row: tallyline_shared_kernel::api::model::RowId,
    pub(crate) prior: tallyline_reference_data::api::model::AcceptedLocalPrior,
}
impl ManualReview {
    /// Borrows distinct accepted-local provenance only when it supplied this row's reference.
    ///
    /// Terminal tolerance basis alone never claims that this value came from a UEX refresh.
    #[must_use]
    pub fn accepted_reference(
        &self,
        row: tallyline_shared_kernel::api::model::RowId,
    ) -> Option<&tallyline_reference_data::api::model::AcceptedLocalPrior> {
        self.accepted_references
            .iter()
            .find(|reference| reference.row == row)
            .map(|reference| &reference.prior)
    }
}
