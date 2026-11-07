// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! The HTTP transport: one `reqwest` client with the explicit TLS stack and capped response bodies.

mod public_contract;

use std::sync::Arc;
use std::time::Duration;

use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue, USER_AGENT};
use rustls::ClientConfig;
use rustls::crypto::aws_lc_rs;
use rustls_platform_verifier::BuilderVerifierExt;
use serde::Serialize;
use serde_json::Value;
use tallyline_submission::api::model::SubmissionRetrySettings;

use crate::api::{BuildError, ReadError, ReaderSettings};
use crate::envelope;

const CLIENT_VERSION_HEADER: &str = "x-client-version";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// The shared HTTP client of the adapter and the base URL its requests are built against.
#[derive(Debug)]
pub(crate) struct Transport {
    client: reqwest::Client,
    settings: ReaderSettings,
    server_time: Option<crate::server_time::ServerTimeTracker>,
    processor: Option<crate::api::ResponseProcessor>,
}

pub(crate) struct CappedResponse {
    pub(crate) http_status: u16,
    pub(crate) cf_ray: Option<String>,
    pub(crate) body: Vec<u8>,
    pub(crate) server_time: Option<tallyline_shared_kernel::api::model::Timestamp>,
}

impl Transport {
    /// Builds the client: `aws-lc-rs` provider, platform verifier, no redirects, fixed headers.
    pub(crate) fn new(settings: &ReaderSettings) -> Result<Self, BuildError> {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, header_value(&settings.identity.user_agent())?);
        headers.insert(
            HeaderName::from_static(CLIENT_VERSION_HEADER),
            header_value(settings.identity.version())?,
        );
        let client = reqwest::Client::builder()
            .tls_backend_preconfigured(tls_config()?)
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .default_headers(headers)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(client_error)?;
        Ok(Self {
            client,
            settings: settings.clone(),
            server_time: None,
            processor: None,
        })
    }

    pub(crate) fn with_processor(mut self, processor: crate::api::ResponseProcessor) -> Self {
        self.processor = Some(processor);
        self
    }
    pub(crate) async fn process<T: Send + 'static>(
        &self,
        work: impl FnOnce() -> Result<T, ReadError> + Send + 'static,
    ) -> Result<T, ReadError> {
        match &self.processor {
            Some(processor) => processor.process(work).await.map_err(|_| {
                ReadError::Transport("response processing is unavailable".to_owned())
            })?,
            None => work(),
        }
    }
    pub(crate) fn with_server_time(
        mut self,
        tracker: crate::server_time::ServerTimeTracker,
    ) -> Self {
        self.server_time = Some(tracker);
        self
    }
    pub(crate) fn clamp(
        &self,
        observed: tallyline_shared_kernel::api::model::Timestamp,
    ) -> tallyline_shared_kernel::api::model::Timestamp {
        self.server_time
            .as_ref()
            .map_or(observed, |tracker| tracker.clamp(observed))
    }
    /// Sends `GET <base><resource>` and returns the `data` of an `ok` envelope.
    ///
    /// The values of `headers` are marked sensitive.
    pub(crate) async fn get(
        &self,
        resource: &str,
        query: &[(&str, &str)],
        headers: &[(&str, &str)],
    ) -> Result<Value, ReadError> {
        let mut url = self.request_url(resource)?;
        if !query.is_empty() {
            url.query_pairs_mut().extend_pairs(query);
        }
        self.check_credentials(headers)?;
        let mut request = self.client.get(url);
        for (name, value) in headers {
            let name = HeaderName::from_bytes(name.as_bytes())
                .map_err(|error| ReadError::Transport(error.to_string()))?;
            let mut value = HeaderValue::from_str(value)
                .map_err(|_| ReadError::Transport(format!("header {name} has an invalid value")))?;
            value.set_sensitive(true);
            request = request.header(name, value);
        }
        let response = request.send().await.map_err(transport_error)?;
        self.read_response(response, headers).await
    }

    /// Sends a JSON `POST` and returns the `data` of its response envelope.
    pub(crate) async fn post<T: Serialize>(
        &self,
        resource: &str,
        payload: &T,
        headers: &[(&str, &str)],
    ) -> Result<Value, ReadError> {
        self.post_with_timeout(resource, payload, headers, REQUEST_TIMEOUT)
            .await
    }

    pub(crate) async fn post_submission(
        &self,
        resource: &str,
        body: Vec<u8>,
        headers: &[(&str, &str)],
    ) -> Result<Value, SubmissionPostFailure> {
        let timeout = SubmissionRetrySettings::default().send_timeout();
        #[cfg(test)]
        let timeout = self.settings.test_submission_timeout.unwrap_or(timeout);
        let request = self
            .post_bytes(resource, body, headers, timeout)
            .map_err(SubmissionPostFailure::Other)?;
        let response = request.send().await.map_err(submission_transport_error)?;
        let http_status = response.status().as_u16();
        self.read_response(response, headers)
            .await
            .map_err(|error| {
                if matches!(
                    error,
                    ReadError::NotJson { .. } | ReadError::Malformed { .. }
                ) {
                    if matches!(http_status, 401 | 403) {
                        return SubmissionPostFailure::Authentication(http_status);
                    }
                    if http_status == 429 {
                        return SubmissionPostFailure::RateLimited;
                    }
                }
                SubmissionPostFailure::Other(error)
            })
    }
    async fn post_with_timeout<T: Serialize>(
        &self,
        resource: &str,
        payload: &T,
        headers: &[(&str, &str)],
        timeout: Duration,
    ) -> Result<Value, ReadError> {
        let response = self
            .post_request(resource, payload, headers, timeout)?
            .send()
            .await
            .map_err(transport_error)?;
        self.read_response(response, headers).await
    }

    fn post_request<T: Serialize>(
        &self,
        resource: &str,
        payload: &T,
        headers: &[(&str, &str)],
        timeout: Duration,
    ) -> Result<reqwest::RequestBuilder, ReadError> {
        let body = serde_json::to_vec(payload).map_err(|error| ReadError::Malformed {
            detail: format!("could not serialise data_submit payload: {error}"),
        })?;
        self.post_bytes(resource, body, headers, timeout)
    }

    fn post_bytes(
        &self,
        resource: &str,
        body: Vec<u8>,
        headers: &[(&str, &str)],
        timeout: Duration,
    ) -> Result<reqwest::RequestBuilder, ReadError> {
        let url = self.request_url(resource)?;
        self.check_credentials(headers)?;
        let mut request = self
            .client
            .post(url)
            .timeout(timeout)
            .header(CONTENT_TYPE, "application/json")
            .body(body);
        for (name, value) in headers {
            let name = HeaderName::from_bytes(name.as_bytes())
                .map_err(|error| ReadError::Transport(error.to_string()))?;
            let mut value = HeaderValue::from_str(value)
                .map_err(|_| ReadError::Transport(format!("header {name} has an invalid value")))?;
            value.set_sensitive(true);
            request = request.header(name, value);
        }
        Ok(request)
    }
    fn check_credentials(&self, headers: &[(&str, &str)]) -> Result<(), ReadError> {
        let sensitive = headers.iter().any(|(name, _)| {
            name.eq_ignore_ascii_case("authorization") || name.eq_ignore_ascii_case("secret-key")
        });
        if sensitive && !self.settings.credentials_allowed() {
            return Err(ReadError::Transport(
                "credentialled expert host requires explicit secret-key disclosure confirmation"
                    .to_owned(),
            ));
        }
        Ok(())
    }

    fn request_url(&self, resource: &str) -> Result<reqwest::Url, ReadError> {
        let url = self
            .settings
            .base_url
            .join(resource)
            .map_err(|_| ReadError::Transport("invalid resource path".to_owned()))?;
        if !self.settings.permits_url(&url) {
            return Err(ReadError::Transport(
                "request URL violates the configured HTTPS origin".to_owned(),
            ));
        }
        Ok(url)
    }

    async fn read_response(
        &self,
        response: reqwest::Response,
        credentials: &[(&str, &str)],
    ) -> Result<Value, ReadError> {
        let response = self.read_capped_response(response, credentials).await?;
        self.process(move || {
            envelope::data(response.http_status, &response.body).map_err(|error| match error {
                ReadError::NotJson { http_status, .. } => ReadError::NotJson {
                    http_status,
                    cf_ray: response.cf_ray,
                },
                ReadError::Rejected {
                    http_status,
                    code,
                    message,
                    data,
                } => {
                    let data = data.map(|data| match data {
                        crate::api::RejectedData::QuotaReset { reset, .. } => {
                            crate::api::RejectedData::QuotaReset {
                                reset,
                                observed_at: response.server_time,
                            }
                        }
                        other @ (crate::api::RejectedData::Row(_)
                        | crate::api::RejectedData::Report { .. }
                        | crate::api::RejectedData::Other) => other,
                    });
                    ReadError::Rejected {
                        http_status,
                        code,
                        message,
                        data,
                    }
                }
                other @ (ReadError::Redirect { .. }
                | ReadError::Malformed { .. }
                | ReadError::BodyTooLarge { .. }
                | ReadError::Transport(_)) => other,
            })
        })
        .await
    }

    async fn read_capped_response(
        &self,
        mut response: reqwest::Response,
        credentials: &[(&str, &str)],
    ) -> Result<CappedResponse, ReadError> {
        let server_time = if let Some(tracker) = &self.server_time {
            let date = if response.headers().get_all("date").iter().count() == 1 {
                response
                    .headers()
                    .get("date")
                    .and_then(|value| value.to_str().ok())
            } else {
                None
            };
            tracker
                .observe(date)
                .map_err(|_| ReadError::Transport("server-time cache is unavailable".to_owned()))?
        } else {
            None
        };
        let http_status = response.status().as_u16();
        if response.status().is_redirection() {
            return Err(ReadError::Redirect { http_status });
        }
        let cf_ray = response
            .headers()
            .get("cf-ray")
            .and_then(|value| value.to_str().ok())
            .filter(|value| valid_ray(value, credentials))
            .map(str::to_owned);
        let limit = self.settings.max_body_bytes;
        if response
            .content_length()
            .is_some_and(|length| usize::try_from(length).map_or(true, |length| length > limit))
        {
            return Err(ReadError::BodyTooLarge { limit });
        }
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(transport_error)? {
            if body.len().saturating_add(chunk.len()) > limit {
                return Err(ReadError::BodyTooLarge { limit });
            }
            body.extend_from_slice(&chunk);
        }
        Ok(CappedResponse {
            http_status,
            cf_ray,
            body,
            server_time,
        })
    }
}

fn header_value(value: &str) -> Result<HeaderValue, BuildError> {
    HeaderValue::from_str(value).map_err(client_error)
}

fn client_error(error: impl ToString) -> BuildError {
    BuildError::Client(error.to_string())
}

fn transport_error(error: reqwest::Error) -> ReadError {
    ReadError::Transport(error.without_url().to_string())
}

fn tls_config() -> Result<ClientConfig, BuildError> {
    let provider = Arc::new(aws_lc_rs::default_provider());
    let config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .and_then(BuilderVerifierExt::with_platform_verifier)
        .map_err(|error| BuildError::Tls(error.to_string()))?
        .with_no_client_auth();
    Ok(config)
}

fn valid_ray(value: &str, credentials: &[(&str, &str)]) -> bool {
    if value.is_empty() || value.len() > 64 {
        return false;
    }
    let identifier = match value.split_once('-') {
        Some((identifier, location))
            if location.len() == 3 && location.bytes().all(|byte| byte.is_ascii_alphabetic()) =>
        {
            identifier
        }
        Some(_) => return false,
        None => value,
    };
    !identifier.is_empty()
        && identifier.bytes().all(|byte| byte.is_ascii_hexdigit())
        && !credentials.iter().any(|(_, secret)| {
            let secret = secret.strip_prefix("Bearer ").unwrap_or(secret);
            !secret.is_empty()
                && value
                    .as_bytes()
                    .windows(secret.len())
                    .any(|fragment| fragment.eq_ignore_ascii_case(secret.as_bytes()))
        })
}

pub(crate) enum SubmissionPostFailure {
    Authentication(u16),
    RateLimited,
    Connect,
    Other(ReadError),
}

fn submission_transport_error(error: reqwest::Error) -> SubmissionPostFailure {
    if error.is_connect() {
        if error.is_timeout() {
            return SubmissionPostFailure::Connect;
        }
        let mut source = std::error::Error::source(&error);
        while let Some(cause) = source {
            if cause
                .downcast_ref::<std::io::Error>()
                .is_some_and(|cause| cause.kind() == std::io::ErrorKind::ConnectionRefused)
            {
                return SubmissionPostFailure::Connect;
            }
            source = cause.source();
        }
    }
    SubmissionPostFailure::Other(transport_error(error))
}
