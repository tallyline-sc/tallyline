// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Read-only credential validation contracts against a local HTTP server.

use crate::api::{ClientIdentity, ReaderSettings, UexReader};
use serde_json::json;
use tallyline_submission::api::CredentialValidator;
use tallyline_submission::api::model::{CredentialFailure, CredentialValues, ValidatedAccount};
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn r_api_4_r_api_7__valid_account_and_empty_filtered_reports_establish_the_username() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/2.0/user/"))
        .and(header("secret-key", "dummy-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status":"ok", "data": {
            "username":"dummy-runner", "is_datarunner":1, "is_datarunner_banned":0, "is_staff":0,
            "email":"discarded@example.invalid", "discord":"discarded-dummy-discord"
        }}))).expect(1).mount(&server).await;
    Mock::given(method("GET"))
        .and(path("/2.0/data_info/"))
        .and(query_param("username", "dummy-runner"))
        .and(query_param("limit", "1"))
        .and(header("secret-key", "dummy-key"))
        .and(header("authorization", "Bearer dummy-token"))
        .and(header("x-client-version", "0.0.0"))
        .and(header("user-agent", "tallyline-test/0.0.0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status":"ok", "data":[]})))
        .expect(1)
        .mount(&server)
        .await;
    let settings = ReaderSettings::for_test_server(
        &format!("{}/2.0/", server.uri()),
        ClientIdentity::new("tallyline-test", "0.0.0"),
    )
    .unwrap()
    .with_expert_host_confirmation();
    let reader = UexReader::new(&settings).unwrap();
    let values =
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
    assert_eq!(
        reader.validate(&values).await,
        Ok(ValidatedAccount::new("dummy-runner".to_owned()))
    );
    let requests = server.received_requests().await.unwrap();
    assert!(
        !requests
            .first()
            .unwrap()
            .headers
            .contains_key("authorization")
    );
}

#[tokio::test]
async fn r_api_4_r_nf_4__remote_error_text_never_crosses_the_credential_boundary() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/2.0/user/"))
        .respond_with(ResponseTemplate::new(403).set_body_json(json!({
            "status":"invalid_secret_key", "message":"dummy-error-secret-sentinel",
            "data":{"secret":"dummy-error-secret-sentinel"}
        })))
        .mount(&server)
        .await;
    let settings = ReaderSettings::for_test_server(
        &format!("{}/2.0/", server.uri()),
        ClientIdentity::new("tallyline-test", "0.0.0"),
    )
    .unwrap()
    .with_expert_host_confirmation();
    let reader = UexReader::new(&settings).unwrap();
    let values =
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
    let outcome = reader.validate(&values).await;
    assert_eq!(outcome, Err(CredentialFailure::InvalidKey));
    assert!(!format!("{outcome:?}").contains("dummy-error-secret-sentinel"));
}

#[tokio::test]
async fn r_api_7__missing_client_version_is_a_client_failure_and_never_a_bad_key() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/2.0/user/"))
        .respond_with(ResponseTemplate::new(403).set_body_json(json!({
            "status":"missing_client_version", "message":"dummy-private-server-message", "data":null
        })))
        .mount(&server)
        .await;
    let settings = ReaderSettings::for_test_server(
        &format!("{}/2.0/", server.uri()),
        ClientIdentity::new("tallyline-test", "0.0.0"),
    )
    .unwrap()
    .with_expert_host_confirmation();
    let reader = UexReader::new(&settings).unwrap();
    let values =
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
    assert_eq!(
        reader.validate(&values).await,
        Err(CredentialFailure::ClientVersionMissing)
    );
}

struct AdmissionStore {
    lost: bool,
}
impl tallyline_submission::api::SecretStore for AdmissionStore {
    async fn load(
        &mut self,
    ) -> Result<
        Option<tallyline_submission::api::model::SavedCredentials>,
        tallyline_submission::api::model::StorageFailure,
    > {
        Ok(None)
    }
    async fn save(
        &mut self,
        _: &tallyline_submission::api::model::SavedCredentials,
    ) -> Result<(), tallyline_submission::api::model::StorageFailure> {
        Ok(())
    }
    async fn save_passive(
        &mut self,
        _: &tallyline_submission::api::model::SavedCredentials,
    ) -> Result<(), tallyline_submission::api::model::StorageFailure> {
        Ok(())
    }
    async fn remove(&mut self) -> Result<(), tallyline_submission::api::model::StorageFailure> {
        Ok(())
    }
    async fn token_admission(
        &mut self,
        token: &str,
    ) -> Result<
        tallyline_submission::api::model::TokenAdmission,
        tallyline_submission::api::model::StorageFailure,
    > {
        use tallyline_submission::api::model::{TokenAdmission, TokenScope};
        let a = TokenScope::new([1; 32]).unwrap();
        Ok(if self.lost {
            TokenAdmission::default()
        } else if token == "dummy-a" {
            TokenAdmission::from_scope(Some(a))
        } else {
            TokenAdmission::distinct_from(vec![a]).unwrap()
        })
    }
}
#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_2_r_sub_11_r_api_3__quota_gates_only_token_validation_route_and_accepts_first_distinct_token()
 {
    use tallyline_submission::api::model::{CredentialCandidate, Persistence, TokenScope};
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/2.0/user/")).and(header("secret-key","dummy-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status":"ok","data":{"username":"dummy-runner","is_datarunner":1,"is_datarunner_banned":0,"is_staff":0}}))).expect(3).mount(&server).await;
    Mock::given(method("GET"))
        .and(path("/2.0/data_info/"))
        .and(header("authorization", "Bearer dummy-first-b"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status":"ok","data":[]})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/2.0/game_versions/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"status":"ok","data":{"live":"4.10.2","ptu":null}})),
        )
        .expect(1)
        .mount(&server)
        .await;
    let settings = ReaderSettings::for_test_server(
        &format!("{}/2.0/", server.uri()),
        ClientIdentity::new("tallyline-test", "0.0.0"),
    )
    .unwrap()
    .with_expert_host_confirmation();
    let held = TokenScope::new([1; 32]).unwrap();
    let reader = UexReader::new(&settings)
        .unwrap()
        .with_authenticated_read_gate(crate::api::AuthenticatedReadGate::new(move |proof| {
            proof.proves_distinct(held)
        }));
    let mut application = tallyline_submission::api::CredentialApplication::default();
    let mut store = AdmissionStore { lost: false };
    for (token, expected) in [
        ("dummy-a", Err(CredentialFailure::Unavailable)),
        (
            "dummy-first-b",
            Ok(ValidatedAccount::new("dummy-runner".to_owned())),
        ),
    ] {
        let mut request = application.begin(CredentialCandidate::new(
            CredentialValues::new("dummy-key".to_owned(), Some(token.to_owned())).unwrap(),
            Persistence::Session,
        ));
        request
            .lookup_token_admission(&mut store, None)
            .await
            .unwrap();
        assert_eq!(request.validate(&reader).await, expected);
    }
    store.lost = true;
    let mut lost = application.begin(CredentialCandidate::new(
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-first-b".to_owned())).unwrap(),
        Persistence::Session,
    ));
    lost.lookup_token_admission(&mut store, None).await.unwrap();
    assert_eq!(
        lost.validate(&reader).await,
        Err(CredentialFailure::Unavailable)
    );
    reader.game_versions().await.unwrap();
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 5);
    for request in requests
        .iter()
        .filter(|request| request.url.path() != "/2.0/data_info/")
    {
        assert!(!request.headers.contains_key("authorization"));
    }
    server.verify().await;
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_2_r_api_3__pending_local_quota_preserves_exact_distinct_token_validation() {
    use tallyline_submission::api::model::{CredentialCandidate, Persistence};
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/2.0/user/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status":"ok","data":{"username":"dummy-runner","is_datarunner":1,"is_datarunner_banned":0,"is_staff":0}})))
        .expect(4).mount(&server).await;
    Mock::given(method("GET"))
        .and(path("/2.0/data_info/"))
        .and(header("authorization", "Bearer dummy-a"))
        .respond_with(
            ResponseTemplate::new(429)
                .set_body_json(json!({"status":"exceeded_daily_quota","data":{"reset":1010}})),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/2.0/data_info/"))
        .and(header("authorization", "Bearer dummy-first-b"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status":"ok","data":[]})))
        .expect(1)
        .mount(&server)
        .await;
    let settings = ReaderSettings::for_test_server(
        &format!("{}/2.0/", server.uri()),
        ClientIdentity::new("tallyline-test", "1.2.3"),
    )
    .unwrap()
    .with_expert_host_confirmation();
    let reader = UexReader::new(&settings).unwrap();
    let mut application = tallyline_submission::api::CredentialApplication::default();
    let mut store = AdmissionStore { lost: false };
    for (token, lost, expected) in [
        ("dummy-a", false, Err(CredentialFailure::Unavailable)),
        ("dummy-a", false, Err(CredentialFailure::Unavailable)),
        (
            "dummy-first-b",
            false,
            Ok(ValidatedAccount::new("dummy-runner".to_owned())),
        ),
        ("dummy-first-b", true, Err(CredentialFailure::Unavailable)),
    ] {
        store.lost = lost;
        let mut request = application.begin(CredentialCandidate::new(
            CredentialValues::new("dummy-key".to_owned(), Some(token.to_owned())).unwrap(),
            Persistence::Session,
        ));
        request
            .lookup_token_admission(&mut store, None)
            .await
            .unwrap();
        assert_eq!(request.validate(&reader).await, expected);
    }
    assert_eq!(reader.take_quota_observations().len(), 1);
    assert_eq!(server.received_requests().await.unwrap().len(), 6);
    server.verify().await;
}
