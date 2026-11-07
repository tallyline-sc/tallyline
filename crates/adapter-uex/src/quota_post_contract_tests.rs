// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Actual HTTP POST admission against pending local and shared token quota evidence.

use crate::api::{AuthenticatedReadGate, ClientIdentity, ReaderSettings, UexReader};
use crate::queue_contract_tests;
use serde_json::json;
use tallyline_shared_kernel::api::model::{SubmissionMode, Timestamp};
use tallyline_submission::api::model::{
    CredentialCandidate, CredentialValues, JobId, PayloadHash, Persistence, SavedCredentials,
    StorageFailure, SubmissionJob, SubmissionRequest, SubmissionResult, SubmissionReturnReason,
    TokenAdmission, TokenScope, ValidatedAccount,
};
use tallyline_submission::api::{
    CredentialApplication, SecretStore, SubmissionGateway, SubmissionRowReceiptVerifier,
};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

struct ScopeStore;
impl SecretStore for ScopeStore {
    async fn load(&mut self) -> Result<Option<SavedCredentials>, StorageFailure> {
        Ok(None)
    }
    async fn save(&mut self, _: &SavedCredentials) -> Result<(), StorageFailure> {
        Ok(())
    }
    async fn save_passive(&mut self, _: &SavedCredentials) -> Result<(), StorageFailure> {
        Ok(())
    }
    async fn remove(&mut self) -> Result<(), StorageFailure> {
        Ok(())
    }
    async fn token_admission(&mut self, token: &str) -> Result<TokenAdmission, StorageFailure> {
        let a = TokenScope::new([1; 32]).unwrap();
        Ok(match token {
            "dummy-a" => TokenAdmission::from_scope(Some(a)),
            "dummy-b" => TokenAdmission::distinct_from(vec![a]).unwrap(),
            _ => TokenAdmission::default(),
        })
    }
}
async fn values(token: &str) -> CredentialValues {
    let mut application = CredentialApplication::default();
    let mut request = application.begin(CredentialCandidate::new(
        CredentialValues::new("dummy-key".to_owned(), Some(token.to_owned())).unwrap(),
        Persistence::Session,
    ));
    request
        .lookup_token_admission(&mut ScopeStore, None)
        .await
        .unwrap();
    application
        .accept(
            request,
            Ok(ValidatedAccount::new("dummy-account".to_owned())),
            Timestamp::from_seconds(1000),
            &mut ScopeStore,
        )
        .await
        .unwrap();
    application
        .credentials_for("dummy-account")
        .unwrap()
        .clone()
}
#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_2_r_sub_11_r_api_3__pending_local_get_quota_withholds_actual_post_and_keeps_exact_distinct_token_eligible()
 {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/2.0/data_info/"))
        .respond_with(
            ResponseTemplate::new(429)
                .set_body_json(json!({"status":"exceeded_daily_quota","data":{"reset":1010}})),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/2.0/data_submit/"))
        .and(header("authorization", "Bearer dummy-b"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"status":"ok","data":{"ids_reports":null,"date_added":100}})),
        )
        .expect(1)
        .mount(&server)
        .await;
    let settings = ReaderSettings::for_test_server(
        &format!("{}/2.0/", server.uri()),
        ClientIdentity::new("tallyline-test", "1.2.3"),
    )
    .unwrap()
    .with_expert_host_confirmation();
    let reader = UexReader::new(&settings)
        .unwrap()
        .with_authenticated_read_gate(AuthenticatedReadGate::new(|_| true));
    let base = queue_contract_tests::released_request().unwrap();
    let mut production = base.header().clone();
    production.mode = SubmissionMode::Production;
    let request = SubmissionRequest::new(production, base.rows().to_vec()).unwrap();
    let job = SubmissionJob::queued(JobId(1), request)
        .unwrap()
        .begin_prepared(
            Timestamp::from_seconds(100),
            PayloadHash::new([7; 32]),
            Timestamp::from_seconds(100),
        )
        .unwrap()
        .complete(SubmissionResult::Unknown, Timestamp::from_seconds(101))
        .unwrap();
    let a = values("dummy-a").await;
    let b = values("dummy-b").await;
    let unknown = values("dummy-unknown").await;
    assert!(reader.verify_row_receipts(&job, &a, None).await.is_none());
    let same = reader
        .submit(reader.prepare(&base).unwrap().0, &a, None)
        .await;
    let distinct = reader
        .submit(reader.prepare(&base).unwrap().0, &b, None)
        .await;
    let unproven = reader
        .submit(reader.prepare(&base).unwrap().0, &unknown, None)
        .await;
    let reader = reader.with_authenticated_read_gate(AuthenticatedReadGate::new(|_| false));
    let shared = reader
        .submit(reader.prepare(&base).unwrap().0, &b, None)
        .await;
    let observations = reader.take_quota_observations();
    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        same,
        SubmissionResult::Returned(SubmissionReturnReason::PreflightChanged)
    );
    assert_eq!(distinct, SubmissionResult::DryRunSucceeded);
    assert_eq!(
        unproven,
        SubmissionResult::Returned(SubmissionReturnReason::PreflightChanged)
    );
    assert_eq!(
        shared,
        SubmissionResult::Returned(SubmissionReturnReason::PreflightChanged)
    );
    assert_eq!(observations.len(), 1);
    assert_eq!(observations[0].scope(), a.token_scope());
    assert_eq!(
        requests.len(),
        2,
        "only the quota GET and exact-distinct POST may reach HTTP"
    );
    assert_eq!(
        requests[1].headers.get("authorization").unwrap(),
        "Bearer dummy-b"
    );
    server.verify().await;
}
