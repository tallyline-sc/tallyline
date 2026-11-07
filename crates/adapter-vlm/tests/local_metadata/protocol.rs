// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Strict version metadata through the public local HTTP adapter.

use std::net::Ipv4Addr;
use std::time::Duration;

use tallyline_recognition::api::model::MetadataRefusal;
use tokio::net::TcpListener;
use tokio::time;

use super::support::{self, Fixture, TestResult};

async fn assert_invalid_metadata(body: &[u8]) -> TestResult {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
    let mut fixture = Fixture::new()?;
    let client = fixture.client(listener.local_addr()?.port(), Duration::from_secs(2), 1024)?;
    let exercise = async {
        let (reply, server) = tokio::join!(client.version(), async {
            let mut connection = support::accept_owned(&listener, &mut fixture).await?;
            support::read_request(&mut connection).await?;
            support::respond(&mut connection, body).await
        });
        server?;
        assert_eq!(reply, Err(MetadataRefusal::InvalidMetadata));
        Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
    };
    time::timeout(Duration::from_secs(5), exercise).await??;
    Ok(())
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_vlm_6__sequence_shaped_version_metadata_is_refused() -> TestResult {
    assert_invalid_metadata(br#"["fixture"]"#).await
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_vlm_6__malformed_version_fields_and_trailing_data_are_refused() -> TestResult {
    for body in [
        br#"{}"#.as_slice(),
        br#"{"version":"fixture","extra":true}"#,
        br#"{"version":"fixture","version":"other"}"#,
        br#"{"version":12}"#,
        br#"{"version":null}"#,
        br#"{"version":""}"#,
        br#"{"version":"   "}"#,
        br#"{"version":"fixture\n"}"#,
        br#"{"version":"fixture"} {}"#,
        br#"null"#,
        br#""fixture""#,
    ] {
        assert_invalid_metadata(body).await?;
    }
    Ok(())
}
