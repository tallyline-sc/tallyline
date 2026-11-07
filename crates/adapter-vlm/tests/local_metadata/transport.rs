// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Independent ownership admission for every fixed local metadata request.

use std::net::Ipv4Addr;
use std::time::Duration;

use tallyline_recognition::api::model::MetadataOwnership;
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use tokio::time;

use super::support::{self, Fixture};

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_vlm_6__each_sequential_get_requires_a_new_observed_actual_connection() {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind fixture");
    let mut fixture = Fixture::new().expect("build fixture");
    let client = fixture
        .client(
            listener.local_addr().expect("listener address").port(),
            Duration::from_secs(2),
            1024,
        )
        .expect("build client");
    let exercise = async {
        let mut previous = None;
        for _ in 0..2 {
            fixture
                .decision
                .send(None)
                .expect("hold each new connection");
            let (reply, attempt) = tokio::join!(client.version(), async {
                let (mut connection, peer) =
                    listener.accept().await.expect("accept new connection");
                let attempt = fixture
                    .attempts
                    .recv()
                    .await
                    .expect("observe each actual connection");
                assert_eq!(attempt.connection().client_port().get(), peer.port());
                assert_ne!(Some(attempt), previous);
                let mut byte = [0_u8; 1];
                assert!(
                    time::timeout(Duration::from_millis(30), connection.read(&mut byte))
                        .await
                        .is_err()
                );
                fixture
                    .decision
                    .send(Some(MetadataOwnership::Owned(attempt)))
                    .expect("admit only this attempt");
                assert!(
                    support::read_request(&mut connection)
                        .await
                        .expect("read fixture request")
                        .starts_with("GET /api/version HTTP/1.1\r\n")
                );
                support::respond(&mut connection, br#"{"version":"fixture"}"#)
                    .await
                    .expect("write fixture response");
                assert_eq!(
                    time::timeout(Duration::from_secs(1), connection.read(&mut byte))
                        .await
                        .expect("no idle pooled connection")
                        .expect("read EOF"),
                    0
                );
                attempt
            });
            assert_eq!(reply.expect("bounded version").version(), "fixture");
            previous = Some(attempt);
        }
    };
    time::timeout(Duration::from_secs(5), exercise)
        .await
        .expect("bounded sequential admission");
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_vlm_6__redirects_cannot_contact_another_listener() {
    use tallyline_recognition::api::model::MetadataRefusal;
    use tokio::io::AsyncWriteExt;
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind fixture");
    let target = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind forbidden redirect target");
    let mut fixture = Fixture::new().expect("build fixture");
    let client = fixture
        .client(
            listener.local_addr().expect("listener address").port(),
            Duration::from_secs(2),
            1024,
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
            let response = format!(
                "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:{}/api/version\r\nContent-Length: 0\r\n\r\n",
                target.local_addr().expect("target address").port()
            );
            connection
                .write_all(response.as_bytes())
                .await
                .expect("return forbidden redirect");
        });
        assert_eq!(reply, Err(MetadataRefusal::InvalidMetadata));
        assert!(
            time::timeout(Duration::from_millis(50), target.accept())
                .await
                .is_err()
        );
        assert!(
            time::timeout(Duration::from_millis(50), listener.accept())
                .await
                .is_err()
        );
    };
    time::timeout(Duration::from_secs(5), exercise)
        .await
        .expect("bounded redirect refusal");
}
