// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact released snapshot translation through the shared UEX transport.

use crate::api::{ReadError, UexReader};
use crate::transport::SubmissionPostFailure;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use tallyline_shared_kernel::api::model::{
    CommodityId, SubmissionMode, TerminalId, Timestamp, TradeSide,
};
use tallyline_submission::api::model::{
    CredentialValues, NotProcessed, PayloadHash, PreparationFailure, SubmissionRequest,
    SubmissionResult, SubmissionReturnReason, VerifiedUexReport,
};
use tallyline_submission::api::{SubmissionGateway, SubmissionReceiptVerifier};

/// An immutable encoded request retained only until one asynchronous dispatch.
pub struct PreparedUexSubmission {
    body: Vec<u8>,
    request: SubmissionRequest,
    date_added: Timestamp,
}

impl SubmissionGateway for UexReader {
    type Prepared = PreparedUexSubmission;

    fn prepare(
        &self,
        request: &SubmissionRequest,
    ) -> Result<(Self::Prepared, PayloadHash), PreparationFailure> {
        PreparedUexSubmission::new_at(request, self.transport.clamp(request.header().observed_at))
    }
    async fn submit(
        &self,
        prepared: Self::Prepared,
        credentials: &CredentialValues,
        release_token: Option<&str>,
    ) -> SubmissionResult {
        if !self.token_request_allowed(credentials) {
            return SubmissionResult::Returned(SubmissionReturnReason::PreflightChanged);
        }
        let Some(token) = credentials.own_token().or(release_token) else {
            return SubmissionResult::Unknown;
        };
        let PreparedUexSubmission {
            body,
            request,
            date_added,
        } = prepared;
        let bearer = format!("Bearer {token}");
        let response = self
            .transport
            .post_submission(
                "data_submit/",
                body,
                &[
                    ("secret-key", credentials.secret_key()),
                    ("Authorization", &bearer),
                ],
            )
            .await;
        if let Err(SubmissionPostFailure::Authentication(http_status)) = &response {
            return self
                .submission_refusal(
                    crate::submission_refusal::RejectedSubmission {
                        http_status: *http_status,
                        code: String::new(),
                        data: None,
                    },
                    crate::submission_refusal::RejectionContext {
                        request: &request,
                        date_added,
                        credentials,
                        token,
                    },
                )
                .await;
        }
        if let Err(SubmissionPostFailure::Other(ReadError::Rejected {
            http_status,
            code,
            data,
            ..
        })) = response
        {
            return self
                .submission_refusal(
                    crate::submission_refusal::RejectedSubmission {
                        http_status,
                        code,
                        data,
                    },
                    crate::submission_refusal::RejectionContext {
                        request: &request,
                        date_added,
                        credentials,
                        token,
                    },
                )
                .await;
        }
        self.transport
            .process(move || {
                Ok(match response {
                    Ok(value) if value.is_object() && value.get("date_added").is_some() => {
                        match request.header().mode {
                            SubmissionMode::DryRun => {
                                if dry_run_success(&value, &request, date_added) {
                                    SubmissionResult::DryRunSucceeded
                                } else {
                                    SubmissionResult::Unknown
                                }
                            }
                            SubmissionMode::Production => {
                                accepted_receipts(&value, &request, date_added)
                            }
                        }
                    }
                    Err(SubmissionPostFailure::RateLimited) => {
                        SubmissionResult::NotProcessed(NotProcessed::RateLimited)
                    }
                    Err(SubmissionPostFailure::Authentication(_)) => SubmissionResult::Unknown,
                    Err(SubmissionPostFailure::Connect) => {
                        SubmissionResult::NotProcessed(NotProcessed::ConnectFailure)
                    }
                    Err(SubmissionPostFailure::Other(ReadError::NotJson {
                        http_status: 404,
                        ..
                    })) => SubmissionResult::Held(
                        tallyline_submission::api::model::SubmissionHoldReason::ApiChanged,
                    ),
                    Ok(_) | Err(SubmissionPostFailure::Other(_)) => SubmissionResult::Unknown,
                })
            })
            .await
            .unwrap_or(SubmissionResult::Unknown)
    }
}

fn payload(
    request: &SubmissionRequest,
    date_added: Timestamp,
    screenshot: Option<&str>,
) -> Option<Value> {
    let header = request.header();
    let mut prices = Vec::with_capacity(request.rows().len());
    for row in request.rows() {
        let mut values = Map::new();
        values.insert("id_commodity".to_owned(), json!(row.commodity().0));
        let (price, stock, status) = match header.side {
            TradeSide::Buy => ("price_buy", "scu_buy", "status_buy"),
            TradeSide::Sell => ("price_sell", "scu_sell", "status_sell"),
        };
        values.insert(
            price.to_owned(),
            Value::Number(row.price().to_string().parse().ok()?),
        );
        values.insert(status.to_owned(), json!(row.status()));
        if !row.stock().is_zero() {
            values.insert(stock.to_owned(), json!(row.stock().whole_units()?));
        }
        prices.push(Value::Object(values));
    }
    let mut payload = json!({"id_terminal": header.terminal.0, "type":"commodity", "is_production":match header.mode {SubmissionMode::DryRun=>0, SubmissionMode::Production=>1}, "game_version":header.game_version, "date_added":date_added.seconds(), "prices":prices});
    if let Some(affinity) = header.faction_affinity {
        payload
            .as_object_mut()?
            .insert("faction_affinity".to_owned(), json!(affinity.value()));
    }
    if let Some(image) = screenshot {
        payload
            .as_object_mut()?
            .insert("screenshot".to_owned(), Value::String(image.to_owned()));
    }
    Some(payload)
}

fn accepted_receipts(
    answer: &Value,
    request: &SubmissionRequest,
    date_added: Timestamp,
) -> SubmissionResult {
    let partial = partial_ids(answer);
    let Some(ids) = id_values(answer) else {
        return SubmissionResult::Partial(partial);
    };
    let mut seen = std::collections::BTreeSet::new();
    if ids.len() != request.rows().len()
        || answer.get("date_added").and_then(number) != Some(date_added.seconds())
        || !ids
            .iter()
            .all(|id| number(id).is_some_and(|id| id > 0 && seen.insert(id)))
    {
        return SubmissionResult::Partial(partial);
    }
    SubmissionResult::ProductionAccepted(partial)
}

impl SubmissionReceiptVerifier for UexReader {
    async fn verify_receipts(
        &self,
        job: &tallyline_submission::api::model::SubmissionJob,
        credentials: &CredentialValues,
        release_token: Option<&str>,
    ) -> Option<Vec<VerifiedUexReport>> {
        let request = job.request();
        let SubmissionResult::ProductionAccepted(received) = job.result()? else {
            return None;
        };
        if job.phase() != tallyline_submission::api::model::JobPhase::Succeeded {
            return None;
        }
        let date_added = job.attempts().last()?.date_added();
        if received.len() != request.rows().len()
            || received.iter().any(|id| id.date_added() != date_added)
        {
            return None;
        }
        let token = credentials.own_token().or(release_token)?;
        let verification = async {
            let mut receipts = Vec::with_capacity(received.len());
            let mut rows = std::collections::BTreeSet::new();
            for id in received {
                let window = self
                    .read_receipt_window(request, date_added, credentials, token, Some(id.id()))
                    .await
                    .ok()?;
                let [proof] = window.receipts.as_slice() else {
                    return None;
                };
                if !rows.insert(proof.row().0) {
                    return None;
                }
                receipts.push(proof.clone());
            }
            Some(receipts)
        };
        tokio::time::timeout(crate::receipt_window::TIMEOUT, verification)
            .await
            .ok()
            .flatten()
    }
}
pub(super) fn number(value: &Value) -> Option<u64> {
    value.as_u64().or_else(|| {
        let text = value.as_str()?;
        if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        text.parse().ok()
    })
}
pub(super) fn lookup(
    record: &Value,
    id: u64,
    request: &SubmissionRequest,
    date_added: Timestamp,
) -> Option<VerifiedUexReport> {
    if number(record.get("id")?)? != id || record.get("type")?.as_str()? != "commodity" {
        return None;
    }
    VerifiedUexReport::from_lookup_at(
        id,
        TerminalId(u32::try_from(number(record.get("id_terminal")?)?).ok()?),
        CommodityId(u32::try_from(number(record.get("id_commodity")?)?).ok()?),
        Timestamp::from_seconds(number(record.get("date_added")?)?),
        request,
        date_added,
    )
    .ok()
}
fn partial_ids(answer: &Value) -> Vec<tallyline_shared_kernel::api::model::UexReportId> {
    let Some(date) = answer.get("date_added").and_then(number) else {
        return Vec::new();
    };
    let Some(ids) = id_values(answer) else {
        return Vec::new();
    };
    ids.iter()
        .filter_map(|id| {
            tallyline_shared_kernel::api::model::UexReportId::new(
                number(id)?,
                Timestamp::from_seconds(date),
            )
        })
        .collect()
}

impl PreparedUexSubmission {
    /// Serialises and hashes the original observation time before asynchronous dispatch.
    ///
    /// # Errors
    /// Refuses unsupported request values without creating a network operation.
    pub fn new(request: &SubmissionRequest) -> Result<(Self, PayloadHash), PreparationFailure> {
        Self::new_at(request, request.header().observed_at)
    }
    /// Serialises an earlier-only wire time without changing the released observation.
    ///
    /// # Errors
    /// Refuses a later wire time or unsupported request values.
    pub fn new_at(
        request: &SubmissionRequest,
        date_added: Timestamp,
    ) -> Result<(Self, PayloadHash), PreparationFailure> {
        Self::new_with_screenshot(request, date_added, None)
    }
    /// Prepares only the already confirmed, encoded private crop supplied by the workflow.
    ///
    /// Run serialisation and hashing on the bounded CPU worker. This adapter checks the wire
    /// bound and canonical Base64; the workflow must first validate privacy, geometry and time.
    ///
    /// # Errors
    /// Refuses a later timestamp, malformed or excessive Base64 and unsupported request values.
    pub fn new_with_screenshot(
        request: &SubmissionRequest,
        date_added: Timestamp,
        screenshot: Option<&str>,
    ) -> Result<(Self, PayloadHash), PreparationFailure> {
        if screenshot.is_some_and(|image| !valid_screenshot(image)) {
            return Err(PreparationFailure::InvalidPayload);
        }
        if date_added > request.header().observed_at {
            return Err(PreparationFailure::InvalidPayload);
        }
        let date_added = Timestamp::from_seconds(date_added.seconds());
        let value =
            payload(request, date_added, screenshot).ok_or(PreparationFailure::InvalidPayload)?;
        let body = serde_json::to_vec(&value).map_err(|_| PreparationFailure::InvalidPayload)?;
        let hash = PayloadHash::new(Sha256::digest(&body).into());
        Ok((
            Self {
                body,
                request: request.clone(),
                date_added,
            },
            hash,
        ))
    }
    /// Returns the exact whole-second timestamp encoded in the immutable body.
    #[must_use]
    pub const fn date_added(&self) -> Timestamp {
        self.date_added
    }
}
fn dry_run_success(answer: &Value, request: &SubmissionRequest, date_added: Timestamp) -> bool {
    if answer.get("date_added").and_then(number) != Some(date_added.seconds()) {
        return false;
    }
    if answer.get("ids_reports") == Some(&Value::Null) {
        return true;
    }
    id_values(answer).is_some_and(|ids| {
        ids.len() == request.rows().len() && ids.iter().all(|id| number(id) == Some(0))
    })
}
fn id_values(answer: &Value) -> Option<&[Value]> {
    match answer.get("ids_reports")? {
        Value::Array(ids) if ids.len() <= 500 => Some(ids),
        value @ Value::String(_) => Some(std::slice::from_ref(value)),
        Value::Null => Some(&[]),
        Value::Array(_) | Value::Bool(_) | Value::Number(_) | Value::Object(_) => None,
    }
}

fn valid_screenshot(value: &str) -> bool {
    if value.is_empty() || value.len() > 10_000_000 || !value.len().is_multiple_of(4) {
        return false;
    }
    let (chunks, _) = value.as_bytes().as_chunks::<4>();
    let mut chunks = chunks.iter();
    while let Some([a, b, c, d]) = chunks.next() {
        let Some((_, second)) = base64_digit(*a).zip(base64_digit(*b)) else {
            return false;
        };
        let valid = match (*c, *d) {
            (b'=', b'=') => chunks.len() == 0 && second & 0x0f == 0,
            (_, b'=') => {
                chunks.len() == 0 && base64_digit(*c).is_some_and(|third| third & 0x03 == 0)
            }
            _ => base64_digit(*c).is_some() && base64_digit(*d).is_some(),
        };
        if !valid {
            return false;
        }
    }
    true
}
fn base64_digit(value: u8) -> Option<u8> {
    match value {
        b'A'..=b'Z' => Some(value - b'A'),
        b'a'..=b'z' => Some(value - b'a' + 26),
        b'0'..=b'9' => Some(value - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}
