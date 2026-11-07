// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Total deadlines and cumulative body bounds through real local HTTP.

use std::net::Ipv4Addr;
use std::time::Duration;

use tallyline_recognition::api::model::{MetadataOwnership, MetadataRefusal};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time;

use super::support::{self, Fixture};

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_vlm_6__declared_oversized_metadata_is_refused_without_waiting_for_the_body() {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind fixture");
    let mut fixture = Fixture::new().expect("build fixture");
    let client = fixture
        .client(
            listener.local_addr().expect("listener address").port(),
            Duration::from_secs(2),
            32,
        )
        .expect("build client");
    let exercise = async {
        let (reply, ()) = tokio::join!(client.version(), async {
            let mut connection = support::accept_owned(&listener, &mut fixture)
                .await
                .expect("admit fixture connection");
            support::read_request(&mut connection)
                .await
                .expect("read fixture request");
            connection
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2048\r\n\r\n")
                .await
                .expect("declare oversized metadata");
            let mut byte = [0_u8; 1];
            assert_eq!(
                time::timeout(Duration::from_millis(500), connection.read(&mut byte))
                    .await
                    .expect("refuse declared oversize promptly")
                    .expect("read refusal EOF"),
                0
            );
        });
        assert_eq!(reply, Err(MetadataRefusal::ResponseTooLarge));
    };
    time::timeout(Duration::from_secs(5), exercise)
        .await
        .expect("bounded body contract");
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_vlm_6__the_total_deadline_closes_a_held_ownership_gate() {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind fixture");
    let mut fixture = Fixture::new().expect("build fixture");
    let client = fixture
        .client(
            listener.local_addr().expect("listener address").port(),
            Duration::from_millis(100),
            1024,
        )
        .expect("build client");
    let exercise = async {
        let (reply, ()) = tokio::join!(client.version(), async {
            let (mut connection, _) = listener.accept().await.expect("accept fixture");
            fixture
                .attempts
                .recv()
                .await
                .expect("held ownership observation");
            let mut byte = [0_u8; 1];
            assert_eq!(
                time::timeout(Duration::from_secs(1), connection.read(&mut byte))
                    .await
                    .expect("total deadline closes socket")
                    .expect("read EOF"),
                0
            );
        });
        assert_eq!(reply, Err(MetadataRefusal::Deadline));
    };
    time::timeout(Duration::from_secs(3), exercise)
        .await
        .expect("bounded gate deadline");
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_vlm_6__chunked_metadata_is_counted_cumulatively_before_parsing() {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind fixture");
    let mut fixture = Fixture::new().expect("build fixture");
    let client = fixture
        .client(
            listener.local_addr().expect("listener address").port(),
            Duration::from_secs(2),
            32,
        )
        .expect("build client");
    let exercise = async {
        let (reply, ()) = tokio::join!(client.version(), async {
            let mut connection = support::accept_owned(&listener, &mut fixture)
                .await
                .expect("admit fixture connection");
            support::read_request(&mut connection)
                .await
                .expect("read fixture request");
            connection.write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n18\r\naaaaaaaaaaaaaaaaaaaaaaaa\r\n").await.expect("write first bounded chunk");
            time::sleep(Duration::from_millis(50)).await;
            connection
                .write_all(b"18\r\nbbbbbbbbbbbbbbbbbbbbbbbb\r\n")
                .await
                .expect("write second bounded chunk");
            let mut byte = [0_u8; 1];
            assert_eq!(
                time::timeout(Duration::from_secs(1), connection.read(&mut byte))
                    .await
                    .expect("cumulative overflow closes socket")
                    .expect("read EOF"),
                0
            );
        });
        assert_eq!(reply, Err(MetadataRefusal::ResponseTooLarge));
    };
    time::timeout(Duration::from_secs(5), exercise)
        .await
        .expect("bounded chunked contract");
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_vlm_6__waiting_for_ownership_does_not_restart_the_body_deadline() {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind fixture");
    let mut fixture = Fixture::new().expect("build fixture");
    let client = fixture
        .client(
            listener.local_addr().expect("listener address").port(),
            Duration::from_millis(600),
            1024,
        )
        .expect("build client");
    let exercise = async {
        let (reply, ()) = tokio::join!(client.version(), async {
            let (mut connection, _) = listener.accept().await.expect("accept fixture");
            let attempt = fixture
                .attempts
                .recv()
                .await
                .expect("hold ownership observation");
            time::sleep(Duration::from_millis(400)).await;
            fixture
                .decision
                .send(Some(MetadataOwnership::Owned(attempt)))
                .expect("admit after part of total budget");
            support::read_request(&mut connection)
                .await
                .expect("read fixture request");
            connection
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 20\r\n\r\n")
                .await
                .expect("stall response body");
            let mut byte = [0_u8; 1];
            assert_eq!(
                time::timeout(Duration::from_millis(450), connection.read(&mut byte))
                    .await
                    .expect("remaining total budget closes response")
                    .expect("read EOF"),
                0
            );
        });
        assert_eq!(reply, Err(MetadataRefusal::Deadline));
    };
    time::timeout(Duration::from_secs(5), exercise)
        .await
        .expect("bounded cumulative deadline");
}
