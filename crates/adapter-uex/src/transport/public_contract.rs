// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Fixed credential-free GET routes for the developer contract checker.

use crate::api::ReadError;
use crate::api::model::PublicContractEndpoint;

use super::{CappedResponse, Transport, transport_error};

impl Transport {
    pub(crate) async fn public_contract_response(
        &self,
        endpoint: PublicContractEndpoint,
    ) -> Result<CappedResponse, ReadError> {
        let mut url = self.request_url(endpoint.resource())?;
        if !endpoint.query().is_empty() {
            url.query_pairs_mut().extend_pairs(endpoint.query());
        }
        let response = self.client.get(url).send().await.map_err(transport_error)?;
        self.read_capped_response(response, &[]).await
    }
}
