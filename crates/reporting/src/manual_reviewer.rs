// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Repository-independent numeric review with one explicit policy and assessment time.

use crate::api::RepositoryError;
use crate::api::model::{
    ManualReview, PriceAssessment, Report, ReportKey, StockAssessment, StockSettings,
};
use crate::price_assessment::PriceReference;
use tallyline_game::api::model::EnvironmentMapping;
use tallyline_reference_data::api::model::{ReferenceSnapshot, ScuTolerance};
use tallyline_shared_kernel::api::model::{CommodityId, Percent, Timestamp};

/// Immutable review policy shared by Draft editing and atomic release/send workflows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManualReviewer {
    mapping: EnvironmentMapping,
    stock_settings: StockSettings,
    price_tolerance_override: Option<Percent>,
}
impl ManualReviewer {
    /// Captures validated stock settings, current channel mapping and the optional price tightening.
    #[must_use]
    pub const fn new(
        mapping: EnvironmentMapping,
        stock_settings: StockSettings,
        price_tolerance_override: Option<Percent>,
    ) -> Self {
        Self {
            mapping,
            stock_settings,
            price_tolerance_override,
        }
    }
    /// Reassesses this exact Report without repository access, using one explicit assessment time.
    ///
    /// # Errors
    /// Refuses mismatched context and unavailable exact arithmetic without changing source values.
    pub fn reassess(
        &self,
        report: Report,
        snapshot: &ReferenceSnapshot,
        now: Timestamp,
    ) -> Result<ManualReview, RepositoryError> {
        if self.mapping.environment(report.key().environment()) != snapshot.environment() {
            return Err(RepositoryError::Invalid);
        }
        let prices = self.price_assessments(&report, snapshot, now);
        if prices
            .iter()
            .any(|assessment| assessment.unavailable().is_some())
        {
            return Err(RepositoryError::AssessmentUnavailable);
        }
        let stocks =
            self.stock_assessments(&report, snapshot, now, self.stock_settings.tolerance());
        if stocks
            .iter()
            .any(|assessment| assessment.unavailable().is_some())
        {
            return Err(RepositoryError::AssessmentUnavailable);
        }
        let statuses = self.status_assessments(&report, snapshot, now);
        let next = report
            .reassess_prices(&prices, snapshot.environment())
            .and_then(|report| report.reassess_stocks(&stocks, snapshot.environment()))
            .and_then(|report| report.reassess_status(&statuses, snapshot.environment()))
            .map_err(|_| RepositoryError::Invalid)?;
        let accepted_references = next
            .rows()
            .iter()
            .filter_map(|row| {
                match snapshot.prior_reference(
                    next.key().terminal(),
                    row.commodity(),
                    next.key().side(),
                ) {
                    Some(tallyline_reference_data::api::model::PriorReference::AcceptedLocal(
                        prior,
                    )) => Some(crate::manual_review::AcceptedReference {
                        row: row.id(),
                        prior: prior.clone(),
                    }),
                    _ => None,
                }
            })
            .collect();
        Ok(ManualReview::new(
            next,
            self.stock_settings,
            prices,
            stocks,
            statuses,
            accepted_references,
        ))
    }
    /// Resolves a channel with the exact immutable mapping used for this review policy.
    #[must_use]
    pub fn environment_for(
        &self,
        channel: tallyline_game::api::model::GameEnvironment,
    ) -> tallyline_reference_data::api::model::UexEnvironment {
        self.mapping.environment(channel)
    }
    pub(crate) fn price_assessments(
        &self,
        report: &Report,
        snapshot: &ReferenceSnapshot,
        now: Timestamp,
    ) -> Vec<PriceAssessment> {
        let tolerance = snapshot.effective_price_tolerance(self.price_tolerance_override);
        let staleness = snapshot.effective_staleness(None);
        report
            .rows()
            .iter()
            .filter_map(|row| {
                let value = row.price()?;
                Some(PriceAssessment::assess(
                    row.id(),
                    value,
                    reference_price(snapshot, report.key(), row.commodity()),
                    tolerance,
                    now,
                    staleness,
                ))
            })
            .collect()
    }

    pub(crate) fn stock_assessments(
        &self,
        report: &Report,
        snapshot: &ReferenceSnapshot,
        now: Timestamp,
        tolerance: ScuTolerance,
    ) -> Vec<StockAssessment> {
        let staleness = snapshot.effective_staleness(None);
        report
            .rows()
            .iter()
            .filter_map(|row| {
                let value = row.stock()?;
                let prior = snapshot.prior_reference(
                    report.key().terminal(),
                    row.commodity(),
                    report.key().side(),
                );
                Some(StockAssessment::assess(
                    row.id(),
                    value,
                    prior,
                    tolerance,
                    now,
                    staleness,
                ))
            })
            .collect()
    }

    pub(crate) fn status_assessments(
        &self,
        report: &Report,
        snapshot: &ReferenceSnapshot,
        now: Timestamp,
    ) -> Vec<crate::api::model::StatusAssessment> {
        report
            .rows()
            .iter()
            .filter_map(|row| {
                let value = row.status()?;
                let prior = snapshot.prior_reference(
                    report.key().terminal(),
                    row.commodity(),
                    report.key().side(),
                );
                let reference = prior.and_then(|prior| prior.values().status);
                let age = prior
                    .filter(|_| reference.is_some())
                    .and_then(|prior| now.millis().checked_sub(prior.modified_at().millis()));
                Some(crate::api::model::StatusAssessment::assess(
                    row.id(),
                    value,
                    reference,
                    age,
                    snapshot.effective_staleness(None).days(),
                ))
            })
            .collect()
    }
}

fn reference_price(
    snapshot: &ReferenceSnapshot,
    key: ReportKey,
    commodity: CommodityId,
) -> Option<PriceReference> {
    snapshot
        .prior_reference(key.terminal(), commodity, key.side())
        .map(PriceReference::terminal)
        .or_else(|| {
            snapshot
                .commodities("")
                .into_iter()
                .find(|item| item.id() == commodity)
                .and_then(|item| item.average_price(snapshot.environment(), key.side()))
                .map(PriceReference::average)
        })
}

impl ManualReviewer {
    pub(crate) fn environment(
        &self,
        channel: tallyline_game::api::model::GameEnvironment,
    ) -> tallyline_shared_kernel::api::model::UexEnvironment {
        self.mapping.environment(channel)
    }
}
