// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Authenticated read quota translation without request or credential persistence.

use crate::api::{ReadError, RejectedData, UexReader};
use tallyline_shared_kernel::api::model::Timestamp;
use tallyline_submission::api::model::{TokenQuotaObservation, TokenQuotaReset, TokenScope};
impl UexReader {
    /// Transfers bounded authenticated-read quota evidence to the application persistence owner.
    pub fn take_quota_observations(&self) -> Vec<TokenQuotaObservation> {
        match self.quota_observations.lock() {
            Ok(mut observations) => observations.take(),
            Err(_) => vec![TokenQuotaObservation::new(None, None)],
        }
    }
    pub(crate) fn observe_read_quota<T>(
        &self,
        result: &Result<T, ReadError>,
        scope: Option<TokenScope>,
    ) -> Result<(), ReadError> {
        let Err(ReadError::Rejected { code, data, .. }) = result else {
            return Ok(());
        };
        if code != "exceeded_daily_quota" {
            return Ok(());
        }
        let reset = match data {
            Some(RejectedData::QuotaReset { reset, observed_at }) => Some(TokenQuotaReset::new(
                Timestamp::from_seconds(*reset),
                *observed_at,
            )),
            _ => None,
        };
        self.quota_observations
            .lock()
            .map_err(|_| {
                ReadError::Transport("authenticated quota observations are unavailable".to_owned())
            })?
            .record(TokenQuotaObservation::new(scope, reset));
        Ok(())
    }
}
