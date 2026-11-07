// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Revocation of held real connections through the public metadata interface.

use std::net::Ipv4Addr;
use std::num::{NonZeroU16, NonZeroU64, NonZeroUsize};
use std::time::Duration;

use tallyline_adapter_vlm::api::{LocalMetadataClient, MetadataBounds};
use tallyline_recognition::api::MetadataOwnershipSession;
use tallyline_recognition::api::model::{
    LoopbackConnection, MetadataAttempt, MetadataGeneration, MetadataOwnership, MetadataRefusal,
    MetadataSessionState,
};
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use tokio::time;

use super::support::{Fixture, FixtureSession, TestResult};

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_vlm_6__cancelling_a_held_observation_closes_without_request_bytes() {
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
        let (reply, ()) = tokio::join!(client.version(), async {
            let (mut connection, _) = listener.accept().await.expect("accept fixture");
            let attempt = fixture
                .attempts
                .recv()
                .await
                .expect("ownership observation starts");
            fixture
                .state
                .send(MetadataSessionState::Revoked(MetadataRefusal::Cancelled))
                .expect("cancel held session");
            let mut byte = [0_u8; 1];
            assert_eq!(
                time::timeout(Duration::from_secs(1), connection.read(&mut byte))
                    .await
                    .expect("cancellation closes promptly")
                    .expect("read EOF"),
                0
            );
            fixture
                .decision
                .send(Some(MetadataOwnership::Owned(attempt)))
                .expect("late observation cannot reopen cancellation");
            assert!(
                time::timeout(Duration::from_millis(50), listener.accept())
                    .await
                    .is_err()
            );
        });
        assert_eq!(reply, Err(MetadataRefusal::Cancelled));
    };
    time::timeout(Duration::from_secs(5), exercise)
        .await
        .expect("bounded cancellation contract");
}

async fn refuse_changed_attempt(
    change: fn(MetadataAttempt) -> MetadataAttempt,
    refusal: MetadataRefusal,
) -> TestResult {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
    let mut fixture = Fixture::new()?;
    let client = fixture.client(listener.local_addr()?.port(), Duration::from_secs(2), 1024)?;
    let exercise = async {
        let (reply, server) = tokio::join!(client.version(), async {
            let (mut connection, _) = listener.accept().await?;
            let attempt = fixture
                .attempts
                .recv()
                .await
                .ok_or("observe actual tuple")?;
            assert_eq!(attempt.generation(), fixture.generation);
            fixture
                .decision
                .send(Some(MetadataOwnership::Owned(change(attempt))))?;
            let mut byte = [0_u8; 1];
            assert_eq!(
                connection.read(&mut byte).await?,
                0,
                "a different observation cannot admit the actual socket"
            );
            Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
        });
        server?;
        assert_eq!(reply, Err(refusal));
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
async fn r_vlm_6__ownership_observations_must_match_actual_tuple_and_generation() {
    refuse_changed_attempt(
        |attempt| {
            MetadataAttempt::new(
                attempt.generation(),
                LoopbackConnection::new(
                    NonZeroU16::new(1).expect("nonzero fixture port"),
                    attempt.connection().server_port(),
                ),
            )
        },
        MetadataRefusal::TupleMismatch,
    )
    .await
    .expect("changed observation is refused");
    refuse_changed_attempt(
        |attempt| {
            MetadataAttempt::new(
                MetadataGeneration::new(
                    NonZeroU64::new(8).expect("nonzero replacement generation"),
                ),
                attempt.connection(),
            )
        },
        MetadataRefusal::GenerationChanged,
    )
    .await
    .expect("changed observation is refused");
}

#[derive(Clone)]
struct ReplacingSession {
    inner: FixtureSession,
    state: tokio::sync::watch::Sender<MetadataSessionState>,
    replacement: MetadataGeneration,
}

impl MetadataOwnershipSession for ReplacingSession {
    fn state(&self) -> MetadataSessionState {
        self.inner.state()
    }

    async fn observe(&self, attempt: MetadataAttempt) -> MetadataOwnership {
        let observation = self.inner.observe(attempt).await;
        assert!(
            self.state
                .send(MetadataSessionState::Current(self.replacement))
                .is_ok()
        );
        observation
    }

    async fn revoked(&self, generation: MetadataGeneration) -> MetadataRefusal {
        time::sleep(Duration::from_millis(50)).await;
        self.inner.revoked(generation).await
    }
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_vlm_6__generation_is_rechecked_after_the_observation_returns() {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind fixture");
    let mut fixture = Fixture::new().expect("build fixture");
    let session = ReplacingSession {
        inner: fixture.session.clone(),
        state: fixture.state.clone(),
        replacement: MetadataGeneration::new(NonZeroU64::new(8).expect("nonzero generation")),
    };
    let client = LocalMetadataClient::loopback(
        NonZeroU16::new(listener.local_addr().expect("listener address").port())
            .expect("nonzero fixture port"),
        session,
        MetadataBounds::new(
            Duration::from_secs(2),
            NonZeroUsize::new(1024).expect("nonzero byte cap"),
        )
        .expect("valid bounds"),
    )
    .expect("current generation");
    let exercise = async {
        let (reply, ()) = tokio::join!(client.version(), async {
            let (mut connection, _) = listener.accept().await.expect("accept fixture");
            let attempt = fixture
                .attempts
                .recv()
                .await
                .expect("observe actual connection");
            fixture
                .decision
                .send(Some(MetadataOwnership::Owned(attempt)))
                .expect("return old-generation observation");
            let mut byte = [0_u8; 1];
            assert_eq!(connection.read(&mut byte).await.expect("read stale EOF"), 0);
        });
        assert_eq!(reply, Err(MetadataRefusal::GenerationChanged));
    };
    time::timeout(Duration::from_secs(5), exercise)
        .await
        .expect("bounded replacement contract");
}
