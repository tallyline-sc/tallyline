// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Public metadata admission over actual loopback connections.

use std::net::{IpAddr, Ipv4Addr};
use std::num::NonZeroU16;
use std::time::Duration;

use tallyline_recognition::api::model::{
    LoopbackConnection, MetadataAttempt, MetadataOwnership, MetadataRefusal,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time;

#[path = "local_metadata/bounds.rs"]
mod bounds;
#[path = "local_metadata/lifecycle.rs"]
mod lifecycle;
#[path = "local_metadata/protocol.rs"]
mod protocol;
#[path = "local_metadata/proxy.rs"]
mod proxy;
#[path = "local_metadata/support.rs"]
mod support;
#[path = "local_metadata/transport.rs"]
mod transport;

use support::{Fixture, TestResult};

const REQUEST_BUDGET: Duration = Duration::from_secs(2);
const TEST_BUDGET: Duration = Duration::from_secs(10);
const QUIET_WINDOW: Duration = Duration::from_millis(50);
const EOF_BUDGET: Duration = Duration::from_secs(1);
const RESPONSE_BYTE_LIMIT: usize = 1024;

async fn a_refused_listener_receives_no_request_bytes(
    observation: MetadataOwnership,
    refusal: MetadataRefusal,
) -> TestResult {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
    let server_port = listener.local_addr()?.port();
    let mut fixture = Fixture::new()?;
    let client = fixture.client(server_port, REQUEST_BUDGET, RESPONSE_BYTE_LIMIT)?;
    let exercise = async {
        let (reply, server) = tokio::join!(client.version(), async {
            let (mut connection, peer) = listener.accept().await?;
            assert_eq!(peer.ip(), IpAddr::V4(Ipv4Addr::LOCALHOST));
            let actual_local = connection.local_addr()?;
            assert_eq!(actual_local.ip(), IpAddr::V4(Ipv4Addr::LOCALHOST));
            assert_eq!(actual_local.port(), server_port);
            let expected = MetadataAttempt::new(
                fixture.generation,
                LoopbackConnection::new(
                    NonZeroU16::new(peer.port()).ok_or("nonzero actual client port")?,
                    NonZeroU16::new(server_port).ok_or("nonzero actual server port")?,
                ),
            );
            assert_eq!(fixture.attempts.recv().await, Some(expected));
            let mut byte = [0_u8; 1];
            assert!(
                time::timeout(QUIET_WINDOW, connection.read(&mut byte))
                    .await
                    .is_err(),
                "held connection stays silent"
            );
            fixture.decision.send(Some(observation))?;
            assert_eq!(
                time::timeout(EOF_BUDGET, connection.read(&mut byte)).await??,
                0,
                "refusal sends no request byte"
            );
            Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
        });
        server?;
        assert_eq!(reply, Err(refusal));
        Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
    };
    time::timeout(TEST_BUDGET, exercise).await??;
    assert!(
        time::timeout(QUIET_WINDOW, listener.accept())
            .await
            .is_err(),
        "refusal does not retry"
    );
    Ok(())
}

async fn only_the_owned_current_connection_receives_version_metadata() -> TestResult {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
    let server_port = listener.local_addr()?.port();
    let mut fixture = Fixture::new()?;
    let client = fixture.client(server_port, REQUEST_BUDGET, RESPONSE_BYTE_LIMIT)?;
    let exercise = async {
        let (reply, server) = tokio::join!(client.version(), async {
            let (mut connection, peer) = listener.accept().await?;
            let expected = MetadataAttempt::new(
                fixture.generation,
                LoopbackConnection::new(
                    NonZeroU16::new(peer.port()).ok_or("nonzero client port")?,
                    NonZeroU16::new(server_port).ok_or("nonzero server port")?,
                ),
            );
            assert_eq!(fixture.attempts.recv().await, Some(expected));
            let mut byte = [0_u8; 1];
            assert!(
                time::timeout(QUIET_WINDOW, connection.read(&mut byte))
                    .await
                    .is_err()
            );
            fixture
                .decision
                .send(Some(MetadataOwnership::Owned(expected)))?;
            let request = support::read_request(&mut connection).await?;
            assert!(request.starts_with("GET /api/version HTTP/1.1\r\n"));
            let headers = request.to_ascii_lowercase();
            for forbidden in [
                "authorization:",
                "proxy-authorization:",
                "cookie:",
                "transfer-encoding:",
            ] {
                assert!(
                    !headers.contains(forbidden),
                    "credential-free bodyless metadata request"
                );
            }
            assert!(
                !headers.contains("content-length:") || headers.contains("content-length: 0\r\n")
            );
            support::respond(
                &mut connection,
                b" \r\n\t{\"version\":\"0.0.0-fixture\"} \t\r\n",
            )
            .await?;
            connection.shutdown().await?;
            Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
        });
        server?;
        let metadata = reply.map_err(|reason| format!("admitted version refused: {reason:?}"))?;
        assert_eq!(metadata.version(), "0.0.0-fixture");
        assert_eq!(metadata.generation(), fixture.generation);
        Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
    };
    time::timeout(TEST_BUDGET, exercise).await??;
    assert!(
        time::timeout(QUIET_WINDOW, listener.accept())
            .await
            .is_err()
    );
    Ok(())
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_vlm_6__metadata_reaches_only_the_admitted_actual_connection() -> TestResult {
    a_refused_listener_receives_no_request_bytes(
        MetadataOwnership::ForeignListener,
        MetadataRefusal::ForeignListener,
    )
    .await?;
    a_refused_listener_receives_no_request_bytes(
        MetadataOwnership::Unknown,
        MetadataRefusal::OwnershipUnknown,
    )
    .await?;
    only_the_owned_current_connection_receives_version_metadata().await
}

#[path = "local_metadata/deadline.rs"]
mod deadline;
