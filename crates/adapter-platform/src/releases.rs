// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Credential-free bounded GitHub release metadata transport.

use rustls_platform_verifier::BuilderVerifierExt;
use std::{sync::Arc, time::Duration};
use tallyline_workflows::api::{
    UpdateCheck,
    model::{ReleaseVersion, UpdateFailure},
};

const RELEASE_PATH: &str = "/repos/tallyline-sc/tallyline/releases/latest";
const BODY_LIMIT: usize = 65_536;

/// A credential-free client restricted to the Tallyline release endpoint.
pub struct ReleaseClient {
    client: reqwest::Client,
    endpoint: String,
}

impl ReleaseClient {
    /// Builds the fixed HTTPS GitHub client with verified platform trust.
    ///
    /// # Errors
    /// Returns a closed failure when TLS or HTTP configuration fails.
    pub fn new(version: &str) -> Result<Self, UpdateFailure> {
        Self::build(format!("https://api.github.com{RELEASE_PATH}"), version)
    }

    /// Builds a loopback-only client for local HTTP contract servers.
    ///
    /// # Errors
    /// Returns a closed failure when client configuration fails.
    pub fn loopback(port: u16, version: &str) -> Result<Self, UpdateFailure> {
        Self::build(format!("http://127.0.0.1:{port}{RELEASE_PATH}"), version)
    }

    fn build(endpoint: String, version: &str) -> Result<Self, UpdateFailure> {
        let tls = rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::aws_lc_rs::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .and_then(BuilderVerifierExt::with_platform_verifier)
        .map_err(|_| UpdateFailure)?
        .with_no_client_auth();
        let client = reqwest::Client::builder()
            .tls_backend_preconfigured(tls)
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .user_agent(format!("Tallyline/{version}"))
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|_| UpdateFailure)?;
        Ok(Self { client, endpoint })
    }
}

#[derive(serde::Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
}

impl UpdateCheck for ReleaseClient {
    async fn latest(&self) -> Result<Option<ReleaseVersion>, UpdateFailure> {
        let mut response = self
            .client
            .get(&self.endpoint)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2026-03-10")
            .send()
            .await
            .map_err(|_| UpdateFailure)?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if response.status() != reqwest::StatusCode::OK {
            return Err(UpdateFailure);
        }
        if response
            .content_length()
            .is_some_and(|length| length > BODY_LIMIT as u64)
        {
            return Err(UpdateFailure);
        }
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| UpdateFailure)? {
            if body.len().saturating_add(chunk.len()) > BODY_LIMIT {
                return Err(UpdateFailure);
            }
            body.extend_from_slice(&chunk);
        }
        let release: Release = serde_json::from_slice(&body).map_err(|_| UpdateFailure)?;
        if release.draft || release.prerelease {
            return Err(UpdateFailure);
        }
        ReleaseVersion::parse(&release.tag_name)
            .map(Some)
            .ok_or(UpdateFailure)
    }
}
