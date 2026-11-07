// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Absolute deadlines after synchronous work at the public metadata seam.

use std::net::Ipv4Addr;
use std::num::{NonZeroU16, NonZeroUsize};
use std::time::Duration;

use tallyline_adapter_vlm::api::{LocalMetadataClient, MetadataBounds};
use tallyline_recognition::api::MetadataOwnershipSession;
use tallyline_recognition::api::model::{
    MetadataAttempt, MetadataGeneration, MetadataOwnership, MetadataRefusal, MetadataSessionState,
};
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio::time;

use super::support::{self, Fixture, FixtureSession, TestResult};

#[derive(Clone, Copy, Eq, PartialEq)]
enum SlowPhase {
    Observation,
    Publication,
}

#[derive(Clone)]
struct SlowSession {
    inner: FixtureSession,
    slow: watch::Receiver<Option<SlowPhase>>,
}

impl MetadataOwnershipSession for SlowSession {
    fn state(&self) -> MetadataSessionState {
        if *self.slow.borrow() == Some(SlowPhase::Publication) {
            std::thread::sleep(Duration::from_millis(500));
        }
        self.inner.state()
    }

    async fn observe(&self, attempt: MetadataAttempt) -> MetadataOwnership {
        let observation = self.inner.observe(attempt).await;
        if *self.slow.borrow() == Some(SlowPhase::Observation) {
            std::thread::sleep(Duration::from_millis(500));
        }
        observation
    }

    async fn revoked(&self, generation: MetadataGeneration) -> MetadataRefusal {
        self.inner.revoked(generation).await
    }
}

async fn late_synchronous_work_is_refused(phase: SlowPhase) -> TestResult {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
    let mut fixture = Fixture::new()?;
    let (slow_sender, slow) = watch::channel(None);
    let session = SlowSession {
        inner: fixture.session.clone(),
        slow,
    };
    let bounds = MetadataBounds::new(
        Duration::from_millis(250),
        NonZeroUsize::new(1024).ok_or("nonzero bound")?,
    )
    .map_err(|reason| format!("fixture bounds: {reason:?}"))?;
    let client = LocalMetadataClient::loopback(
        NonZeroU16::new(listener.local_addr()?.port()).ok_or("nonzero port")?,
        session,
        bounds,
    )
    .map_err(|reason| format!("fixture client: {reason:?}"))?;
    let exercise = async {
        let (reply, server) = tokio::join!(client.version(), async {
            let (mut connection, _) = listener.accept().await?;
            let attempt = fixture
                .attempts
                .recv()
                .await
                .ok_or("actual ownership attempt")?;
            if phase == SlowPhase::Observation {
                slow_sender.send(Some(phase))?;
            }
            fixture
                .decision
                .send(Some(MetadataOwnership::Owned(attempt)))?;
            match phase {
                SlowPhase::Observation => {
                    let mut byte = [0_u8; 1];
                    assert_eq!(
                        connection.read(&mut byte).await?,
                        0,
                        "an observation completing after the deadline cannot release HTTP bytes"
                    );
                }
                SlowPhase::Publication => {
                    support::read_request(&mut connection).await?;
                    slow_sender.send(Some(phase))?;
                    support::respond(&mut connection, br#"{"version":"fixture"}"#).await?;
                }
            }
            Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
        });
        server?;
        assert_eq!(
            reply,
            Err(MetadataRefusal::Deadline),
            "synchronous work cannot publish a late successful result"
        );
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
async fn r_vlm_6__a_long_observation_poll_cannot_release_a_socket_after_deadline() -> TestResult {
    late_synchronous_work_is_refused(SlowPhase::Observation).await
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_vlm_6__a_long_final_state_read_cannot_publish_metadata_after_deadline() -> TestResult {
    late_synchronous_work_is_refused(SlowPhase::Publication).await
}
