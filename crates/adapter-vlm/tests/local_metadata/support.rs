// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Held ownership observations for public real-socket contracts.

use std::error::Error;
use std::num::{NonZeroU16, NonZeroU64, NonZeroUsize};
use std::time::Duration;

use tallyline_adapter_vlm::api::{LocalMetadataClient, MetadataBounds};
use tallyline_recognition::api::MetadataOwnershipSession;
use tallyline_recognition::api::model::{
    MetadataAttempt, MetadataGeneration, MetadataOwnership, MetadataRefusal, MetadataSessionState,
};
use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, watch};
pub(super) type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Clone)]
pub(super) struct FixtureSession {
    pub(super) state: watch::Receiver<MetadataSessionState>,
    pub(super) observed: mpsc::Sender<MetadataAttempt>,
    pub(super) decision: watch::Receiver<Option<MetadataOwnership>>,
}

impl MetadataOwnershipSession for FixtureSession {
    fn state(&self) -> MetadataSessionState {
        *self.state.borrow()
    }

    async fn observe(&self, attempt: MetadataAttempt) -> MetadataOwnership {
        if self.observed.send(attempt).await.is_err() {
            return MetadataOwnership::Unknown;
        }
        let mut decision = self.decision.clone();
        match decision.wait_for(Option::is_some).await {
            Ok(value) => match *value {
                Some(observation) => observation,
                None => MetadataOwnership::Unknown,
            },
            Err(_) => MetadataOwnership::Unknown,
        }
    }

    async fn revoked(&self, generation: MetadataGeneration) -> MetadataRefusal {
        let mut state = self.state.clone();
        match state
            .wait_for(|value| *value != MetadataSessionState::Current(generation))
            .await
        {
            Ok(value) => match *value {
                MetadataSessionState::Current(_) => MetadataRefusal::GenerationChanged,
                MetadataSessionState::Revoked(reason) => reason,
            },
            Err(_) => MetadataRefusal::OwnershipUnknown,
        }
    }
}

pub(super) struct Fixture {
    pub(super) session: FixtureSession,
    pub(super) state: watch::Sender<MetadataSessionState>,
    pub(super) decision: watch::Sender<Option<MetadataOwnership>>,
    pub(super) attempts: mpsc::Receiver<MetadataAttempt>,
    pub(super) generation: MetadataGeneration,
}

impl Fixture {
    pub(super) fn new() -> TestResult<Self> {
        let generation =
            MetadataGeneration::new(NonZeroU64::new(7).ok_or("nonzero fixture generation")?);
        let (state, state_receiver) = watch::channel(MetadataSessionState::Current(generation));
        let (observed, attempts) = mpsc::channel(4);
        let (decision, decision_receiver) = watch::channel(None);
        Ok(Self {
            session: FixtureSession {
                state: state_receiver,
                observed,
                decision: decision_receiver,
            },
            state,
            decision,
            attempts,
            generation,
        })
    }

    pub(super) fn client(
        &self,
        port: u16,
        timeout: Duration,
        bytes: usize,
    ) -> TestResult<LocalMetadataClient<FixtureSession>> {
        let bounds = MetadataBounds::new(
            timeout,
            NonZeroUsize::new(bytes).ok_or("nonzero fixture cap")?,
        )
        .map_err(|reason| format!("fixture bounds refused: {reason:?}"))?;
        LocalMetadataClient::loopback(
            NonZeroU16::new(port).ok_or("nonzero fixture port")?,
            self.session.clone(),
            bounds,
        )
        .map_err(|reason| format!("fixture session refused: {reason:?}").into())
    }
}

pub(super) async fn accept_owned(
    listener: &TcpListener,
    fixture: &mut Fixture,
) -> TestResult<TcpStream> {
    let (connection, _) = listener.accept().await?;
    let attempt = fixture
        .attempts
        .recv()
        .await
        .ok_or("observe actual connection")?;
    fixture
        .decision
        .send(Some(MetadataOwnership::Owned(attempt)))?;
    Ok(connection)
}

pub(super) async fn read_request(connection: &mut TcpStream) -> TestResult<String> {
    let mut request = Vec::new();
    let mut byte = [0_u8; 1];
    while !request.ends_with(b"\r\n\r\n") {
        assert_eq!(connection.read(&mut byte).await?, 1);
        request.push(byte[0]);
        assert!(request.len() <= 1024, "fixture request stays bounded");
    }
    Ok(String::from_utf8(request)?)
}

pub(super) async fn respond(connection: &mut TcpStream, body: &[u8]) -> TestResult {
    use tokio::io::AsyncWriteExt;
    let headers = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len());
    connection.write_all(headers.as_bytes()).await?;
    connection.write_all(body).await?;
    Ok(())
}
