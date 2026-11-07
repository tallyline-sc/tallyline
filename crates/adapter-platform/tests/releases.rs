// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Real HTTP adapter contracts through the public update workflow.

use tallyline_adapter_platform::api::ReleaseClient;
use tallyline_workflows::api::{
    check_update,
    model::{ReleaseVersion, UpdateStatus},
};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_nf_7__latest_standard_release_is_actionable_without_sending_credentials() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/repos/tallyline-sc/tallyline/releases/latest"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"tag_name":"v2.0.1","draft":false,"prerelease":false,"html_url":"https://untrusted.invalid/","assets":[{"browser_download_url":"https://untrusted.invalid/code"}]})))
        .mount(&server).await;
    let client = ReleaseClient::loopback(server.address().port(), "0.0.0-dev").unwrap();
    let status = check_update(&client, ReleaseVersion::parse("v1.0.0").unwrap()).await;
    let UpdateStatus::Available(version) = status else {
        panic!("missing release")
    };
    assert_eq!(
        version.release_url(),
        "https://github.com/tallyline-sc/tallyline/releases/tag/v2.0.1"
    );
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let request = requests.first().unwrap();
    assert_eq!(
        request.headers.get("user-agent").unwrap(),
        "Tallyline/0.0.0-dev"
    );
    assert!(!request.headers.contains_key("authorization"));
    assert!(!request.headers.contains_key("secret-key"));
    assert!(request.body.is_empty());
    assert!(request.url.query().is_none());
}
