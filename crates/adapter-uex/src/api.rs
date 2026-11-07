// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! UEX client settings, credentials, request outcomes and the read and dry-run operations.

pub mod model;

use std::fmt;
#[cfg(test)]
use std::time::Duration;
use tallyline_reference_data::api::model::Terminal;

use crate::transport::Transport;
use crate::wire;
use model::{
    DryRunReport, GameVersions, PriceRow, ReportAccess, StatusLevels, SubmitSuccess, TerminalId,
    UexAccount,
};

/// Warning that must precede explicit confirmation of a credentialled expert host.
pub const EXPERT_HOST_WARNING: &str =
    "Warning: this expert host receives your UEX secret key and app token.";

const DEFAULT_MAX_BODY_BYTES: usize = 16 * 1024 * 1024;
#[cfg(test)]
const LOOPBACK_HOSTS: [&str; 3] = ["127.0.0.1", "localhost", "[::1]"];

/// The client name and build version sent as `User-Agent` and `X-Client-Version` (R-API-7).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClientIdentity {
    name: String,
    version: String,
}

impl ClientIdentity {
    /// Creates the identity from the client's name and its build version.
    #[must_use]
    pub fn new(name: &str, version: &str) -> Self {
        Self {
            name: name.to_owned(),
            version: version.to_owned(),
        }
    }

    pub(crate) fn user_agent(&self) -> String {
        format!("{}/{}", self.name, self.version)
    }

    pub(crate) fn version(&self) -> &str {
        &self.version
    }
}

/// The settings of a [`UexReader`]: base URL, identity and response size cap.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReaderSettings {
    pub(crate) base_url: reqwest::Url,
    pub(crate) identity: ClientIdentity,
    pub(crate) max_body_bytes: usize,
    pub(crate) expert_host_confirmed: bool,
    #[cfg(test)]
    pub(crate) test_http: bool,
    #[cfg(test)]
    pub(crate) test_submission_timeout: Option<Duration>,
}

impl ReaderSettings {
    /// Creates settings with a 16 MiB response cap, 30 s read and 90 s submission timeouts.
    ///
    /// The base URL ends with a slash, uses `https` and has no user information, query or
    /// fragment (R-API-3). Every request must retain its configured HTTPS origin.
    ///
    /// # Errors
    ///
    /// [`SettingsError::InvalidBaseUrl`] when the URL has no authority, contains user information,
    /// a query or fragment, cannot be parsed or lacks a trailing slash;
    /// [`SettingsError::InsecureScheme`] for any other scheme.
    pub fn new(base_url: &str, identity: ClientIdentity) -> Result<Self, SettingsError> {
        let (_, address) = base_url
            .split_once("://")
            .ok_or(SettingsError::InvalidBaseUrl)?;
        if address
            .split(['/', '\\', '?', '#'])
            .next()
            .is_some_and(|authority| authority.contains('@'))
        {
            return Err(SettingsError::InvalidBaseUrl);
        }
        let parsed = reqwest::Url::parse(base_url).map_err(|_| SettingsError::InvalidBaseUrl)?;
        if !parsed.path().ends_with('/')
            || parsed.query().is_some()
            || parsed.fragment().is_some()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
        {
            return Err(SettingsError::InvalidBaseUrl);
        }
        if parsed.scheme() != "https" {
            return Err(SettingsError::InsecureScheme);
        }
        Ok(Self {
            base_url: parsed,
            identity,
            max_body_bytes: DEFAULT_MAX_BODY_BYTES,
            expert_host_confirmed: false,
            #[cfg(test)]
            test_http: false,
            #[cfg(test)]
            test_submission_timeout: None,
        })
    }

    #[cfg(test)]
    pub(crate) fn for_test_server(
        base_url: &str,
        identity: ClientIdentity,
    ) -> Result<Self, SettingsError> {
        let parsed = reqwest::Url::parse(base_url).map_err(|_| SettingsError::InvalidBaseUrl)?;
        if !test_loopback(&parsed) {
            return Err(SettingsError::InsecureScheme);
        }
        let mut secure = parsed.clone();
        secure
            .set_scheme("https")
            .map_err(|()| SettingsError::InvalidBaseUrl)?;
        let mut settings = Self::new(secure.as_str(), identity)?;
        settings.base_url = parsed;
        settings.test_http = true;
        Ok(settings)
    }

    #[cfg(test)]
    pub(crate) fn with_test_submission_timeout(mut self, timeout: Duration) -> Self {
        assert!(self.test_http);
        self.test_submission_timeout = Some(timeout);
        self
    }

    pub(crate) fn permits_url(&self, url: &reqwest::Url) -> bool {
        let secure = url.scheme() == "https";
        #[cfg(test)]
        let secure = secure || (self.test_http && test_loopback(url));
        secure
            && url.origin() == self.base_url.origin()
            && url.username().is_empty()
            && url.password().is_none()
            && url.fragment().is_none()
    }

    /// Permits credentialled requests to this configured expert origin.
    ///
    /// The caller must first display [`EXPERT_HOST_WARNING`] and obtain the user's explicit
    /// confirmation. This is not required for public reference GETs or the primary UEX origin.
    #[must_use]
    pub fn with_expert_host_confirmation(self) -> Self {
        Self {
            expert_host_confirmed: true,
            ..self
        }
    }

    pub(crate) fn credentials_allowed(&self) -> bool {
        self.expert_host_confirmed
            || (self.base_url.scheme() == "https"
                && self.base_url.host_str() == Some("api.uexcorp.uk")
                && self.base_url.port_or_known_default() == Some(443))
    }

    /// Returns these settings with another response body cap in bytes (R-API-5).
    ///
    /// # Errors
    ///
    /// [`SettingsError::ZeroBodyLimit`] when `limit` is zero.
    pub fn with_max_body_bytes(self, limit: usize) -> Result<Self, SettingsError> {
        if limit == 0 {
            return Err(SettingsError::ZeroBodyLimit);
        }
        Ok(Self {
            max_body_bytes: limit,
            ..self
        })
    }
}

#[cfg(test)]
fn test_loopback(url: &reqwest::Url) -> bool {
    url.scheme() == "http"
        && url
            .host_str()
            .is_some_and(|host| LOOPBACK_HOSTS.contains(&host))
}

/// Why [`ReaderSettings`] refused a value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsError {
    /// The base URL has invalid authority, user information, query, fragment or trailing slash.
    InvalidBaseUrl,
    /// The base URL does not use `https`.
    InsecureScheme,
    /// The response body cap is zero.
    ZeroBodyLimit,
}

/// The user's UEX secret key, sent as the `secret-key` header; masked in debug output.
#[derive(Clone, PartialEq, Eq)]
pub struct SecretKey(String);

impl SecretKey {
    /// Wraps the secret key.
    #[must_use]
    pub fn new(key: &str) -> Self {
        Self(key.to_owned())
    }

    pub(crate) fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SecretKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretKey(***)")
    }
}

/// The user's own UEX app token, sent as `Authorization: Bearer`; masked in debug output.
#[derive(Clone, PartialEq, Eq)]
pub struct AppToken(String);

impl AppToken {
    /// Wraps the app token.
    #[must_use]
    pub fn new(token: &str) -> Self {
        Self(token.to_owned())
    }

    pub(crate) fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for AppToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("AppToken(***)")
    }
}

/// Why building a [`UexReader`] failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuildError {
    /// The TLS configuration with the `aws-lc-rs` provider and the platform verifier failed.
    Tls(String),
    /// The HTTP client could not be built.
    Client(String),
}

/// Why a read call failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadError {
    /// UEX answered with an error envelope; `code` is its `status` string.
    Rejected {
        /// The HTTP status of the answer.
        http_status: u16,
        /// The UEX code from the envelope's `status`, such as `invalid_secret_key`.
        code: String,
        /// The envelope's `message`, empty when UEX sent none.
        message: String,
        /// Structured error details such as `data.row` or `data.report`.
        data: Option<RejectedData>,
    },
    /// The answer was not JSON, for example the HTML page of an unknown resource.
    NotJson {
        /// The HTTP status of the answer.
        http_status: u16,
        /// A bounded validated Cloudflare Ray identifier, without response body content.
        cf_ray: Option<String>,
    },
    /// UEX answered with a redirect, which is never followed (R-API-3).
    Redirect {
        /// The HTTP status of the answer.
        http_status: u16,
    },
    /// The answer was JSON but not of the documented shape or out of the documented range.
    Malformed {
        /// What did not match, without any response content.
        detail: String,
    },
    /// The response body exceeded the configured cap (R-API-5).
    BodyTooLarge {
        /// The cap in bytes.
        limit: usize,
    },
    /// The request failed before an answer was read: connection, TLS or timeout.
    Transport(String),
}

/// Sanitised structured fields returned with a rejected UEX request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RejectedData {
    /// The supplied Unix-second app-token quota reset.
    QuotaReset {
        /// The raw unsigned Unix-second reset.
        reset: u64,
        /// The independently parsed Date header of this response.
        observed_at: Option<tallyline_shared_kernel::api::model::Timestamp>,
    },
    /// Zero-based index of the `prices[]` row that failed validation.
    Row(usize),
    /// Report identity attached to `duplicated_report`.
    Report {
        /// UEX terminal ID.
        id_terminal: u64,
        /// UEX commodity ID.
        id_commodity: u64,
    },
    /// UEX returned structured data outside the shapes currently modelled here.
    Other,
}

/// The UEX API 2.0 reader: GET requests only, no redirects, a capped body size.
#[derive(Debug)]
pub struct UexReader {
    pub(crate) transport: Transport,
    pub(crate) quota_observations:
        std::sync::Mutex<tallyline_submission::api::model::TokenQuotaObservations>,
    pub(crate) read_gate: Option<AuthenticatedReadGate>,
}

/// The UEX submission client. Its only write operation is a `data_submit` dry run.
#[derive(Debug)]
pub struct UexSubmitter {
    transport: Transport,
}

impl UexSubmitter {
    /// Builds the submitter with the same TLS and response safeguards as [`UexReader`].
    ///
    /// # Errors
    ///
    /// [`BuildError::Tls`] or [`BuildError::Client`] when the TLS stack or HTTP client fails.
    pub fn new(settings: &ReaderSettings) -> Result<Self, BuildError> {
        Ok(Self {
            transport: Transport::new(settings)?,
        })
    }

    /// Submits a report with `is_production=0` and parses the success response.
    ///
    /// # Errors
    ///
    /// Returns a [`ReadError`] for an HTTP, UEX-envelope, or response-shape failure.
    pub async fn dry_run(
        &self,
        token: &AppToken,
        key: &SecretKey,
        report: &DryRunReport,
    ) -> Result<SubmitSuccess, ReadError> {
        let bearer = format!("Bearer {}", token.expose());
        let data = self
            .transport
            .post(
                "data_submit/",
                report,
                &[("authorization", &bearer), ("secret-key", key.expose())],
            )
            .await?;
        serde_json::from_value(data).map_err(|error| ReadError::Malformed {
            detail: format!("data_submit success data: {error}"),
        })
    }
}

impl UexReader {
    /// Builds a shared client whose response parsing uses the application's bounded CPU worker.
    ///
    /// # Errors
    /// Returns the same TLS or client initialisation failure as the standalone constructor.
    pub fn new_with_processor(
        settings: &ReaderSettings,
        processor: ResponseProcessor,
    ) -> Result<Self, BuildError> {
        Ok(Self {
            transport: Transport::new(settings)?.with_processor(processor),
            quota_observations: Default::default(),
            read_gate: None,
        })
    }
    /// Adds shared response-date tracking without replacing the injected processing boundary.
    #[must_use]
    pub fn with_server_time(mut self, tracker: model::ServerTimeTracker) -> Self {
        self.transport = self.transport.with_server_time(tracker);
        self
    }
    /// Builds the shared client with an owned injected-clock response-date tracker.
    ///
    /// # Errors
    /// Returns the same TLS or client initialisation failure as the ordinary constructor.
    pub fn new_with_server_time(
        settings: &ReaderSettings,
        tracker: model::ServerTimeTracker,
    ) -> Result<Self, BuildError> {
        Ok(Self {
            transport: Transport::new(settings)?.with_server_time(tracker),
            quota_observations: Default::default(),
            read_gate: None,
        })
    }
    /// Builds the client with the `aws-lc-rs` crypto provider and the platform verifier.
    ///
    /// # Errors
    ///
    /// [`BuildError::Tls`] or [`BuildError::Client`] when the TLS stack or the client fails.
    pub fn new(settings: &ReaderSettings) -> Result<Self, BuildError> {
        let transport = Transport::new(settings)?;
        Ok(Self {
            transport,
            quota_observations: Default::default(),
            read_gate: None,
        })
    }

    /// Reads selectable commodity terminals without credentials or LIVE filtering.
    ///
    /// # Errors
    /// Returns a transport, envelope or malformed-reference failure.
    pub async fn terminals(&self) -> Result<Vec<Terminal>, ReadError> {
        let data = self
            .transport
            .get("terminals/", &[("type", "commodity")], &[])
            .await?;
        self.transport
            .process(move || crate::reference_wire::terminals(data))
            .await
    }
    /// Reads `GET /game_versions`: the current LIVE and PTU version.
    ///
    /// # Errors
    ///
    /// A [`ReadError`] for every answer other than a well-formed `ok` envelope.
    pub async fn game_versions(&self) -> Result<GameVersions, ReadError> {
        let data = self.transport.get("game_versions/", &[], &[]).await?;
        self.transport
            .process(move || wire::game_versions(data))
            .await
    }

    /// Reads `GET /commodities_status`: the status levels of each side.
    ///
    /// # Errors
    ///
    /// A [`ReadError`] for every answer other than a well-formed `ok` envelope.
    pub async fn commodity_status_levels(&self) -> Result<StatusLevels, ReadError> {
        let data = self.transport.get("commodities_status/", &[], &[]).await?;
        self.transport
            .process(move || wire::status_levels(data))
            .await
    }

    /// Reads `GET /commodities_prices?id_terminal=…`: the price rows of one terminal.
    ///
    /// # Errors
    ///
    /// A [`ReadError`] for every answer other than a well-formed `ok` envelope.
    pub async fn price_rows(&self, terminal: TerminalId) -> Result<Vec<PriceRow>, ReadError> {
        let terminal = terminal.0.to_string();
        let data = self
            .transport
            .get("commodities_prices/", &[("id_terminal", &terminal)], &[])
            .await?;
        self.transport.process(move || wire::price_rows(data)).await
    }

    /// Reads `GET /user` with the secret key alone and keeps only the account state.
    ///
    /// The e-mail address and Discord name in the answer are dropped here (O-97).
    ///
    /// # Errors
    ///
    /// A [`ReadError`]; an invalid key is `Rejected` with the code `invalid_secret_key`.
    pub async fn account(&self, key: &SecretKey) -> Result<UexAccount, ReadError> {
        let data = self
            .transport
            .get("user/", &[], &[("secret-key", key.expose())])
            .await?;
        self.transport.process(move || wire::account(data)).await
    }

    /// Reads `GET /data_info` for `username` with `limit=1` to check the app token and the key.
    ///
    /// # Errors
    ///
    /// A [`ReadError`]; a refused token is `Rejected` with `not_allowed` or `access_denied`.
    pub async fn report_access(
        &self,
        token: &AppToken,
        key: &SecretKey,
        username: &str,
    ) -> Result<ReportAccess, ReadError> {
        let bearer = format!("Bearer {}", token.expose());
        let data = self
            .transport
            .get(
                "data_info/",
                &[("username", username), ("limit", "1")],
                &[("authorization", &bearer), ("secret-key", key.expose())],
            )
            .await?;
        self.transport
            .process(move || wire::report_access(&data))
            .await
    }
}

impl ReadError {
    /// Maps only the documented actual submission answer to account screenshot evidence.
    /// GET user data and all other failures establish no screenshot policy.
    #[must_use]
    pub fn screenshot_requirement(
        &self,
    ) -> Option<tallyline_submission::api::model::ScreenshotRequirement> {
        match self {
            Self::Rejected { code, .. } if code == "screenshot_required" => {
                Some(tallyline_submission::api::model::ScreenshotRequirement::Required)
            }
            Self::Rejected { .. }
            | Self::NotJson { .. }
            | Self::Redirect { .. }
            | Self::Malformed { .. }
            | Self::BodyTooLarge { .. }
            | Self::Transport(_) => None,
        }
    }
}

pub use crate::response_processing::{
    ProcessingUnavailable, ResponseExecutor, ResponseProcessor, ResponseTask,
};

pub use crate::authenticated_read_gate::AuthenticatedReadGate;
