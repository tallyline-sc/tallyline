// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Fixed credential-free local version reads with explicit total bounds.

use std::num::{NonZeroU16, NonZeroUsize};
use std::sync::Arc;
use std::time::Duration;

use reqwest::{Client, redirect, retry};
use rustls::ClientConfig;
use rustls::crypto::aws_lc_rs;
use serde::Deserialize;
use tallyline_recognition::api::MetadataOwnershipSession;
use tallyline_recognition::api::model::{
    MetadataGeneration, MetadataRefusal, MetadataSessionState,
};
use tokio::time::{self, Instant};

use crate::metadata_connector::{self, MetadataConnector};

/// Explicit total operation time and cumulative response-body limit.
#[derive(Clone, Copy, Debug)]
pub struct MetadataBounds {
    timeout: Duration,
    response_bytes: NonZeroUsize,
}

impl MetadataBounds {
    /// Validates explicit metadata bounds without choosing product defaults.
    ///
    /// # Errors
    /// Returns Configuration if the total duration is zero or cannot form a deadline.
    pub fn new(timeout: Duration, response_bytes: NonZeroUsize) -> Result<Self, MetadataRefusal> {
        if timeout.is_zero() || Instant::now().checked_add(timeout).is_none() {
            return Err(MetadataRefusal::Configuration);
        }
        Ok(Self {
            timeout,
            response_bytes,
        })
    }
}

/// Bounded version metadata bound to the observed owned generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VersionMetadata {
    version: String,
    generation: MetadataGeneration,
}

impl VersionMetadata {
    /// Returns the bounded version string without granting runtime admission.
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Returns the generation whose connected ownership was observed.
    #[must_use]
    pub const fn generation(&self) -> MetadataGeneration {
        self.generation
    }
}

/// Reads only GET /api/version from one explicitly supplied loopback session.
pub struct LocalMetadataClient<T> {
    port: NonZeroU16,
    session: T,
    generation: MetadataGeneration,
    bounds: MetadataBounds,
}

impl<T: MetadataOwnershipSession> LocalMetadataClient<T> {
    /// Binds a credential-free client to a current owned generation and literal 127.0.0.1.
    ///
    /// # Errors
    /// Returns the session refusal when no current generation exists.
    pub fn loopback(
        port: NonZeroU16,
        session: T,
        bounds: MetadataBounds,
    ) -> Result<Self, MetadataRefusal> {
        let generation = match session.state() {
            MetadataSessionState::Current(generation) => generation,
            MetadataSessionState::Revoked(reason) => return Err(reason),
        };
        Ok(Self {
            port,
            session,
            generation,
            bounds,
        })
    }

    /// Reads bounded version metadata through an observed actual connection.
    ///
    /// # Errors
    /// Returns a closed ownership, transport, deadline or metadata refusal.
    pub async fn version(&self) -> Result<VersionMetadata, MetadataRefusal> {
        let deadline = Instant::now()
            .checked_add(self.bounds.timeout)
            .ok_or(MetadataRefusal::Configuration)?;
        metadata_connector::current_generation(&self.session, self.generation)?;
        metadata_connector::within_deadline(deadline)?;
        tokio::select! {
            biased;
            reason = self.session.revoked(self.generation) => Err(reason),
            result = time::timeout_at(deadline, self.read_version(deadline)) => result.map_err(|_| MetadataRefusal::Deadline)?,
        }
    }

    async fn read_version(&self, deadline: Instant) -> Result<VersionMetadata, MetadataRefusal> {
        let client = Client::builder()
            .tls_backend_preconfigured(tls_config()?)
            .no_proxy()
            .redirect(redirect::Policy::none())
            .retry(retry::never())
            .http1_only()
            .pool_max_idle_per_host(0)
            .connector_layer(MetadataConnector::new(
                self.session.clone(),
                self.generation,
                self.port,
                deadline,
            ))
            .build()
            .map_err(|_| MetadataRefusal::Configuration)?;
        metadata_connector::within_deadline(deadline)?;
        let mut response = client
            .get(format!("http://127.0.0.1:{}/api/version", self.port))
            .send()
            .await
            .map_err(|error| metadata_connector::transport_refusal(&error))?;
        if !response.status().is_success() {
            return Err(MetadataRefusal::InvalidMetadata);
        }
        if response
            .content_length()
            .is_some_and(|length| length > self.bounds.response_bytes.get() as u64)
        {
            return Err(MetadataRefusal::ResponseTooLarge);
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|error| metadata_connector::transport_refusal(&error))?
        {
            if chunk.len() > self.bounds.response_bytes.get().saturating_sub(bytes.len()) {
                return Err(MetadataRefusal::ResponseTooLarge);
            }
            bytes.extend_from_slice(&chunk);
        }
        if bytes.iter().find(|byte| !byte.is_ascii_whitespace()) != Some(&b'{') {
            return Err(MetadataRefusal::InvalidMetadata);
        }
        let metadata: VersionResponse =
            serde_json::from_slice(&bytes).map_err(|_| MetadataRefusal::InvalidMetadata)?;
        if metadata.version.trim().is_empty() || metadata.version.chars().any(char::is_control) {
            return Err(MetadataRefusal::InvalidMetadata);
        }
        metadata_connector::current_generation(&self.session, self.generation)?;
        metadata_connector::within_deadline(deadline)?;
        Ok(VersionMetadata {
            version: metadata.version,
            generation: self.generation,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VersionResponse {
    version: String,
}

fn tls_config() -> Result<ClientConfig, MetadataRefusal> {
    use rustls_platform_verifier::BuilderVerifierExt;

    let provider = Arc::new(aws_lc_rs::default_provider());
    ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .and_then(BuilderVerifierExt::with_platform_verifier)
        .map_err(|_| MetadataRefusal::Configuration)
        .map(|builder| builder.with_no_client_auth())
}
